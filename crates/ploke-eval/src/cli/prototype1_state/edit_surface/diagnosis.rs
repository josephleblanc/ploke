use crate::cli::prototype1_state::history::{EvaluationPayload, EvidenceRef, surface_attempt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Limiter {
    InvalidCandidateGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureKind {
    SemanticEditResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnosis {
    pub(crate) limiter: Limiter,
    pub(crate) failure_kind: FailureKind,
    pub(crate) source_refs: Vec<EvidenceRef>,
    pub(crate) surface_attempt: surface_attempt::Evidence,
}

pub(crate) fn classify(payload: &EvaluationPayload) -> Option<Diagnosis> {
    let surface_attempt = payload.surface_attempt.as_ref()?;
    match surface_attempt.outcome {
        surface_attempt::Outcome::Rejected { .. } if payload.artifact.is_none() => {
            Some(Diagnosis {
                limiter: Limiter::InvalidCandidateGeneration,
                failure_kind: FailureKind::SemanticEditResolution,
                source_refs: payload.source_refs.clone(),
                surface_attempt: surface_attempt.clone(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{FailureKind, Limiter, classify};
    use crate::cli::prototype1_state::history::{
        EvaluationPayload, EvidenceRef, ProcedureRef, SelectionProjectionFailure,
        SelectionProjectionFailureKind, SubjectRef, surface_attempt,
    };

    #[test]
    fn classify_rejected_surface_attempt_without_artifact_as_semantic_edit_resolution() {
        let payload = EvaluationPayload::builder(
            SubjectRef::new("candidate:rejected-attempt:plan_index=0"),
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        )
        .source_ref(EvidenceRef::new("history:surface_attempt:rejected"))
        .surface_attempt_evidence(surface_attempt::Evidence::rejected(
            "prototype1:tui-edit-surface:deterministic-v1",
            "proposal-rejected",
            "run-rejected",
            "ploke_tui_tools",
            PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
            "one or more touched spans were rejected",
        ))
        .build();

        let diagnosis = classify(&payload).expect("diagnosis");
        assert_eq!(diagnosis.limiter, Limiter::InvalidCandidateGeneration);
        assert_eq!(diagnosis.failure_kind, FailureKind::SemanticEditResolution);
        assert_eq!(diagnosis.source_refs, payload.source_refs);
        assert_eq!(
            diagnosis.surface_attempt,
            payload.surface_attempt.expect("attempt")
        );
    }

    #[test]
    fn payload_without_surface_attempt_is_not_semantic_edit_resolution_diagnosis() {
        let payload = EvaluationPayload::builder(
            SubjectRef::new("candidate:projection-only:plan_index=0"),
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        )
        .projection_failure(
            SelectionProjectionFailure::committed(
                SelectionProjectionFailureKind::MissingSelectionInput,
                None,
                Some(
                    "log-only rejected apply mention should not count as typed attempt evidence"
                        .to_string(),
                ),
            )
            .expect("projection failure"),
        )
        .build();

        assert_eq!(classify(&payload), None);
    }
}
