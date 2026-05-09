use std::path::PathBuf;
use std::sync::Arc;

use ploke_core::{EmbeddingData, TrackingHash};
use ploke_db::NodeType;
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db, workspace_root};
use uuid::Uuid;

use crate::cli::prototype1_state::history::EvidenceRef;
use crate::loop_graph::ArtifactId;

use super::{diagnosis, graph, harness, surface, tui};
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

fn tui_rule(text: &str) -> tui::Rule {
    tui::Rule::named("tui-tool-edit-surface", "v1", text)
}

fn tui_source(text: &str) -> tui::Source {
    tui::Source::derived("tui-tool-edit-surface", "v1", text)
}

fn tui_projection(graph_projection: &graph::Projection) -> tui::Projection {
    tui::Projector::new(
        "projection:tui-tools",
        tui_source("graph projection for tui tool edit surface"),
        [tui_rule(
            "include crates/ploke-tui/src/tools and rag edit files",
        )],
    )
    .project(graph_projection)
}

fn broad_objective_spec(summary: &str) -> surface::ObjectiveSpec {
    surface::ObjectiveSpec::new(
        summary,
        surface::ObjectiveKind::BroadRulingParent,
        surface::TargetMetric::OperationalAndProtocolScore,
        surface::WritableIntent::BroadEditableSurface,
    )
    .with_constraints([
        surface::ObjectiveConstraint::PreserveProtectedCore,
        surface::ObjectiveConstraint::NoPolicySurfaceMutation,
    ])
    .with_success_criteria([
        surface::SuccessCriterion::CandidateGenerationSucceeds,
        surface::SuccessCriterion::ProtectedCoreUntouched,
    ])
    .with_requested_candidates(2)
}

#[test]
fn broad_surface_admits_writable_touch_outside_protected_core() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling.clone()),
            ],
        )
        .expect("bounds");
    let child_span = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let sibling_span = graph
        .resolve(&projection, &bounds, &sibling)
        .expect("resolve sibling");
    let objective = surface::EditObjective::new(
        broad_objective_spec("improve broad parent quality"),
        [],
        [EvidenceRef::new("history:context:score-trend")],
    );
    let editable = surface::EditableSurface::broad(
        objective,
        artifact.reference().clone(),
        bounds,
        surface::ProtectedCore::new([child_span]),
    )
    .expect("editable surface");
    let touch = surface::Touch::new(sibling_span, "fn sibling() {}");

    let check = editable
        .grant()
        .check(surface::Draft {
            proposal: "proposal:broad-allowed",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[touch],
        })
        .expect("touch outside protected core should pass");

    let (base, after, touches) = check.into_parts();
    assert_eq!(base, artifact.reference().clone());
    assert_eq!(after, aref("artifact:after", "tree:after"));
    assert_eq!(touches.len(), 1);
}

#[test]
fn broad_surface_rejects_touch_inside_protected_core() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling),
            ],
        )
        .expect("bounds");
    let child_span = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let objective = surface::EditObjective::new(
        broad_objective_spec("improve broad parent quality"),
        [],
        [EvidenceRef::new("history:context:score-trend")],
    );
    let editable = surface::EditableSurface::broad(
        objective,
        artifact.reference().clone(),
        bounds,
        surface::ProtectedCore::new([child_span.clone()]),
    )
    .expect("editable surface");
    let touch = surface::Touch::new(child_span.clone(), "fn child() {}");

    let err = editable
        .grant()
        .check(surface::Draft {
            proposal: "proposal:broad-forbidden",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[touch],
        })
        .expect_err("protected core touch must fail");

    assert!(matches!(err, surface::Error::Forbidden(span) if span == child_span));
}

#[test]
fn broad_surface_objective_records_context_without_diagnosis_specificity() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling),
            ],
        )
        .expect("bounds");
    let child_span = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let objective = surface::EditObjective::new(
        broad_objective_spec("improve broad parent quality"),
        [],
        [
            EvidenceRef::new("history:context:score-trend"),
            EvidenceRef::new("history:context:recent-attempts"),
        ],
    );
    let editable = surface::EditableSurface::broad(
        objective.clone(),
        artifact.reference().clone(),
        bounds,
        surface::ProtectedCore::new([child_span]),
    )
    .expect("editable surface");

    assert_eq!(
        editable.objective().intent(),
        "improve broad parent quality"
    );
    assert!(editable.objective().diagnosis_refs().is_empty());
    assert_eq!(
        editable.objective().context_refs(),
        &[
            EvidenceRef::new("history:context:score-trend"),
            EvidenceRef::new("history:context:recent-attempts")
        ]
    );
    assert_eq!(editable.objective(), &objective);
}

