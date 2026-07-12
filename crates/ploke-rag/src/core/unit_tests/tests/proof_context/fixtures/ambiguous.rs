use super::super::super::*;

#[tokio::test]
async fn proof_context_target_seed_preserves_ambiguous_dynamic_candidates() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let sibling = unique_id_by_name(&db, "function", "local_target")?;
    let target = unique_id_by_name(&db, "function", "other_target")?;
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_if_ambiguous_function_item"),
    )?;
    let callers = db.callers_for_target(target)?;
    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    let expected_count = callers
        .iter()
        .map(|caller| {
            if caller.status.status == ploke_db::call_graph::CallStatusKind::Resolved {
                3
            } else {
                2
            }
        })
        .sum::<usize>();
    assert_eq!(
        count, expected_count,
        "target-centered ambiguous proof projection should project resolved rows with call edges and candidate-only ambiguous rows without call edges: {callers:#?}"
    );

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.enabled = false;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.proof_context_degraded(),
        "projected target-centered proof facts should enable RAG proof context"
    );

    let proof_context = rag.collect_proof_context(&[(target, 1.0)])?;
    let rows = proof_context
        .get(&target)
        .expect("target candidate seed should receive linked ambiguous dynamic proof rows");
    let mut expected = vec![target.to_string(), sibling.to_string()];
    expected.sort();
    assert_ambiguous_candidate_rows(
        rows,
        owner,
        &expected,
        "target candidate seed should link the ambiguous dynamic proof rows",
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_preserves_direct_self_field_candidates() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &method_by_impl_self_query("DirectSelfFieldDispatcher", "invoke"),
    )?;
    let candidates = [
        unique_id_by_name(&db, "function", "direct_self_field_local")?,
        unique_id_by_name(&db, "function", "direct_self_field_other")?,
    ];
    for target in candidates {
        assert_eq!(
            db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?,
            2,
            "target-centered direct self-field candidate proof projection should only project call_site and ambiguous call_resolution rows"
        );
    }

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.enabled = false;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.proof_context_degraded(),
        "projected direct self-field candidate proof facts should enable RAG proof context"
    );

    let mut expected = candidates.iter().map(Uuid::to_string).collect::<Vec<_>>();
    expected.sort();
    for target in candidates {
        let proof_context = rag.collect_proof_context(&[(target, 1.0)])?;
        let rows = proof_context.get(&target).unwrap_or_else(|| {
            panic!("direct self-field candidate seed {target} should receive proof rows")
        });
        assert_ambiguous_candidate_rows(
            rows,
            owner,
            &expected,
            "direct self-field target seed should link the candidate-only dynamic proof rows",
        );
    }

    Ok(())
}

fn assert_ambiguous_candidate_rows(
    rows: &[ProofContextInfo],
    owner: Uuid,
    expected: &[String],
    label: &str,
) {
    let owner = owner.to_string();
    let site = rows
        .iter()
        .find(|row| row.kind == "call_site" && row.caller_def_id.as_deref() == Some(owner.as_str()))
        .unwrap_or_else(|| panic!("{label}: missing linked call_site row: {rows:#?}"));
    let site = site
        .call_site_id
        .as_deref()
        .unwrap_or_else(|| panic!("{label}: call_site row should carry call_site_id"));
    let resolution = rows
        .iter()
        .find(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site)
                && row.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| panic!("{label}: missing ambiguous call_resolution row: {rows:#?}"));
    let mut actual = resolution.candidate_def_ids.clone();
    actual.sort();
    assert_eq!(actual, expected, "{label}: candidate targets");
    assert!(
        rows.iter()
            .all(|row| { row.kind != "call_edge" || row.call_site_id.as_deref() != Some(site) }),
        "{label}: candidate-only proof rows should not fabricate call_edge facts: {rows:#?}"
    );
}
