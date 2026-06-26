use super::*;

pub(super) fn evaluate_crown_ruling_invariants(
    rows: &[ProofFactRow],
) -> Vec<ProofInvariantFinding> {
    let lineages = lineage_keys(rows);
    if lineages.is_empty() {
        return vec![finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Pass,
            "at most one admitted Crown<Ruling> authority is recorded for this lineage".to_string(),
            None,
        )];
    }

    lineages
        .iter()
        .map(|build_domain_id| evaluate_crown_ruling_lineage(rows, build_domain_id.as_deref()))
        .collect()
}

fn evaluate_crown_ruling_lineage(
    rows: &[ProofFactRow],
    build_domain_id: Option<&str>,
) -> ProofInvariantFinding {
    let authority_blockers = lineage_authority_blockers(rows, build_domain_id);
    let call_site_id = first_process_site(rows, build_domain_id);
    if !authority_blockers.is_empty() {
        return finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Blocked,
            format!(
                "authority evidence is blocked: {}",
                authority_blockers.join(", ")
            ),
            call_site_id,
        );
    }

    let crown_count = lineage_authority_count(rows, build_domain_id, "crown_ruling");
    if crown_count > 1 {
        return finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Fail,
            format!("multiple Crown<Ruling> authorities admitted in one lineage: {crown_count}"),
            call_site_id,
        );
    }
    if crown_count == 0 && has_process_effect(rows, build_domain_id) {
        return finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Blocked,
            "ambiguous lineage: no admitted Crown<Ruling> authority evidence".to_string(),
            call_site_id,
        );
    }

    finding(
        "crown_ruling_lineage_uniqueness",
        ProofInvariantStatus::Pass,
        "at most one admitted Crown<Ruling> authority is recorded for this lineage".to_string(),
        call_site_id,
    )
}