#[test]
fn surface_request_admits_parent_context_into_broad_surface() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling.clone()),
            ],
        )
        .expect("bounds");
    let child_span = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve protected child");
    let sibling_span = graph
        .resolve(&projection, &bounds, &sibling)
        .expect("resolve writable sibling");
    let objective_spec = surface::ObjectiveSpec::new(
        "let the parent improve the edit harness while preserving protected form",
        surface::ObjectiveKind::ReduceKnownFailure {
            limiter: diagnosis::Limiter::InvalidCandidateGeneration,
            failure_kind: diagnosis::FailureKind::SemanticEditResolution,
        },
        surface::TargetMetric::InvalidEditSurfaceCandidates,
        surface::WritableIntent::SemanticResolution,
    )
    .with_constraints([
        surface::ObjectiveConstraint::PreserveProtectedCore,
        surface::ObjectiveConstraint::ExactResolutionOnly,
    ])
    .with_success_criteria([
        surface::SuccessCriterion::CandidateGenerationSucceeds,
        surface::SuccessCriterion::ProtectedCoreUntouched,
        surface::SuccessCriterion::MetricImproves(
            surface::TargetMetric::InvalidEditSurfaceCandidates,
        ),
    ])
    .with_requested_candidates(1);
    let objective = surface::EditObjective::new(
        objective_spec.clone(),
        [EvidenceRef::new(
            "history:diagnosis:semantic-edit-resolution",
        )],
        [
            EvidenceRef::new("history:context:recent-child-outcomes"),
            EvidenceRef::new("history:context:score-trend"),
        ],
    );

    let editable = surface::SurfaceRequest::broad(
        objective.clone(),
        artifact.reference().clone(),
        bounds,
        surface::ProtectedCore::new([child_span.clone()]),
    )
    .admit()
    .expect("surface request should admit broad surface");

    assert_eq!(editable.objective(), &objective);
    assert_eq!(
        editable.objective().diagnosis_refs(),
        &[EvidenceRef::new(
            "history:diagnosis:semantic-edit-resolution"
        )]
    );
    assert_eq!(
        editable.objective().context_refs(),
        &[
            EvidenceRef::new("history:context:recent-child-outcomes"),
            EvidenceRef::new("history:context:score-trend")
        ]
    );
    assert_eq!(editable.objective().spec(), &objective_spec);
    assert!(matches!(
        editable.objective().spec().kind(),
        surface::ObjectiveKind::ReduceKnownFailure {
            limiter: diagnosis::Limiter::InvalidCandidateGeneration,
            failure_kind: diagnosis::FailureKind::SemanticEditResolution,
        }
    ));
    assert_eq!(
        editable.objective().spec().target_metric(),
        surface::TargetMetric::InvalidEditSurfaceCandidates
    );
    assert_eq!(
        editable.objective().spec().writable_intent(),
        surface::WritableIntent::SemanticResolution
    );
    assert_eq!(
        editable.objective().spec().constraints(),
        &[
            surface::ObjectiveConstraint::PreserveProtectedCore,
            surface::ObjectiveConstraint::ExactResolutionOnly,
        ]
    );
    assert_eq!(
        editable.objective().spec().success_criteria(),
        &[
            surface::SuccessCriterion::CandidateGenerationSucceeds,
            surface::SuccessCriterion::ProtectedCoreUntouched,
            surface::SuccessCriterion::MetricImproves(
                surface::TargetMetric::InvalidEditSurfaceCandidates,
            ),
        ]
    );
    assert_eq!(editable.objective().spec().requested_candidates(), Some(1));

    let allowed = surface::Touch::new(sibling_span, "fn sibling() {}");
    editable
        .grant()
        .check(surface::Draft {
            proposal: "proposal:parent-context-broad-surface",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[allowed],
        })
        .expect("ordinary broad-surface write should pass");

    let forbidden = surface::Touch::new(child_span.clone(), "fn child() {}");
    let err = editable
        .grant()
        .check(surface::Draft {
            proposal: "proposal:parent-context-protected-core",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[forbidden],
        })
        .expect_err("protected-core write should fail");

    assert!(matches!(err, surface::Error::Forbidden(span) if span == child_span));
}

