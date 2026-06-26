use super::*;

use super::helpers::{
    assert_result_ok, execute_fixture_request, execute_fixture_tool_request, ui_field,
};
use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallExpansionKind, CallReceiverInfo, CallResolutionKind,
    CallSiteKind, CallStatusKind, CallTargetKind, ConciseContext, RequestCodeContextResult,
};
use ploke_db::Database;
use ploke_test_utils::setup_db_full_multi_embedding;
use std::sync::Arc;
use uuid::Uuid;

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

    let result = execute_fixture_request(&db, "instance_value", 1, "method_call_context").await?;
    assert_result_ok(&result, "instance_value", 1, "fixture_call_graph");

    let method_part = result
        .context
        .iter()
        .find(|part| part.id == method_owner)
        .expect("request_code_context should materialize the method-call caller owner");
    let method_call = method_part
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
        .expect("method caller should retain outgoing call context to the seed target");
    assert_resolved_target(method_call, target, CallTargetKind::Method);
    assert_incoming_expansion(method_part, method_call, target);

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

#[tokio::test]
async fn request_code_context_ui_payload_reports_context_carrier_counts() -> color_eyre::Result<()>
{
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));

    let tool_result =
        execute_fixture_tool_request(&db, "local_target", 1, "context_count_fields").await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "local_target", 1, "fixture_call_graph");

    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");

    let expected_call_context = result
        .context
        .iter()
        .map(|part| part.call_context.len())
        .sum::<usize>();
    let expected_type_context = result
        .context
        .iter()
        .filter(|part| part.type_context.is_some())
        .count();
    let expected_call_expansion = result
        .context
        .iter()
        .filter(|part| part.call_expansion.is_some())
        .count();
    let expected_proof_context = result
        .context
        .iter()
        .map(|part| part.proof_context.len())
        .sum::<usize>();

    assert_eq!(
        ui_field(payload, "type_context"),
        expected_type_context.to_string()
    );
    assert_eq!(
        ui_field(payload, "call_context"),
        expected_call_context.to_string()
    );
    assert_eq!(
        ui_field(payload, "call_expansion"),
        expected_call_expansion.to_string()
    );
    assert_eq!(
        ui_field(payload, "proof_context"),
        expected_proof_context.to_string()
    );

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_projected_proof_context() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        3,
        "single resolved path call should project call_site, call_edge, and call_resolution facts"
    );

    let tool_result =
        execute_fixture_tool_request(&db, "call_crate_local_target", 1, "projected_proof_context")
            .await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "call_crate_local_target", 1, "fixture_call_graph");
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "projected proof facts should avoid degraded proof-context note: {result:#?}"
    );

    let owner_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .expect("request_code_context should materialize the projected proof owner");
    let proof_rows = &owner_part.proof_context;
    let owner_id = owner.to_string();
    let target_id = target.to_string();
    assert_eq!(
        proof_rows.len(),
        3,
        "owner part should carry projected proof rows: {proof_rows:#?}"
    );
    let site = proof_rows
        .iter()
        .find(|row| row.kind == "call_site")
        .expect("projected proof context should include call_site fact");
    let site_id = site
        .call_site_id
        .as_deref()
        .expect("call_site proof fact should carry call_site_id");
    assert_eq!(site.caller_def_id.as_deref(), Some(owner_id.as_str()));
    assert_eq!(
        site.build_domain_id.as_deref(),
        Some("bd:fixture-call-graph")
    );
    assert!(
        proof_rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.caller_def_id.as_deref() == Some(owner_id.as_str())
                && row.callee_def_id.as_deref() == Some(target_id.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
        }),
        "projected proof context should include resolved call_edge fact: {proof_rows:#?}"
    );
    assert!(
        proof_rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.resolution_state.as_deref() == Some("resolved")
                && row.resolved_def_id.as_deref() == Some(target_id.as_str())
        }),
        "projected proof context should include resolved call_resolution fact: {proof_rows:#?}"
    );

    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let proof_context_count = result
        .context
        .iter()
        .map(|part| part.proof_context.len())
        .sum::<usize>();
    assert!(
        proof_context_count >= proof_rows.len(),
        "UI proof-context count should include at least the owner proof rows: {result:#?}"
    );
    assert_eq!(
        ui_field(payload, "proof_context"),
        proof_context_count.to_string()
    );

    Ok(())
}

#[tokio::test]
async fn request_code_context_surfaces_degraded_proof_context_note() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));

    let result = execute_fixture_request(&db, "local_target", 1, "proof_context_degraded").await?;
    assert_result_ok(&result, "local_target", 1, "fixture_call_graph");

    let note = result
        .note
        .as_deref()
        .expect("call-graph fixture without proof facts should surface proof-context degradation");
    assert!(
        note.contains("Proof-context expansion is unavailable"),
        "unexpected request_code_context note: {note}"
    );
    assert!(
        result
            .next_steps
            .iter()
            .any(|step| step.contains("Project proof facts")),
        "proof-context degradation should include proof projection recovery steps: {:#?}",
        result.next_steps
    );

    Ok(())
}

fn assert_resolved_target(call: &CallContextInfo, target: Uuid, relation: CallTargetKind) {
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, relation);
}

fn assert_incoming_expansion(part: &ConciseContext, call: &CallContextInfo, target: Uuid) {
    assert_expansion(
        part,
        target,
        target,
        call.site_id,
        CallExpansionKind::IncomingCaller,
    );
}

fn assert_expansion(
    part: &ConciseContext,
    seed: Uuid,
    target: Uuid,
    site: Uuid,
    relation: CallExpansionKind,
) {
    let expansion = part
        .call_expansion
        .expect("expanded part should carry call-expansion provenance");
    assert_eq!(expansion.seed_id, seed);
    assert_eq!(expansion.relation, relation);
    assert_eq!(expansion.call_site_id, site);
    assert_eq!(expansion.target_id, target);
    assert_eq!(expansion.distance, 1);
}

fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}
