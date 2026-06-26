use super::ProofFactRow;

pub(super) fn is_proof_evidence(row: &ProofFactRow) -> bool {
    matches!(
        row.evidence_use.as_deref(),
        Some("proof_only" | "proof_and_navigation")
    )
}

pub(super) fn is_process_effect_class(effect_class: &str) -> bool {
    matches!(
        effect_class,
        "operating_system_process_create" | "operating_system_process_replace"
    )
}
