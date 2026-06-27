use super::super::assertions::{assert_incoming_expansion, assert_resolved_target};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_trait_dispatch_target_callers_with_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        owner: &'a str,
    }

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_trait_self_query(
            "LocalDispatchTrait",
            "TraitDispatchTarget",
            "trait_value",
        ),
    )?;
    let cases = [
        Case {
            label: "initialized trait-dispatch caller",
            owner: "call_initialized_local_trait_method",
        },
        Case {
            label: "reference-chain trait-object caller",
            owner: "call_reference_chain_trait_object_binding_method",
        },
    ];

    let result =
        execute_fixture_request(&db, "144 trait_value", 5, "trait_dispatch_call_context").await?;
    assert_result_ok(&result, "144 trait_value", 5, "fixture_call_graph");

    for case in cases {
        let owner = one_uuid(&db, &function_in_module_query(&["crate"], case.owner))?;
        let caller_part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!("request_code_context should materialize the {}", case.label)
            });
        let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Method
                    && call.callee
                        == CallCalleeInfo::Method {
                            name: "trait_value".to_string(),
                            receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                                name: "value".to_string(),
                                init_path: vec!["TraitDispatchTarget".to_string()],
                            }),
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should retain outgoing trait-dispatch context to the seed target",
                    case.label
                )
            });
        assert_resolved_target(call, target, CallTargetKind::Method);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}