#[test]
fn replay_shaped_rejected_surface_attempt_admits_semantic_edit_surface_request() {
    let (artifact, graph, _, child, sibling) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(
            &projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling.clone()),
            ],
        )
        .expect("bounds");
    let child_span = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve protected child");
    let sibling_span = graph
        .resolve(&projection, &bounds, &sibling)
        .expect("resolve writable sibling");

    let payload = crate::cli::prototype1_state::history::EvaluationPayload::builder(
        crate::cli::prototype1_state::history::SubjectRef::new(
            "candidate:replay-shaped-rejected-surface-attempt",
        ),
        crate::cli::prototype1_state::history::ProcedureRef::new(
            crate::successor_selection::PROCEDURE_ID,
        ),
    )
    .source_ref(crate::cli::prototype1_state::history::EvidenceRef::new(
        "history:surface_attempt:replay-shaped",
    ))
    .surface_attempt_evidence(
        crate::cli::prototype1_state::history::surface_attempt::Evidence::rejected(
            "prototype1:tui-edit-surface:deterministic-v1",
            "proposal-rejected",
            "run-rejected",
            "ploke_tui_tools",
            PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
            "one or more touched spans were rejected",
        ),
    )
    .build();

    let diagnosis = diagnosis::classify(&payload).expect("diagnosis");
    let context_refs = vec![
        crate::cli::prototype1_state::history::EvidenceRef::new(
            "history:context:replay-shaped-parent-route",
        ),
        crate::cli::prototype1_state::history::EvidenceRef::new("history:context:graph-bounds"),
    ];
    let (objective, request) = super::semantic_resolution(
        &diagnosis,
        artifact.reference().clone(),
        bounds,
        surface::ProtectedCore::new([child_span.clone()]),
        context_refs.clone(),
    );
    let editable = request.admit().expect("surface request should admit");

    assert_eq!(
        objective.spec().kind(),
        surface::ObjectiveKind::ReduceKnownFailure {
            limiter: diagnosis::Limiter::InvalidCandidateGeneration,
            failure_kind: diagnosis::FailureKind::SemanticEditResolution,
        }
    );
    assert_eq!(
        objective.spec().target_metric(),
        surface::TargetMetric::InvalidEditSurfaceCandidates
    );
    assert_eq!(
        objective.spec().writable_intent(),
        surface::WritableIntent::SemanticResolution
    );
    assert_eq!(
        objective.spec().constraints(),
        &[
            surface::ObjectiveConstraint::PreserveProtectedCore,
            surface::ObjectiveConstraint::ExactResolutionOnly,
        ]
    );
    assert_eq!(
        objective.spec().success_criteria(),
        &[
            surface::SuccessCriterion::CandidateGenerationSucceeds,
            surface::SuccessCriterion::ProtectedCoreUntouched,
            surface::SuccessCriterion::MetricImproves(
                surface::TargetMetric::InvalidEditSurfaceCandidates,
            ),
        ]
    );
    assert_eq!(objective.spec().requested_candidates(), Some(1));
    assert_eq!(
        objective.diagnosis_refs(),
        &[crate::cli::prototype1_state::history::EvidenceRef::new(
            "history:surface_attempt:replay-shaped",
        )]
    );
    assert_eq!(objective.context_refs(), context_refs.as_slice());
    assert_eq!(editable.objective(), &objective);

    let allowed = surface::Touch::new(sibling_span, "fn sibling() {}");
    editable
        .grant()
        .check(surface::Draft {
            proposal: "proposal:replay-shaped-semantic-route",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[allowed],
        })
        .expect("ordinary write should pass");

    let forbidden = surface::Touch::new(child_span.clone(), "fn child() {}");
    let err = editable
        .grant()
        .check(surface::Draft {
            proposal: "proposal:replay-shaped-semantic-route-protected-core",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[forbidden],
        })
        .expect_err("protected-core write should fail");

    assert!(matches!(err, surface::Error::Forbidden(span) if span == child_span));
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
fn tui_projection_digest_is_deterministic_from_canonical_inputs() {
    let (artifact, graph, _, _, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let projector = tui::Projector::new(
        "projection:tui-tools",
        tui_source("graph projection for tui tool edit surface"),
        [tui_rule(
            "include crates/ploke-tui/src/tools and rag edit files",
        )],
    );

    let projection = projector.project(&graph_projection);
    let same = projector.project(&graph_projection);

    assert_eq!(projection.id(), "projection:tui-tools");
    assert_eq!(projection.hash(), same.hash());
    assert_eq!(projection.artifact(), artifact.reference());
    assert_ne!(projection.hash(), &href("projection:hash"));
}

#[test]
fn tui_projection_hash_cannot_be_supplied_as_proof() {
    let (artifact, graph, _, _, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let projection = tui_projection(&graph_projection);

    assert_ne!(projection.hash(), &href("caller:supplied"));
    assert_eq!(projection.artifact(), artifact.reference());
}

#[test]
fn tui_rules_and_bounds_record_digest_and_source() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");
    let rule = tui::Rule::inline(
        "inline-edit-surface",
        "v1",
        "include ploke-tui rag editing surface",
    );
    let projection = tui::Projector::new(
        "projection:tui-tools",
        tui::Source::derived(
            "tui-tool-edit-surface",
            "v1",
            "graph bounds from tui tool surface rule",
        ),
        [rule.clone()],
    )
    .project(&graph_projection);
    let bounds = tui::Bounds::new(projection, graph_bounds).expect("adapter bounds");

    assert_eq!(bounds.rules(), &[rule]);
    assert!(matches!(bounds.rules()[0].source(), tui::Source::Inline(_)));
    assert!(!bounds.rules()[0].digest().as_str().is_empty());
    assert!(matches!(bounds.source(), tui::Source::Derived(_)));
    assert!(!bounds.digest().as_str().is_empty());
}

#[test]
fn tui_rule_definition_changes_bounds_digest() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");
    let first = tui::Projector::new(
        "projection:tui-tools",
        tui_source("graph projection for tui tool edit surface"),
        [tui::Rule::named(
            "tui-tool-edit-surface",
            "v1",
            "include tools and rag editing",
        )],
    )
    .project(&graph_projection);
    let second = tui::Projector::new(
        "projection:tui-tools",
        tui_source("graph projection for tui tool edit surface changed"),
        [tui::Rule::named(
            "tui-tool-edit-surface",
            "v1",
            "include tools only",
        )],
    )
    .project(&graph_projection);

    let first = tui::Bounds::new(first, graph_bounds.clone()).expect("first bounds");
    let second = tui::Bounds::new(second, graph_bounds).expect("second bounds");

    assert_ne!(first.rules()[0].digest(), second.rules()[0].digest());
    assert_ne!(first.digest(), second.digest());
}

