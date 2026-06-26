use super::super::super::super::super::*;
use super::super::super::helpers::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_collection_reads_real_fixture_blocker_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let macro_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_scoped_macro"),
    )?;
    let ambiguous_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_ambiguous_trait_method"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable blocker call context"
    );

    let call_context = rag.collect_call_context(&[(macro_owner, 1.0), (ambiguous_owner, 1.0)])?;
    let macro_context = call_context
        .get(&macro_owner)
        .expect("macro owner should receive outgoing call context");
    assert_eq!(
        macro_context.len(),
        1,
        "macro owner context: {macro_context:#?}"
    );
    let macro_call = &macro_context[0];
    assert_eq!(macro_call.kind, CallSiteKind::Macro);
    assert_eq!(
        macro_call.callee,
        CallCalleeInfo::Macro {
            name: "crate::crate_scoped_macro".to_string(),
        }
    );
    assert_eq!(macro_call.status, CallStatusKind::Unsupported);
    assert!(macro_call.resolution.is_none());
    assert!(
        macro_call.targets.is_empty(),
        "macro blocker rows must not fabricate RAG targets: {macro_call:#?}"
    );

    let ambiguous_context = call_context
        .get(&ambiguous_owner)
        .expect("ambiguous owner should receive outgoing call context");
    assert_eq!(
        ambiguous_context.len(),
        1,
        "ambiguous owner context: {ambiguous_context:#?}"
    );
    let ambiguous_call = &ambiguous_context[0];
    assert_eq!(ambiguous_call.kind, CallSiteKind::Method);
    assert_eq!(
        ambiguous_call.callee,
        CallCalleeInfo::Method {
            name: "overlap".to_string(),
            receiver: Some(CallReceiverInfo::LocalBinding {
                name: "value".to_string(),
            }),
        }
    );
    assert_eq!(ambiguous_call.status, CallStatusKind::Ambiguous);
    assert!(ambiguous_call.resolution.is_none());
    assert!(
        ambiguous_call.targets.is_empty(),
        "ambiguous blocker rows must not fabricate RAG targets: {ambiguous_call:#?}"
    );

    Ok(())
}
