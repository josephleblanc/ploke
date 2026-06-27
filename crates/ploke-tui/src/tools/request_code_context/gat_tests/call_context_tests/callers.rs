use super::assertions::{
    assert_expansion, assert_incoming_expansion, assert_resolved_target, path,
};
use super::*;

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

#[tokio::test]
async fn request_code_context_returns_constructor_target_callers_with_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        fixture: &'a str,
        search_term: &'a str,
        top_k: usize,
        target: ConstructorTarget<'a>,
        owner_module: &'a [&'a str],
        owner: &'a str,
        path: &'a [&'a str],
        relation: CallTargetKind,
    }

    enum ConstructorTarget<'a> {
        Struct {
            module: &'a [&'a str],
            name: &'a str,
        },
        Variant {
            enum_name: &'a str,
            name: &'a str,
        },
    }

    let cases = [
        Case {
            label: "tuple constructor",
            fixture: "fixture_call_graph",
            search_term: "pub struct NewType",
            top_k: 1,
            target: ConstructorTarget::Struct {
                module: &["crate"],
                name: "NewType",
            },
            owner_module: &["crate"],
            owner: "call_new_type_constructor",
            path: &["NewType"],
            relation: CallTargetKind::TupleStructConstructor,
        },
        Case {
            label: "enum variant constructor",
            fixture: "fixture_nodes",
            search_term: "Variant1",
            top_k: 10,
            target: ConstructorTarget::Variant {
                enum_name: "EnumWithData",
                name: "Variant1",
            },
            owner_module: &["crate", "imports"],
            owner: "use_imported_items",
            path: &["EnumWithData", "Variant1"],
            relation: CallTargetKind::EnumVariantConstructor,
        },
    ];

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let target = match case.target {
            ConstructorTarget::Struct { module, name } => {
                one_uuid(&db, &struct_in_module_query(module, name))?
            }
            ConstructorTarget::Variant { enum_name, name } => {
                one_uuid(&db, &variant_by_enum_query(enum_name, name))?
            }
        };
        let owner = one_uuid(
            &db,
            &function_in_module_query(case.owner_module, case.owner),
        )?;

        let result = execute_fixture_request(
            &db,
            case.search_term,
            case.top_k,
            "constructor_call_context",
        )
        .await?;
        assert_result_ok(&result, case.search_term, case.top_k, case.fixture);
        assert!(
            result.context.iter().any(|part| part.id == target),
            "request_code_context should materialize the {} target seed",
            case.label
        );

        let caller_part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} caller owner",
                    case.label
                )
            });
        let expected_path = path(case.path);
        let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: expected_path.clone(),
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} caller should retain outgoing call context to the seed target",
                    case.label
                )
            });
        assert_resolved_target(call, target, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_function_and_dynamic_owner_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        search_term: &'a str,
        owner: &'a str,
        call_kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;

    let cases = [
        Case {
            label: "ordinary path caller",
            search_term: "call_crate_local_target",
            owner: "call_crate_local_target",
            call_kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["crate", "local_target"]),
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "dynamic function caller",
            search_term: "call_parenthesized_local_target",
            owner: "call_parenthesized_local_target",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "aliased indexed dynamic function caller",
            search_term: "call_aliased_indexed_named_field_function_binding",
            owner: "call_aliased_indexed_named_field_function_binding",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];

    for case in cases {
        let owner = one_uuid(&db, &function_in_module_query(&["crate"], case.owner))?;
        let result =
            execute_fixture_request(&db, case.search_term, 1, "local_target_call_context").await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        let target_part = result
            .context
            .iter()
            .find(|part| part.id == target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the local_target outgoing callee for {}",
                    case.label
                )
            });
        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let call = owner_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == case.call_kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing call context to local_target",
                    case.label
                )
            });
        assert_resolved_target(call, target, case.relation);
        assert_expansion(
            target_part,
            owner,
            target,
            call.site_id,
            CallExpansionKind::OutgoingTarget,
        );
    }

    Ok(())
}
