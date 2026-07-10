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
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site" && row.caller_def_id.as_deref() == Some(&owner.to_string())
        }),
        "target candidate seed should link the ambiguous dynamic call site: {rows:#?}"
    );
    let mut expected = vec![target.to_string(), sibling.to_string()];
    expected.sort();
    let resolution = rows
        .iter()
        .find(|row| {
            row.kind == "call_resolution" && row.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| {
            panic!(
                "target candidate seed should link the ambiguous dynamic resolution row: {rows:#?}"
            )
        });
    let mut actual = resolution.candidate_def_ids.clone();
    actual.sort();
    assert_eq!(
        actual, expected,
        "RAG proof context should preserve all ambiguous sibling candidates"
    );

    Ok(())
}
