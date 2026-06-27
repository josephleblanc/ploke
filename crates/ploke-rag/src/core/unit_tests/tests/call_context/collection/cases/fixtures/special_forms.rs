use super::super::super::super::super::*;
use super::super::super::helpers::*;

#[tokio::test]
async fn call_context_collection_reads_real_special_form_rows() -> Result<(), Error> {
    struct Case {
        label: &'static str,
        owner: Uuid,
        target: Uuid,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = [
        Case {
            label: "generic function turbofish",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_generic_identity_turbofish"),
            )?,
            target: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "generic_identity"),
            )?,
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["generic_identity".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "generic method turbofish",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_method_turbofish"),
            )?,
            target: one_uuid(
                &db,
                &method_by_impl_self_query("GenericMethodTarget", "generic_instance"),
            )?,
            kind: CallSiteKind::Method,
            callee: CallCalleeInfo::Method {
                name: "generic_instance".to_string(),
                receiver: Some(CallReceiverInfo::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: vec!["GenericMethodTarget".to_string()],
                }),
            },
            relation: CallTargetKind::Method,
        },
        Case {
            label: "unsafe local function",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_unsafe_function"),
            )?,
            target: one_uuid(&db, &function_in_module_query(&["crate"], "unsafe_target"))?,
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["unsafe_target".to_string()],
            },
            relation: CallTargetKind::Function,
        },
    ];
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable special-form call context"
    );

    let seeds = cases
        .iter()
        .map(|case| (case.owner, 1.0))
        .collect::<Vec<_>>();
    let call_context = rag.collect_call_context(&seeds)?;

    for case in &cases {
        let context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner should receive outgoing call context", case.label));
        assert_eq!(
            context.len(),
            1,
            "{} owner context: {context:#?}",
            case.label
        );
        let call = &context[0];
        assert_eq!(call.kind, case.kind);
        assert_eq!(call.callee, case.callee);
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, case.target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}
