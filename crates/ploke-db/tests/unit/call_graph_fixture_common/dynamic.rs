use super::*;

pub(in crate::unit) const AMBIGUOUS_DYNAMIC_OWNERS: [&str; 2] = [
    "call_if_ambiguous_function_item",
    "call_match_ambiguous_function_item",
];

pub(in crate::unit) fn dynamic_candidates(db: &Database) -> Result<Vec<Uuid>, DbError> {
    let mut expected = vec![
        function_id_by_name(db, "local_target")?,
        function_id_by_name(db, "other_target")?,
    ];
    expected.sort_unstable();
    Ok(expected)
}

pub(in crate::unit) fn candidate_strings(candidates: &[Uuid]) -> Vec<String> {
    let mut expected = candidates
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    expected
}

pub(in crate::unit) fn assert_dynamic_candidates(
    row: &CallContextRow,
    owner: Uuid,
    expected: &[Uuid],
    label: &str,
) {
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path, None);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert_eq!(
        row.targets.len(),
        expected.len(),
        "{label} targets: {row:#?}"
    );
    assert!(row.targets.iter().all(|target| {
        target.relation == CallRelationKind::DynamicFunction
            && target.source_kind == CallSiteKind::Dynamic
            && target.target_kind == CallTargetKind::Function
    }));

    let mut actual = row
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "{label} should expose proven ambiguous dynamic candidates"
    );
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct ResolvedDynamicContextCase {
    pub(in crate::unit) owner: &'static str,
    pub(in crate::unit) path: &'static [&'static str],
    pub(in crate::unit) expected_rows: usize,
}

pub(in crate::unit) fn assert_resolved_dynamic_context_cases(
    db: &Database,
    target: Uuid,
    cases: &[ResolvedDynamicContextCase],
) -> Result<(), DbError> {
    for case in cases {
        let owner = function_id_by_name(db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            case.expected_rows,
            "{} context rows: {context:#?}",
            case.owner
        );

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessDynamicContextCase {
    pub(in crate::unit) owner: &'static str,
    pub(in crate::unit) path: Option<&'static [&'static str]>,
    pub(in crate::unit) status: CallStatusKind,
}

pub(in crate::unit) fn assert_targetless_dynamic_context_cases(
    db: &Database,
    cases: &[TargetlessDynamicContextCase],
) -> Result<(), DbError> {
    for case in cases {
        let owner = function_id_by_name(db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} context rows: {context:#?}",
            case.owner
        );

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.site.path, case.path.map(path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.status.status, case.status);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{} dynamic failure must not fabricate targets: {row:#?}",
            case.owner
        );
    }

    Ok(())
}

pub(in crate::unit) fn resolved_dynamic_proof_edges(
    db: &Database,
    target: Uuid,
    cases: &[ResolvedDynamicContextCase],
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let mut expected = Vec::new();

    for case in cases {
        let owner = function_id_by_name(db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            case.expected_rows,
            "{} context rows: {context:#?}",
            case.owner
        );
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        let expected_count = context
            .iter()
            .map(|row| 2 + row.targets.len())
            .sum::<usize>();
        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, expected_count, "{} proof fact count", case.owner);
        expected.push(OwnerProofEdge {
            owner,
            site: row.site.id,
            span: row.site.span,
            target,
        });
    }

    Ok(expected)
}

pub(in crate::unit) fn assert_candidate_proof(
    facts: &[serde_json::Value],
    site: &str,
    expected: &[String],
    label: &str,
) {
    assert_eq!(facts.len(), 2, "{label} proof facts: {facts:#?}");

    let resolution = facts
        .iter()
        .find(|fact| {
            fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some("call_resolution")
                && fact.get("call_site_id").and_then(serde_json::Value::as_str) == Some(site)
        })
        .unwrap_or_else(|| panic!("{label} should project call_resolution proof fact"));
    assert_eq!(
        resolution
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("type_resolution_missing")
    );
    let mut actual = resolution
        .get("candidate_def_ids")
        .and_then(serde_json::Value::as_array)
        .expect("ambiguous dynamic resolution should carry candidate_def_ids")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("candidate_def_ids should contain string IDs")
                .to_string()
        })
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(actual, expected, "{label} candidate_def_ids");
}

pub(in crate::unit) fn assert_candidate_blocker(
    db: &Database,
    site: &str,
    label: &str,
) -> Result<(), DbError> {
    let rows = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site)
                && proof.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "{label} projected ambiguous blocker proof rows: {rows:#?}"
    );
    Ok(())
}
