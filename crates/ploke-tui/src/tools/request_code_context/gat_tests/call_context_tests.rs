use super::*;

use crate::app::commands::harness::TestRuntime;
use crate::user_config::RetrievalStrategyUser;
use ploke_core::ArcStr;
use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallExpansionKind, CallReceiverInfo, CallResolutionKind,
    CallSiteKind, CallStatusKind, CallTargetKind, ConciseContext, RequestCodeContextResult,
};
use ploke_db::Database;
use ploke_db::bm25_index::bm25_service::Bm25Status;
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_test_utils::setup_db_full_multi_embedding;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
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
    let field = |name: &str| {
        payload
            .fields
            .iter()
            .find(|field| field.name.as_ref() == name)
            .unwrap_or_else(|| panic!("missing {name} field in payload: {payload:#?}"))
            .value
            .as_ref()
            .to_string()
    };

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

    assert_eq!(field("type_context"), expected_type_context.to_string());
    assert_eq!(field("call_context"), expected_call_context.to_string());
    assert_eq!(field("call_expansion"), expected_call_expansion.to_string());
    assert_eq!(field("proof_context"), expected_proof_context.to_string());

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

async fn execute_fixture_request(
    db: &Arc<Database>,
    search_term: &str,
    top_k: usize,
    call_id: &'static str,
) -> color_eyre::Result<RequestCodeContextResult> {
    let tool_result = execute_fixture_tool_request(db, search_term, top_k, call_id).await?;
    Ok(serde_json::from_str(&tool_result.content)?)
}

async fn execute_fixture_tool_request(
    db: &Arc<Database>,
    search_term: &str,
    top_k: usize,
    call_id: &'static str,
) -> color_eyre::Result<ToolResult> {
    let rt = TestRuntime::new_with_embedding_processor(db, EmbeddingProcessor::new_mock());
    rt.setup_loaded_standalone_crate(ploke_test_utils::workspace_root())
        .await;
    let state = rt.state_arc();
    {
        let mut cfg = state.config.write().await;
        cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
        cfg.rag.top_k = top_k;
        cfg.rag.per_part_max_tokens = 4096;
        cfg.token_limit = 65_536;
    }
    let rag = state
        .rag
        .as_ref()
        .expect("test runtime should provide RagService")
        .clone();
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture projection should enable call-context expansion"
    );
    rag.bm25_rebuild().await?;
    let mut ready = false;
    for _ in 0..50 {
        match rag.bm25_status().await? {
            Bm25Status::Ready { docs } if docs > 0 => {
                ready = true;
                break;
            }
            Bm25Status::Error(err) => panic!("BM25 rebuild failed: {err}"),
            _ => sleep(Duration::from_millis(50)).await,
        }
    }
    assert!(
        ready,
        "BM25 index must become ready before request_code_context"
    );

    let ctx = super::super::super::Ctx {
        state,
        event_bus: Arc::new(crate::EventBus::new(crate::EventBusCaps::default())),
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from(call_id),
    };
    let tool_result = RequestCodeContextGat::execute(
        RequestCodeContextParams {
            token_budget_per_result: Some(4096),
            token_budget_total: Some(65_536),
            search_term: Some(Cow::Borrowed(search_term)),
        },
        ctx,
    )
    .await?;

    Ok(tool_result)
}

fn assert_result_ok(
    result: &RequestCodeContextResult,
    search_term: &str,
    top_k: usize,
    fixture: &str,
) {
    assert!(
        result.ok,
        "request_code_context returned error for {fixture}: {result:#?}"
    );
    assert_eq!(result.search_term, search_term);
    assert_eq!(result.top_k, top_k);
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| !note.contains("Call-context expansion is unavailable")),
        "call-context degradation should not be surfaced for {fixture}: {result:#?}"
    );
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