#[test]
fn tui_material_span_converts_to_touch_after_expected_hash_check() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let bounds = tui::Bounds::new(projection, graph_bounds).expect("adapter bounds");
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
    let projection = tui_projection(&graph_projection);
    let bounds = tui::Bounds::new(projection, graph_bounds).expect("adapter bounds");
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
    let other = surface::Artifact::new(
        aref("artifact:other", "tree:other"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v1"))],
    );
    let other_graph_projection = graph.project(&other).expect("project other artifact");
    let stale_projection = tui::Projector::new(
        "projection:tui-tools",
        tui_source("graph projection for tui tool edit surface"),
        [tui_rule(
            "include crates/ploke-tui/src/tools and rag edit files",
        )],
    )
    .project(&other_graph_projection);

    let err = tui::Bounds::new(stale_projection, graph_bounds)
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
    let projection = tui_projection(&graph_projection);
    let bounds = tui::Bounds::new(projection, graph_bounds).expect("adapter bounds");
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
    let projection = tui_projection(&graph_projection);
    let bounds = tui::Bounds::new(projection.clone(), graph_bounds).expect("adapter bounds");
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
    let projection = tui_projection(&graph_projection);
    let bounds =
        tui::Bounds::new(projection.clone(), graph_bounds.clone()).expect("adapter bounds");
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
    assert!(rejected.is_rejected());

    let reported = tui::Apply::from_results(
        proposal,
        check,
        vec![
            tui::Write::applied(&child_touch, href("file:lib:v2")),
            tui::Write::applied(&sibling_touch, href("file:lib:v2")),
        ],
    )
    .expect("all apply evidence");

    assert!(reported.is_reported());
}

