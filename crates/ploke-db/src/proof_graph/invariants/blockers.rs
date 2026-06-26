use std::collections::BTreeSet;

use super::{ProofFactRow, ProofScope, is_process_effect_class, is_proof_evidence};

pub(super) fn proof_blocker_reasons(rows: &[ProofFactRow], scope: &ProofScope) -> Vec<String> {
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

pub(super) fn active_proof_blocker_reasons(rows: &[ProofFactRow]) -> Vec<String> {
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
                || matches!(
                    row.status.as_deref(),
                    Some("blocked" | "unresolved" | "externally_summarized")
                ) =>
        {
            Some(
                row.blocker_reason
                    .clone()
                    .unwrap_or_else(|| expansion_boundary_gap_reason(row)),
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

fn expansion_boundary_gap_reason(row: &ProofFactRow) -> String {
    match row.detail.as_deref() {
        Some("proc_macro_derive" | "proc_macro_attribute" | "proc_macro_function") => {
            "proc_macro_summary_missing".to_string()
        }
        Some("build_script") => "build_script_summary_missing".to_string(),
        Some("external_summary") => "external_dependency_summary_missing".to_string(),
        _ => "macro_expansion_not_available".to_string(),
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

fn blocker_label(row: &ProofFactRow) -> String {
    row.blocker_reason
        .clone()
        .or_else(|| row.detail.clone())
        .unwrap_or_else(|| row.fact_id.clone())
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
