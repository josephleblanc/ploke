use super::{ProofFactRow, ProofScope, is_proof_evidence};

pub(super) fn scoped_authority_blockers(rows: &[ProofFactRow], scope: &ProofScope) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| authority_matches_scope(row, scope))
        .filter(|row| matches!(row.status.as_deref(), Some("blocked" | "rejected")))
        .map(authority_label)
        .collect()
}

pub(super) fn lineage_authority_blockers(
    rows: &[ProofFactRow],
    build_domain_id: Option<&str>,
) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| lineage_matches(row, build_domain_id))
        .filter(|row| matches!(row.status.as_deref(), Some("blocked" | "rejected")))
        .map(authority_label)
        .collect()
}

pub(super) fn scoped_authority_count(
    rows: &[ProofFactRow],
    scope: &ProofScope,
    term: &str,
) -> usize {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| authority_matches_scope(row, scope))
        .filter(|row| row.effect_class.as_deref() == Some(term))
        .filter(|row| row.status.as_deref() == Some("admitted"))
        .count()
}

pub(super) fn lineage_authority_count(
    rows: &[ProofFactRow],
    build_domain_id: Option<&str>,
    term: &str,
) -> usize {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| lineage_matches(row, build_domain_id))
        .filter(|row| row.effect_class.as_deref() == Some(term))
        .filter(|row| row.status.as_deref() == Some("admitted"))
        .count()
}

fn authority_label(row: &ProofFactRow) -> String {
    row.effect_class
        .clone()
        .or_else(|| row.detail.clone())
        .unwrap_or_else(|| row.fact_id.clone())
}

fn authority_matches_scope(row: &ProofFactRow, scope: &ProofScope) -> bool {
    row.call_site_id.as_deref() == Some(scope.call_site_id.as_str())
        && row.build_domain_id.as_deref() == scope.build_domain_id.as_deref()
}

fn lineage_matches(row: &ProofFactRow, build_domain_id: Option<&str>) -> bool {
    row.build_domain_id.as_deref() == build_domain_id
}
