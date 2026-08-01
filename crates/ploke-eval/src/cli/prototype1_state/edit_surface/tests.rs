use std::path::PathBuf;
use std::sync::Arc;

use ploke_core::{EmbeddingData, TrackingHash};
use ploke_db::NodeType;
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db, workspace_root};
use tokio::sync::oneshot;
use tokio::time::{Duration, Instant, sleep, timeout};
use uuid::Uuid;

use crate::cli::prototype1_state::edit_surface::route::semantic_resolution;
use crate::cli::prototype1_state::history::EvidenceRef;
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};

use super::{diagnosis, graph, harness, request_policy, surface, tui};
use graph::View;
use harness::Harness;

fn href(value: &str) -> surface::Hash {
    surface::Hash::new(value)
}

fn aref(id: &str, hash: &str) -> surface::Ref {
    surface::Ref::new(ArtifactId::new(id), href(hash))
}

fn surface_policy(value: &str) -> surface::SurfacePolicyId {
    surface::SurfacePolicyId::new(value)
}

fn artifact_coordinate(runtime: RuntimeId, artifact_id: ArtifactId) -> Coordinate {
    Coordinate {
        runtime_id: runtime,
        target: OperationTarget::Artifact { artifact_id },
    }
}

fn granted_coordinate(artifact: &surface::Artifact) -> Coordinate {
    artifact_coordinate(RuntimeId(Uuid::nil()), artifact.reference().id().clone())
}

