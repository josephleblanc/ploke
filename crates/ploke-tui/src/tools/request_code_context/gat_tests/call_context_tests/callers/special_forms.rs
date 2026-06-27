use super::super::assertions::{assert_expansion, assert_resolved_target, path};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_special_form_call_context() -> color_eyre::Result<()> {
    struct Case {
        label: &'static str,
        search_term: &'static str,
        call_id: &'static str,
        owner: Uuid,
        target: Uuid,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = [
        Case {
            label: "generic function turbofish",
            search_term: "call_generic_identity_turbofish",
            call_id: "special_form_generic_function_call_context",
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
                path: path(&["generic_identity"]),
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "generic method turbofish",
            search_term: "call_method_turbofish",
            call_id: "special_form_generic_method_call_context",
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
                    type_path: path(&["GenericMethodTarget"]),
                }),
            },
            relation: CallTargetKind::Method,
        },
        Case {
            label: "unsafe local function",
            search_term: "call_unsafe_function",
            call_id: "special_form_unsafe_function_call_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_unsafe_function"),
            )?,
            target: one_uuid(&db, &function_in_module_query(&["crate"], "unsafe_target"))?,
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["unsafe_target"]),
            },
            relation: CallTargetKind::Function,
        },
    ];

    for case in cases {
        let result = execute_fixture_request(&db, case.search_term, 1, case.call_id).await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let target_part = result
            .context
            .iter()
            .find(|part| part.id == case.target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} target",
                    case.label
                )
            });
        let call = owner_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == case.target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing special-form call context",
                    case.label
                )
            });
        assert_resolved_target(call, case.target, case.relation);
        assert_expansion(
            target_part,
            case.owner,
            case.target,
            call.site_id,
            CallExpansionKind::OutgoingTarget,
        );
    }

    Ok(())
}