#[test]
fn tui_apply_wrong_after_artifact_hash_fails() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let bounds =
        tui::Bounds::new(projection.clone(), graph_bounds.clone()).expect("adapter bounds");
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
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        graph_bounds,
        surface::Area::new([touch.span().clone()]),
    )
    .expect("grant");
    let proposal = tui::Proposal::stage(tui::Stage {
        proposal: "proposal:apply",
        run: "run:apply",
        base: artifact.reference(),
        after: aref("artifact:after", "tree:after"),
        projection: &projection,
        touches: vec![touch.clone()],
        auto_apply: false,
    })
    .expect("stage");
    let check = grant.check(proposal.draft()).expect("surface check");
    let reported = tui::Apply::from_results(
        proposal,
        check,
        vec![tui::Write::applied(&touch, href("file:lib:v2"))],
    )
    .expect("reported apply");
    let wrong_after = surface::Artifact::new(
        aref("artifact:after", "tree:wrong"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v2"))],
    );

    let err = reported
        .validate(&wrong_after)
        .expect_err("wrong after artifact must fail");

    assert!(matches!(err, tui::Error::AfterArtifactMismatch { .. }));
}

#[test]
fn tui_apply_wrong_touched_after_hash_fails() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let bounds =
        tui::Bounds::new(projection.clone(), graph_bounds.clone()).expect("adapter bounds");
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
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        graph_bounds,
        surface::Area::new([touch.span().clone()]),
    )
    .expect("grant");
    let after = aref("artifact:after", "tree:after");
    let proposal = tui::Proposal::stage(tui::Stage {
        proposal: "proposal:apply",
        run: "run:apply",
        base: artifact.reference(),
        after: after.clone(),
        projection: &projection,
        touches: vec![touch.clone()],
        auto_apply: false,
    })
    .expect("stage");
    let check = grant.check(proposal.draft()).expect("surface check");
    let reported = tui::Apply::from_results(
        proposal,
        check,
        vec![tui::Write::applied(&touch, href("file:lib:v2"))],
    )
    .expect("reported apply");
    let wrong_after = surface::Artifact::new(
        after,
        [(PathBuf::from("src/lib.rs"), href("file:lib:wrong"))],
    );

    let err = reported
        .validate(&wrong_after)
        .expect_err("wrong touched after hash must fail");

    assert!(matches!(
        err,
        tui::Error::AfterHashMismatch {
            expected,
            actual,
            ..
        } if expected == href("file:lib:v2") && actual == href("file:lib:wrong")
    ));
}

#[test]
fn tui_apply_valid_after_artifact_validation_produces_delta() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let bounds =
        tui::Bounds::new(projection.clone(), graph_bounds.clone()).expect("adapter bounds");
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
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        graph_bounds,
        surface::Area::new([touch.span().clone()]),
    )
    .expect("grant");
    let after = aref("artifact:after", "tree:after");
    let proposal = tui::Proposal::stage(tui::Stage {
        proposal: "proposal:apply",
        run: "run:apply",
        base: artifact.reference(),
        after: after.clone(),
        projection: &projection,
        touches: vec![touch.clone()],
        auto_apply: false,
    })
    .expect("stage");
    let check = grant.check(proposal.draft()).expect("surface check");
    let reported = tui::Apply::from_results(
        proposal,
        check,
        vec![tui::Write::applied(&touch, href("file:lib:v2"))],
    )
    .expect("reported apply");
    let after_artifact = surface::Artifact::new(
        after.clone(),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v2"))],
    );

    let applied = reported
        .validate(&after_artifact)
        .expect("valid after artifact");

    assert!(applied.is_applied());
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

