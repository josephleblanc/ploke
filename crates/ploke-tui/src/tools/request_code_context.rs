use ploke_core::rag_types::{
    AssembledContext, AssembledMeta, ConciseContext, ContextStats, RequestCodeContextResult,
};
use ploke_db::get_by_id::{GetNodeInfo, NodePaths};

use super::*;
use ploke_core::RetrievalScope;

// Canonical schema: RequestCodeContextArgs {
//     token_budget_per_result,
//     token_budget_total,
//     search_term
// }
pub struct RequestCodeContext {
    rag: Arc<RagService>,
}

#[derive(Clone, PartialOrd, PartialEq, Deserialize)]
pub struct RequestCodeContextInput {
    #[serde(default, alias = "token_budget")]
    pub token_budget_per_result: Option<u32>,
    #[serde(default)]
    pub token_budget_total: Option<u32>,
    #[serde(default)]
    pub search_term: Option<String>,
}

lazy_static::lazy_static! {
    static ref REQUEST_CODE_CONTEXT_PARAMETERS: serde_json::Value = json!({
            "type": "object",
            "properties": {
                "search_term": {
                    "type": "string",
                    "description": "Search query for code graph retrieval. Good values include identifiers, module names, file names, error names, type names, or concise code terms."
                },
                "token_budget_per_result": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional maximum tokens per returned code snippet."
                },
                "token_budget_total": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional maximum total tokens across all returned snippets."
                }
            }
        }
    );
}

fn zero_result_note(search_term: &str) -> String {
    format!(
        "No indexed snippets matched `{search_term}`. This tool is broad retrieval, not exact symbol lookup."
    )
}

fn zero_result_next_steps() -> Vec<String> {
    vec![
        "Retry with an exact symbol, type, method, module, or error name if you know one."
            .to_string(),
        "If you know the area but not the symbol, use list_dir or read_file on likely directories/files."
            .to_string(),
        "If you know the item name, switch to code_item_lookup for exact-definition lookup."
            .to_string(),
    ]
}

fn stale_context_next_steps() -> Vec<String> {
    vec![
        "Refresh or re-resolve the target before relying on this context.".to_string(),
        "Use read_file for the suspected file if you need the current live content immediately."
            .to_string(),
        "Retry request_code_context after the workspace index has been refreshed.".to_string(),
    ]
}

fn type_context_degraded_note() -> String {
    "Type-context expansion is unavailable for this workspace index; results reflect BM25/dense retrieval only (no typed-graph neighbors)."
        .to_string()
}

fn type_context_degraded_next_steps() -> Vec<String> {
    vec![
        "Re-index the workspace with a typed type-graph build if typed neighbors are required."
            .to_string(),
        "Use code_item_lookup or read_file when you need exact definitions rather than broad retrieval."
            .to_string(),
    ]
}

fn apply_type_context_degraded_note(result: &mut RequestCodeContextResult) {
    let note = type_context_degraded_note();
    match result.note.as_mut() {
        Some(existing) => {
            existing.push_str("\n\n");
            existing.push_str(&note);
        }
        None => result.note = Some(note),
    }
    result.next_steps.extend(type_context_degraded_next_steps());
}

fn summarize_request_code_context_result(
    result: &mut RequestCodeContextResult,
    stats: &ContextStats,
) -> String {
    if result.context.is_empty() {
        if stats.skipped_io_errors > 0 {
            result.note = Some(format!(
                "Context degraded: skipped {} snippets because indexed file hashes did not match the live files.",
                stats.skipped_io_errors
            ));
            result.next_steps = stale_context_next_steps();
            format!(
                "Context degraded: 0 snippets returned, {} stale snippets skipped",
                stats.skipped_io_errors
            )
        } else {
            result.note = Some(zero_result_note(&result.search_term));
            result.next_steps = zero_result_next_steps();
            "No code context found (0 snippets)".to_string()
        }
    } else if stats.skipped_io_errors > 0 {
        result.note = Some(format!(
            "Context degraded: returned {} snippets but skipped {} stale snippets whose indexed file hashes did not match the live files.",
            result.context.len(),
            stats.skipped_io_errors
        ));
        result.next_steps = stale_context_next_steps();
        format!(
            "Context degraded: {} snippets returned, {} stale snippets skipped",
            result.context.len(),
            stats.skipped_io_errors
        )
    } else {
        format!("Context assembled: {} snippets", result.context.len())
    }
}

