use std::collections::{BTreeMap, BTreeSet};

use super::{ProofInvariantFinding, ProofInvariantStatus, rows::ProofFactRow};

mod crown;
mod detached;

pub(super) fn evaluate_proof_invariants(rows: &[ProofFactRow]) -> Vec<ProofInvariantFinding> {
    let mut findings = detached::evaluate_detached_process_invariants(rows);
    findings.extend(crown::evaluate_crown_ruling_invariants(rows));
    findings
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProofScope {
    call_site_id: String,
    build_domain_id: Option<String>,
}

fn process_scopes(rows: &[ProofFactRow]) -> Vec<ProofScope> {
    let site_domains = call_site_domains(rows);
    let mut scopes = BTreeMap::<String, Option<String>>::new();
    for row in rows.iter().filter(|row| {
        row.kind == "effect_seed"
            && is_proof_evidence(row)
            && row
                .effect_class
                .as_deref()
                .is_some_and(is_process_effect_class)
    }) {
        let Some(call_site_id) = row.call_site_id.clone() else {
            continue;
        };
        let build_domain_id = row
            .build_domain_id
            .clone()
            .or_else(|| site_domains.get(&call_site_id).cloned());
        let entry = scopes
            .entry(call_site_id)
            .or_insert_with(|| build_domain_id.clone());
        if entry.is_none() && build_domain_id.is_some() {
            *entry = build_domain_id;
        }
    }
    scopes
        .into_iter()
        .map(|(call_site_id, build_domain_id)| ProofScope {
            call_site_id,
            build_domain_id,
        })
        .collect()
}

fn unscoped_process_effect_count(rows: &[ProofFactRow]) -> usize {
    rows.iter()
        .filter(|row| row.kind == "effect_seed")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| {
            row.effect_class
                .as_deref()
                .is_some_and(is_process_effect_class)
        })
        .filter(|row| row.call_site_id.is_none())
        .count()
}

fn call_site_domains(rows: &[ProofFactRow]) -> BTreeMap<String, String> {
    rows.iter()
        .filter(|row| row.kind == "call_site")
        .filter_map(|row| Some((row.call_site_id.clone()?, row.build_domain_id.clone()?)))
        .collect()
}

fn lineage_keys(rows: &[ProofFactRow]) -> BTreeSet<Option<String>> {
    let mut keys = BTreeSet::new();
    keys.extend(
        process_scopes(rows)
            .into_iter()
            .map(|scope| scope.build_domain_id),
    );
    keys.extend(
        rows.iter()
            .filter(|row| row.kind == "authority")
            .filter(|row| is_proof_evidence(row))
            .map(|row| row.build_domain_id.clone()),
    );
    keys
}

fn proof_blocker_reasons(rows: &[ProofFactRow], scope: &ProofScope) -> Vec<String> {
    let mut reasons: Vec<_> = rows
        .iter()
        .filter(|row| row.kind == "proof_blocker")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| is_active_blocker_status(row))
        .filter(|row| blocker_matches_scope(row, scope))
        .map(blocker_label)
        .collect();
    reasons.extend(derived_proof_gap_reasons(rows, Some(scope)));
    dedupe_reasons(reasons)
}

fn active_proof_blocker_reasons(rows: &[ProofFactRow]) -> Vec<String> {
    let mut reasons: Vec<_> = rows
        .iter()
        .filter(|row| row.kind == "proof_blocker")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| is_active_blocker_status(row))
        .filter(|row| no_process_scope_blocker_can_affect_detached_process(row, rows))
        .map(blocker_label)
        .collect();
    reasons.extend(derived_proof_gap_reasons(rows, None));
    dedupe_reasons(reasons)
}

fn derived_proof_gap_reasons(rows: &[ProofFactRow], scope: Option<&ProofScope>) -> Vec<String> {
    rows.iter()
        .filter(|row| derived_gap_matches_scope(row, rows, scope))
        .filter_map(|row| derived_gap_reason(row, rows))
        .collect()
}