#[tokio::test(flavor = "multi_thread")]
async fn real_tui_resolver_touch_is_checked_before_adapter_apply() {
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let runtime = ploke_tui::app::commands::harness::TestRuntime::new(&fixture_db);
    let fixture_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    runtime
        .setup_loaded_standalone_crate(fixture_root.clone())
        .await;
    let state = runtime.state_arc();
    let expected_path = fixture_root.join("src/structs.rs");
    let mut expected_nodes = ploke_db::helpers::graph_resolve_exact(
        &fixture_db,
        NodeType::Struct.relation_str(),
        &expected_path,
        &["crate".to_string(), "structs".to_string()],
        "SampleStruct",
    )
    .expect("fixture struct resolves");
    assert_eq!(expected_nodes.len(), 1, "fixture canonical must be unique");
    let expected_node = expected_nodes.remove(0);
    let expected_hash = href(&expected_node.file_tracking_hash.0.to_string());
    let expected_target = graph::Target::new(expected_path.clone(), "SampleStruct");
    let request = ploke_tui::rag::utils::ApplyCodeEditRequest {
        edits: vec![ploke_tui::rag::utils::Edit::Canonical {
            file: "src/structs.rs".to_string(),
            canon: "crate::structs::SampleStruct".to_string(),
            node_type: NodeType::Struct,
            code: "pub struct SampleStruct { pub field: String, pub new_field: i32, }".to_string(),
        }],
        confidence: Some(0.95),
    };
    let writes = ploke_tui::rag::tools::resolve_code_edit_request(&state, &request)
        .await
        .expect("resolve edit request");
    assert_eq!(writes.len(), 1, "fixture canonical edit should resolve");
    let write = &writes[0];
    assert_eq!(write.file_path, expected_path);
    assert_eq!(write.expected_file_hash, expected_node.file_tracking_hash);
    assert_eq!(write.start_byte, expected_node.start_byte);
    assert_eq!(write.end_byte, expected_node.end_byte);

    let artifact = surface::Artifact::new(
        aref("artifact:base", "tree:base"),
        [(expected_path.clone(), expected_hash.clone())],
    );
    let graph = graph::Mock::new(
        vec![graph::Node::new(
            expected_target.clone(),
            expected_path.clone(),
            expected_node.start_byte,
            expected_node.end_byte,
        )],
        [],
    );
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(
            &graph_projection,
            &[graph::Rule::Include(expected_target.clone())],
        )
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let bounds = tui::Bounds::new(projection.clone(), graph_bounds.clone()).expect("bounds");
    let wrong_hash_artifact = surface::Artifact::new(
        aref("artifact:base", "tree:base"),
        [(expected_path.clone(), href("file:wrong"))],
    );
    let hash_err = bounds
        .touches(
            &wrong_hash_artifact,
            std::slice::from_ref(&expected_target),
            &writes,
        )
        .expect_err("independent hash mismatch must fail before apply");
    assert!(matches!(hash_err, tui::Error::ExpectedHashMismatch { .. }));
    let resolved = bounds
        .touches(&artifact, std::slice::from_ref(&expected_target), &writes)
        .expect("lower resolved writes");
    let touches = resolved.into_vec();
    let grant = surface::Grant::new(
        artifact.reference().clone(),
        graph_bounds,
        surface::Area::new(touches.iter().map(|touch| touch.span().clone())),
    )
    .expect("grant");
    let harness = harness::Mock::new(graph);
    let after = aref("artifact:after", "tree:after");
    let (proposal, _run) = harness
        .propose(harness::Input {
            proposal: "proposal:real-tui-resolver",
            run: "run:real-tui-resolver",
            base: artifact.reference(),
            after: after.clone(),
            touches: touches.clone(),
        })
        .expect("propose");
    let check = grant.check(proposal.draft()).expect("grant check");
    let applied = harness
        .apply_checked(proposal, check)
        .expect("checked apply");

    assert_eq!(applied.delta().base(), artifact.reference());
    assert_eq!(applied.delta().after(), &after);
    assert_eq!(applied.delta().touches(), touches.as_slice());
}

#[test]
fn tui_bounds_touches_requires_one_target_per_write() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("bounds");
    let projection = tui_projection(&graph_projection);
    let bounds = tui::Bounds::new(projection, graph_bounds).expect("bounds");
    let writes = vec![ploke_core::WriteSnippetData {
        id: Uuid::new_v4(),
        name: "child".to_string(),
        file_path: PathBuf::from("src/lib.rs"),
        expected_file_hash: TrackingHash(Uuid::new_v4()),
        start_byte: 10,
        end_byte: 40,
        replacement: "fn child() {}".to_string(),
        namespace: Uuid::new_v4(),
    }];

    let err = bounds
        .touches(&artifact, &[], &writes)
        .expect_err("target/write mismatch must fail");
    assert!(matches!(
        err,
        tui::Error::TargetWriteCountMismatch {
            targets: 0,
            writes: 1
        }
    ));
}
