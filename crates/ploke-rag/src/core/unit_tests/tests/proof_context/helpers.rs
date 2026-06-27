use super::super::*;

pub(super) fn assert_projected_owner_rows(rows: &[ProofContextInfo], owner: Uuid, target: Uuid) {
    let owner = owner.to_string();
    let target = target.to_string();
    assert_eq!(rows.len(), 3, "projected proof context rows: {rows:#?}");
    let site = rows
        .iter()
        .find(|row| row.kind == "call_site")
        .expect("proof context should include linked call_site fact");
    let site_id = site
        .call_site_id
        .as_deref()
        .expect("call_site proof fact should carry call_site_id");
    assert_eq!(
        site.build_domain_id.as_deref(),
        Some("bd:fixture-call-graph"),
        "call_site proof fact should preserve the build domain that scopes linked rows"
    );
    assert_eq!(site.caller_def_id.as_deref(), Some(owner.as_str()));

    let edge = rows
        .iter()
        .find(|row| row.kind == "call_edge")
        .expect("proof context should include linked call_edge fact");
    assert_eq!(edge.call_site_id.as_deref(), Some(site_id));
    assert_eq!(edge.caller_def_id.as_deref(), Some(owner.as_str()));
    assert_eq!(edge.callee_def_id.as_deref(), Some(target.as_str()));
    assert_eq!(edge.resolution_state.as_deref(), Some("resolved"));

    let resolution = rows
        .iter()
        .find(|row| row.kind == "call_resolution")
        .expect("proof context should include linked call_resolution fact");
    assert_eq!(resolution.call_site_id.as_deref(), Some(site_id));
    assert_eq!(resolution.resolution_state.as_deref(), Some("resolved"));
}

pub(super) fn assert_resolved_call(rows: &[ProofContextInfo], owner: Uuid, target: Uuid) {
    assert_resolved_call_for_domain(rows, owner, target, "bd:fixture-call-graph");
}

pub(super) fn assert_resolved_call_for_domain(
    rows: &[ProofContextInfo],
    owner: Uuid,
    target: Uuid,
    domain: &str,
) {
    let owner = owner.to_string();
    let target = target.to_string();
    let edge = rows
        .iter()
        .find(|row| {
            row.kind == "call_edge"
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.callee_def_id.as_deref() == Some(target.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
        })
        .unwrap_or_else(|| panic!("proof context should include resolved call_edge: {rows:#?}"));
    let site_id = edge
        .call_site_id
        .as_deref()
        .expect("call_edge proof fact should carry call_site_id");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.build_domain_id.as_deref() == Some(domain)
        }),
        "proof context should include resolved call_site fact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.resolution_state.as_deref() == Some("resolved")
                && row.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "proof context should include resolved call_resolution fact: {rows:#?}"
    );
}

pub(super) fn assert_blocked_resolution(rows: &[ProofContextInfo], owner: Uuid, reason: &str) {
    let owner = owner.to_string();
    let site_ids = rows
        .iter()
        .filter(|row| {
            row.kind == "call_site" && row.caller_def_id.as_deref() == Some(owner.as_str())
        })
        .filter_map(|row| row.call_site_id.as_deref())
        .collect::<Vec<_>>();
    assert!(
        !site_ids.is_empty(),
        "proof context should include blocked owner call_site facts: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.resolution_state.as_deref() == Some("blocked")
                && row.blocker_reason.as_deref() == Some(reason)
                && row
                    .call_site_id
                    .as_deref()
                    .is_some_and(|site| site_ids.contains(&site))
        }),
        "proof context should include blocked call_resolution reason {reason}: {rows:#?}"
    );
}
