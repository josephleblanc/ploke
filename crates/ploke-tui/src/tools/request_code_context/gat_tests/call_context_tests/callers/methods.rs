use super::super::assertions::{assert_incoming_expansion, assert_resolved_target};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_method_target_callers_with_call_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let method_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_local_instance_method"),
    )?;
    let nested_ref_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_typed_double_reference_local_instance_method",
        ),
    )?;
    let method_callers = [
        (method_owner, "method-call owner"),
        (nested_ref_owner, "nested-reference method owner"),
    ];

    let result = execute_fixture_request(&db, "instance_value", 1, "method_call_context").await?;
    assert_result_ok(&result, "instance_value", 1, "fixture_call_graph");

    for (owner, label) in method_callers {
        let part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| panic!("request_code_context should materialize the {label}"));
        let call = part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Method
                    && call.callee
                        == CallCalleeInfo::Method {
                            name: "instance_value".to_string(),
                            receiver: Some(CallReceiverInfo::TypedLocalBinding {
                                name: "value".to_string(),
                                type_path: vec!["LocalAssoc".to_string()],
                            }),
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!("{label} should retain outgoing call context to the seed target")
            });
        assert_resolved_target(call, target, CallTargetKind::Method);
        assert_incoming_expansion(part, call, target);
    }

    Ok(())
}
