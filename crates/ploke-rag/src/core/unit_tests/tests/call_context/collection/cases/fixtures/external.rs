use super::super::super::super::super::*;
use super::super::super::helpers::*;
#[tokio::test]
async fn call_context_collection_reads_real_fixture_external_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let string_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_prelude_string_new"),
    )?;
    let literal_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_literal_str_to_string"),
    )?;
    let vec_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_vec_len_external"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable external call context"
    );

    let call_context =
        rag.collect_call_context(&[(string_owner, 1.0), (literal_owner, 1.0), (vec_owner, 1.0)])?;

    let string_context = call_context
        .get(&string_owner)
        .expect("String::new owner should receive outgoing call context");
    assert_eq!(
        string_context.len(),
        1,
        "String::new owner context: {string_context:#?}"
    );
    let string_call = &string_context[0];
    assert_eq!(string_call.kind, CallSiteKind::Path);
    assert_eq!(
        string_call.callee,
        CallCalleeInfo::Path {
            path: vec!["String".to_string(), "new".to_string()],
        }
    );
    assert_eq!(string_call.status, CallStatusKind::External);
    assert!(string_call.resolution.is_none());
    assert!(
        string_call.targets.is_empty(),
        "external path calls must not fabricate RAG targets: {string_call:#?}"
    );

    let literal_context = call_context
        .get(&literal_owner)
        .expect("literal method owner should receive outgoing call context");
    assert_eq!(
        literal_context.len(),
        1,
        "literal method owner context: {literal_context:#?}"
    );
    let literal_call = &literal_context[0];
    assert_eq!(literal_call.kind, CallSiteKind::Method);
    assert_eq!(
        literal_call.callee,
        CallCalleeInfo::Method {
            name: "to_string".to_string(),
            receiver: Some(CallReceiverInfo::Literal),
        }
    );
    assert_eq!(literal_call.status, CallStatusKind::External);
    assert!(literal_call.resolution.is_none());
    assert!(
        literal_call.targets.is_empty(),
        "external literal method calls must not fabricate RAG targets: {literal_call:#?}"
    );

    let vec_context = call_context
        .get(&vec_owner)
        .expect("typed Vec owner should receive outgoing call context");
    assert_eq!(
        vec_context.len(),
        2,
        "typed Vec owner context: {vec_context:#?}"
    );
    let vec_new = vec_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Vec".to_string(), "new".to_string()],
                    }
        })
        .expect("Vec::new path call should stay visible");
    assert_eq!(vec_new.status, CallStatusKind::External);
    assert!(vec_new.resolution.is_none());
    assert!(vec_new.targets.is_empty());

    let vec_len = vec_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "len".to_string(),
                        receiver: Some(CallReceiverInfo::TypedLocalBinding {
                            name: "value".to_string(),
                            type_path: vec!["Vec".to_string()],
                        }),
                    }
        })
        .expect("typed Vec::len method call should stay visible");
    assert_eq!(vec_len.status, CallStatusKind::External);
    assert!(vec_len.resolution.is_none());
    assert!(
        vec_len.targets.is_empty(),
        "external Vec::len calls must not fabricate RAG targets: {vec_len:#?}"
    );

    Ok(())
}
