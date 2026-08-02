use super::*;

pub(super) fn evaluate_detached_process_invariants(
    rows: &[ProofFactRow],
) -> Vec<ProofInvariantFinding> {
    let scopes = process_scopes(rows);
    let unscoped_process_count = unscoped_process_effect_count(rows);
    let mut findings = Vec::new();
    if unscoped_process_count > 0 {
        findings.push(finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Blocked,
            format!(
                "detached process proof evidence lacks call_site_id for {unscoped_process_count} effect seed(s)"
            ),
            None,
        ));
    }
    if scopes.is_empty() {
        let blocker_reasons = active_proof_blocker_reasons(rows);
        if !blocker_reasons.is_empty() {
            findings.push(finding(
                "detached_process_successor_handoff",
                ProofInvariantStatus::Blocked,
                format!(
                    "proof blockers prevent detached-process proof: {}",
                    blocker_reasons.join(", ")
                ),
                None,
            ));
        } else if findings.is_empty() {
            findings.push(finding(
                "detached_process_successor_handoff",
                ProofInvariantStatus::Pass,
                "no proof-only detached process effects recorded".to_string(),
                None,
            ));
        }
        return findings;
    }

    findings.extend(
        scopes
            .iter()
            .map(|scope| evaluate_detached_process_scope(rows, scope)),
    );
    findings
}

fn evaluate_detached_process_scope(
    rows: &[ProofFactRow],
    scope: &ProofScope,
) -> ProofInvariantFinding {
    let blocker_reasons = proof_blocker_reasons(rows, scope);
    if !blocker_reasons.is_empty() {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Blocked,
            format!(
                "proof blockers prevent detached-process proof: {}",
                blocker_reasons.join(", ")
            ),
            Some(scope.call_site_id.clone()),
        );
    }
    let authority_blockers = scoped_authority_blockers(rows, scope);
    if !authority_blockers.is_empty() {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Blocked,
            format!(
                "authority evidence is blocked: {}",
                authority_blockers.join(", ")
            ),
            Some(scope.call_site_id.clone()),
        );
    }

    let successor_count = scoped_authority_count(rows, scope, "successor");
    let parent_count = scoped_authority_count(rows, scope, "parent_lineage");
    let predecessor_count = scoped_authority_count(rows, scope, "predecessor_retired");
    let crown_count = scoped_authority_count(rows, scope, "crown_ruling");

    if successor_count == 0 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            "detached process create lacks admitted successor handoff".to_string(),
            Some(scope.call_site_id.clone()),
        );
    }
    if successor_count > 1 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            format!("expected exactly one detached successor Parent, found {successor_count}"),
            Some(scope.call_site_id.clone()),
        );
    }
    if parent_count != 1 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            format!("expected exactly one admitted Parent lineage boundary, found {parent_count}"),
            Some(scope.call_site_id.clone()),
        );
    }
    if predecessor_count == 0 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            "successor admission lacks predecessor retirement or lock evidence".to_string(),
            Some(scope.call_site_id.clone()),
        );
    }
    if crown_count != 1 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            format!("successor handoff requires exactly one Crown<Ruling>, found {crown_count}"),
            Some(scope.call_site_id.clone()),
        );
    }

    finding(
        "detached_process_successor_handoff",
        ProofInvariantStatus::Pass,
        "detached process is covered by exactly one admitted successor Parent with predecessor retired and one Crown<Ruling>".to_string(),
        Some(scope.call_site_id.clone()),
    )
}