fn granted_surface(
    artifact: &surface::Artifact,
    bounds: graph::Bounds,
    write: surface::Area,
) -> surface::Grant {
    surface::Grant::for_coordinate(
        granted_coordinate(artifact),
        surface_policy("surface-policy:test-grant-v1"),
        artifact.reference().clone(),
        bounds,
        write,
    )
    .expect("grant")
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

#[cfg(feature = "live_api_tests")]
fn live_openrouter_env_or_skip(test_name: &str) -> Option<ploke_tui::test_harness::OpenRouterEnv> {
    let env = ploke_tui::test_harness::openrouter_env();
    if env.is_none() {
        let message = format!(
            "skipping {test_name}: no OpenRouter credentials found through ploke-tui \
             test_harness::openrouter_env() (OPENROUTER_API_KEY process env or .env); \
             live 7.6 Router path was not exercised"
        );
        if strict_live_tests_requested() {
            panic!("{message}; PLOKE_RUN_LIVE_TESTS requested live execution");
        }
        eprintln!("{message}");
    }
    env
}

#[cfg(feature = "live_api_tests")]
fn live_google_env_or_skip(test_name: &str) -> bool {
    use ploke_llm::router_only::google::Google;

    crate::test_support::install_default_google_route_env();
    let route_config_available = Google::route_config_available().is_ok();
    let auth_config_available = Google::auth_config_available().is_ok();
    if route_config_available && auth_config_available {
        return true;
    }

    let missing = match (route_config_available, auth_config_available) {
        (false, false) => "GOOGLE_PROJECT_ID/GOOGLE_REGION route config and Google ADC auth",
        (false, true) => "GOOGLE_PROJECT_ID/GOOGLE_REGION route config",
        (true, false) => "Google ADC auth",
        (true, true) => unreachable!("handled above"),
    };
    let message = format!(
        "skipping {test_name}: active eval model uses the direct Google route, but missing {missing}; \
         live 7.6 Router path was not exercised"
    );
    if strict_live_tests_requested() {
        panic!("{message}; PLOKE_RUN_LIVE_TESTS requested live execution");
    }
    eprintln!("{message}");
    false
}

#[cfg(feature = "live_api_tests")]
fn strict_live_tests_requested() -> bool {
    std::env::var("PLOKE_RUN_LIVE_TESTS")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
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
    assert_eq!(
        editable.grant().coordinate(),
        &granted_coordinate(&artifact)
    );
    assert_eq!(
        editable.grant().policy().as_str(),
        "surface-policy:broad-v1"
    );
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
    assert_eq!(check.coordinate(), &granted_coordinate(&artifact));
    assert_eq!(check.policy().as_str(), "surface-policy:broad-v1");

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
    assert_eq!(
        editable.grant().coordinate(),
        &granted_coordinate(&artifact)
    );
    assert_eq!(
        editable.grant().policy().as_str(),
        "surface-policy:semantic-resolution-v1"
    );

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
            "workspace_except_ploke_eval",
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
    let (objective, request) = semantic_resolution(
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
    let grant = granted_surface(
        &artifact,
        bounds,
        surface::Area::new([graph::Span::new(
            child.clone(),
            "src/lib.rs",
            10,
            20,
            href("file:lib:v1"),
        )]),
    );
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
    let grant = granted_surface(
        &artifact,
        bounds,
        surface::Area::new([graph::Span::new(
            child,
            "src/lib.rs",
            10,
            40,
            href("file:lib:changed"),
        )]),
    );
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
    let grant = granted_surface(&artifact, bounds, surface::Area::new([resolved.clone()]));
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
    let grant = granted_surface(&artifact, bounds, surface::Area::new([resolved.clone()]));
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
    let grant = granted_surface(
        &artifact,
        initial,
        surface::Area::new([root_span, child_span]),
    );

    let ancestors = graph
        .bounds(&projection, &[graph::Rule::Ancestors(child)])
        .expect("ancestor bounds");
    let narrowed = grant.narrow(ancestors).expect("ancestor narrowing");
    assert_eq!(narrowed.coordinate(), grant.coordinate());
    assert_eq!(narrowed.policy(), grant.policy());
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
fn coordinate_target_artifact_mismatch_is_rejected() {
    let (artifact, graph, _, child, _) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(&projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let resolved = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let coordinate = artifact_coordinate(
        RuntimeId(Uuid::from_u128(1)),
        ArtifactId::new("artifact:other"),
    );

    let err = surface::Grant::for_coordinate(
        coordinate,
        surface_policy("surface-policy:broad-v1"),
        artifact.reference().clone(),
        bounds,
        surface::Area::new([resolved]),
    )
    .expect_err("coordinate target artifact mismatch must fail");

    assert!(matches!(
        err,
        surface::Error::CoordinateTargetMismatch {
            grant_artifact,
            coordinate_artifact,
        } if grant_artifact == ArtifactId::new("artifact:base")
            && coordinate_artifact == ArtifactId::new("artifact:other")
    ));
}

#[test]
fn accepted_coordinate_grants_expose_coordinate_and_policy() {
    let (artifact, graph, _, child, _) = fixture();
    let projection = graph.project(&artifact).expect("project artifact");
    let bounds = graph
        .bounds(&projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let resolved = graph
        .resolve(&projection, &bounds, &child)
        .expect("resolve child");
    let coordinate = artifact_coordinate(
        RuntimeId(Uuid::from_u128(2)),
        artifact.reference().id().clone(),
    );
    let policy = surface_policy("surface-policy:semantic-resolution-v1");

    let grant = surface::Grant::for_coordinate(
        coordinate.clone(),
        policy.clone(),
        artifact.reference().clone(),
        bounds,
        surface::Area::new([resolved.clone()]),
    )
    .expect("grant");

    assert_eq!(grant.coordinate(), &coordinate);
    assert_eq!(grant.policy(), &policy);
    assert_eq!(
        grant.policy().as_str(),
        "surface-policy:semantic-resolution-v1"
    );

    let check = grant
        .check(surface::Draft {
            proposal: "proposal:authority-carrying-grant",
            base: artifact.reference(),
            after: &aref("artifact:after", "tree:after"),
            touches: &[surface::Touch::new(resolved, "fn child() {}")],
        })
        .expect("surface check");
    assert_eq!(check.coordinate(), &coordinate);
    assert_eq!(check.policy(), &policy);

    let (base, after, touches) = check.into_parts();
    assert_eq!(base, artifact.reference().clone());
    assert_eq!(after, aref("artifact:after", "tree:after"));
    assert_eq!(touches.len(), 1);
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum BoundaryOutcome {
    Retry(RetryReason),
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RetryReason {
    NoEdit,
    OutsideSurface,
    Protected,
    PartialApply,
    Validation,
    Stage,
    Surface,
}

fn boundary_outcome(
    proposal: &str,
    run: &str,
    base: &surface::Artifact,
    projection: &tui::Projection,
    grant: &surface::Grant,
    touches: Vec<surface::Touch>,
    writes: Vec<tui::Write>,
    after: &surface::Artifact,
) -> BoundaryOutcome {
    if touches.is_empty() {
        return BoundaryOutcome::Retry(RetryReason::NoEdit);
    }

    let proposal = match tui::Proposal::stage(tui::Stage {
        proposal,
        run,
        base: base.reference(),
        after: after.reference().clone(),
        projection,
        touches,
        auto_apply: false,
    }) {
        Ok(proposal) => proposal,
        Err(_) => return BoundaryOutcome::Retry(RetryReason::Stage),
    };
    let check = match grant.check(proposal.draft()) {
        Ok(check) => check,
        Err(surface::Error::Forbidden(_)) => {
            return BoundaryOutcome::Retry(RetryReason::Protected);
        }
        Err(surface::Error::OutsideGraph(_) | surface::Error::OutsideMaterial(_)) => {
            return BoundaryOutcome::Retry(RetryReason::OutsideSurface);
        }
        Err(_) => return BoundaryOutcome::Retry(RetryReason::Surface),
    };
    let apply = match tui::Apply::from_results(proposal, check, writes) {
        Ok(apply) => apply,
        Err(_) => return BoundaryOutcome::Retry(RetryReason::Validation),
    };
    if apply.is_rejected() {
        return BoundaryOutcome::Retry(RetryReason::PartialApply);
    }

    match apply.validate(after) {
        Ok(apply) if apply.is_applied() => BoundaryOutcome::Applied,
        Ok(_) | Err(_) => BoundaryOutcome::Retry(RetryReason::Validation),
    }
}

fn first_applied(
    outcomes: impl IntoIterator<Item = BoundaryOutcome>,
    max_attempts: usize,
) -> Result<usize, RetryReason> {
    let mut last = RetryReason::NoEdit;
    for (index, outcome) in outcomes.into_iter().take(max_attempts).enumerate() {
        match outcome {
            BoundaryOutcome::Applied => return Ok(index + 1),
            BoundaryOutcome::Retry(reason) => last = reason,
        }
    }
    Err(last)
}

#[test]
fn adapter_boundary_rejects_no_edit_before_apply() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child)])
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let grant = granted_surface(&artifact, graph_bounds, surface::Area::new([]));
    let after = surface::Artifact::new(
        aref("artifact:after", "tree:after"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v1"))],
    );

    let outcome = boundary_outcome(
        "proposal:no-edit",
        "run:no-edit",
        &artifact,
        &projection,
        &grant,
        vec![],
        vec![],
        &after,
    );

    assert_eq!(outcome, BoundaryOutcome::Retry(RetryReason::NoEdit));
}

#[test]
fn adapter_boundary_rejects_out_of_surface_and_protected_touches() {
    let (artifact, graph, _, child, sibling) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let child_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive child bounds");
    let all_bounds = graph
        .bounds(
            &graph_projection,
            &[
                graph::Rule::Include(child.clone()),
                graph::Rule::Include(sibling.clone()),
            ],
        )
        .expect("derive all bounds");
    let projection = tui_projection(&graph_projection);
    let child_span = graph
        .resolve(&graph_projection, &child_bounds, &child)
        .expect("resolve child");
    let sibling_span = graph
        .resolve(&graph_projection, &all_bounds, &sibling)
        .expect("resolve sibling");
    let after = surface::Artifact::new(
        aref("artifact:after", "tree:after"),
        [(PathBuf::from("src/lib.rs"), href("file:lib:v2"))],
    );

    let surface_only_child = granted_surface(
        &artifact,
        child_bounds.clone(),
        surface::Area::new([child_span.clone()]),
    );
    let outside = boundary_outcome(
        "proposal:outside",
        "run:outside",
        &artifact,
        &projection,
        &surface_only_child,
        vec![surface::Touch::new(sibling_span, "fn sibling() {}")],
        vec![],
        &after,
    );
    assert_eq!(outside, BoundaryOutcome::Retry(RetryReason::OutsideSurface));

    let protected = surface::Grant::for_coordinate_with_forbidden(
        granted_coordinate(&artifact),
        surface_policy("surface-policy:test-protected"),
        artifact.reference().clone(),
        child_bounds,
        surface::Area::new([child_span.clone()]),
        surface::Area::new([child_span.clone()]),
    )
    .expect("protected grant");
    let protected_touch = surface::Touch::new(child_span, "fn child() {}");
    let rejected = boundary_outcome(
        "proposal:protected",
        "run:protected",
        &artifact,
        &projection,
        &protected,
        vec![protected_touch],
        vec![],
        &after,
    );

    assert_eq!(rejected, BoundaryOutcome::Retry(RetryReason::Protected));
}

#[test]
fn adapter_boundary_admits_successful_bounded_candidate() {
    let (artifact, graph, _, child, _) = fixture();
    let graph_projection = graph.project(&artifact).expect("project artifact");
    let graph_bounds = graph
        .bounds(&graph_projection, &[graph::Rule::Include(child.clone())])
        .expect("derive bounds");
    let projection = tui_projection(&graph_projection);
    let span = graph
        .resolve(&graph_projection, &graph_bounds, &child)
        .expect("resolve child");
    let touch = surface::Touch::new(span.clone(), "fn child() {}");
    let grant = granted_surface(&artifact, graph_bounds, surface::Area::new([span.clone()]));
    let after_hash = href("file:lib:v2");
    let after = surface::Artifact::new(
        aref("artifact:after", "tree:after"),
        [(PathBuf::from("src/lib.rs"), after_hash.clone())],
    );
    let write = tui::Write::applied(&touch, after_hash);

    let outcome = boundary_outcome(
        "proposal:success",
        "run:success",
        &artifact,
        &projection,
        &grant,
        vec![touch],
        vec![write],
        &after,
    );

    assert_eq!(outcome, BoundaryOutcome::Applied);
}

#[test]
fn adapter_boundary_shapes_retry_until_success_or_exhaustion() {
    let success = first_applied(
        [
            BoundaryOutcome::Retry(RetryReason::Protected),
            BoundaryOutcome::Retry(RetryReason::PartialApply),
            BoundaryOutcome::Applied,
        ],
        3,
    );
    assert_eq!(success, Ok(3));

    let exhausted = first_applied(
        [
            BoundaryOutcome::Retry(RetryReason::NoEdit),
            BoundaryOutcome::Retry(RetryReason::OutsideSurface),
        ],
        2,
    );
    assert_eq!(exhausted, Err(RetryReason::OutsideSurface));
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
    let grant = granted_surface(
        &artifact,
        graph_bounds,
        surface::Area::new([child_touch.span().clone(), sibling_touch.span().clone()]),
    );
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
    let grant = granted_surface(
        &artifact,
        graph_bounds,
        surface::Area::new([touch.span().clone()]),
    );
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
    let grant = granted_surface(
        &artifact,
        graph_bounds,
        surface::Area::new([touch.span().clone()]),
    );
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
    let grant = granted_surface(
        &artifact,
        graph_bounds,
        surface::Area::new([touch.span().clone()]),
    );
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
    let grant = granted_surface(
        &artifact,
        graph_bounds,
        surface::Area::new(touches.iter().map(|touch| touch.span().clone())),
    );
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

#[cfg(feature = "live_api_tests")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "live Router provider test; run explicitly with --ignored"]
async fn live_tui_router_staged_proposal_lowers_to_checked_artifact_delta() {
    use ploke_tui::AppEvent;
    use ploke_tui::app::commands::harness::TestAppAccessor as _;
    use ploke_tui::app_state::commands::StateCommand;
    use ploke_tui::app_state::events::SystemEvent;

    const TEST_NAME: &str = "live_tui_router_staged_proposal_lowers_to_checked_artifact_delta";

    let model_id = crate::model_registry::load_active_model()
        .expect("load active eval model config")
        .model_id;
    let direct_google = crate::model_registry::load_model_registry()
        .expect("load eval model registry")
        .data
        .iter()
        .find(|item| item.id == model_id)
        .is_some_and(|item| item.route_source.is_direct_google());
    if direct_google {
        if !live_google_env_or_skip(TEST_NAME) {
            return;
        }
    } else if live_openrouter_env_or_skip(TEST_NAME).is_none() {
        return;
    }
    let provider_key = if direct_google {
        None
    } else {
        Some(
            crate::provider_prefs::load_provider_for_model(&model_id)
                .expect("load eval provider preferences")
                .unwrap_or_else(|| {
                    panic!(
                        "OpenRouter-routed active eval model {model_id} has no selected provider in provider preferences"
                    )
                }),
        )
    };
    let _llm_guard = crate::test_support::llm_lock().lock().await;

    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let fixture_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    let runtime = ploke_tui::app::commands::harness::TestRuntime::new(&fixture_db)
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus()
        .spawn_llm_manager()
        .spawn_observability();
    runtime
        .setup_loaded_standalone_crate(fixture_root.clone())
        .await;
    let state = runtime.state_arc();
    {
        use ploke_llm::router_only::{RouterVariants, google::Google, openrouter::OpenRouter};

        let mut cfg = state.config.write().await;
        cfg.active_model = model_id.clone();
        if direct_google {
            cfg.active_router = RouterVariants::Google(Google);
        } else {
            cfg.active_router = RouterVariants::OpenRouter(OpenRouter);
            cfg.model_registry
                .select_model_provider(&model_id, provider_key.as_ref());
        }
        cfg.llm_timeout_secs = 90;
        cfg.chat_policy.tool_call_timeout_secs = 30;
        cfg.chat_policy.tool_call_chain_limit = 4;
        cfg.chat_policy.error_retry_limit = 1;
        cfg.chat_policy.length_retry_limit = 0;
    }

    let events = runtime.events_builder().build_all();
    let mut debug_rx = events
        .app_actor_events
        .debug_string_rx
        .expect("debug receiver");
    let mut realtime_rx = events.event_bus_events.realtime_tx_rx;
    let mut background_rx = events.event_bus_events.background_tx_rx;
    let app = runtime.into_app_with_state_pwd(fixture_root.clone()).await;
    let cmd_tx = app.state_cmd_tx();

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

    let prompt = r#"Call the apply_code_edit tool exactly once. Do not call any other tool. Do not answer in prose before the tool call.
Use exactly this JSON payload:
{"edits":[{"file":"src/structs.rs","canon":"crate::structs::SampleStruct","node_type":"struct","code":"pub struct SampleStruct {\n    pub field: String,\n    pub live_router_7_6: bool,\n}"}],"confidence":0.99}"#;

    let user_message_id = Uuid::new_v4();
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();
    cmd_tx
        .send(StateCommand::AddUserMessage {
            content: prompt.to_string(),
            new_user_msg_id: user_message_id,
            completion_tx,
        })
        .await
        .expect("send user message");
    cmd_tx
        .send(StateCommand::ScanForChange { scan_tx })
        .await
        .expect("send scan request");
    cmd_tx
        .send(StateCommand::EmbedMessage {
            new_msg_id: user_message_id,
            completion_rx,
            scan_rx,
        })
        .await
        .expect("send embed request");

    let mut saw_tool_request = false;
    let mut completed_payload = None;
    let mut terminal = None;
    let mut terminal_seen_at = None;
    let mut terminal_error_detail = None;
    let staged = timeout(Duration::from_secs(120), async {
        let mut last_event = String::new();
        loop {
            if let Some(proposal) = {
                let proposals = state.proposals.read().await;
                proposals.values().next().cloned()
            } {
                return proposal;
            }

            if terminal_seen_at
                .is_some_and(|seen_at: Instant| seen_at.elapsed() >= Duration::from_secs(1))
            {
                panic!(
                    "live turn finished without staging proposal; terminal={terminal:?}; detail={terminal_error_detail:?}; last_event={last_event}"
                );
            }

            tokio::select! {
                debug = debug_rx.recv() => {
                    if let Some(debug) = debug {
                        last_event = debug.as_str().to_string();
                    }
                }
                realtime = realtime_rx.recv() => {
                    match realtime {
                        Ok(AppEvent::System(SystemEvent::ToolCallRequested { tool_call, .. })) => {
                            saw_tool_request = true;
                            last_event = format!("tool requested: {:?}", tool_call.function.name);
                        }
                        Ok(AppEvent::System(SystemEvent::ToolCallCompleted { content, .. })) => {
                            completed_payload = Some(content.clone());
                            last_event = format!("tool completed: {content}");
                        }
                        Ok(AppEvent::System(SystemEvent::ToolCallFailed { error, .. })) => {
                            last_event = format!("tool failed: {error}");
                        }
                        Ok(AppEvent::System(SystemEvent::ChatTurnFinished { outcome, summary, assistant_message_id, error_id, .. })) => {
                            terminal_error_detail = {
                                let chat = state.chat.0.read().await;
                                chat.messages.get(&assistant_message_id).map(|message| {
                                    format!(
                                        "assistant_message_id={assistant_message_id}; error_id={error_id:?}; status={:?}; content={}",
                                        message.status,
                                        message.content
                                    )
                                })
                            };
                            terminal = Some((outcome, summary));
                            terminal_seen_at = Some(Instant::now());
                        }
                        Ok(other) => {
                            last_event = format!("{other:?}");
                        }
                        Err(_) => {}
                    }
                }
                background = background_rx.recv() => {
                    if let Ok(event) = background {
                        last_event = format!("{event:?}");
                    }
                }
                _ = sleep(Duration::from_millis(50)) => {}
            }
        }
    })
    .await
    .expect("live turn should stage an edit proposal");

    let post_stage_deadline = Instant::now() + Duration::from_secs(30);
    while terminal.is_none() && Instant::now() < post_stage_deadline {
        tokio::select! {
            debug = debug_rx.recv() => {
                let _ = debug;
            }
            realtime = realtime_rx.recv() => {
                match realtime {
                    Ok(AppEvent::System(SystemEvent::ToolCallRequested { .. })) => {
                        saw_tool_request = true;
                    }
                    Ok(AppEvent::System(SystemEvent::ToolCallCompleted { content, .. })) => {
                        completed_payload = Some(content);
                    }
                    Ok(AppEvent::System(SystemEvent::ChatTurnFinished { outcome, summary, .. })) => {
                        terminal = Some((outcome, summary));
                    }
                    Ok(_) | Err(_) => {}
                }
            }
            background = background_rx.recv() => {
                let _ = background;
            }
            _ = sleep(Duration::from_millis(50)) => {}
        }
    }

    assert!(saw_tool_request, "live Router turn did not request a tool");
    assert!(
        completed_payload
            .as_deref()
            .is_some_and(|payload| payload.contains("\"staged\":1")),
        "expected staged apply_code_edit completion, got {completed_payload:?}"
    );
    assert!(
        terminal.is_some(),
        "live turn did not reach terminal event after staging"
    );
    assert!(staged.is_semantic, "live proposal must be semantic");
    assert_eq!(staged.edits.len(), 1, "expected one live staged edit");
    let write = &staged.edits[0];
    assert_eq!(write.file_path, expected_path);
    assert_eq!(write.expected_file_hash, expected_node.file_tracking_hash);
    assert_eq!(write.start_byte, expected_node.start_byte);
    assert_eq!(write.end_byte, expected_node.end_byte);

    let artifact = surface::Artifact::new(
        aref("artifact:live-base", "tree:live-base"),
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
    let resolved = bounds
        .touches(
            &artifact,
            std::slice::from_ref(&expected_target),
            &staged.edits,
        )
        .expect("eval lowers live TUI writes into checked touches");
    let touches = resolved.into_vec();
    let grant = granted_surface(
        &artifact,
        graph_bounds,
        surface::Area::new(touches.iter().map(|touch| touch.span().clone())),
    );

    let objective = surface::EditObjective::new(
        broad_objective_spec("live ploke-tui semantic edit proposal through Router"),
        [],
        [EvidenceRef::new("history:context:live-router-7-6")],
    );
    let request_core = ploke_llm::request::ChatCompReqCore::default().with_model(model_id.clone());
    let default_core = ploke_llm::request::ChatCompReqCore::default();
    let provider_preferences = if direct_google {
        None
    } else {
        let cfg = state.config.read().await;
        Some(
            cfg.model_registry
                .models
                .get(&model_id.key)
                .and_then(|prefs| prefs.selected_provider_preferences())
                .expect("selected provider preferences"),
        )
    };
    let params = ploke_llm::LLMParameters::default();
    // The live TUI path still lacks full outbound request capture. Bind only
    // the current client-policy shape plus explicit unknown payload hashes to
    // the actual staged live proposal identity, without claiming replay or
    // provider-side completeness.
    let live_request_policy_receipt = if direct_google {
        request_policy::Receipt::google(
            artifact.reference().id().clone(),
            &objective,
            &request_core,
            &default_core,
            &params,
            &params,
        )
    } else {
        request_policy::Receipt::openrouter(
            artifact.reference().id().clone(),
            &objective,
            &request_core,
            &default_core,
            &params,
            &params,
            provider_preferences.as_ref(),
        )
    }
    .bind_proposal(staged.proposal_id.to_string(), "run:live-router-7-6")
    .with_request_payload_hash(request_policy::PayloadHash::unknown(
        "live 7.6 path does not expose serialized outbound request payload",
    ))
    .with_response_payload_hash(request_policy::PayloadHash::unknown(
        "live 7.6 path does not expose normalized response payload",
    ));
    live_request_policy_receipt
        .verify_current_client_policy_shape()
        .expect("reconstructed current client-policy shape should be internally consistent");
    assert_eq!(
        live_request_policy_receipt.base_artifact_id(),
        artifact.reference().id(),
        "live request-policy receipt remains tied to checked base Artifact"
    );
    request_policy::ProposalProducer::Router {
        request_policy: live_request_policy_receipt.clone(),
    }
    .verify_complete(
        artifact.reference().id(),
        &staged.proposal_id.to_string(),
        "run:live-router-7-6",
    )
    .expect("live Router proposal receipt should admit against the staged proposal");
    let err = request_policy::ProposalProducer::Router {
        request_policy: live_request_policy_receipt,
    }
    .verify_complete(
        artifact.reference().id(),
        "proposal:mismatch",
        "run:live-router-7-6",
    )
    .expect_err("mismatched live proposal binding must reject");
    assert!(err.contains("does not match admitted proposal"));

    let after = aref("artifact:live-after", "tree:live-after");
    let proposal = tui::Proposal::stage(tui::Stage {
        proposal: &staged.proposal_id.to_string(),
        run: "run:live-router-7-6",
        base: artifact.reference(),
        after: after.clone(),
        projection: &projection,
        touches: touches.clone(),
        auto_apply: false,
    })
    .expect("stage eval proposal from TUI result");
    let check = grant.check(proposal.draft()).expect("grant check");
    let after_hash = href("file:live-after");
    let reported = tui::Apply::from_results(
        proposal,
        check,
        touches
            .iter()
            .map(|touch| tui::Write::applied(touch, after_hash.clone()))
            .collect(),
    )
    .expect("apply evidence is admitted only after check");
    let after_artifact = surface::Artifact::new(after.clone(), [(expected_path, after_hash)]);
    let applied = reported
        .validate(&after_artifact)
        .expect("validated apply evidence");
    let delta = applied.delta().expect("applied evidence yields delta");

    assert_eq!(delta.base(), artifact.reference());
    assert_eq!(delta.after(), &after);
    assert_eq!(delta.touches(), touches.as_slice());
}
