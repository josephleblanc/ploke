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
