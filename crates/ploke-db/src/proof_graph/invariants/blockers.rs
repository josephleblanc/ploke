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

pub(super) fn derived_proof_blocker_reason(
    row: &ProofFactRow,
    rows: &[ProofFactRow],
) -> Option<String> {
    if row.kind == "proof_blocker" {
        return None;
    }
    if !is_proof_evidence(row) && !is_navigation_only_unresolved_process_call(row, rows) {
        return None;
    }
    derived_gap_reason(row, rows)
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
    if build_domain_reference_missing(row, rows) {
        return Some("canonical_identity_mismatch".to_string());
    }

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
            let default_reason = expansion_boundary_gap_reason(row);
            if row.status.as_deref() == Some("externally_summarized")
                && default_reason == "external_dependency_summary_missing"
                && external_summary_gap_is_discharged(row, rows, &default_reason)
            {
                None
            } else {
                Some(row.blocker_reason.clone().unwrap_or(default_reason))
            }
        }
        "external_summary"
            if row.blocker_reason.is_some()
                || matches!(row.status.as_deref(), Some("blocked" | "rejected")) =>
        {
            Some(row.blocker_reason.clone().unwrap_or_else(|| {
                row.detail
                    .clone()
                    .unwrap_or_else(|| "external_dependency_summary_missing".to_string())
            }))
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
            Some("externally_summarized")
                if external_summary_gap_is_discharged(
                    row,
                    rows,
                    "external_dependency_summary_missing",
                ) =>
            {
                None
            }
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

fn build_domain_reference_missing(row: &ProofFactRow, rows: &[ProofFactRow]) -> bool {
    if row.kind == "build_domain" {
        return false;
    }
    let Some(build_domain_id) = row.build_domain_id.as_deref() else {
        return false;
    };
    !rows.iter().any(|candidate| {
        candidate.kind == "build_domain"
            && candidate.build_domain_id.as_deref() == Some(build_domain_id)
    })
}

fn external_summary_gap_is_discharged(
    row: &ProofFactRow,
    rows: &[ProofFactRow],
    default_reason: &str,
) -> bool {
    row.blocker_reason
        .as_deref()
        .is_none_or(|reason| reason == default_reason)
        && has_matching_admitted_external_summary(row, rows)
}

fn has_matching_admitted_external_summary(row: &ProofFactRow, rows: &[ProofFactRow]) -> bool {
    let Some(external_summary_id) = row.external_summary_id.as_deref() else {
        return false;
    };
    rows.iter().any(|summary| {
        summary.kind == "external_summary"
            && is_proof_evidence(summary)
            && summary.external_summary_id.as_deref() == Some(external_summary_id)
            && summary.status.as_deref() == Some("admitted")
            && summary.summary_class.as_deref() != Some("opaque_blocked")
            && summary_allows_external_summary_boundary(summary)
            && external_summary_domain_matches(row, summary, rows)
    })
}

fn summary_allows_external_summary_boundary(summary: &ProofFactRow) -> bool {
    summary
        .allowed_effects
        .iter()
        .any(|effect| effect == "external_summary_boundary")
}

fn external_summary_domain_matches(
    row: &ProofFactRow,
    summary: &ProofFactRow,
    rows: &[ProofFactRow],
) -> bool {
    if let Some(row_domain) = row.build_domain_id.as_deref() {
        return summary.build_domain_id.as_deref() == Some(row_domain);
    }

    let call_site_domains = linked_call_site_domains(row, rows);
    if call_site_domains.is_empty() {
        return row.call_site_id.is_none();
    }
    if call_site_domains.len() > 1 {
        return false;
    }
    match (
        call_site_domains.first().copied(),
        summary.build_domain_id.as_deref(),
    ) {
        (Some(row_domain), Some(summary_domain)) => row_domain == summary_domain,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn linked_call_site_domains<'a>(row: &ProofFactRow, rows: &'a [ProofFactRow]) -> BTreeSet<&'a str> {
    let Some(call_site_id) = row.call_site_id.as_deref() else {
        return BTreeSet::new();
    };
    rows.iter()
        .filter(|candidate| candidate.kind == "call_site")
        .filter(|candidate| candidate.call_site_id.as_deref() == Some(call_site_id))
        .filter_map(|candidate| candidate.build_domain_id.as_deref())
        .collect()
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
