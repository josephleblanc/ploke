use super::super::*;

pub(super) fn assert_projected_owner_rows(
    rows: &[ploke_core::rag_types::ProofContextInfo],
    owner: Uuid,
    target: Uuid,
) {
    let owner = owner.to_string();
    let target = target.to_string();
    assert_eq!(rows.len(), 3, "projected proof context rows: {rows:#?}");
    let site = rows
        .iter()
        .find(|row| row.kind == "call_site")
        .expect("projected proof context should include call_site fact");
    let site_id = site
        .call_site_id
        .as_deref()
        .expect("call_site proof fact should carry call_site_id");
    assert_eq!(site.caller_def_id.as_deref(), Some(owner.as_str()));
    assert_eq!(
        site.build_domain_id.as_deref(),
        Some("bd:fixture-call-graph")
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.callee_def_id.as_deref() == Some(target.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
        }),
        "projected proof context should include resolved call_edge fact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.resolution_state.as_deref() == Some("resolved")
                && row.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "projected proof context should include resolved call_resolution fact: {rows:#?}"
    );
}
