use super::*;

pub(in crate::unit) fn proof_rows_for_site(
    rows: &[ProofGraphContextRow],
    site: Uuid,
) -> Vec<&ProofGraphContextRow> {
    let site = site.to_string();
    rows.iter()
        .filter(|row| row.call_site_id.as_deref() == Some(site.as_str()))
        .collect()
}

pub(in crate::unit) fn proof_kind_count(rows: &[&ProofGraphContextRow], kind: &str) -> usize {
    rows.iter().filter(|row| row.kind == kind).count()
}

pub(in crate::unit) fn proof_fact_for_kind<'a>(
    rows: &'a [&ProofGraphContextRow],
    kind: &str,
) -> &'a ProofGraphContextRow {
    let matches = rows
        .iter()
        .copied()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind} proof fact; rows: {rows:#?}"
    );
    matches[0]
}

pub(in crate::unit) fn assert_resolved_site_proof(
    label: &str,
    rows: &[ProofGraphContextRow],
    owner: Uuid,
    site: Uuid,
    target: Uuid,
) {
    let site_rows = proof_rows_for_site(rows, site);
    assert_eq!(
        site_rows.len(),
        3,
        "{label} should return linked call_site, call_edge, and call_resolution facts for {site}: {site_rows:#?}"
    );
    assert_eq!(proof_kind_count(&site_rows, "call_site"), 1);
    assert_eq!(proof_kind_count(&site_rows, "call_edge"), 1);
    assert_eq!(proof_kind_count(&site_rows, "call_resolution"), 1);

    let owner = owner.to_string();
    let target = target.to_string();
    let site_fact = proof_fact_for_kind(&site_rows, "call_site");
    assert_eq!(site_fact.caller_def_id.as_deref(), Some(owner.as_str()));

    let edge = proof_fact_for_kind(&site_rows, "call_edge");
    assert_eq!(edge.caller_def_id.as_deref(), Some(owner.as_str()));
    assert_eq!(edge.callee_def_id.as_deref(), Some(target.as_str()));
    assert_eq!(edge.blocker_reason, None);

    let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
    assert_eq!(resolution.blocker_reason, None);
}
