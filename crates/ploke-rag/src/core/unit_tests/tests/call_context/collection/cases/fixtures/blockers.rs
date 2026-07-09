use super::super::super::super::super::*;
use super::super::super::helpers::*;
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
    let unsupported_method_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate", "trait_scope", "without_trait_import"],
            "call_unimported_trait_method",
        ),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable blocker call context"
    );

    let call_context = rag.collect_call_context(&[
        (macro_owner, 1.0),
        (ambiguous_owner, 1.0),
        (unsupported_method_owner, 1.0),
    ])?;
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

    let unsupported_method_context = call_context
        .get(&unsupported_method_owner)
        .expect("unsupported method owner should receive outgoing call context");
    assert_eq!(
        unsupported_method_context.len(),
        1,
        "unsupported method owner context: {unsupported_method_context:#?}"
    );
    let unsupported_method_call = &unsupported_method_context[0];
    assert_eq!(unsupported_method_call.kind, CallSiteKind::Method);
    assert_eq!(
        unsupported_method_call.callee,
        CallCalleeInfo::Method {
            name: "scoped_value".to_string(),
            receiver: Some(CallReceiverInfo::LocalBinding {
                name: "value".to_string(),
            }),
        }
    );
    assert_eq!(unsupported_method_call.status, CallStatusKind::Unsupported);
    assert!(unsupported_method_call.resolution.is_none());
    assert!(
        unsupported_method_call.targets.is_empty(),
        "unsupported method blocker rows must not fabricate RAG targets: {unsupported_method_call:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_generic_self_field_receiver_blockers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_nodes",
    )?));
    let cases = [
        (
            "generic str self-field len",
            one_uuid(
                &db,
                &method_by_impl_self_query("GenericStruct", "get_str_len"),
            )?,
            "len",
        ),
        (
            "generic SimpleTrait self-field into",
            one_uuid(
                &db,
                &method_by_impl_trait_self_query("SimpleTrait", "GenericStruct", "trait_method"),
            )?,
            "into",
        ),
    ];
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture_nodes call_graph schema should enable self-field blocker call context"
    );

    let hits = cases
        .iter()
        .map(|(_, owner, _)| (*owner, 1.0))
        .collect::<Vec<_>>();
    let call_context = rag.collect_call_context(&hits)?;

    for (label, owner, method) in cases {
        let owner_context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{label} owner should receive outgoing call context"));
        assert_eq!(
            owner_context.len(),
            1,
            "{label} owner context: {owner_context:#?}"
        );
        let call = &owner_context[0];
        assert_eq!(call.kind, CallSiteKind::Method, "{label}");
        assert_eq!(
            call.callee,
            CallCalleeInfo::Method {
                name: method.to_string(),
                receiver: Some(CallReceiverInfo::SelfField {
                    path: vec!["value".to_string()],
                }),
            },
            "{label}"
        );
        // tests/fixture_crates/fixture_nodes/src/impls.rs:77 and :103:
        // keep generic self-field receiver rows visible without inventing
        // a target before generic field receiver typing can prove one.
        assert_eq!(call.status, CallStatusKind::Unsupported, "{label}");
        assert!(call.resolution.is_none(), "{label}: {call:#?}");
        assert!(
            call.targets.is_empty(),
            "{label} self-field blocker must not fabricate RAG targets: {call:#?}"
        );
    }

    Ok(())
}
