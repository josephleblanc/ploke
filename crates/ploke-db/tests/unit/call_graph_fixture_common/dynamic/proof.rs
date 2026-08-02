use super::*;

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

pub(in crate::unit) struct ResolvedDynamicProofBatch<'a> {
    pub(in crate::unit) label: &'a str,
    pub(in crate::unit) cases: &'a [ResolvedDynamicContextCase],
    pub(in crate::unit) count: ProofEdgeCount,
}

pub(in crate::unit) fn assert_fixture_resolved_dynamic_proof_batches(
    batches: &[ResolvedDynamicProofBatch<'_>],
) -> Result<(), DbError> {
    for batch in batches {
        let db = setup_call_graph_fixture_db("fixture_call_graph")?;
        let target = function_id_by_name(&db, "local_target")?;
        let expected_edges = resolved_dynamic_proof_edges(&db, target, batch.cases)?;

        assert_owner_proof_edges(
            &db,
            batch.label,
            &expected_edges,
            "fixture_call_graph/src/lib.rs",
            "dynamic_dispatch_unbounded",
            batch.count,
        )?;
    }

    Ok(())
}

pub(in crate::unit) fn assert_candidate_proof(
    facts: &[serde_json::Value],
    site: &str,
    expected: &[String],
    label: &str,
) {
    assert_eq!(facts.len(), 2, "{label} proof facts: {facts:#?}");
    assert_candidate_resolution(facts, site, expected, label);
}

pub(in crate::unit) fn assert_returned_callable_candidate_proof(
    facts: &[serde_json::Value],
    site: &str,
    expected: &[String],
    label: &str,
) {
    assert_eq!(facts.len(), 3, "{label} proof facts: {facts:#?}");
    assert_candidate_resolution(facts, site, expected, label);
    assert!(
        facts.iter().any(|fact| {
            fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some("binding_evidence")
                && fact.get("call_site_id").and_then(serde_json::Value::as_str) == Some(site)
                && fact
                    .get("binding_evidence_kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("returned_callable")
                && fact
                    .get("resolution_state")
                    .and_then(serde_json::Value::as_str)
                    == Some("ambiguous")
                && fact.get("evidence_use").and_then(serde_json::Value::as_str)
                    == Some("proof_only")
        }),
        "{label} should include returned-callable binding evidence: {facts:#?}"
    );
}

pub(in crate::unit) fn assert_self_field_callable_candidate_proof(
    facts: &[serde_json::Value],
    site: &str,
    expected: &[String],
    label: &str,
) {
    assert_eq!(facts.len(), 3, "{label} proof facts: {facts:#?}");
    assert_candidate_resolution(facts, site, expected, label);
    assert!(
        facts.iter().any(|fact| {
            fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some("binding_evidence")
                && fact.get("call_site_id").and_then(serde_json::Value::as_str) == Some(site)
                && fact
                    .get("binding_evidence_kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("self_field_callable")
                && fact
                    .get("resolution_state")
                    .and_then(serde_json::Value::as_str)
                    == Some("ambiguous")
                && fact.get("evidence_use").and_then(serde_json::Value::as_str)
                    == Some("proof_only")
        }),
        "{label} should include self-field callable binding evidence: {facts:#?}"
    );
}

fn assert_candidate_resolution(
    facts: &[serde_json::Value],
    site: &str,
    expected: &[String],
    label: &str,
) {
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