// --- GAT-based tool impl ---
use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestCodeContextParams<'a> {
    #[serde(default, alias = "token_budget")]
    pub token_budget_per_result: Option<u32>,
    #[serde(default)]
    pub token_budget_total: Option<u32>,
    #[serde(borrow)]
    pub search_term: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestCodeContextParamsOwned {
    #[serde(default, alias = "token_budget")]
    pub token_budget_per_result: Option<u32>,
    #[serde(default)]
    pub token_budget_total: Option<u32>,
    pub search_term: Option<String>,
}

#[derive(Default)]
pub struct RequestCodeContextGat {
    pub budget: TokenBudget,
    pub strategy: RetrievalStrategy,
}

impl super::Tool for RequestCodeContextGat {
    type Output = ploke_core::rag_types::RequestCodeContextResult;
    type OwnedParams = RequestCodeContextParamsOwned;
    type Params<'de>
        = RequestCodeContextParams<'de>
    where
        Self: 'de;

    fn name() -> ToolName {
        ToolName::RequestCodeContext
    }
    fn description() -> ToolDescription {
        Self::name().description()
    }
    fn schema() -> &'static serde_json::Value {
        REQUEST_CODE_CONTEXT_PARAMETERS.deref()
    }

    fn build(_ctx: &super::Ctx) -> Self {
        RequestCodeContextGat::default()
    }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        RequestCodeContextParamsOwned {
            token_budget_per_result: params.token_budget_per_result,
            token_budget_total: params.token_budget_total,
            search_term: params.search_term.as_ref().map(|s| s.to_string()),
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::utils::{calc_top_k_for_budget, max_results_for_budget};
        use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};
        if let Some(parse_failure) = ctx
            .state
            .with_system_read(|sys| sys.last_parse_failure().cloned())
            .await
        {
            return Err(tool_ui_error(parse_failure.message.clone()));
        }
        let rag = match &ctx.state.rag {
            Some(r) => r.clone(),
            None => {
                return Err(ploke_error::Error::Internal(
                    ploke_error::InternalError::CompilerError(
                        "RAG service unavailable".to_string(),
                    ),
                ));
            }
        };
        let mut search_term_opt = params
            .search_term
            .as_ref()
            .map(|s| s.as_ref().trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if search_term_opt.is_none() {
            use crate::chat_history::MessageKind;
            let history = ctx.state.chat.read().await;
            let mut last_user: Option<String> = None;
            for msg in history.iter_path() {
                if matches!(msg.kind, MessageKind::User) {
                    last_user = Some(msg.content.clone());
                }
            }
            search_term_opt = last_user
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
        let Some(search_term) = search_term_opt else {
            return Err(ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(
                    "No query available (search_term missing or empty)".to_string(),
                ),
            ));
        };
        let cfg = ctx.state.config.read().await;
        let token_budget_total = params.token_budget_total.unwrap_or(cfg.token_limit).max(1);
        let token_budget_per_result = params
            .token_budget_per_result
            .unwrap_or(cfg.rag.per_part_max_tokens as u32)
            .max(1);
        let per_result_max = (token_budget_per_result as usize)
            .min(token_budget_total as usize)
            .max(1);
        let top_k = calc_top_k_for_budget(token_budget_total)
            .min(max_results_for_budget(
                token_budget_total,
                token_budget_per_result,
            ))
            .min(cfg.rag.top_k);
        let budget = TokenBudget {
            max_total: token_budget_total as usize,
            per_part_max: per_result_max,
            ..Default::default()
        };
        let strategy = cfg.rag.strategy.to_runtime();
        drop(cfg);
        let AssembledContext { parts, stats } = rag
            .get_context(
                &search_term,
                top_k,
                &budget,
                &strategy,
                RetrievalScope::LoadedWorkspace,
            )
            .await?;

        let assembled_meta = AssembledMeta {
            search_term,
            top_k,
            kind: ContextPartKind::Code,
        };
        tracing::debug!(?parts, ?stats);
        let mut result = RequestCodeContextResult::from_assembled(parts, assembled_meta);
        let summary = summarize_request_code_context_result(&mut result, &stats);
        if rag.type_context_degraded() {
            apply_type_context_degraded_note(&mut result);
        }
        let mut ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("search_term", result.search_term.as_str())
            .with_field(
                "token_budget_per_result",
                token_budget_per_result.to_string(),
            )
            .with_field("token_budget_total", token_budget_total.to_string())
            .with_field("top_k", result.top_k.to_string())
            .with_field("returned", result.context.len().to_string());
        if let Some(note) = result.note.as_ref() {
            let details = std::iter::once(note.as_str().to_string())
                .chain(
                    result
                        .next_steps
                        .iter()
                        .enumerate()
                        .map(|(idx, step)| format!("{}. {}", idx + 1, step)),
                )
                .collect::<Vec<_>>()
                .join("\n");
            ui_payload = ui_payload.with_details(details);
        }
        let serialized = serde_json::to_string(&result).expect("serialization");
        Ok(ToolResult {
            content: serialized,
            ui_payload: Some(ui_payload),
        })
    }
}
#[cfg(test)]
mod gat_tests {
    //! TUI tool coverage boundary for typed type context:
    //!
    //! - Quarantined: strict direct `request_code_context` matrix payload
    //!   assertions. Production retrieval can return a matrix terminal as an
    //!   ordinary search hit before type-context expansion reaches it, so these
    //!   rows must not drive BM25 ranking or `top_k` changes.
    //! - Ignored/live: `LiveIgnored` rows exercise the production model/tool
    //!   path and assert `ToolCallRequested`, `ToolCallCompleted`, and payload
    //!   type context. They do not trust final model wording as proof.
    //! - Not covered here: exact DB coordinates, parser source coordinates, or
    //!   unbounded recursive type grammar. Those stay in parser/DB matrix tests.