fn derived_gap_matches_scope(
    row: &ProofFactRow,
    rows: &[ProofFactRow],
    scope: Option<&ProofScope>,
) -> bool {
    if !is_proof_evidence(row) && !is_navigation_only_unresolved_process_call(row, rows) {
        return false;
    }
    match scope {
        Some(scope) => blocker_matches_scope(row, scope),
        None if is_navigation_only_unresolved_process_call(row, rows) => true,
        None => no_process_scope_blocker_can_affect_detached_process(row, rows),
    }
}

fn derived_gap_reason(row: &ProofFactRow, rows: &[ProofFactRow]) -> Option<String> {
    match row.kind.as_str() {
        "cfg_domain"
            if row.blocker_reason.is_some()
                || matches!(row.status.as_deref(), Some("blocked" | "rejected")) =>
        {
            Some(
                row.blocker_reason
                    .clone()
                    .unwrap_or_else(|| "cfg_domain_not_materialized".to_string()),
            )
        }
        "rustc_invocation"
            if row.blocker_reason.is_some()
                || matches!(row.status.as_deref(), Some("blocked" | "rejected")) =>
        {
            Some(
                row.blocker_reason
                    .clone()
                    .unwrap_or_else(|| "rustc_invocation_evidence_missing".to_string()),
            )
        }
        "expansion_boundary"
            if row.blocker_reason.is_some()
                || matches!(row.status.as_deref(), Some("blocked" | "unresolved")) =>
        {
            Some(
                row.blocker_reason
                    .clone()
                    .unwrap_or_else(|| "macro_expansion_not_available".to_string()),
            )
        }
        "call_edge" => {
            if call_site_identity_mismatch_reason(row, rows).is_some() {
                Some("canonical_identity_mismatch".to_string())
            } else if row.evidence_use.as_deref() == Some("navigation_only")
                && row.resolution_state.as_deref() != Some("resolved")
            {
                Some("navigation-only unresolved process call".to_string())
            } else if row.resolution_state.as_deref() != Some("resolved")
                || row.callee_def_id.is_none()
            {
                Some("type_resolution_missing".to_string())
            } else {
                None
            }
        }
        "call_resolution" => match row.resolution_state.as_deref() {
            Some("externally_summarized") => Some(
                row.blocker_reason
                    .clone()
                    .unwrap_or_else(|| "external_dependency_summary_missing".to_string()),
            ),
            Some("candidate_set" | "ambiguous" | "unresolved" | "blocked") => Some(
                row.blocker_reason
                    .clone()
                    .unwrap_or_else(|| "type_resolution_missing".to_string()),
            ),
            _ => None,
        },
        _ => None,
    }
}

fn call_site_identity_mismatch_reason(row: &ProofFactRow, rows: &[ProofFactRow]) -> Option<()> {
    let call_site_id = row.call_site_id.as_deref()?;
    let call_site_callers = rows
        .iter()
        .filter(|candidate| candidate.kind == "call_site")
        .filter(|candidate| candidate.call_site_id.as_deref() == Some(call_site_id))
        .filter_map(|candidate| candidate.caller_def_id.as_deref())
        .collect::<BTreeSet<_>>();
    if call_site_callers.len() > 1 {
        return Some(());
    }
    let call_site_caller = call_site_callers.iter().next()?;
    (Some(*call_site_caller) != row.caller_def_id.as_deref()).then_some(())
}

fn is_navigation_only_unresolved_process_call(row: &ProofFactRow, rows: &[ProofFactRow]) -> bool {
    row.kind == "call_edge"
        && row.evidence_use.as_deref() == Some("navigation_only")
        && row.resolution_state.as_deref() != Some("resolved")
        && row.call_site_id.as_ref().is_some_and(|call_site_id| {
            rows.iter().any(|candidate| {
                candidate.kind == "effect_seed"
                    && candidate.evidence_use.as_deref() == Some("navigation_only")
                    && candidate.call_site_id.as_ref() == Some(call_site_id)
                    && candidate
                        .effect_class
                        .as_deref()
                        .is_some_and(is_process_effect_class)
            })
        })
}

