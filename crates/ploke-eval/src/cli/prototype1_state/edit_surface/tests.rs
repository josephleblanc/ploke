use std::path::PathBuf;

use ploke_core::{EmbeddingData, TrackingHash};
use uuid::Uuid;

use crate::loop_graph::ArtifactId;

use super::{graph, harness, surface, tui};
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

#[test]
fn tui_projection_binding_carries_artifact_identity_and_projection_hash() {
    let artifact = aref("artifact:base", "tree:base");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.clone(),
    );

    assert_eq!(projection.id(), "projection:tui-tools");
    assert_eq!(projection.hash(), &href("projection:hash"));
    assert_eq!(projection.artifact(), &artifact);
}

#[test]
fn tui_rules_and_bounds_record_digest_and_source() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.reference().clone(),
    );
    let rule = tui::Rule::inline("include ploke-tui rag editing surface");
    let bounds = tui::Bounds::new(
        projection,
        [rule.clone()],
        tui::Source::Derived("graph bounds from tui tool surface rule".to_string()),
        graph_bounds,
    )
    .expect("adapter bounds");

    assert_eq!(bounds.rules(), &[rule]);
    assert!(matches!(bounds.rules()[0].source(), tui::Source::Inline(_)));
    assert!(!bounds.rules()[0].digest().as_str().is_empty());
    assert!(matches!(bounds.source(), tui::Source::Derived(_)));
    assert!(!bounds.digest().as_str().is_empty());
}

#[test]
fn tui_material_span_converts_to_touch_after_expected_hash_check() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.reference().clone(),
    );
    let bounds = tui::Bounds::new(
        projection,
        [tui::Rule::named("tui-tool-edit-surface")],
        tui::Source::Named("tui-tool-edit-surface".to_string()),
        graph_bounds,
    )
    .expect("adapter bounds");
    let material = tui::MaterialSpan::new(
        child,
        "src/lib.rs",
        10,
        40,
        href("file:lib:v1"),
        tui::MaterialSource::DbExact {
            relation: "function".to_string(),
            canon: "crate::child".to_string(),
        },
    );

    let touch = bounds
        .touch(&artifact, material, "fn child() {}")
        .expect("validated touch");

    assert_eq!(touch.span().path(), &PathBuf::from("src/lib.rs"));
    assert_eq!(touch.replacement(), "fn child() {}");
}

#[test]
fn tui_material_span_rejects_expected_hash_mismatch_before_touch() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.reference().clone(),
    );
    let bounds = tui::Bounds::new(
        projection,
        [tui::Rule::named("tui-tool-edit-surface")],
        tui::Source::Named("tui-tool-edit-surface".to_string()),
        graph_bounds,
    )
    .expect("adapter bounds");
    let material = tui::MaterialSpan::new(
        child,
        "src/lib.rs",
        10,
        40,
        href("file:lib:stale"),
        tui::MaterialSource::DbExact {
            relation: "function".to_string(),
            canon: "crate::child".to_string(),
        },
    );

    let err = bounds
        .touch(&artifact, material, "fn child() {}")
        .expect_err("stale material hash must fail");

    assert!(matches!(
        err,
        tui::Error::ExpectedHashMismatch {
            expected,
            actual,
            ..
        } if expected == href("file:lib:stale") && actual == href("file:lib:v1")
    ));
}

#[test]
fn tui_adapter_rejects_stale_projection_artifact_mismatch() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");
    let stale_projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        aref("artifact:other", "tree:other"),
    );

    let err = tui::Bounds::new(
        stale_projection,
        [tui::Rule::named("tui-tool-edit-surface")],
        tui::Source::Named("tui-tool-edit-surface".to_string()),
        graph_bounds,
    )
    .expect_err("projection artifact mismatch must fail");

    assert!(matches!(err, tui::Error::StaleProjection));
}

#[test]
fn tui_adapter_rejects_out_of_bounds_canonical_target() {
    let (artifact, graph, _, child, sibling) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.reference().clone(),
    );
    let bounds = tui::Bounds::new(
        projection,
        [tui::Rule::named("tui-tool-edit-surface")],
        tui::Source::Named("tui-tool-edit-surface".to_string()),
        graph_bounds,
    )
    .expect("adapter bounds");
    let material = tui::MaterialSpan::new(
        sibling.clone(),
        "src/lib.rs",
        50,
        90,
        href("file:lib:v1"),
        tui::MaterialSource::DbExact {
            relation: "function".to_string(),
            canon: "crate::sibling".to_string(),
        },
    );

    let err = bounds
        .touch(&artifact, material, "fn sibling() {}")
        .expect_err("out-of-bounds canonical target must fail");

    assert!(matches!(err, tui::Error::OutsideBounds(target) if target == sibling));
}