    use super::*;

    #[test]
    fn params_deserialize_and_into_owned() {
        let raw =
            r#"{"token_budget_per_result":512,"token_budget_total":1536,"search_term":"foo bar"}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget_per_result, Some(512));
        assert_eq!(params.token_budget_total, Some(1536));
        assert_eq!(params.search_term.as_deref(), Some("foo bar"));
        let owned = RequestCodeContextGat::into_owned(&params);
        assert_eq!(owned.token_budget_per_result, Some(512));
        assert_eq!(owned.token_budget_total, Some(1536));
        assert_eq!(owned.search_term.as_deref(), Some("foo bar"));
    }

    #[test]
    fn params_missing_search_term_still_parses() {
        let raw = r#"{"token_budget_per_result":256,"token_budget_total":768}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget_per_result, Some(256));
        assert_eq!(params.token_budget_total, Some(768));
        assert!(params.search_term.is_none());
    }

    #[test]
    fn legacy_token_budget_alias_maps_to_per_result() {
        let raw = r#"{"token_budget":256}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget_per_result, Some(256));
        assert!(params.token_budget_total.is_none());
        assert!(params.search_term.is_none());
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    #[tokio::test]
    async fn request_code_context_degrades_on_non_typed_db() -> color_eyre::Result<()> {
        use crate::app::commands::harness::TestRuntime;
        use crate::user_config::RetrievalStrategyUser;
        use ploke_core::ArcStr;
        use ploke_core::rag_types::RequestCodeContextResult;
        use ploke_db::bm25_index::bm25_service::Bm25Status;
        use ploke_db::Database;
        use ploke_embed::indexer::EmbeddingProcessor;
        use ploke_test_utils::fixture_dbs::backup_fixture_path_or_seed;
        use ploke_test_utils::{FIXTURE_NODES_CANONICAL, workspace_root};
        use std::borrow::Cow;
        use std::sync::Arc;
        use tokio::time::{Duration, sleep};
        use uuid::Uuid;

        let snapshot = backup_fixture_path_or_seed(&FIXTURE_NODES_CANONICAL)?;
        let db = Arc::new(
            Database::create_new_backup_default(&snapshot)
                .await
                .map_err(color_eyre::eyre::Report::from)?,
        );
        assert!(
            !db.has_typed_type_graph_relations()?,
            "stale plain starting-db restore must lack typed-graph relations for this regression"
        );
        let rt = TestRuntime::new_with_embedding_processor(&db, EmbeddingProcessor::new_mock());
        rt.setup_loaded_standalone_crate(workspace_root()).await;
        let state = rt.state_arc();
        {
            let mut cfg = state.config.write().await;
            cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
            cfg.token_limit = 4096;
        }
        let rag = state
            .rag
            .as_ref()
            .expect("test runtime should provide RagService")
            .clone();
        assert!(
            rag.type_context_degraded(),
            "RagService must record degraded type-context when relations are absent"
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

        let ctx = super::super::Ctx {
            state,
            event_bus: Arc::new(crate::EventBus::new(crate::EventBusCaps::default())),
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: ArcStr::from("degraded_type_context"),
        };
        let tool_result = RequestCodeContextGat::execute(
            RequestCodeContextParams {
                token_budget_per_result: Some(512),
                token_budget_total: Some(2048),
                search_term: Some(Cow::Borrowed("method")),
            },
            ctx,
        )
        .await?;

        let parsed: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
        let note = parsed
            .note
            .as_deref()
            .expect("degraded type-context must surface a note for the model");
        assert!(
            note.contains("Type-context expansion is unavailable"),
            "unexpected note: {note}"
        );
        assert!(
            !parsed.next_steps.is_empty(),
            "degraded type-context must include next_steps"
        );
        Ok(())
    }

    #[test]
    fn stale_snippet_skips_are_model_visible_degraded_context() {
        let mut result = RequestCodeContextResult::from_assembled(
            Vec::new(),
            AssembledMeta {
                search_term: "helpers".to_string(),
                top_k: 3,
                kind: ContextPartKind::Code,
            },
        );
        let stats = ContextStats {
            skipped_io_errors: 1,
            ..Default::default()
        };

        let summary = summarize_request_code_context_result(&mut result, &stats);

        assert_eq!(
            summary,
            "Context degraded: 0 snippets returned, 1 stale snippets skipped"
        );
        assert!(result.ok);
        assert!(
            result
                .note
                .as_deref()
                .is_some_and(|note| note.contains("indexed file hashes did not match"))
        );
        assert!(
            result
                .next_steps
                .iter()
                .any(|step| step.contains("Refresh or re-resolve"))
        );
    }

    #[test]
    fn name_desc_and_schema_present() {
        assert!(matches!(
            RequestCodeContextGat::name(),
            ToolName::RequestCodeContext
        ));
        assert_eq!(
            RequestCodeContextGat::description(),
            ToolName::RequestCodeContext.description()
        );
        let schema = RequestCodeContextGat::schema();
        let obj = schema.as_object().expect("schema obj");
        assert!(obj.contains_key("properties"));
    }

    #[test]
    fn de_to_value() -> color_eyre::Result<()> {
        let def = <RequestCodeContextGat as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        eprintln!("{}", serde_json::to_string_pretty(&v)?);
        let expected = json!({
            "type": "function",
            "function": {
                "name": "request_code_context",
                "description": "Search the indexed workspace code graph and return ranked code snippets within per-result and total token budgets. Use this as the default broad code search when you have identifiers, module or file names, error names, type names, or other code terms but do not yet know an exact path. It currently uses sparse vector search with BM25 over the loaded workspace.\n",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "search_term": {
                            "type": "string",
                            "description": "Search query for code graph retrieval. Good values include identifiers, module names, file names, error names, type names, or concise code terms."
                        },
                        "token_budget_per_result": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Optional maximum tokens per returned code snippet."
                        },
                        "token_budget_total": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Optional maximum total tokens across all returned snippets."
                        }
                    }
                }
            }
        });
        assert_eq!(expected, v);
        Ok(())
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    #[tokio::test]
    #[ignore = "quarantined: do not force BM25/top_k behavior to satisfy matrix payload assertions"]
    async fn request_code_context_tool_emits_matrix_type_context() -> color_eyre::Result<()> {
        use crate::app::commands::harness::TestRuntime;
        use crate::app_state::events::SystemEvent;
        use crate::user_config::RetrievalStrategyUser;
        use crate::{AppEvent, EventBus, EventBusCaps, EventPriority};
        use ploke_core::ArcStr;
        use ploke_core::rag_types::RequestCodeContextResult;
        use ploke_core::tool_types::FunctionMarker;
        use ploke_db::bm25_index::bm25_service::Bm25Status;
        use ploke_embed::indexer::EmbeddingProcessor;
        use ploke_llm::response::{FunctionCall, ToolCall};
        use tokio::time::{Duration, sleep, timeout};
        use uuid::Uuid;

        let cases = ploke_test_utils::positive_type_shape_cases()
            .iter()
            .filter(|case| covers(case, ploke_test_utils::ShapePipelineCoverage::TuiTool))
            .collect::<Vec<_>>();
        assert!(
            !cases.is_empty(),
            "shared type-shape matrix should include direct TUI tool rows"
        );

        for case in cases {
            let db = std::sync::Arc::new(ploke_test_utils::fresh_backup_fixture_db(
                case.fixture.searchable_fixture(),
            )?);
            let rt = TestRuntime::new_with_embedding_processor(&db, EmbeddingProcessor::new_mock());
            rt.setup_loaded_standalone_crate(ploke_test_utils::workspace_root())
                .await;
            let state = rt.state_arc();
            {
                let mut cfg = state.config.write().await;
                // Keep production `rag.top_k`; sparse avoids the mock dense embedder.
                cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
                cfg.token_limit = 4096;
            }
            let rag = state
                .rag
                .as_ref()
                .expect("test runtime should provide RagService")
                .clone();
            rag.bm25_rebuild().await?;
            let mut ready_docs = None;
            for _ in 0..50 {
                match rag.bm25_status().await? {
                    Bm25Status::Ready { docs } if docs > 0 => {
                        ready_docs = Some(docs);
                        break;
                    }
                    Bm25Status::Error(err) => panic!("BM25 rebuild failed: {err}"),
                    _ => sleep(Duration::from_millis(50)).await,
                }
            }
            assert!(
                ready_docs.is_some(),
                "BM25 index did not become ready before request_code_context for {}",
                case.name
            );

            let owner_id = resolve_matrix_owner(&state.db, case.owner)?;
            let expected_seed_ids =
                resolve_matrix_expected_type_context_seed_ids(&state.db, case, owner_id)?;
            let expected_target_id = resolve_matrix_target(&state.db, case.terminal)?;
            let event_bus = std::sync::Arc::new(EventBus::new(EventBusCaps::default()));
            let mut event_rx = event_bus.subscribe(EventPriority::Realtime);
            let request_id = Uuid::new_v4();
            let parent_id = Uuid::new_v4();
            let call_id = ArcStr::from("matrix_request_code_context");
            let ctx = super::super::Ctx {
                state,
                event_bus: std::sync::Arc::clone(&event_bus),
                request_id,
                parent_id,
                call_id: call_id.clone(),
            };
            let tool_call = ToolCall {
                call_id: call_id.clone(),
                call_type: FunctionMarker,
                function: FunctionCall {
                    name: ToolName::RequestCodeContext,
                    arguments: serde_json::json!({
                        "search_term": case.search_term,
                        "token_budget": 4096
                    })
                    .to_string(),
                },
                extra_content: None,
            };

            super::super::process_tool(tool_call, ctx).await?;

            let completed = timeout(Duration::from_secs(5), async {
                loop {
                    match event_rx.recv().await {
                        Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                            request_id: event_request_id,
                            call_id: event_call_id,
                            content,
                            ..
                        })) if event_request_id == request_id && event_call_id == call_id => {
                            break content;
                        }
                        Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                            request_id: event_request_id,
                            call_id: event_call_id,
                            error,
                            ..
                        })) if event_request_id == request_id && event_call_id == call_id => {
                            panic!("request_code_context failed for {}: {error}", case.name);
                        }
                        Ok(_) => {}
                        Err(err) => panic!(
                            "event channel closed before ToolCallCompleted for {}: {err}",
                            case.name
                        ),
                    }
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!("timed out waiting for ToolCallCompleted for {}", case.name)
            });

            let result: RequestCodeContextResult = serde_json::from_str(&completed)?;
            assert_matrix_payload_has_type_context(
                case,
                expected_target_id,
                &expected_seed_ids,
                &result,
            );
        }
        Ok(())
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "live OpenRouter model/tool matrix test; requires OPENROUTER_API_KEY"]
    async fn live_request_code_context_matrix_uses_production_tool_payload()
    -> color_eyre::Result<()> {
        use crate::app::commands::harness::TestRuntime;
        use crate::app_state::events::SystemEvent;
        use crate::app_state::handlers::chat::add_msg_immediate;
        use crate::chat_history::MessageKind;
        use crate::llm::manager::events::{ContextPlan, ContextPlanMessage};
        use crate::llm::manager::{ChatEvt, LlmEvent, RequestMessage};
        use crate::user_config::RetrievalStrategyUser;
        use crate::{AppEvent, EventPriority};
        use ploke_core::rag_types::RequestCodeContextResult;
        use ploke_core::tool_types::ToolName;
        use ploke_embed::indexer::EmbeddingProcessor;
        use tokio::time::{Duration, timeout};
        use uuid::Uuid;

        if crate::test_harness::openrouter_env().is_none() {
            eprintln!(
                "skipping live_request_code_context_matrix_uses_production_tool_payload: OPENROUTER_API_KEY not set"
            );
            return Ok(());
        }

        let cases = ploke_test_utils::positive_type_shape_cases()
            .iter()
            .filter(|case| covers(case, ploke_test_utils::ShapePipelineCoverage::LiveIgnored))
            .collect::<Vec<_>>();
        assert!(
            !cases.is_empty(),
            "shared type-shape matrix should include ignored live rows"
        );

        for case in cases {
            let db = std::sync::Arc::new(ploke_test_utils::fresh_backup_fixture_db(
                case.fixture.searchable_fixture(),
            )?);
            let rt = TestRuntime::new_with_embedding_processor(&db, EmbeddingProcessor::new_mock())
                .spawn_state_manager()
                .spawn_llm_manager();
            rt.setup_loaded_standalone_crate(ploke_test_utils::workspace_root())
                .await;
            let state = rt.state_arc();
            let event_bus = rt.event_bus_arc();
            {
                let mut cfg = state.config.write().await;
                // Keep production `rag.top_k`; sparse avoids the mock dense embedder.
                cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
                cfg.token_limit = 4096;
                cfg.llm_timeout_secs = 90;
            }
            let rag = state
                .rag
                .as_ref()
                .expect("test runtime should provide RagService")
                .clone();
            rag.bm25_rebuild().await?;

            let owner_id = resolve_matrix_owner(&state.db, case.owner)?;
            let expected_seed_ids =
                resolve_matrix_expected_type_context_seed_ids(&state.db, case, owner_id)?;
            let expected_target_id = resolve_matrix_target(&state.db, case.terminal)?;

            let mut event_rx = event_bus.subscribe(EventPriority::Realtime);
            let user_msg_id = Uuid::new_v4();
            let request_msg_id = Uuid::new_v4();
            let prompt = format!(
                "Call request_code_context with search_term {:?} and token_budget 4096. Do not answer from memory.",
                case.search_term
            );
            add_msg_immediate(
                &state,
                &event_bus,
                user_msg_id,
                prompt.clone(),
                MessageKind::User,
            )
            .await;
            event_bus.send(AppEvent::Llm(LlmEvent::ChatCompletion(
                ChatEvt::PromptConstructed {
                    parent_id: user_msg_id,
                    formatted_prompt: vec![RequestMessage::new_user(prompt)],
                    context_plan: ContextPlan {
                        plan_id: Uuid::new_v4(),
                        parent_id: user_msg_id,
                        estimated_total_tokens: 64,
                        included_messages: vec![ContextPlanMessage {
                            message_id: Some(user_msg_id),
                            kind: MessageKind::User,
                            estimated_tokens: 64,
                        }],
                        excluded_messages: Vec::new(),
                        included_rag_parts: Vec::new(),
                        rag_stats: None,
                    },
                },
            )));
            event_bus.send(AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Request {
                parent_id: user_msg_id,
                request_msg_id,
            })));

            let mut requested_tool_ids = None;
            let mut completed_payload = None;
            let mut saw_finished = false;
            timeout(Duration::from_secs(120), async {
                while !(requested_tool_ids.is_some() && completed_payload.is_some() && saw_finished)
                {
                    match event_rx.recv().await {
                        Ok(AppEvent::System(SystemEvent::ToolCallRequested {
                            request_id,
                            tool_call,
                            ..
                        })) if tool_call.function.name == ToolName::RequestCodeContext => {
                            if requested_tool_ids.is_none() {
                                requested_tool_ids = Some((request_id, tool_call.call_id.clone()));
                            }
                        }
                        Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                            request_id,
                            call_id,
                            content,
                            ..
                        })) if requested_tool_ids.as_ref().is_some_and(
                            |(expected_request_id, expected_call_id)| {
                                request_id == *expected_request_id
                                    && call_id.as_ref() == expected_call_id.as_ref()
                            },
                        ) =>
                        {
                            if serde_json::from_str::<RequestCodeContextResult>(&content).is_ok() {
                                completed_payload = Some(content);
                            }
                        }
                        Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                            request_id,
                            call_id,
                            error,
                            ..
                        })) if requested_tool_ids.as_ref().is_some_and(
                            |(expected_request_id, expected_call_id)| {
                                request_id == *expected_request_id
                                    && call_id.as_ref() == expected_call_id.as_ref()
                            },
                        ) =>
                        {
                            panic!(
                                "request_code_context failed in live matrix test for {}: {error}",
                                case.name
                            );
                        }
                        Ok(AppEvent::System(SystemEvent::ChatTurnFinished {
                            request_id, ..
                        })) if requested_tool_ids.as_ref().is_some_and(
                            |(expected_request_id, _)| request_id == *expected_request_id,
                        ) =>
                        {
                            saw_finished = true;
                        }
                        Ok(_) => {}
                        Err(err) => panic!(
                            "event channel closed before live matrix completion for {}: {err}",
                            case.name
                        ),
                    }
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for live request_code_context events for {}",
                    case.name
                )
            });

            let result: RequestCodeContextResult =
                serde_json::from_str(&completed_payload.expect("tool payload"))?;
            assert_matrix_payload_has_type_context(
                case,
                expected_target_id,
                &expected_seed_ids,
                &result,
            );
        }
        Ok(())
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn assert_matrix_payload_has_type_context(
        case: &ploke_test_utils::TypeShapeCase,
        expected_target_id: uuid::Uuid,
        expected_seed_ids: &[uuid::Uuid],
        result: &ploke_core::rag_types::RequestCodeContextResult,
    ) {
        let expected_relation = type_context_kind(case.type_context_relation);
        assert!(
            result.ok,
            "{} returned error payload: {result:#?}",
            case.name
        );
        assert!(
            result.context.iter().any(|context| {
                context.id == expected_target_id
                    && context.type_context.is_some_and(|info| {
                        info.relation == expected_relation
                            && expected_seed_ids.contains(&info.seed_id)
                            && info.distance == case.depth
                    })
            }),
            "{} should expose traversal-derived type_context for target {expected_target_id} with distance {} in request_code_context payload; result: {result:#?}",
            case.name,
            case.depth
        );
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn covers(
        case: &ploke_test_utils::TypeShapeCase,
        coverage: ploke_test_utils::ShapePipelineCoverage,
    ) -> bool {
        case.coverage.iter().any(|candidate| *candidate == coverage)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn resolve_matrix_target(
        db: &ploke_db::Database,
        selector: ploke_test_utils::TargetSelector,
    ) -> color_eyre::Result<uuid::Uuid> {
        match selector {
            ploke_test_utils::TargetSelector::StructByName { name } => one_uuid(
                db,
                &format!(r#"?[id] := *struct {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            ploke_test_utils::TargetSelector::StructInModule { module_path, name } => {
                one_uuid(db, &struct_in_module_query(module_path, name))
            }
            ploke_test_utils::TargetSelector::EnumByName { name } => one_uuid(
                db,
                &format!(r#"?[id] := *enum {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            ploke_test_utils::TargetSelector::TraitInModule { module_path, name } => {
                one_uuid(db, &trait_in_module_query(module_path, name))
            }
            ploke_test_utils::TargetSelector::TraitInFile { file_suffix, name } => {
                one_uuid_by_file_suffix(db, &trait_in_file_query(name), file_suffix)
            }
            ploke_test_utils::TargetSelector::TypeAlias { name } => one_uuid(
                db,
                &format!(r#"?[id] := *type_alias {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            ploke_test_utils::TargetSelector::Union { name } => one_uuid(
                db,
                &format!(r#"?[id] := *union {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            other => Err(color_eyre::eyre::eyre!(
                "TUI matrix resolver does not materialize target selector {other:?}"
            )),
        }
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn type_context_kind(
        relation: ploke_db::TypeContextRelation,
    ) -> ploke_core::rag_types::TypeContextKind {
        match relation {
            ploke_db::TypeContextRelation::SameResolvedType => {
                ploke_core::rag_types::TypeContextKind::SameResolvedType
            }
            ploke_db::TypeContextRelation::UsesTypeNested => {
                ploke_core::rag_types::TypeContextKind::UsesTypeNested
            }
            ploke_db::TypeContextRelation::TypeDefinitionImpact => {
                ploke_core::rag_types::TypeContextKind::TypeDefinitionImpact
            }
            ploke_db::TypeContextRelation::ImplOfTrait => {
                ploke_core::rag_types::TypeContextKind::ImplOfTrait
            }
            ploke_db::TypeContextRelation::ImplSelfType => {
                ploke_core::rag_types::TypeContextKind::ImplSelfType
            }
            ploke_db::TypeContextRelation::AliasExpansion => {
                ploke_core::rag_types::TypeContextKind::AliasExpansion
            }
            ploke_db::TypeContextRelation::TraitBound => {
                ploke_core::rag_types::TypeContextKind::TraitBound
            }
            ploke_db::TypeContextRelation::IteratorSurface => {
                ploke_core::rag_types::TypeContextKind::IteratorSurface
            }
            ploke_db::TypeContextRelation::ConstGenericAlias => {
                ploke_core::rag_types::TypeContextKind::ConstGenericAlias
            }
        }
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn resolve_matrix_expected_type_context_seed_ids(
        db: &ploke_db::Database,
        case: &ploke_test_utils::TypeShapeCase,
        owner_id: uuid::Uuid,
    ) -> color_eyre::Result<Vec<uuid::Uuid>> {
        let mut seeds = vec![owner_id];
        match case.owner {
            ploke_test_utils::OwnerSelector::FieldByStructInModule {
                module_path,
                struct_name,
                ..
            } => {
                seeds.push(one_uuid(
                    db,
                    &struct_in_module_query(module_path, struct_name),
                )?);
            }
            ploke_test_utils::OwnerSelector::FieldByStructInFile {
                file_suffix,
                struct_name,
                ..
            } => {
                seeds.push(one_uuid_by_file_suffix(
                    db,
                    &struct_in_file_query(struct_name),
                    file_suffix,
                )?);
            }
            _ => {}
        }
        seeds.sort();
        seeds.dedup();
        Ok(seeds)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn resolve_matrix_owner(
        db: &ploke_db::Database,
        selector: ploke_test_utils::OwnerSelector,
    ) -> color_eyre::Result<uuid::Uuid> {
        match selector {
            ploke_test_utils::OwnerSelector::FunctionInModule { module_path, name } => {
                one_uuid(db, &function_in_module_query(module_path, name))
            }
            ploke_test_utils::OwnerSelector::FunctionInFile { file_suffix, name } => {
                one_uuid_by_file_suffix(db, &function_in_file_query(name), file_suffix)
            }
            ploke_test_utils::OwnerSelector::MethodByImplSelf { self_type, method } => {
                one_uuid(db, &method_by_impl_self_query(self_type, method))
            }
            ploke_test_utils::OwnerSelector::MethodByImplTraitAndSelf {
                trait_name,
                self_type,
                method,
            } => one_uuid(
                db,
                &method_by_impl_trait_self_query(trait_name, self_type, method),
            ),
            ploke_test_utils::OwnerSelector::FieldByStructInModule {
                module_path,
                struct_name,
                field_index,
            } => {
                let struct_id = one_uuid(db, &struct_in_module_query(module_path, struct_name))?;
                one_uuid(
                    db,
                    &format!(
                        r#"?[id] :=
                            *field {{
                                id,
                                owner_id: to_uuid("{struct_id}"),
                                index: {field_index} @ 'NOW'
                            }}"#
                    ),
                )
            }
            ploke_test_utils::OwnerSelector::FieldByStructInFile {
                file_suffix,
                struct_name,
                field_index,
            } => {
                let struct_id =
                    one_uuid_by_file_suffix(db, &struct_in_file_query(struct_name), file_suffix)?;
                one_uuid(
                    db,
                    &format!(
                        r#"?[id] :=
                            *field {{
                                id,
                                owner_id: to_uuid("{struct_id}"),
                                index: {field_index} @ 'NOW'
                            }}"#
                    ),
                )
            }
            other => Err(color_eyre::eyre::eyre!(
                "TUI matrix resolver does not materialize owner selector {other:?}"
            )),
        }
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn one_uuid(db: &ploke_db::Database, script: &str) -> color_eyre::Result<uuid::Uuid> {
        let rows = db.raw_query(script)?;
        assert_eq!(
            rows.rows.len(),
            1,
            "expected exactly one UUID for query:\n{script}\nrows: {:#?}",
            rows.rows
        );
        Ok(ploke_db::to_uuid(&rows.rows[0][0])?)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn one_uuid_by_file_suffix(
        db: &ploke_db::Database,
        script: &str,
        file_suffix: &str,
    ) -> color_eyre::Result<uuid::Uuid> {
        let rows = db.raw_query(script)?;
        let matching = rows
            .rows
            .iter()
            .filter_map(|row| {
                let file_path = match &row[1] {
                    cozo::DataValue::Str(path) => path.as_str(),
                    _ => return None,
                };
                file_path.ends_with(file_suffix).then(|| row[0].clone())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "expected exactly one row in file suffix {file_suffix}; rows: {:#?}",
            rows.rows
        );
        Ok(ploke_db::to_uuid(&matching[0])?)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn module_path(items: &[&str]) -> String {
        format!(
            "[{}]",
            items
                .iter()
                .map(|item| format!("\"{item}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn function_in_module_query(module_path_items: &[&str], name: &str) -> String {
        let module_path = module_path(module_path_items);
        format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        )
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn function_in_file_query(name: &str) -> String {
        item_in_file_query("function", name)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn struct_in_file_query(name: &str) -> String {
        item_in_file_query("struct", name)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn item_in_file_query(relation: &str, name: &str) -> String {
        format!(
            r#"?[id, file_path] :=
                *{relation} {{ id, name: "{name}" @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
        )
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn struct_in_module_query(module_path_items: &[&str], name: &str) -> String {
        let module_path = module_path(module_path_items);
        format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *struct {{ id, name: "{name}" @ 'NOW' }}"#
        )
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn trait_in_file_query(name: &str) -> String {
        item_in_file_query("trait", name)
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn trait_in_module_query(module_path_items: &[&str], name: &str) -> String {
        let module_path = module_path(module_path_items);
        format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *trait {{ id, name: "{name}" @ 'NOW' }}"#
        )
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn method_by_impl_self_query(self_type: &str, method: &str) -> String {
        format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type}" @ 'NOW' }}"#
        )
    }

    #[cfg(all(feature = "test_harness", feature = "typed_type_graph"))]
    fn method_by_impl_trait_self_query(trait_name: &str, self_type: &str, method: &str) -> String {
        format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        )
    }
}
