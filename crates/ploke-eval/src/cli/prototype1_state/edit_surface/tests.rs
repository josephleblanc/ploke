use std::path::PathBuf;

use crate::loop_graph::ArtifactId;

use super::{graph, harness, surface};
use graph::View;
use harness::Harness;

fn href(value: &str) -> surface::Hash {
    surface::Hash::new(value)
}

fn aref(id: &str, hash: &str) -> surface::Ref {
    surface::Ref::new(ArtifactId::new(id), href(hash))
}

fn target(name: &str) -> graph::Target {
    graph::Target::new("src/lib.rs", name)
}

fn fixture() -> (
    surface::Artifact,
    graph::Mock,
    graph::Target,
    graph::Target,
    graph::Target,
) {
    let root = target("root");
    let child = target("child");
    let sibling = target("sibling");
    let artifact = surface::Artifact::new(
        aref("artifact:base", "tree:base"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v1"))],
    );
    let graph = graph::Mock::new(
        vec![
            graph::Node::new(root.clone(), "src/lib.rs", 0, 100),
            graph::Node::new(child.clone(), "src/lib.rs", 10, 40),
            graph::Node::new(sibling.clone(), "src/lib.rs", 50, 90),
        ],
        [
            (root.clone(), child.clone()),
            (root.clone(), sibling.clone()),
        ],
    );
    (artifact, graph, root, child, sibling)
}

#[test]
fn graph_projection_is_tied_to_artifact_identity() {
    let (artifact, graph, _, _, _) = fixture();

    let projection = graph.project(&artifact).expect("project artifact");

    assert_eq!(projection.artifact(), artifact.reference());

    let other = surface::Artifact::new(
        aref("artifact:base", "tree:other"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v1"))],
    );
    let other_projection = graph.project(&other).expect("project other artifact");
    assert_ne!(projection.artifact(), other_projection.artifact());
}

#[test]
fn rule_derived_bounds_include_one_target_and_exclude_another() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");

    let bounds = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling.clone()),
                graph::Rule::Exclude(sibling.clone()),
            ],
        )
        .expect("derive bounds");

    assert!(bounds.span(&child).is_some());
    assert!(bounds.span(&sibling).is_none());
}