fn dedupe_reasons(reasons: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    reasons
        .into_iter()
        .filter(|reason| seen.insert(reason.clone()))
        .collect()
}

fn no_process_scope_blocker_can_affect_detached_process(
    blocker: &ProofFactRow,
    rows: &[ProofFactRow],
) -> bool {
    let Some(call_site_id) = blocker.call_site_id.as_deref() else {
        return true;
    };

    !rows
        .iter()
        .any(|row| row.kind == "call_site" && row.call_site_id.as_deref() == Some(call_site_id))
}

fn is_active_blocker_status(row: &ProofFactRow) -> bool {
    matches!(row.status.as_deref(), Some("blocked" | "rejected"))
}

fn scoped_authority_blockers(rows: &[ProofFactRow], scope: &ProofScope) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| authority_matches_scope(row, scope))
        .filter(|row| matches!(row.status.as_deref(), Some("blocked" | "rejected")))
        .map(authority_label)
        .collect()
}

fn lineage_authority_blockers(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| lineage_matches(row, build_domain_id))
        .filter(|row| matches!(row.status.as_deref(), Some("blocked" | "rejected")))
        .map(authority_label)
        .collect()
}

fn authority_label(row: &ProofFactRow) -> String {
    row.effect_class
        .clone()
        .or_else(|| row.detail.clone())
        .unwrap_or_else(|| row.fact_id.clone())
}

fn blocker_label(row: &ProofFactRow) -> String {
    row.blocker_reason
        .clone()
        .or_else(|| row.detail.clone())
        .unwrap_or_else(|| row.fact_id.clone())
}

fn has_process_effect(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> bool {
    process_scopes(rows)
        .iter()
        .any(|scope| scope.build_domain_id.as_deref() == build_domain_id)
}

fn first_process_site(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> Option<String> {
    process_scopes(rows)
        .into_iter()
        .find(|scope| scope.build_domain_id.as_deref() == build_domain_id)
        .map(|scope| scope.call_site_id)
}

fn scoped_authority_count(rows: &[ProofFactRow], scope: &ProofScope, term: &str) -> usize {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| authority_matches_scope(row, scope))
        .filter(|row| row.effect_class.as_deref() == Some(term))
        .filter(|row| row.status.as_deref() == Some("admitted"))
        .count()
}

fn lineage_authority_count(
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

fn authority_matches_scope(row: &ProofFactRow, scope: &ProofScope) -> bool {
    row.call_site_id.as_deref() == Some(scope.call_site_id.as_str())
        && row.build_domain_id.as_deref() == scope.build_domain_id.as_deref()
}

fn blocker_matches_scope(row: &ProofFactRow, scope: &ProofScope) -> bool {
    if row.call_site_id.as_deref().is_some() {
        return row.call_site_id.as_deref() == Some(scope.call_site_id.as_str());
    }
    if row.build_domain_id.is_none() {
        return true;
    }
    row.build_domain_id.as_deref().is_some()
        && row.build_domain_id.as_deref() == scope.build_domain_id.as_deref()
}

fn lineage_matches(row: &ProofFactRow, build_domain_id: Option<&str>) -> bool {
    row.build_domain_id.as_deref() == build_domain_id
}

fn is_proof_evidence(row: &ProofFactRow) -> bool {
    matches!(
        row.evidence_use.as_deref(),
        Some("proof_only" | "proof_and_navigation")
    )
}

fn is_process_effect_class(effect_class: &str) -> bool {
    matches!(
        effect_class,
        "operating_system_process_create" | "operating_system_process_replace"
    )
}

fn finding(
    invariant: &str,
    status: ProofInvariantStatus,
    reason: String,
    call_site_id: Option<String>,
) -> ProofInvariantFinding {
    ProofInvariantFinding {
        invariant: invariant.to_string(),
        status,
        reason,
        call_site_id,
    }
}