#[test]
fn tui_adapter_rejects_auto_apply_stage_request() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.reference().clone(),
    );
    let bounds = tui::Bounds::new(
        projection.clone(),
        [tui::Rule::named("tui-tool-edit-surface")],
        tui::Source::Named("tui-tool-edit-surface".to_string()),
        graph_bounds,
    )
    .expect("adapter bounds");
    let touch = bounds
        .touch(
            &artifact,
            tui::MaterialSpan::new(
                child,
                "src/lib.rs",
                10,
                40,
                href("file:lib:v1"),
                tui::MaterialSource::DbExact {
                    relation: "function".to_string(),
                    canon: "crate::child".to_string(),
                },
            ),
            "fn child() {}",
        )
        .expect("touch");

    let err = tui::Proposal::stage(tui::Stage {
        proposal: "proposal:auto",
        run: "run:auto",
        base: artifact.reference(),
        after: aref("artifact:after", "tree:after"),
        projection: &projection,
        touches: vec![touch],
        auto_apply: true,
    })
    .expect_err("auto apply must be rejected");

    assert!(matches!(err, tui::Error::AutoApply));
}

#[test]
fn tui_apply_evidence_is_all_applied_or_rejected() {
    let (artifact, graph, _, child, sibling) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(
            &graph_projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling.clone()),
            ],
        )
        .expect("derive bounds");
    let projection = tui::Projection::new(
        "projection:tui-tools",
        href("projection:hash"),
        artifact.reference().clone(),
    );
    let bounds = tui::Bounds::new(
        projection.clone(),
        [tui::Rule::named("tui-tool-edit-surface")],
        tui::Source::Named("tui-tool-edit-surface".to_string()),
        graph_bounds.clone(),
    )
    .expect("adapter bounds");
    let child_touch = bounds
        .touch(
            &artifact,
            tui::MaterialSpan::new(
                child,
                "src/lib.rs",
                10,
                40,
                href("file:lib:v1"),
                tui::MaterialSource::DbExact {
                    relation: "function".to_string(),
                    canon: "crate::child".to_string(),
                },
            ),
            "fn child() {}",
        )
        .expect("child touch");
    let sibling_touch = bounds
        .touch(
            &artifact,
            tui::MaterialSpan::new(
                sibling,
                "src/lib.rs",
                50,
                90,
                href("file:lib:v1"),
                tui::MaterialSource::DbExact {
                    relation: "function".to_string(),
                    canon: "crate::sibling".to_string(),
                },
            ),
            "fn sibling() {}",
        )
        .expect("sibling touch");
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        graph_bounds,
        surface::Area::new([child_touch.span().clone(), sibling_touch.span().clone()]),
    )
    .expect("grant");
    let proposal = tui::Proposal::stage(tui::Stage {
        proposal: "proposal:apply",
        run: "run:apply",
        base: artifact.reference(),
        after: aref("artifact:after", "tree:after"),
        projection: &projection,
        touches: vec![child_touch.clone(), sibling_touch.clone()],
        auto_apply: false,
    })
    .expect("stage");
    let check = grant.check(proposal.draft()).expect("surface check");

    let rejected = tui::Apply::from_results(
        proposal.clone(),
        check.clone(),
        vec![tui::Write::applied(&child_touch, href("file:lib:v2"))],
    )
    .expect("partial apply evidence");
    assert!(matches!(rejected, tui::Apply::Rejected { .. }));

    let applied = tui::Apply::from_results(
        proposal,
        check,
        vec![
            tui::Write::applied(&child_touch, href("file:lib:v2")),
            tui::Write::applied(&sibling_touch, href("file:lib:v2")),
        ],
    )
    .expect("all apply evidence");

    assert!(matches!(applied, tui::Apply::Applied { .. }));
}

#[test]
fn tui_adapter_wraps_db_resolved_embedding_data() {
    let target = graph::Target::new("src/lib.rs", "child");
    let hash = TrackingHash(Uuid::new_v4());
    let data = EmbeddingData {
        id: Uuid::new_v4(),
        name: "child".to_string(),
        file_path: PathBuf::from("src/lib.rs"),
        file_tracking_hash: hash,
        start_byte: 10,
        end_byte: 40,
        node_tracking_hash: TrackingHash(Uuid::new_v4()),
        namespace: Uuid::new_v4(),
    };

    let material = tui::MaterialSpan::from_embedding(target, "function", "crate::child", &data);

    assert_eq!(material.path(), &PathBuf::from("src/lib.rs"));
    assert_eq!(
        material.expected_hash(),
        &surface::Hash::new(hash.0.to_string())
    );
    assert!(matches!(
        material.source(),
        tui::MaterialSource::DbExact { .. }
    ));
}