#[test]
fn bounds_use_projection_edges() {
    let root = target("root");
    let child = target("child");
    let sibling = target("sibling");
    let artifact = surface::Artifact::new(
        aref("artifact:base", "tree:base"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v1"))],
    );
    let nodes = vec![
        graph::Node::new(root.clone(), "src/lib.rs", 0, 100),
        graph::Node::new(child.clone(), "src/lib.rs", 10, 40),
        graph::Node::new(sibling.clone(), "src/lib.rs", 50, 90),
    ];
    let projected = graph::Mock::new(nodes.clone(), [(root.clone(), child.clone())]);
    let stale = graph::Mock::new(
        nodes,
        [
            (root.clone(), child.clone()),
            (root.clone(), sibling.clone()),
            (sibling.clone(), child.clone()),
        ],
    );
    let projection = projected.project(&artifact).expect("project artifact");

    let descendants = stale
        .bounds(&projection, &[graph::Rule::Descendants(root.clone())])
        .expect("descendant bounds");
    let ancestors = stale
        .bounds(&projection, &[graph::Rule::Ancestors(child.clone())])
        .expect("ancestor bounds");

    assert!(descendants.span(&child).is_some());
    assert!(descendants.span(&sibling).is_none());
    assert!(ancestors.span(&root).is_some());
    assert!(ancestors.span(&sibling).is_none());
}

#[test]
fn resolving_target_outside_bounds_fails() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(&projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");

    let err = graph
        .resolve(&projection, &bounds, &sibling)
        .expect_err("outside target must fail");

    assert!(matches!(err, graph::Error::Outside(_)));
}

#[test]
fn material_span_outside_writable_surface_fails() {
    let (artifact, graph, _, child, _) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(&projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let allowed = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        bounds,
        surface::Area::new([graph::Span::new(
            child.clone(),
            "src/lib.rs",
            10,
            20,
            href("file:lib:v1"),
        )]),
    )
    .expect("grant");
    let touch = surface::Touch::new(allowed, "replacement");
    let err = grant
        .check(surface::Draft {
            proposal: "proposal:outside",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[touch],
        })
        .expect_err("outside material span must fail");

    assert!(matches!(err, surface::Error::OutsideMaterial(_)));
}

#[test]
fn expected_file_hash_mismatch_fails() {
    let (artifact, graph, _, child, _) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(&projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let resolved = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        bounds,
        surface::Area::new([graph::Span::new(
            child,
            "src/lib.rs",
            10,
            40,
            href("file:lib:changed"),
        )]),
    )
    .expect("grant");
    let touch = surface::Touch::new(resolved.clone(), "replacement");
    let err = grant
        .check(surface::Draft {
            proposal: "proposal:hash",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[touch],
        })
        .expect_err("hash mismatch must fail before proposal application");

    assert!(matches!(
        err,
        surface::Error::HashMismatch {
            expected,
            actual,
            ..
        } if expected == href("file:lib:changed") && actual == href("file:lib:v1")
    ));

    assert_eq!(resolved.hash(), &href("file:lib:v1"));
}

#[test]
fn valid_proposal_produces_artifact_delta_result() {
    let (artifact, graph, _, child, _) = fixture();
    let harness = harness::Mock::new(graph);
    let projection = harness
        .graph()
        .project(&artifact)
        .expect("project artifact");
    let bounds = harness
        .graph()
        .bounds(&projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let resolved = harness
        .graph()
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        bounds,
        surface::Area::new([resolved.clone()]),
    )
    .expect("grant");
    let after = aref("artifact:after", "tree:after");
    let touch = surface::Touch::new(resolved, "fn child() {}");
    let (proposal, _run) = harness
        .propose(harness::Input {
            proposal: "proposal:valid",
            run: "run:valid",
            base: artifact.reference(),
            after: after.clone(),
            touches: vec![touch.clone()],
        })
        .expect("propose");
    let check = grant.check(proposal.draft()).expect("check proposal");
    let applied = harness
        .apply_checked(proposal, check)
        .expect("apply checked proposal");

    assert_eq!(applied.delta().base(), artifact.reference());
    assert_eq!(applied.delta().after(), &after);
    assert_eq!(applied.delta().touches(), &[touch]);
}

#[test]
fn checked_apply_rejects_mismatch() {
    let (artifact, graph, _, child, _) = fixture();
    let harness = harness::Mock::new(graph);
    let projection = harness
        .graph()
        .project(&artifact)
        .expect("project artifact");
    let bounds = harness
        .graph()
        .bounds(&projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let resolved = harness
        .graph()
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        bounds,
        surface::Area::new([resolved.clone()]),
    )
    .expect("grant");
    let after = aref("artifact:after", "tree:after");
    let touch = surface::Touch::new(resolved, "fn child() {}");
    let (proposal, _run) = harness
        .propose(harness::Input {
            proposal: "proposal:valid",
            run: "run:valid",
            base: artifact.reference(),
            after: after.clone(),
            touches: vec![touch.clone()],
        })
        .expect("propose");
    let check = grant.check(proposal.draft()).expect("check proposal");
    let (other, _run) = harness
        .propose(harness::Input {
            proposal: "proposal:other",
            run: "run:other",
            base: artifact.reference(),
            after,
            touches: vec![touch],
        })
        .expect("propose other");

    let err = harness
        .apply_checked(other, check)
        .expect_err("checked apply must reject a different proposal");

    assert!(matches!(err, harness::Error::CheckMismatch));
}

#[test]
fn parent_ancestor_rules_can_narrow_but_not_widen_grant() {
    let (artifact, graph, root, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let initial = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(root.clone()),
                graph::Rule::Include(child.clone()),
            ],
        )
        .expect("initial bounds");
    let root_span = graph
        .resolve(&projection, &initial, &root)
        .expect("resolve root");
    let child_span = graph
        .resolve(&projection, &initial, &child)
        .expect("resolve child");
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        initial,
        surface::Area::new([root_span, child_span]),
    )
    .expect("grant");

    let ancestors = graph
        .bounds(&projection, &[graph::Rule::Ancestors(child)])
        .expect("ancestor bounds");
    let narrowed = grant.narrow(ancestors).expect("ancestor narrowing");
    let sibling_descendants = graph
        .bounds(&projection, &[graph::Rule::Descendants(root)])
        .expect("descendant bounds");
    let widened = narrowed
        .narrow(sibling_descendants)
        .expect_err("sibling target must not widen grant");

    assert!(matches!(widened, surface::Error::Widens));
    assert!(projection.span(&sibling).is_some());
}
