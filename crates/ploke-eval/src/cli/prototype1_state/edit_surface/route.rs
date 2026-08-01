use crate::cli::prototype1_state::history::EvidenceRef;

use super::{diagnosis, graph, surface};

pub(crate) fn semantic_resolution(
    diagnosis: &diagnosis::Diagnosis,
    artifact: surface::Ref,
    graph: graph::Bounds,
    protected_core: surface::ProtectedCore,
    context_refs: impl IntoIterator<Item = EvidenceRef>,
) -> (surface::EditObjective, surface::SurfaceRequest) {
    let objective = surface::EditObjective::new(
        surface::ObjectiveSpec::new(
            "replay-shaped rejected semantic edit attempt",
            surface::ObjectiveKind::ReduceKnownFailure {
                limiter: diagnosis.limiter,
                failure_kind: diagnosis.failure_kind,
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
        .with_requested_candidates(1),
        diagnosis.source_refs.clone(),
        context_refs,
    );
    let request =
        surface::SurfaceRequest::broad(objective.clone(), artifact, graph, protected_core);
    (objective, request)
}
