use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::Database;
use ploke_db::bm25_index::bm25_service::Bm25Status;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_embed::config::{OpenRouterConfig, TruncatePolicy};
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexingStatus};
use ploke_embed::providers::openrouter::OpenRouterBackend;
use ploke_llm::embeddings::{
    EmbClientConfig, EmbeddingInput, EmbeddingRequest, HasDims, HasEmbeddings,
};
use ploke_llm::manager::RequestMessage;
use ploke_llm::request::{endpoint::Endpoint, models::ResponseItem};
use ploke_llm::router_only::{
    HasEndpoint,
    openrouter::{
        EmbeddingProviderPrefs, OpenRouter, OpenRouterModelId, ProviderPreferences,
        embed::OpenRouterEmbeddingFields,
    },
};
use ploke_llm::{LlmRoute, ModelId, ProviderKey, ProviderSlug, SupportsTools};
use ploke_records::agent_turn::{
    AgentTurnArtifactRecord as PersistedAgentTurnArtifactRecord, AgentTurnSummaryRecord,
    AgentTurnTraceRecord, ExpectedFileChangeRecord as PersistedExpectedFileChangeRecord,
    FinishReasonRecord, FunctionCallMarker, LlmMetadataRecord as PersistedLlmMetadataRecord,
    LlmResponseRecord as PersistedLlmResponseRecord,
    MessageSnapshotRecord as PersistedMessageSnapshotRecord, ObservedTurnEventRecord,
    PatchArtifactRecord as PersistedPatchArtifactRecord,
    PerformanceMetricsRecord as PersistedPerformanceMetricsRecord,
    ProposalSnapshotRecord as PersistedProposalSnapshotRecord, ProviderFunctionCallRecord,
    ProviderToolCallRecord, RequestMessageRecord as PersistedRequestMessageRecord,
    RequestRoleRecord, TokenUsageRecord as PersistedTokenUsageRecord,
    ToolCompletedRecord as PersistedToolCompletedRecord, ToolErrorCodeRecord, ToolErrorWireRecord,
    ToolFailedRecord as PersistedToolFailedRecord, ToolLlmErrorPayloadRecord,
    ToolRequestRecord as PersistedToolRequestRecord, ToolRetryContextFieldRecord,
    ToolRetryContextRecord, ToolRetryContextValueRecord, ToolUiFieldRecord, ToolUiPayloadRecord,
    ToolVerbosityRecord, TurnFinishedRecord as PersistedTurnFinishedRecord,
};
use ploke_records::evaluation::{
    BENCHMARK_PATCH_PROJECTION_SCHEMA_V1, BenchmarkCheckoutRef, BenchmarkPatchProjectionRecord,
    MultiSweBenchTarget, PatchProjectionCheck, PatchProjectionCheckState, RunArtifactRef,
    SubmissionPatchRef,
};
use ploke_records::record::ToRecord;
use ploke_records::tool_contracts::{PersistedToolCallArguments, ToolCallArguments};
use ploke_tui::AppEvent;
use ploke_tui::app::App;
use ploke_tui::app::commands::harness::TestAppAccessor;
use ploke_tui::app::commands::harness::{TestRuntime, TestRuntimeActorGuard};
use ploke_tui::app::view::components::model_browser::tool_capable_provider_key;
use ploke_tui::app_state::AppState;
use ploke_tui::app_state::core::ParseFailure;
use ploke_tui::app_state::core::{DiffPreview, EditProposalStatus, RuntimeConfig};
use ploke_tui::app_state::events::SystemEvent;
use ploke_tui::llm::{ChatEvt, LlmEvent};
use ploke_tui::parser::{resolve_index_target, run_parse_resolved};
use ploke_tui::user_config::{
    ChatPolicy, ChatTimeoutStrategy, RetrievalStrategyUser, ToolLoopMode,
};
use ploke_tui::utils::parse_errors::FlattenedParserDiagnostic;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep};
use tracing::{info, warn};
use uuid::Uuid;

use crate::LlmResponseRecord;
use crate::inner::core::{RegisteredRunRole, RunIntent};
use crate::inner::registry::{RunLifecyclePhase, RunPhaseStatus, RunRegistration};
use crate::layout;
use crate::model_registry::resolve_model_for_run;
use crate::provider_prefs::load_provider_for_model;
use crate::record::{
    CrateIndexStatus, IndexedCrateSummary, PackagingPhase, ParseErrorSummary, ParseFailureRecord,
    RunRecord, RunRecordBuilder, RunTimingSummary, SetupPhase, SubmissionArtifactState,
    write_compressed_record,
};
use crate::run_history::record_last_run;
use crate::run_registry::{persist_registration, register_live_run, storage_roots_for_instance};
use crate::spec::{PrepareError, PreparedMsbBatch, PreparedSingleRun, RunSource};
use crate::tracing_setup::current_full_response_log_path;

use super::*;

mod tests {
    use super::*;

    use crate::EvalBudget;
    use ploke_db::multi_embedding::schema::EmbeddingSetExt;
    use ploke_llm::Router as _;
    use ploke_llm::response::FunctionCall;
    use ploke_tui::CancelChatToken;
    use ploke_tui::app_state::commands::StateCommand;
    use ploke_tui::app_state::core::{CreateProposal, EditProposal};
    use ploke_tui::app_state::events::MessageUpdatedEvent;
    use ploke_tui::app_state::state_manager;
    use ploke_tui::event_bus::{EventBus, EventBusCaps, EventPriority};
    use ploke_tui::test_utils::mock::create_mock_app_state;
    use ploke_tui::tools::ToolVerbosity;
    use ploke_tui::tools::{FunctionMarker, ToolCall, ToolName};
    use ploke_tui::user_config::CommandStyle;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::{broadcast, mpsc, watch};
    use tokio::time::Duration;
    use tracing_subscriber::fmt::SubscriberBuilder;
    use uuid::Uuid;

    fn init_tracing() {
        let _ = SubscriberBuilder::default()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_test_writer()
            .try_init();
    }

    fn hold_env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct EvalHomeGuard {
        old: Option<OsString>,
    }

    impl EvalHomeGuard {
        fn set_to(path: &Path) -> Self {
            let old = std::env::var_os("PLOKE_EVAL_HOME");
            unsafe {
                std::env::set_var("PLOKE_EVAL_HOME", path);
            }
            Self { old }
        }
    }

    impl Drop for EvalHomeGuard {
        fn drop(&mut self) {
            if let Some(old) = self.old.take() {
                unsafe {
                    std::env::set_var("PLOKE_EVAL_HOME", old);
                }
            } else {
                unsafe {
                    std::env::remove_var("PLOKE_EVAL_HOME");
                }
            }
        }
    }

    fn run_git_test(repo_root: &Path, args: &[&str], label: &str) {
        let status = Command::new("git")
            .current_dir(repo_root)
            .args(args)
            .status()
            .unwrap_or_else(|err| panic!("{label} failed to start: {err}"));
        assert!(status.success(), "{label} failed with status {status}");
    }

    fn msb_patch_artifact(applied: bool, any_expected_file_changed: bool) -> PatchArtifact {
        PatchArtifact {
            edit_proposals: if applied {
                vec![ProposalSnapshotRecord {
                    request_id: "request-1".to_string(),
                    call_id: "call-1".to_string(),
                    status: "Applied".to_string(),
                    files: vec!["src/lib.rs".to_string()],
                    preview_mode: "diff".to_string(),
                }]
            } else {
                Vec::new()
            },
            create_proposals: Vec::new(),
            applied,
            all_proposals_applied: applied,
            expected_file_changes: vec![ExpectedFileChangeRecord {
                path: "src/lib.rs".to_string(),
                existed_before: true,
                exists_after: true,
                before_sha256: Some("before".to_string()),
                after_sha256: Some(
                    if any_expected_file_changed {
                        "after"
                    } else {
                        "before"
                    }
                    .to_string(),
                ),
                changed: any_expected_file_changed,
            }],
            any_expected_file_changed,
            all_expected_files_changed: any_expected_file_changed,
        }
    }

    fn msb_patch_artifact_without_expected_file_changes(applied: bool) -> PatchArtifact {
        PatchArtifact {
            edit_proposals: if applied {
                vec![ProposalSnapshotRecord {
                    request_id: "request-1".to_string(),
                    call_id: "call-1".to_string(),
                    status: "Applied".to_string(),
                    files: vec!["src/lib.rs".to_string()],
                    preview_mode: "diff".to_string(),
                }]
            } else {
                Vec::new()
            },
            create_proposals: Vec::new(),
            applied,
            all_proposals_applied: applied,
            expected_file_changes: Vec::new(),
            any_expected_file_changed: false,
            all_expected_files_changed: false,
        }
    }

    fn dirty_msb_prepared_for_submission(
        tmp: &tempfile::TempDir,
        task_id: &str,
        number: u64,
    ) -> (PreparedSingleRun, PathBuf) {
        let repo_root = tmp.path().join("repo");
        let src_dir = repo_root.join("src");
        let file = src_dir.join("lib.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(&repo_root, &["init"], "git init");
        run_git_test(
            &repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            &repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&file, "fn main() {}\n").expect("write initial file");
        run_git_test(&repo_root, &["add", "src/lib.rs"], "git add");
        run_git_test(&repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        fs::write(&file, "fn main() {\n    println!(\"hi\");\n}\n").expect("write modified file");

        let prepared = PreparedSingleRun {
            task_id: task_id.to_string(),
            repo_root,
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: format!("acme__repo-{number}"),
                org: "acme".to_string(),
                repo: "repo".to_string(),
                number,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("src/lib.rs")],
            })),
            campaign: None,
        };
        let run_output_dir = tmp
            .path()
            .join("out")
            .join("runs")
            .join(format!("run-{number}"));
        fs::create_dir_all(&run_output_dir).expect("run output dir");
        (prepared, run_output_dir)
    }

    fn clean_msb_prepared_for_submission(
        tmp: &tempfile::TempDir,
        task_id: &str,
        number: u64,
    ) -> (PreparedSingleRun, PathBuf) {
        let repo_root = tmp.path().join("repo");
        let src_dir = repo_root.join("src");
        let file = src_dir.join("lib.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(&repo_root, &["init"], "git init");
        run_git_test(
            &repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            &repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&file, "fn main() {}\n").expect("write initial file");
        run_git_test(&repo_root, &["add", "src/lib.rs"], "git add");
        run_git_test(&repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        let prepared = PreparedSingleRun {
            task_id: task_id.to_string(),
            repo_root,
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: format!("acme__repo-{number}"),
                org: "acme".to_string(),
                repo: "repo".to_string(),
                number,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("src/lib.rs")],
            })),
            campaign: None,
        };
        let run_output_dir = tmp
            .path()
            .join("out")
            .join("runs")
            .join(format!("run-{number}"));
        fs::create_dir_all(&run_output_dir).expect("run output dir");
        (prepared, run_output_dir)
    }

    fn test_eval_embedding_selection() -> EvalEmbeddingSelection {
        let model: ResponseItem = serde_json::from_value(serde_json::json!({
            "id": OPENROUTER_CODESTRAL_MODEL,
            "name": "Codestral Embed",
            "created": 1_i64,
            "description": "test embedding model",
            "architecture": {
                "modality": "text->embeddings",
                "input_modalities": ["text"],
                "output_modalities": ["embeddings"],
                "tokenizer": "Mistral",
                "instruct_type": null
            },
            "pricing": {
                "prompt": "0.00000015",
                "completion": "0"
            },
            "top_provider": {
                "context_length": 32768,
                "max_completion_tokens": null,
                "is_moderated": false
            },
            "context_length": 32768
        }))
        .expect("test embedding model parses");

        EvalEmbeddingSelection {
            model,
            provider: None,
            dimensions: 1536,
        }
    }

    fn test_prepared_run(temp: &tempfile::TempDir) -> PreparedSingleRun {
        PreparedSingleRun {
            task_id: "benchmark-turn-test".to_string(),
            repo_root: temp.path().to_path_buf(),
            output_dir: temp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix benchmark turn capture".to_string()),
                body: Some("repro".to_string()),
                body_path: None,
            },
            base_sha: None,
            head_sha: None,
            budget: crate::spec::EvalBudget {
                max_turns: 4,
                max_tool_calls: 8,
                wall_clock_secs: 2,
            },
            source: None,
            campaign: None,
        }
    }

    fn test_chat_response_event(content: &str) -> AppEvent {
        AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Response {
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            content: content.to_string(),
            model: "test-model".to_string(),
            metadata: ploke_llm::types::meta::LLMMetadata {
                model: "test-model".to_string(),
                usage: ploke_llm::response::TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                },
                finish_reason: ploke_llm::response::FinishReason::Stop,
                processing_time: std::time::Duration::from_millis(10),
                cost: 0.0,
                performance: ploke_llm::types::meta::PerformanceMetrics {
                    tokens_per_second: 100.0,
                    time_to_first_token: std::time::Duration::from_millis(1),
                    queue_time: std::time::Duration::from_millis(0),
                },
            },
            usage: ploke_llm::manager::events::UsageMetrics {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                latency_ms: 10,
            },
        }))
    }

    fn test_turn_finished_event() -> AppEvent {
        AppEvent::System(SystemEvent::ChatTurnFinished {
            session_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            assistant_message_id: Uuid::new_v4(),
            outcome: "success".to_string(),
            error_id: None,
            summary: "done".to_string(),
            attempts: 1,
        })
    }

    async fn build_benchmark_turn_test_app(
        state: Arc<AppState>,
    ) -> (
        App,
        mpsc::Sender<ploke_tui::app::commands::harness::DebugStateCommand>,
        mpsc::Receiver<ploke_tui::app::commands::harness::DebugStateCommand>,
        broadcast::Receiver<AppEvent>,
        broadcast::Receiver<AppEvent>,
        Arc<EventBus>,
    ) {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (debug_tx, debug_rx) =
            mpsc::channel::<ploke_tui::app::commands::harness::DebugStateCommand>(8);
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(128);
        let (rag_tx, _rag_rx) = mpsc::channel(8);
        let (cancel_tx, _cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        tokio::spawn(state_manager(
            Arc::clone(&state),
            cmd_rx,
            Arc::clone(&event_bus),
            rag_tx,
        ));
        let app = App::new(
            CommandStyle::Slash,
            state,
            cmd_tx,
            &event_bus,
            "mock-model".to_string(),
            ToolVerbosity::Normal,
            cancel_tx,
            std::env::current_dir().expect("current dir"),
        );
        let realtime_rx = event_bus.subscribe(EventPriority::Realtime);
        let background_rx = event_bus.subscribe(EventPriority::Background);
        (
            app,
            debug_tx,
            debug_rx,
            realtime_rx,
            background_rx,
            event_bus,
        )
    }

    fn test_provider_key() -> ProviderKey {
        ProviderKey::new("deepinfra").expect("provider key")
    }

    fn test_model_response_item(
        id: &str,
        route_source: ploke_llm::request::models::ModelRouteSource,
    ) -> ResponseItem {
        let mut item: ResponseItem = serde_json::from_value(serde_json::json!({
            "id": id,
            "name": id,
            "created": 0,
            "description": "test model",
            "architecture": {
                "input_modalities": ["text"],
                "modality": "text->text",
                "output_modalities": ["text"],
                "tokenizer": "Gemini"
            },
            "top_provider": {
                "is_moderated": false
            },
            "pricing": {
                "prompt": 0.0,
                "completion": 0.0
            },
            "supported_parameters": ["tools"]
        }))
        .expect("test response item");
        item.route_source = route_source;
        item
    }

    #[test]
    fn direct_google_ignores_openrouter_provider_preference() {
        let selected_model = test_model_response_item(
            "google/gemini-3.5-flash",
            ploke_llm::request::models::ModelRouteSource::DirectGoogle,
        );
        let preferred_provider = ProviderKey::new("google-ai-studio").expect("provider key");

        let requested_provider =
            provider_request_for_selected_model(&selected_model, None, Some(&preferred_provider));

        assert!(requested_provider.is_none());
    }

    #[test]
    fn direct_nebius_ignores_openrouter_provider_preference() {
        let selected_model = test_model_response_item(
            "meta-llama/Meta-Llama-3.1-70B-Instruct",
            ploke_llm::request::models::ModelRouteSource::DirectNebius,
        );
        let preferred_provider = ProviderKey::new("deepinfra").expect("provider key");

        let requested_provider =
            provider_request_for_selected_model(&selected_model, None, Some(&preferred_provider));

        assert!(requested_provider.is_none());
    }

    #[test]
    fn direct_nebius_load_preference_suppresses_persisted_openrouter_provider() {
        let _lock = hold_env_lock();
        let tmp = tempdir().expect("tempdir");
        let _guard = EvalHomeGuard::set_to(tmp.path());
        let models_dir = tmp.path().join("models");
        fs::create_dir_all(&models_dir).expect("models dir");
        fs::write(
            models_dir.join("provider-preferences.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_providers": {
                    "meta-llama/Meta-Llama-3.1-70B-Instruct": {
                        "slug": "deepinfra"
                    }
                }
            }))
            .expect("provider prefs json"),
        )
        .expect("write provider prefs");
        let selected_model = test_model_response_item(
            "meta-llama/Meta-Llama-3.1-70B-Instruct",
            ploke_llm::request::models::ModelRouteSource::DirectNebius,
        );

        let provider =
            load_provider_preference_for_selected_model(&selected_model, None).expect("preference");

        assert!(provider.is_none());
    }

    #[test]
    fn explicit_provider_is_still_validated_for_direct_google() {
        let selected_model = test_model_response_item(
            "google/gemini-3.5-flash",
            ploke_llm::request::models::ModelRouteSource::DirectGoogle,
        );
        let explicit_provider = ProviderKey::new("google-ai-studio").expect("provider key");

        let requested_provider =
            provider_request_for_selected_model(&selected_model, Some(&explicit_provider), None);

        assert_eq!(requested_provider, Some(&explicit_provider));
    }

    #[test]
    fn openrouter_uses_preferred_provider_when_explicit_missing() {
        let selected_model = test_model_response_item(
            "google/gemini-3.5-flash",
            ploke_llm::request::models::ModelRouteSource::OpenRouter,
        );
        let preferred_provider = ProviderKey::new("google-ai-studio").expect("provider key");

        let requested_provider =
            provider_request_for_selected_model(&selected_model, None, Some(&preferred_provider));

        assert_eq!(requested_provider, Some(&preferred_provider));
    }

    #[tokio::test]
    async fn resolve_route_for_direct_nebius_returns_direct_route_without_openrouter_endpoint() {
        let selected_model = test_model_response_item(
            "meta-llama/Meta-Llama-3.1-70B-Instruct",
            ploke_llm::request::models::ModelRouteSource::DirectNebius,
        );
        let nebius = ProviderKey::new("nebius").expect("nebius provider key");

        let route = tokio::time::timeout(
            Duration::from_millis(500),
            resolve_route_for_model(&selected_model, Some(&nebius)),
        )
        .await
        .expect("direct Nebius route resolution should not wait on OpenRouter endpoint fetch")
        .expect("resolve direct Nebius route");

        assert!(route.is_direct_nebius());
        assert!(matches!(
            route.router(),
            ploke_llm::router_only::RouterVariants::Nebius(_)
        ));
        assert!(route.provider_key().is_none());
        assert_eq!(route.selected_provider_slug(), "nebius");
        assert!(selected_endpoint_provenance(&route).is_none());
    }

    #[cfg(feature = "live_api_tests")]
    fn live_google_model_id() -> ModelId {
        let raw = std::env::var("PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID")
            .or_else(|_| std::env::var("PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID"))
            .or_else(|_| std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL"))
            .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string());
        let model = if raw.contains('/') {
            raw
        } else {
            format!("google/{raw}")
        };
        model.parse().expect("live Google model id")
    }

    #[cfg(feature = "live_api_tests")]
    async fn live_google_route() -> Option<LlmRoute> {
        use ploke_llm::Router;
        use ploke_llm::router_only::google::Google;

        crate::test_support::install_default_google_route_env();
        if let Err(error) = Google::route_config_available() {
            eprintln!(
                "{}",
                Google::with_local_auth_preflight_hint(format!(
                    "skipping live Google route test: route config unavailable: {error}"
                ))
            );
            return None;
        }
        if let Err(error) = Google::auth_config_available() {
            eprintln!(
                "{}",
                Google::with_local_auth_preflight_hint(format!(
                    "skipping live Google route test: ADC config unavailable: {error}"
                ))
            );
            return None;
        }
        if let Err(error) = Google::resolve_bearer_token().await {
            eprintln!(
                "{}",
                Google::with_local_auth_preflight_hint(format!(
                    "skipping live Google route test: ADC token unavailable: {error}"
                ))
            );
            return None;
        }

        let model_id = live_google_model_id();
        let registry = match crate::model_registry::fetch_google_model_registry().await {
            Ok(registry) => registry,
            Err(error) => {
                eprintln!(
                    "{}",
                    Google::with_local_auth_preflight_hint(format!(
                        "skipping live Google route test: catalog unavailable: {error}"
                    ))
                );
                return None;
            }
        };
        let selected_model = registry
            .data
            .into_iter()
            .find(|item| item.id == model_id)
            .unwrap_or_else(|| panic!("Google model registry catalog missing '{model_id}'"));
        assert!(
            selected_model.route_source.is_direct_google(),
            "expected direct Google registry row for '{}'",
            selected_model.id
        );
        assert!(
            selected_model.supports_tools(),
            "live Google route test requires a tool-capable model"
        );

        let google = ProviderKey::new("google").expect("google provider key");
        let route = resolve_route_for_model(&selected_model, Some(&google))
            .await
            .expect("resolve direct Google route");
        assert!(route.is_direct_google());
        assert!(matches!(
            route.router(),
            ploke_llm::router_only::RouterVariants::Google(_)
        ));
        assert!(route.provider_key().is_none());
        assert_eq!(route.selected_provider_slug(), "google");
        assert!(selected_endpoint_provenance(&route).is_none());
        Some(route)
    }

    #[cfg(feature = "live_api_tests")]
    fn is_google_quota_error(error: &ploke_llm::LlmError) -> bool {
        let text = format!("{error:?}");
        text.contains("RESOURCE_EXHAUSTED") || text.contains("429")
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    #[ignore = "live Google API test for ploke-eval route resolution and direct content response"]
    async fn live_google_resolved_route_returns_content_success_or_quota() {
        let Some(route) = live_google_route().await else {
            return;
        };
        let request = ploke_llm::router_only::google::Google::default_chat_completion()
            .with_model(route.model().clone())
            .with_message(RequestMessage::new_user(
                "Reply with the exact text: ploke-google-router-ok".to_string(),
            ))
            .with_max_tokens(64)
            .with_temperature(0.0);
        let client = reqwest::Client::new();
        let cfg = ploke_llm::ChatHttpConfig::default();
        let step = match ploke_llm::chat_step(&client, &request, &cfg).await {
            Ok(step) => step,
            Err(error) if is_google_quota_error(&error) => {
                println!("live Google direct route reached Google quota response");
                return;
            }
            Err(error) => panic!("live Google chat_step failed: {error:?}"),
        };

        match step.outcome {
            ploke_llm::ChatStepOutcome::Content { content, .. } => {
                let content = content.expect("Google content response");
                assert!(
                    content.contains("ploke-google-router-ok"),
                    "expected sentinel content in Google response, got: {content}"
                );
                println!("live Google direct route returned sentinel content");
            }
            other => {
                panic!("expected Google content response through ploke-eval route, got {other:?}")
            }
        }
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    #[ignore = "live Google API test for ploke-eval route resolution and forced tool-call response"]
    async fn live_google_resolved_route_forces_list_dir_tool_call_success_or_quota() {
        let Some(route) = live_google_route().await else {
            return;
        };
        let request = ploke_llm::router_only::google::Google::default_chat_completion()
            .with_model(route.model().clone())
            .with_message(RequestMessage::new_user(
                "Call the list_dir tool exactly once for dir \".\". Do not answer in prose."
                    .to_string(),
            ))
            .with_max_tokens(128)
            .with_temperature(0.0)
            .with_tools(Some(vec![
                <ploke_tui::tools::list_dir::ListDir as ploke_tui::tools::Tool>::tool_def(),
            ]))
            .with_tool_choice(Some(ploke_llm::request::endpoint::ToolChoice::Function {
                r#type: FunctionMarker,
                function: ploke_llm::request::endpoint::ToolChoiceFunction {
                    name: ToolName::ListDir.as_str().to_string(),
                },
            }));
        let client = reqwest::Client::new();
        let cfg = ploke_llm::ChatHttpConfig::default();
        let step = match ploke_llm::chat_step(&client, &request, &cfg).await {
            Ok(step) => step,
            Err(error) if is_google_quota_error(&error) => {
                println!("live Google forced tool route reached Google quota response");
                return;
            }
            Err(error) => panic!("live Google forced tool-call chat_step failed: {error:?}"),
        };

        match step.outcome {
            ploke_llm::ChatStepOutcome::ToolCalls { calls, .. } => {
                assert_eq!(calls.len(), 1, "expected exactly one Google tool call");
                assert_eq!(calls[0].function.name, ToolName::ListDir);
                assert!(
                    calls[0].function.arguments.contains("\"dir\""),
                    "expected list_dir arguments to include dir: {}",
                    calls[0].function.arguments
                );
                println!("live Google direct route returned forced list_dir tool call");
            }
            other => {
                panic!("expected Google tool call through ploke-eval route, got {other:?}");
            }
        }
    }

    #[test]
    fn benchmark_runtime_config_overrides_llm_timeout() {
        let model = "google/gemini-2.5-flash"
            .parse::<ploke_llm::ModelId>()
            .expect("model id");
        let route = LlmRoute::google(model, true);
        let mut cfg = RuntimeConfig {
            llm_timeout_secs: 30,
            ..RuntimeConfig::default()
        };

        configure_headless_benchmark_chat(&mut cfg, &route);

        assert_eq!(cfg.llm_timeout_secs, ploke_llm::LLM_TIMEOUT_SECS);
        assert_eq!(cfg.chat_policy.tool_call_timeout_secs, 60);
        assert_eq!(cfg.chat_policy.timeout_base_secs, 5);
        assert_eq!(cfg.chat_policy.error_retry_limit, 3);
        assert!(matches!(
            cfg.chat_policy.timeout_strategy,
            ChatTimeoutStrategy::Backoff { attempts: Some(3) }
        ));
        assert!(cfg.editing.auto_confirm_edits);
    }

    #[test]
    fn direct_google_runtime_config_sets_router_without_provider_pin() {
        let model = "google/gemini-2.5-flash"
            .parse::<ploke_llm::ModelId>()
            .expect("model id");
        let route = LlmRoute::google(model.clone(), true);
        let mut cfg = RuntimeConfig::default();

        configure_eval_model_runtime(&mut cfg, &route);

        assert_eq!(cfg.active_model, model);
        assert!(matches!(
            cfg.active_router,
            ploke_llm::router_only::RouterVariants::Google(_)
        ));
        assert!(
            cfg.model_registry.models.is_empty(),
            "direct Google route must not create OpenRouter provider prefs"
        );
    }

    #[test]
    fn direct_google_execution_log_serializes_without_selected_endpoint() {
        let model = "google/gemini-2.5-flash"
            .parse::<ploke_llm::ModelId>()
            .expect("model id");
        let route = LlmRoute::google(model.clone(), true);
        assert!(selected_endpoint_provenance(&route).is_none());

        let log = ExecutionLog {
            task_id: "case-123".to_string(),
            run_arm: RunArm::structured_current_policy_treatment(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/out"),
            selected_model: model,
            selected_provider: Some(route.selected_provider_slug()),
            selected_endpoint: selected_endpoint_provenance(&route),
            full_response_trace: None,
            steps: vec!["load_manifest".to_string()],
        };

        let value = serde_json::to_value(&log).expect("serialize execution log");
        assert_eq!(value["selected_provider"], "google");
        assert!(
            value.get("selected_endpoint").is_none(),
            "direct Google execution logs must not serialize OpenRouter endpoint provenance"
        );
    }

    #[test]
    fn eval_embedding_preflight_request_prefers_provider_but_allows_fallbacks() {
        let selection = test_eval_embedding_selection();
        let provider = test_provider_key();

        let request = eval_embedding_preflight_request(&selection.model, Some(&provider));
        let value = serde_json::to_value(&request).expect("serialize preflight request");

        assert_eq!(
            value["model"],
            serde_json::json!(OPENROUTER_CODESTRAL_MODEL)
        );
        assert_eq!(value["input_type"], serde_json::json!("code-snippet"));
        assert_eq!(value["provider"]["order"], serde_json::json!(["deepinfra"]));
        assert_eq!(
            value["provider"]["allow_fallbacks"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn eval_embedding_config_prefers_provider_but_allows_fallbacks() {
        let mut selection = test_eval_embedding_selection();
        selection.provider = Some(test_provider_key());

        let cfg = eval_embedding_config(&selection);

        assert_eq!(cfg.provider_order, Some(vec!["deepinfra".to_string()]));
        assert_eq!(cfg.allow_fallbacks, Some(true));
        assert_eq!(cfg.dimensions, Some(1536));
        assert_eq!(cfg.request_dimensions, None);
    }

    #[tokio::test]
    async fn runner_component_setup_emits_tracing() {
        init_tracing();
        info!("starting eval runner component smoke test");

        let db = init_runtime_db().expect("init runtime db");
        info!("runtime database setup completed");

        // -- checking database's active embedding set --
        let currently_active_set: EmbeddingSet = db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(?currently_active_set);

        let selection = test_eval_embedding_selection();
        let _processor = eval_embedding_processor(&selection).expect("init embedding processor");
        info!("embedding processor setup completed");

        // -- checking database's active embedding set --
        let currently_active_set: EmbeddingSet = db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(?currently_active_set);
    }

    #[tokio::test]
    async fn runner_activation_switches_active_set() {
        init_tracing();

        let runtime_db = init_runtime_db().expect("init runtime db");
        let selection = test_eval_embedding_selection();
        let processor = eval_embedding_processor(&selection).expect("init embedding processor");
        let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, processor);
        let state = runtime.state_arc();

        let before: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set before activation");
        info!(?before, "active embedding set before activation");

        activate_eval_embedding_runtime(&state, &selection)
            .expect("activate eval embedding runtime");

        let after: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set after activation");
        info!(?after, "active embedding set after activation");

        assert_ne!(before.hash_id(), after.hash_id());
        assert_eq!(after.hash_id(), eval_embedding_set(&selection).hash_id());
    }

    #[tokio::test]
    async fn starting_db_cache_round_trip_restores_snapshot() {
        init_tracing();

        let cache_root = tempdir().expect("cache root");
        let prepared = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: cache_root.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: Some("def456".to_string()),
            budget: EvalBudget::default(),
            source: None,
            campaign: None,
        };

        let db = init_runtime_db().expect("init runtime db");
        let snapshot_path = cache_root.path().join("starting.sqlite");
        persist_db_snapshot(
            Arc::clone(&db),
            snapshot_path.clone(),
            "test starting snapshot",
        )
        .await
        .expect("persist snapshot");

        let selection = test_eval_embedding_selection();

        let paths =
            persist_starting_db_cache_at(cache_root.path(), &prepared, &selection, &snapshot_path)
                .await
                .expect("persist cache");
        assert!(paths.snapshot.exists());
        assert!(paths.metadata.exists());

        let loaded = load_cached_starting_db_at(cache_root.path(), &prepared, &selection)
            .expect("load cache hit");
        let loaded = loaded.expect("cache should be reusable");
        assert_eq!(loaded.snapshot, paths.snapshot);

        let restored = Database::create_new_backup_default(&loaded.snapshot)
            .await
            .expect("restore cached snapshot");
        assert!(
            restored
                .is_embedding_set_registered()
                .expect("embedding set relation"),
            "restored snapshot should contain embedding set relation"
        );
    }

    #[test]
    fn starting_db_cache_miss_when_metadata_changes() {
        let cache_root = tempdir().expect("cache root");
        let prepared_a = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: cache_root.path().join("out-a"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: Some("def456".to_string()),
            budget: EvalBudget::default(),
            source: None,
            campaign: None,
        };
        let prepared_b = PreparedSingleRun {
            base_sha: Some("different".to_string()),
            ..prepared_a.clone()
        };

        let selection = test_eval_embedding_selection();
        let paths = starting_db_cache_paths_at(cache_root.path(), &prepared_a, &selection);
        assert_ne!(
            paths.snapshot,
            starting_db_cache_paths_at(cache_root.path(), &prepared_b, &selection).snapshot
        );
        assert!(
            load_cached_starting_db_at(cache_root.path(), &prepared_a, &selection)
                .expect("empty cache should not error")
                .is_none()
        );
    }

    #[test]
    fn starting_db_cache_key_differs_by_typed_graph_surface() {
        let prepared = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: Some("def456".to_string()),
            budget: EvalBudget::default(),
            source: None,
            campaign: None,
        };
        let selection = test_eval_embedding_selection();
        let baseline = starting_db_cache_metadata(&prepared, &selection);

        assert_eq!(
            baseline.typed_type_graph,
            cfg!(feature = "typed_type_graph")
        );

        let mut with_typed = baseline.clone();
        with_typed.typed_type_graph = true;
        let mut without_typed = baseline.clone();
        without_typed.typed_type_graph = false;

        assert_ne!(with_typed, without_typed);
        assert_ne!(
            starting_db_cache_key_for_metadata(&with_typed),
            starting_db_cache_key_for_metadata(&without_typed),
            "typed-graph surface must change cache key and metadata"
        );

        let legacy_json = serde_json::json!({
            "version": STARTING_DB_CACHE_VERSION,
            "task_id": prepared.task_id,
            "repo_root": prepared.repo_root,
            "checkout_sha": prepared.base_sha,
            "embedding_provider": baseline.embedding_provider,
            "embedding_model": baseline.embedding_model,
            "embedding_dimensions": baseline.embedding_dimensions,
            "embedding_dtype": baseline.embedding_dtype,
        });
        let legacy: StartingDbCacheMetadata =
            serde_json::from_value(legacy_json).expect("legacy metadata should parse");
        assert!(!legacy.typed_type_graph);
    }

    #[test]
    fn starting_db_cache_miss_when_repo_root_changes() {
        let cache_root = tempdir().expect("cache root");
        let prepared_a = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: PathBuf::from("/tmp/shared/BurntSushi/ripgrep"),
            output_dir: cache_root.path().join("out-a"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: Some("def456".to_string()),
            budget: EvalBudget::default(),
            source: None,
            campaign: None,
        };
        let prepared_b = PreparedSingleRun {
            repo_root: PathBuf::from("/tmp/node/instance-targets/campaign/BurntSushi/ripgrep"),
            ..prepared_a.clone()
        };

        let selection = test_eval_embedding_selection();
        assert_ne!(
            starting_db_cache_paths_at(cache_root.path(), &prepared_a, &selection).snapshot,
            starting_db_cache_paths_at(cache_root.path(), &prepared_b, &selection).snapshot,
            "path-sensitive indexed DBs must not be reused across checkout roots"
        );
        assert_ne!(
            starting_db_cache_metadata(&prepared_a, &selection),
            starting_db_cache_metadata(&prepared_b, &selection)
        );
    }

    #[test]
    fn truncate_preview_limits_length() {
        let preview = truncate_preview("abcdef", 4);
        assert_eq!(preview, "abcd...<truncated 2 chars>");
    }

    #[test]
    fn indexing_status_artifact_for_parse_failure_uses_failed_status() {
        let err = PrepareError::IndexingFailed {
            detail: "Parse failed for crate: /tmp/ripgrep/crates/cli".to_string(),
        };

        let artifact = indexing_status_artifact_for_error(&err, None).expect("indexing artifact");
        assert_eq!(artifact.status, "failed");
        assert!(artifact.detail.contains("Parse failed for crate"));
    }

    #[test]
    fn parse_failure_artifact_preserves_nested_diagnostics() {
        let artifact = parse_failure_artifact_for_state(ParseFailure {
            target_dir: PathBuf::from("/tmp/repo"),
            message: "Parse failed for crate: /tmp/repo".to_string(),
            occurred_at_ms: 123,
            diagnostics: vec![FlattenedParserDiagnostic {
                diagnostic_path: "root.errors[0]".to_string(),
                depth: 1,
                kind: "syn_parse".to_string(),
                summary: "Syn parsing error: bad token".to_string(),
                detail: Some("bad token".to_string()),
                source_path: Some(PathBuf::from("/tmp/repo/src/lib.rs")),
                line: Some(7),
                column: Some(3),
                end_line: None,
                end_column: None,
                start: None,
                end: None,
                context: Vec::new(),
                emission_site_file: Some("src/error.rs".to_string()),
                emission_site_line: Some(10),
                emission_site_column: Some(20),
                backtrace: Some("stack".to_string()),
            }],
        });

        assert_eq!(artifact.target_dir, PathBuf::from("/tmp/repo"));
        assert_eq!(artifact.diagnostics.len(), 1);
        assert_eq!(
            artifact.diagnostics[0].source_path,
            Some(PathBuf::from("/tmp/repo/src/lib.rs"))
        );
    }

    #[test]
    fn indexing_status_artifact_for_timeout_uses_timed_out_status() {
        let err = PrepareError::Timeout {
            phase: "indexing_completed",
            secs: 300,
        };

        let progress = IndexingProgressArtifact {
            raw_status: "running".to_string(),
            recent_processed: 42,
            num_not_proc: 100,
            current_file: Some(PathBuf::from("/tmp/repo/src/lib.rs")),
            errors: vec![],
            observed_at_ms: 1234,
        };
        let artifact = indexing_status_artifact_for_error(&err, Some(progress.clone()))
            .expect("timeout artifact");
        assert_eq!(artifact.status, "timed_out");
        assert!(artifact.detail.contains("300 seconds"));
        let last_progress = artifact.last_progress.expect("last progress");
        assert_eq!(last_progress.raw_status, progress.raw_status);
        assert_eq!(last_progress.recent_processed, 42);
        assert_eq!(
            last_progress.current_file,
            Some(PathBuf::from("/tmp/repo/src/lib.rs"))
        );
    }

    #[test]
    fn indexing_status_artifact_ignores_non_indexing_errors() {
        let err = PrepareError::Timeout {
            phase: "benchmark_turn",
            secs: 10,
        };

        assert!(indexing_status_artifact_for_error(&err, None).is_none());
    }

    #[test]
    fn run_arm_constants_match_phase_two_surface() {
        let control = RunArm::shell_only_control();
        let treatment = RunArm::structured_current_policy_treatment();

        assert_eq!(control.role, RunArmRole::Control);
        assert_eq!(control.command, "run single setup");
        assert_eq!(treatment.role, RunArmRole::Treatment);
        assert_eq!(treatment.command, "run single agent");
        assert_eq!(RunArm::for_agent_mode(false), control);
        assert_eq!(RunArm::for_agent_mode(true), treatment);
    }

    #[test]
    fn execution_log_serializes_explicit_run_arm() {
        let log = ExecutionLog {
            task_id: "case-123".to_string(),
            run_arm: RunArm::shell_only_control(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/out"),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            selected_provider: Some("friendli".to_string()),
            selected_endpoint: Some(SelectedEndpointProvenance {
                provider_name: "Friendli".to_string(),
                provider_slug: "friendli".to_string(),
                endpoint_name: "Friendli | moonshotai/kimi-k2.5".to_string(),
                endpoint_model_name: "Kimi K2.5".to_string(),
                quantization: Some("fp4".to_string()),
            }),
            full_response_trace: Some(PathBuf::from("/tmp/out/llm-full-responses.jsonl")),
            steps: vec!["load_manifest".to_string()],
        };

        let value = serde_json::to_value(&log).expect("serialize execution log");
        assert_eq!(value["run_arm"]["id"], "shell-only");
        assert_eq!(value["run_arm"]["role"], "control");
        assert_eq!(value["run_arm"]["command"], "run single setup");
        assert_eq!(value["selected_endpoint"]["provider_slug"], "friendli");
        assert_eq!(value["selected_endpoint"]["quantization"], "fp4");
        assert_eq!(
            value["full_response_trace"],
            serde_json::Value::String("/tmp/out/llm-full-responses.jsonl".to_string())
        );
    }

    #[test]
    fn build_agent_issue_prompt_includes_core_fields() {
        let prepared = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: None,
            budget: EvalBudget::default(),
            source: None,
            campaign: None,
        };

        let prompt = build_agent_issue_prompt(&prepared);
        assert!(prompt.contains("case-123"));
        assert!(prompt.contains("/tmp/repo"));
        assert!(prompt.contains("abc123"));
        assert!(prompt.contains("Fix the thing"));
        assert!(prompt.contains("The body text."));
        assert!(prompt.contains("A bare cargo command may run against the focused crate"));
        assert!(prompt.contains("Do not claim formatting or cargo fmt was checked"));
    }

    fn validation_prepared_run(repo_root: PathBuf) -> PreparedSingleRun {
        PreparedSingleRun {
            task_id: "case-validation".to_string(),
            repo_root,
            output_dir: PathBuf::from("/tmp/out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix validation".to_string()),
                body: Some("Body".to_string()),
                body_path: None,
            },
            base_sha: Some("base".to_string()),
            head_sha: None,
            budget: EvalBudget::default(),
            source: None,
            campaign: None,
        }
    }

    fn validation_patch_artifact(changed_path: &str) -> PatchArtifact {
        PatchArtifact {
            edit_proposals: Vec::new(),
            create_proposals: Vec::new(),
            applied: true,
            all_proposals_applied: true,
            expected_file_changes: vec![ExpectedFileChangeRecord {
                path: changed_path.to_string(),
                existed_before: true,
                exists_after: true,
                before_sha256: Some("before".to_string()),
                after_sha256: Some("after".to_string()),
                changed: true,
            }],
            any_expected_file_changed: true,
            all_expected_files_changed: true,
        }
    }

    fn validation_patch_artifact_with_applied_files(
        changed_path: &str,
        applied_files: Vec<String>,
    ) -> PatchArtifact {
        let mut artifact = validation_patch_artifact(changed_path);
        artifact.edit_proposals = vec![ProposalSnapshotRecord {
            request_id: "req-applied".to_string(),
            call_id: "call-applied".to_string(),
            status: "Applied".to_string(),
            files: applied_files,
            preview_mode: "codeblock".to_string(),
        }];
        artifact
    }

    fn validation_turn_artifact(
        changed_path: &str,
        events: Vec<ObservedTurnEvent>,
    ) -> AgentTurnArtifact {
        AgentTurnArtifact {
            task_id: "case-validation".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "prompt".to_string(),
            user_message_id: "user-validation".to_string(),
            events,
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: validation_patch_artifact(changed_path),
            llm_prompt: Vec::new(),
            llm_response: None,
        }
    }

    fn cargo_request(
        call_id: &str,
        command: &str,
        scope: &str,
        package: Option<&str>,
    ) -> ObservedTurnEvent {
        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String(command.to_string()),
        );
        args.insert(
            "scope".to_string(),
            serde_json::Value::String(scope.to_string()),
        );
        if let Some(package) = package {
            args.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }
        ObservedTurnEvent::ToolRequested(ToolRequestRecord {
            request_id: format!("req-{call_id}"),
            parent_id: format!("parent-{call_id}"),
            call_id: call_id.to_string(),
            tool: "cargo".to_string(),
            arguments: serde_json::Value::Object(args).to_string().into(),
        })
    }

    fn cargo_completed(
        call_id: &str,
        command: &str,
        scope: &str,
        manifest_path: PathBuf,
        ok: bool,
    ) -> ObservedTurnEvent {
        let content = serde_json::json!({
            "ok": ok,
            "command": command,
            "scope": scope,
            "manifest_path": manifest_path.display().to_string()
        })
        .to_string();
        ObservedTurnEvent::ToolCompleted(ToolCompletedRecord {
            request_id: format!("req-{call_id}"),
            parent_id: format!("parent-{call_id}"),
            call_id: call_id.to_string(),
            tool: "cargo".to_string(),
            content,
            ui_payload: None,
            latency_ms: 1,
        })
    }

    fn ns_patch_request(
        call_id: &str,
        file: &str,
        diff: &str,
        reasoning: &str,
    ) -> ObservedTurnEvent {
        let args = serde_json::json!({
            "patches": [{
                "file": file,
                "diff": diff,
                "reasoning": reasoning,
            }]
        });
        ObservedTurnEvent::ToolRequested(ToolRequestRecord {
            request_id: format!("req-{call_id}"),
            parent_id: format!("parent-{call_id}"),
            call_id: call_id.to_string(),
            tool: "non_semantic_patch".to_string(),
            arguments: args.to_string().into(),
        })
    }

    #[test]
    fn validation_audit_flags_final_cargo_manifest_that_misses_changed_file() {
        let repo_root = PathBuf::from("/tmp/ripgrep");
        let prepared = validation_prepared_run(repo_root.clone());
        let changed_path = "crates/printer/src/util.rs";
        let events = vec![
            cargo_request("call-1", "test", "focused", Some("grep-printer")),
            cargo_completed(
                "call-1",
                "test",
                "workspace",
                repo_root.join("Cargo.toml"),
                true,
            ),
            cargo_request("call-2", "test", "focused", None),
            cargo_completed(
                "call-2",
                "test",
                "focused",
                repo_root.join("crates/globset/Cargo.toml"),
                true,
            ),
        ];
        let artifact = validation_turn_artifact(changed_path, events);

        let audit = build_agent_validation_audit(&prepared, &artifact);

        assert_eq!(audit.changed_paths, vec![changed_path.to_string()]);
        assert_eq!(audit.cargo_calls.len(), 2);
        assert!(audit.successful_cargo_covering_changed_files);
        assert!(!audit.final_cargo_covers_changed_files);
        assert_eq!(
            audit
                .final_cargo_call
                .as_ref()
                .map(|call| call.package.as_ref()),
            Some(None)
        );
        assert!(
            audit
                .warnings
                .iter()
                .any(|warning| warning.contains("crates/globset/Cargo.toml"))
        );
        assert!(
            audit
                .warnings
                .iter()
                .any(|warning| warning.contains("no formatting check evidence recorded"))
        );
    }

    #[test]
    fn validation_audit_includes_applied_proposal_paths_beyond_expected_files() {
        let repo_root = PathBuf::from("/tmp/ripgrep");
        let prepared = validation_prepared_run(repo_root.clone());
        let expected_changed_path = "crates/printer/src/util.rs";
        let exported_patch_path = "crates/printer/src/standard.rs";
        let mut artifact = validation_turn_artifact(expected_changed_path, Vec::new());
        artifact.patch_artifact = validation_patch_artifact_with_applied_files(
            expected_changed_path,
            vec![
                repo_root.join(expected_changed_path).display().to_string(),
                repo_root.join(exported_patch_path).display().to_string(),
            ],
        );

        let audit = build_agent_validation_audit(&prepared, &artifact);

        assert_eq!(
            audit.changed_paths,
            vec![
                expected_changed_path.to_string(),
                exported_patch_path.to_string()
            ]
        );
        assert_eq!(audit.patch_quality.changed_path_count, 2);
        assert_eq!(audit.patch_quality.production_changed_path_count, 2);
    }

    #[test]
    fn validation_audit_flags_expected_output_patch_candidates() {
        let repo_root = PathBuf::from("/tmp/ripgrep");
        let prepared = validation_prepared_run(repo_root.clone());
        let changed_path = "crates/printer/src/util.rs";
        let events = vec![ns_patch_request(
            "call-patch",
            changed_path,
            r#"@@
 #[test]
 fn replacement_lookahead_bug_2208() {
-    assert_eq!(actual, "1:foo\n3:foo\n");
+    assert_eq!(actual, "1:foo\n2:foo\n");
 }
"#,
            "adjust expected output after the focused regression failed",
        )];
        let artifact = validation_turn_artifact(changed_path, events);

        let audit = build_agent_validation_audit(&prepared, &artifact);

        assert_eq!(audit.patch_quality.edit_request_count, 1);
        assert_eq!(audit.patch_quality.test_edit_request_count, 1);
        assert_eq!(audit.patch_quality.production_edit_request_count, 0);
        assert!(audit.patch_quality.mostly_test_edit_requests);
        assert_eq!(audit.patch_quality.expected_output_edit_candidates.len(), 1);
        let candidate = &audit.patch_quality.expected_output_edit_candidates[0];
        assert_eq!(candidate.call_id, "call-patch");
        assert_eq!(candidate.tool, "non_semantic_patch");
        assert_eq!(candidate.path, changed_path);
        assert!(candidate.evidence.contains("1:foo\\n3:foo"));
        assert!(
            audit
                .warnings
                .iter()
                .any(|warning| warning.contains("expected-output/assertion change"))
        );
    }

    #[test]
    fn maybe_build_msb_submission_record_uses_benchmark_identity() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path();
        let src_dir = repo_root.join("src");
        let file = src_dir.join("lib.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(repo_root, &["init"], "git init");
        run_git_test(
            repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&file, "fn main() {}\n").expect("write initial file");
        run_git_test(repo_root, &["add", "src/lib.rs"], "git add");
        run_git_test(repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        fs::write(&file, "fn main() {\n    println!(\"hi\");\n}\n").expect("write modified file");

        let prepared = PreparedSingleRun {
            task_id: "BurntSushi__ripgrep-2209".to_string(),
            repo_root: repo_root.to_path_buf(),
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2209,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("src/lib.rs")],
            })),
            campaign: None,
        };

        let record = maybe_build_msb_submission_record(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
        )
        .expect("submission record result")
        .expect("submission record");
        assert_eq!(record.org, "BurntSushi");
        assert_eq!(record.repo, "ripgrep");
        assert_eq!(record.number, 2209);
        assert!(record.fix_patch.starts_with("diff --git"));
    }

    #[test]
    fn maybe_build_msb_submission_record_skips_setup_only_runs() {
        let prepared = PreparedSingleRun {
            task_id: "BurntSushi__ripgrep-2209".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: PathBuf::from("/tmp/dataset.jsonl"),
                dataset_url: None,
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2209,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("crates/printer/src/util.rs")],
            })),
            campaign: None,
        };

        let record = maybe_build_msb_submission_record(&prepared, &RunArm::shell_only_control())
            .expect("setup-only submission result");
        assert!(record.is_none());
    }

    #[test]
    fn allocate_run_output_dir_nests_unique_run_directories() {
        let tmp = tempdir().expect("tempdir");
        let instance_dir = tmp.path().join("instances").join("org__repo-1");
        fs::create_dir_all(&instance_dir).expect("instance dir");

        let first = allocate_run_output_dir(
            &instance_dir,
            &RunArm::structured_current_policy_treatment(),
        )
        .expect("first dir");
        let second = allocate_run_output_dir(
            &instance_dir,
            &RunArm::structured_current_policy_treatment(),
        )
        .expect("second dir");

        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(instance_dir.join("runs").as_path()));
        assert_eq!(second.parent(), Some(instance_dir.join("runs").as_path()));
    }

    #[test]
    fn collect_submission_fix_patch_exports_git_diff_and_jsonl_shape() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path();
        let src_dir = repo_root.join("src");
        let file = src_dir.join("lib.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(repo_root, &["init"], "git init");
        run_git_test(
            repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&file, "fn main() {}\n").expect("write initial file");
        run_git_test(repo_root, &["add", "src/lib.rs"], "git add");
        run_git_test(repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        fs::write(&file, "fn main() {\n    println!(\"hi\");\n}\n").expect("write modified file");

        let prepared = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: repo_root.to_path_buf(),
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: "acme__repo-1".to_string(),
                org: "acme".to_string(),
                repo: "repo".to_string(),
                number: 1,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("src/lib.rs")],
            })),
            campaign: None,
        };

        let fix_patch = collect_submission_fix_patch(&prepared).expect("submission patch");
        assert!(fix_patch.contains("diff --git a/src/lib.rs b/src/lib.rs"));
        assert!(fix_patch.contains("+    println!(\"hi\");"));

        let record = maybe_build_msb_submission_record(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
        )
        .expect("submission result")
        .expect("submission");
        let jsonl_path = tmp.path().join("submission.jsonl");
        write_jsonl_line(&jsonl_path, &record).expect("write jsonl");

        let line = fs::read_to_string(&jsonl_path).expect("read jsonl");
        let parsed: MultiSweBenchSubmissionRecord =
            serde_json::from_str(line.trim()).expect("parse jsonl line");
        assert_eq!(parsed.org, "acme");
        assert_eq!(parsed.repo, "repo");
        assert_eq!(parsed.number, 1);
        assert!(parsed.fix_patch.contains("diff --git"));
    }

    #[test]
    fn write_msb_submission_artifact_writes_treatment_submission_into_run_dir() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path();
        let src_dir = repo_root.join("src");
        let file = src_dir.join("lib.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(repo_root, &["init"], "git init");
        run_git_test(
            repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&file, "fn main() {}\n").expect("write initial file");
        run_git_test(repo_root, &["add", "src/lib.rs"], "git add");
        run_git_test(repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        fs::write(&file, "fn main() {\n    println!(\"hi\");\n}\n").expect("write modified file");

        let prepared = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: repo_root.to_path_buf(),
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: "acme__repo-1".to_string(),
                org: "acme".to_string(),
                repo: "repo".to_string(),
                number: 1,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("src/lib.rs")],
            })),
            campaign: None,
        };
        let run_output_dir = tmp.path().join("out").join("runs").join("run-123");
        fs::create_dir_all(&run_output_dir).expect("run output dir");

        let submission_artifact = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            Some(&msb_patch_artifact(true, true)),
        )
        .expect("write submission")
        .expect("submission artifact");

        assert_eq!(
            submission_artifact.path,
            run_output_dir.join("multi-swe-bench-submission.jsonl")
        );
        assert_eq!(
            submission_artifact.patch_projection_path,
            run_output_dir.join("benchmark-patch-projection.json")
        );
        assert!(submission_artifact.patch_projection_path.is_file());
        let line = fs::read_to_string(&submission_artifact.path).expect("read submission");
        let parsed: MultiSweBenchSubmissionRecord =
            serde_json::from_str(line.trim()).expect("parse submission");
        assert_eq!(parsed.org, "acme");
        assert_eq!(parsed.repo, "repo");
        assert_eq!(parsed.number, 1);
        assert_eq!(submission_artifact.fix_patch, parsed.fix_patch);
        assert!(
            parsed
                .fix_patch
                .contains("diff --git a/src/lib.rs b/src/lib.rs")
        );
        assert!(parsed.fix_patch.contains("+    println!(\"hi\");"));
    }

    #[test]
    fn write_msb_submission_artifact_uses_repo_diff_only_for_partial_apply_state() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path();
        let src_dir = repo_root.join("src");
        let lib_file = src_dir.join("lib.rs");
        let main_file = src_dir.join("main.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(repo_root, &["init"], "git init");
        run_git_test(
            repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&lib_file, "pub fn helper() {}\n").expect("write initial lib file");
        fs::write(&main_file, "fn main() {}\n").expect("write initial main file");
        run_git_test(repo_root, &["add", "src/lib.rs", "src/main.rs"], "git add");
        run_git_test(repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        fs::write(&lib_file, "pub fn helper() {\n    println!(\"hi\");\n}\n")
            .expect("write modified lib file");

        let prepared = PreparedSingleRun {
            task_id: "case-partial".to_string(),
            repo_root: repo_root.to_path_buf(),
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: "acme__repo-2".to_string(),
                org: "acme".to_string(),
                repo: "repo".to_string(),
                number: 2,
                language: Some("rust".to_string()),
                expected_patch_files: vec![
                    PathBuf::from("src/lib.rs"),
                    PathBuf::from("src/main.rs"),
                ],
            })),
            campaign: None,
        };
        let run_output_dir = tmp.path().join("out").join("runs").join("run-456");
        fs::create_dir_all(&run_output_dir).expect("run output dir");

        let submission_artifact = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            Some(&msb_patch_artifact(true, true)),
        )
        .expect("write submission")
        .expect("submission artifact");

        let line = fs::read_to_string(&submission_artifact.path).expect("read submission");
        let parsed: MultiSweBenchSubmissionRecord =
            serde_json::from_str(line.trim()).expect("parse submission");
        assert_eq!(submission_artifact.fix_patch, parsed.fix_patch);
        assert!(
            parsed
                .fix_patch
                .contains("diff --git a/src/lib.rs b/src/lib.rs"),
            "submission should include the applied lib.rs change"
        );
        assert!(
            parsed.fix_patch.contains("+    println!(\"hi\");"),
            "submission should include the actual applied hunk"
        );
        assert!(
            !parsed
                .fix_patch
                .contains("diff --git a/src/main.rs b/src/main.rs"),
            "submission should not invent a diff for an expected-but-unchanged file"
        );
    }

    #[test]
    fn write_msb_submission_artifact_rejects_nonempty_patch_without_applied_evidence() {
        let tmp = tempdir().expect("tempdir");
        let (prepared, run_output_dir) =
            dirty_msb_prepared_for_submission(&tmp, "case-missing-evidence", 3);

        let packaging_err = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            Some(&msb_patch_artifact(false, false)),
        )
        .expect_err("non-empty submission must require applied same-run patch evidence");
        let packaging_err_text = packaging_err.to_string();
        assert!(
            packaging_err_text.contains("same run"),
            "error should explain same-run patch evidence requirement: {packaging_err_text}"
        );
        assert!(
            !run_output_dir
                .join("multi-swe-bench-submission.jsonl")
                .exists(),
            "invalid submission must not be written"
        );
    }

    #[test]
    fn write_msb_submission_artifact_rejects_nonempty_patch_without_patch_artifact() {
        let tmp = tempdir().expect("tempdir");
        let (prepared, run_output_dir) =
            dirty_msb_prepared_for_submission(&tmp, "case-no-patch-artifact", 4);

        let packaging_err = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            None,
        )
        .expect_err("non-empty submission must require a same-run patch artifact");
        let packaging_err_text = packaging_err.to_string();
        assert!(
            packaging_err_text.contains("patch artifact evidence"),
            "error should explain missing patch artifact evidence: {packaging_err_text}"
        );
        assert!(
            !run_output_dir
                .join("multi-swe-bench-submission.jsonl")
                .exists(),
            "invalid submission must not be written"
        );
        assert!(
            !run_output_dir
                .join("benchmark-patch-projection.json")
                .exists(),
            "invalid projection must not be written"
        );
    }

    #[test]
    fn write_msb_submission_artifact_rejects_nonempty_patch_without_expected_file_change() {
        let tmp = tempdir().expect("tempdir");
        let (prepared, run_output_dir) =
            dirty_msb_prepared_for_submission(&tmp, "case-unmatched-evidence", 5);

        let packaging_err = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            Some(&msb_patch_artifact(true, false)),
        )
        .expect_err("non-empty submission must require expected benchmark file evidence");
        let packaging_err_text = packaging_err.to_string();
        assert!(
            packaging_err_text.contains("expected benchmark file change"),
            "error should explain expected-file evidence requirement: {packaging_err_text}"
        );
        assert!(
            !run_output_dir
                .join("multi-swe-bench-submission.jsonl")
                .exists(),
            "invalid submission must not be written"
        );
    }

    #[test]
    fn write_msb_submission_artifact_rejects_nonempty_patch_without_expected_file_list() {
        let tmp = tempdir().expect("tempdir");
        let (prepared, run_output_dir) =
            dirty_msb_prepared_for_submission(&tmp, "case-empty-expected-file-evidence", 7);

        let packaging_err = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            Some(&msb_patch_artifact_without_expected_file_changes(true)),
        )
        .expect_err("non-empty submission must require expected benchmark file evidence");
        let packaging_err_text = packaging_err.to_string();
        assert!(
            packaging_err_text.contains("expected benchmark file evidence"),
            "error should explain missing expected-file evidence: {packaging_err_text}"
        );
        assert!(
            !run_output_dir
                .join("multi-swe-bench-submission.jsonl")
                .exists(),
            "invalid submission must not be written"
        );
    }

    #[test]
    fn write_msb_submission_artifact_allows_empty_patch_without_patch_artifact() {
        let tmp = tempdir().expect("tempdir");
        let (prepared, run_output_dir) =
            clean_msb_prepared_for_submission(&tmp, "case-empty-no-patch-artifact", 6);

        let submission_artifact = write_msb_submission_artifact(
            &prepared,
            &RunArm::structured_current_policy_treatment(),
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            None,
        )
        .expect("empty submission should not require patch artifact evidence")
        .expect("submission artifact");

        assert!(submission_artifact.fix_patch.trim().is_empty());
        let line = fs::read_to_string(&submission_artifact.path).expect("read submission");
        let parsed: MultiSweBenchSubmissionRecord =
            serde_json::from_str(line.trim()).expect("parse submission");
        assert!(parsed.fix_patch.trim().is_empty());
        assert!(submission_artifact.patch_projection_path.is_file());
    }

    #[test]
    fn packaging_write_failure_marks_packaging_phase_failed_after_real_repo_diff_exists() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path().join("repo");
        let src_dir = repo_root.join("src");
        let file = src_dir.join("lib.rs");
        fs::create_dir_all(&src_dir).expect("src dir");

        run_git_test(&repo_root, &["init"], "git init");
        run_git_test(
            &repo_root,
            &["config", "user.name", "Ploke Eval"],
            "git config name",
        );
        run_git_test(
            &repo_root,
            &["config", "user.email", "ploke-eval@example.com"],
            "git config email",
        );

        fs::write(&file, "fn main() {}\n").expect("write initial file");
        run_git_test(&repo_root, &["add", "src/lib.rs"], "git add");
        run_git_test(&repo_root, &["commit", "-m", "base"], "git commit");

        let base_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"], "git rev-parse HEAD")
            .expect("base sha")
            .expect("stdout")
            .trim()
            .to_string();

        fs::write(&file, "fn main() {\n    println!(\"hi\");\n}\n").expect("write modified file");

        let prepared = PreparedSingleRun {
            task_id: "case-packaging-failure".to_string(),
            repo_root: repo_root.clone(),
            output_dir: tmp.path().join("out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("The body text.".to_string()),
                body_path: None,
            },
            base_sha: Some(base_sha),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(crate::spec::MultiSweBenchSource {
                dataset_file: tmp.path().join("dataset.jsonl"),
                dataset_url: None,
                instance_id: "acme__repo-3".to_string(),
                org: "acme".to_string(),
                repo: "repo".to_string(),
                number: 3,
                language: Some("rust".to_string()),
                expected_patch_files: vec![PathBuf::from("src/lib.rs")],
            })),
            campaign: None,
        };

        let fix_patch = collect_submission_fix_patch(&prepared).expect("submission patch");
        assert!(
            fix_patch.contains("diff --git a/src/lib.rs b/src/lib.rs"),
            "setup should produce a real repo diff before packaging fails"
        );

        let run_output_dir = tmp
            .path()
            .join("out")
            .join("runs")
            .join("run-packaging-failure");
        fs::create_dir_all(&run_output_dir).expect("run output dir");
        fs::create_dir_all(run_output_dir.join("multi-swe-bench-submission.jsonl"))
            .expect("poison submission artifact path with directory");

        let run_arm = RunArm::structured_current_policy_treatment();
        let packaging_err = write_msb_submission_artifact(
            &prepared,
            &run_arm,
            &run_output_dir,
            &run_output_dir.join("run.json"),
            &run_output_dir.join("record.json.gz"),
            Some(&msb_patch_artifact(true, true)),
        )
        .expect_err("submission write should fail when target path is a directory");
        let packaging_err_text = packaging_err.to_string();
        assert!(
            packaging_err_text.contains("multi-swe-bench-submission.jsonl"),
            "packaging error should mention the submission artifact path: {packaging_err_text}"
        );

        let mut registration = RunRegistration::register_with_run_id(
            crate::inner::core::RunIntent {
                task_id: prepared.task_id.clone(),
                repo_root: prepared.repo_root.clone(),
                storage_roots: crate::inner::core::RunStorageRoots::new(
                    tmp.path().join("registries"),
                    tmp.path().join("instances"),
                ),
                base_sha: prepared.base_sha.clone(),
                budget: prepared.budget.clone(),
                model_id: Some("test-model".to_string()),
                provider_slug: Some("test-provider".to_string()),
                campaign_id: None,
                batch_id: None,
                run_arm_id: RunArm::structured_current_policy_treatment().id,
                run_role: crate::inner::core::RegisteredRunRole::Treatment,
            },
            "run-packaging-failure",
        )
        .expect("register run");

        registration.update_phase(
            RunLifecyclePhase::Packaging,
            RunPhaseStatus::InProgress,
            Some("writing benchmark submission artifact".to_string()),
        );
        let mut run_record = RunRecord::new(&prepared, run_arm);
        record_packaging_failure(
            &mut run_record,
            &mut registration,
            chrono::Utc::now().to_rfc3339(),
            packaging_err_text.clone(),
        );
        registration.mark_failed(packaging_err_text.clone());

        assert_eq!(
            registration.lifecycle.execution_status,
            crate::inner::registry::RunExecutionStatus::Failed
        );
        assert_eq!(
            registration.lifecycle.packaging.status,
            RunPhaseStatus::Failed
        );
        assert_eq!(
            registration.lifecycle.packaging.detail.as_deref(),
            Some(packaging_err_text.as_str())
        );
        assert_eq!(
            registration.lifecycle.submission_status,
            crate::inner::registry::RunSubmissionStatus::Missing
        );
        assert_eq!(
            run_record
                .phases
                .packaging
                .as_ref()
                .expect("packaging phase")
                .submission_artifact_state,
            SubmissionArtifactState::Missing
        );
        let record_path = run_output_dir.join("record.json.gz");
        write_compressed_record(&record_path, &run_record).expect("write failure record");
        let read_back =
            crate::record::read_compressed_record(&record_path).expect("read failure record");
        assert_eq!(
            read_back
                .phases
                .packaging
                .as_ref()
                .expect("read packaging phase")
                .submission_artifact_state,
            SubmissionArtifactState::Missing,
            "packaging failure should remain visible in the durable run record"
        );
        assert_ne!(
            registration.lifecycle.setup.status,
            RunPhaseStatus::Failed,
            "packaging failure should not be misclassified as setup failure"
        );
    }

    #[tokio::test]
    async fn collect_patch_artifact_snapshots_applied_proposals() {
        let state = create_mock_app_state();
        let state = Arc::new(state);
        {
            let mut proposals = state.proposals.write().await;
            proposals.insert(
                ploke_core::PROJECT_NAMESPACE_UUID,
                EditProposal {
                    proposal_id: ploke_tui::app_state::core::derive_edit_proposal_id(
                        ploke_core::PROJECT_NAMESPACE_UUID,
                        &ploke_core::ArcStr::from("edit-call"),
                    ),
                    request_id: ploke_core::PROJECT_NAMESPACE_UUID,
                    parent_id: ploke_core::PROJECT_NAMESPACE_UUID,
                    call_id: ploke_core::ArcStr::from("edit-call"),
                    proposed_at_ms: 1,
                    edits: Vec::new(),
                    files: vec![PathBuf::from("src/lib.rs")],
                    edits_ns: Vec::new(),
                    preview: DiffPreview::UnifiedDiff {
                        text: "--- a/src/lib.rs\n+++ b/src/lib.rs\n".to_string(),
                    },
                    status: EditProposalStatus::Applied,
                    is_semantic: true,
                },
            );
        }
        {
            let mut creates = state.create_proposals.write().await;
            creates.insert(
                ploke_core::PROJECT_NAMESPACE_UUID,
                CreateProposal {
                    request_id: ploke_core::PROJECT_NAMESPACE_UUID,
                    parent_id: ploke_core::PROJECT_NAMESPACE_UUID,
                    call_id: ploke_core::ArcStr::from("create-call"),
                    proposed_at_ms: 2,
                    creates: Vec::new(),
                    files: vec![PathBuf::from("src/new.rs")],
                    preview: DiffPreview::CodeBlocks {
                        per_file: Vec::new(),
                    },
                    status: EditProposalStatus::Applied,
                },
            );
        }

        let patch = collect_patch_artifact(&state).await;
        assert!(patch.applied);
        assert!(patch.all_proposals_applied);
        assert_eq!(patch.edit_proposals.len(), 1);
        assert_eq!(patch.create_proposals.len(), 1);
        assert_eq!(patch.edit_proposals[0].status, "Applied");
        assert_eq!(patch.create_proposals[0].files, vec!["src/new.rs"]);
        assert!(patch.expected_file_changes.is_empty());
    }

    #[tokio::test]
    async fn collect_patch_artifact_marks_partial_apply_as_applied_but_not_all_applied() {
        let state = create_mock_app_state();
        let state = Arc::new(state);
        {
            let mut proposals = state.proposals.write().await;
            let request_id = uuid::Uuid::new_v4();
            let call_id = ploke_core::ArcStr::from("edit-call-partial");
            proposals.insert(
                ploke_tui::app_state::core::derive_edit_proposal_id(request_id, &call_id),
                EditProposal {
                    proposal_id: ploke_tui::app_state::core::derive_edit_proposal_id(
                        request_id, &call_id,
                    ),
                    request_id,
                    parent_id: ploke_core::PROJECT_NAMESPACE_UUID,
                    call_id,
                    proposed_at_ms: 1,
                    edits: Vec::new(),
                    files: vec![PathBuf::from("src/lib.rs")],
                    edits_ns: Vec::new(),
                    preview: DiffPreview::UnifiedDiff {
                        text: "--- a/src/lib.rs\n+++ b/src/lib.rs\n".to_string(),
                    },
                    status: EditProposalStatus::PartiallyApplied("applied 1/2 files".to_string()),
                    is_semantic: true,
                },
            );
        }

        let patch = collect_patch_artifact(&state).await;
        assert!(patch.applied);
        assert!(!patch.all_proposals_applied);
    }

    #[test]
    fn expected_file_change_records_hash_transition() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path();
        let file = repo_root.join("src/lib.rs");
        fs::create_dir_all(file.parent().expect("parent")).expect("create dir");
        fs::write(&file, "before\n").expect("write before");

        let baselines =
            snapshot_expected_files(repo_root, &[PathBuf::from("src/lib.rs")]).expect("baseline");
        fs::write(&file, "after\n").expect("write after");

        let changes = collect_expected_file_changes(&baselines).expect("changes");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "src/lib.rs");
        assert!(changes[0].existed_before);
        assert!(changes[0].exists_after);
        assert!(changes[0].changed);
        assert_ne!(changes[0].before_sha256, changes[0].after_sha256);
    }

    #[tokio::test]
    async fn handle_benchmark_event_records_prompt_tool_message_and_finish() {
        let state = Arc::new(create_mock_app_state());
        let root_id = {
            let chat = state.chat.read().await;
            chat.current
        };
        let user_id = ploke_core::PROJECT_NAMESPACE_UUID;
        {
            let mut chat = state.chat.write().await;
            chat.add_message_user(root_id, user_id, "hello".to_string())
                .expect("add user message");
        }

        let mut artifact = AgentTurnArtifact {
            task_id: "case-123".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "prompt".to_string(),
            user_message_id: user_id.to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };
        let mut tool_request_started_at = HashMap::new();

        handle_benchmark_event(
            &mut artifact,
            &state,
            AppEvent::MessageUpdated(MessageUpdatedEvent::new(user_id)),
            &mut tool_request_started_at,
        )
        .await;

        handle_benchmark_event(
            &mut artifact,
            &state,
            AppEvent::System(SystemEvent::ToolCallRequested {
                request_id: ploke_core::PROJECT_NAMESPACE_UUID,
                parent_id: user_id,
                tool_call: ToolCall {
                    call_id: ploke_core::ArcStr::from("call-1"),
                    call_type: FunctionMarker,
                    function: FunctionCall {
                        name: ToolName::ApplyCodeEdit,
                        arguments: "{}".into(),
                    },
                    extra_content: None,
                },
            }),
            &mut tool_request_started_at,
        )
        .await;

        tokio::time::sleep(Duration::from_millis(5)).await;

        handle_benchmark_event(
            &mut artifact,
            &state,
            AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: ploke_core::PROJECT_NAMESPACE_UUID,
                parent_id: user_id,
                call_id: ploke_core::ArcStr::from("call-1"),
                content: "ok".to_string(),
                ui_payload: None,
            }),
            &mut tool_request_started_at,
        )
        .await;

        handle_benchmark_event(
            &mut artifact,
            &state,
            AppEvent::System(SystemEvent::ChatTurnFinished {
                session_id: ploke_core::PROJECT_NAMESPACE_UUID,
                request_id: ploke_core::PROJECT_NAMESPACE_UUID,
                parent_id: user_id,
                assistant_message_id: ploke_core::PROJECT_NAMESPACE_UUID,
                outcome: "success".to_string(),
                error_id: None,
                summary: "done".to_string(),
                attempts: 1,
            }),
            &mut tool_request_started_at,
        )
        .await;

        assert!(artifact.prompt_debug.is_none());
        assert!(artifact.terminal_record.is_some());
        assert!(
            artifact
                .events
                .iter()
                .any(|ev| matches!(ev, ObservedTurnEvent::ToolRequested(_)))
        );
        assert!(
            artifact
                .events
                .iter()
                .any(|ev| matches!(ev, ObservedTurnEvent::ToolCompleted(_)))
        );
        let latency_ms = artifact
            .events
            .iter()
            .rev()
            .find_map(|ev| match ev {
                ObservedTurnEvent::ToolCompleted(record) => Some(record.latency_ms),
                _ => None,
            })
            .expect("expected tool completion latency");
        assert!(latency_ms > 0, "expected a nonzero captured tool latency");
        assert!(
            artifact
                .events
                .iter()
                .any(|ev| matches!(ev, ObservedTurnEvent::MessageUpdated(_)))
        );
        assert_eq!(
            artifact.terminal_record.as_ref().unwrap().outcome,
            "success"
        );
        assert_eq!(artifact.terminal_record.as_ref().unwrap().summary, "done");
    }

    #[test]
    fn agent_turn_projection_converts_live_tool_payload_to_record_schema() {
        let call_id = ploke_core::ArcStr::from("call-1");
        let mut payload = ploke_tui::tools::ToolUiPayload::new(
            ToolName::ApplyCodeEdit,
            call_id.clone(),
            "edit staged",
        )
        .with_field("status", "staged")
        .with_details("ready")
        .with_verbosity(ToolVerbosity::Verbose);
        let tool_error = ploke_tui::tools::ToolError::new(
            ToolName::ApplyCodeEdit,
            ploke_tui::tools::ToolErrorCode::InvalidFormat,
            "invalid patch",
        )
        .field("patch");
        payload.error = Some(tool_error.to_wire());
        payload.error_code = Some(ploke_tui::tools::ToolErrorCode::InvalidFormat);

        let artifact = AgentTurnArtifact {
            task_id: "case-123".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "prompt".to_string(),
            user_message_id: Uuid::new_v4().to_string(),
            events: vec![ObservedTurnEvent::ToolCompleted(ToolCompletedRecord {
                request_id: Uuid::new_v4().to_string(),
                parent_id: Uuid::new_v4().to_string(),
                call_id: call_id.to_string(),
                tool: ToolName::ApplyCodeEdit.as_str().to_string(),
                content: "ok".to_string(),
                ui_payload: Some(payload),
                latency_ms: 17,
            })],
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };

        let trace = AgentTurnTraceRecord(artifact.to_record());
        let ObservedTurnEventRecord::ToolCompleted(completed) = &trace.0.events[0] else {
            panic!("expected tool completed event");
        };
        let projected = completed.ui_payload.as_ref().expect("projected ui payload");
        assert_eq!(projected.summary, "edit staged");
        assert_eq!(projected.verbosity, ToolVerbosityRecord::Verbose);
        assert_eq!(
            projected.error_code,
            Some(ToolErrorCodeRecord::InvalidFormat)
        );
        assert_eq!(projected.fields[0].name, "status");
        assert_eq!(projected.fields[0].value, "staged");
        assert_eq!(
            projected
                .error
                .as_ref()
                .map(|error| error.llm.message.as_str()),
            Some("invalid patch")
        );

        let encoded = serde_json::to_string(&trace).expect("serialize projected trace");
        let decoded: ploke_records::agent_turn::AgentTurnTraceRecord =
            serde_json::from_str(&encoded).expect("deserialize projected trace");
        let ObservedTurnEventRecord::ToolCompleted(decoded_completed) = &decoded.0.events[0] else {
            panic!("expected decoded tool completed event");
        };
        assert_eq!(
            decoded_completed
                .ui_payload
                .as_ref()
                .and_then(|payload| payload.error_code),
            Some(ToolErrorCodeRecord::InvalidFormat)
        );
    }

    #[tokio::test]
    async fn handle_benchmark_event_captures_prompt_constructed() {
        use ploke_llm::manager::Role;
        use ploke_tui::llm::{ChatEvt, LlmEvent};

        let state = Arc::new(create_mock_app_state());
        let mut artifact = AgentTurnArtifact {
            task_id: "test".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "test".to_string(),
            user_message_id: Uuid::new_v4().to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };

        // Create a PromptConstructed event with sample messages
        let parent_id = Uuid::new_v4();
        let formatted_prompt = vec![
            RequestMessage {
                role: Role::User,
                content: "Hello, fix this bug".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            RequestMessage {
                role: Role::Assistant,
                content: "I'll help you fix it".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let context_plan = ploke_tui::llm::ContextPlan {
            plan_id: Uuid::new_v4(),
            parent_id,
            estimated_total_tokens: 100,
            included_messages: Vec::new(),
            excluded_messages: Vec::new(),
            included_rag_parts: Vec::new(),
            rag_stats: None,
        };

        let event = AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::PromptConstructed {
            parent_id,
            formatted_prompt: formatted_prompt.clone(),
            context_plan,
        }));
        let mut tool_request_started_at = HashMap::new();

        // Handle the event
        handle_benchmark_event(&mut artifact, &state, event, &mut tool_request_started_at).await;

        // Verify the prompt was captured
        assert_eq!(
            artifact.llm_prompt.len(),
            2,
            "expected 2 messages in llm_prompt"
        );
        assert_eq!(artifact.llm_prompt[0].role, Role::User);
        assert_eq!(artifact.llm_prompt[0].content, "Hello, fix this bug");
        assert_eq!(artifact.llm_prompt[1].role, Role::Assistant);
        assert_eq!(artifact.llm_prompt[1].content, "I'll help you fix it");
        assert!(
            artifact.prompt_debug.is_some(),
            "prompt_debug should be set"
        );
    }

    #[tokio::test]
    async fn handle_benchmark_event_captures_llm_response() {
        use ploke_tui::llm::{ChatEvt, LlmEvent};

        let state = Arc::new(create_mock_app_state());
        let mut artifact = AgentTurnArtifact {
            task_id: "test".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "test".to_string(),
            user_message_id: Uuid::new_v4().to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };

        // Create a Response event
        let event = AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Response {
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            content: "Here is the fix you requested".to_string(),
            model: "test-model".to_string(),
            metadata: ploke_llm::types::meta::LLMMetadata {
                model: "test-model".to_string(),
                usage: ploke_llm::response::TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                },
                finish_reason: ploke_llm::response::FinishReason::Stop,
                processing_time: std::time::Duration::from_millis(500),
                cost: 0.001,
                performance: ploke_llm::types::meta::PerformanceMetrics {
                    tokens_per_second: 100.0,
                    time_to_first_token: std::time::Duration::from_millis(100),
                    queue_time: std::time::Duration::from_millis(50),
                },
            },
            usage: ploke_llm::manager::events::UsageMetrics {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                latency_ms: 500,
            },
        }));
        let mut tool_request_started_at = HashMap::new();

        // Handle the event
        handle_benchmark_event(&mut artifact, &state, event, &mut tool_request_started_at).await;

        // Verify the response was captured
        assert_eq!(
            artifact.llm_response,
            Some("Here is the fix you requested".to_string())
        );
    }

    #[tokio::test]
    async fn handle_benchmark_event_captures_structured_llm_response() {
        use ploke_llm::response::{FinishReason, TokenUsage};
        use ploke_tui::llm::{ChatEvt, LlmEvent};

        let state = Arc::new(create_mock_app_state());
        let mut artifact = AgentTurnArtifact {
            task_id: "test".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "test".to_string(),
            user_message_id: Uuid::new_v4().to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };

        // Create a Response event with full metadata
        let event = AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Response {
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            content: "The fix is to add a null check".to_string(),
            model: "anthropic/claude-3-sonnet".to_string(),
            metadata: ploke_llm::types::meta::LLMMetadata {
                model: "anthropic/claude-3-sonnet".to_string(),
                usage: TokenUsage {
                    prompt_tokens: 250,
                    completion_tokens: 75,
                    total_tokens: 325,
                },
                finish_reason: FinishReason::Stop,
                processing_time: std::time::Duration::from_millis(1200),
                cost: 0.0024,
                performance: ploke_llm::types::meta::PerformanceMetrics {
                    tokens_per_second: 62.5,
                    time_to_first_token: std::time::Duration::from_millis(300),
                    queue_time: std::time::Duration::from_millis(50),
                },
            },
            usage: ploke_llm::manager::events::UsageMetrics {
                prompt_tokens: 250,
                completion_tokens: 75,
                total_tokens: 325,
                latency_ms: 1200,
            },
        }));
        let mut tool_request_started_at = HashMap::new();

        // Handle the event
        handle_benchmark_event(&mut artifact, &state, event, &mut tool_request_started_at).await;

        // Verify a structured LlmResponse event was captured (Phase 1D)
        let llm_response_events: Vec<_> = artifact
            .events
            .iter()
            .filter_map(|ev| match ev {
                ObservedTurnEvent::LlmResponse(record) => Some(record),
                _ => None,
            })
            .collect();

        assert_eq!(
            llm_response_events.len(),
            1,
            "expected exactly one LlmResponse event"
        );

        let record = &llm_response_events[0];
        assert_eq!(record.content, "The fix is to add a null check");
        assert_eq!(record.model, "anthropic/claude-3-sonnet");

        // Verify token usage was captured structurally
        let usage = record.usage.as_ref().expect("usage should be present");
        assert_eq!(usage.prompt_tokens, 250);
        assert_eq!(usage.completion_tokens, 75);
        assert_eq!(usage.total_tokens, 325);

        // Verify finish reason was captured
        assert_eq!(record.finish_reason, Some(FinishReason::Stop));

        // Verify full metadata was captured
        assert!(record.metadata.is_some());
        let metadata = record.metadata.as_ref().unwrap();
        assert_eq!(metadata.model, "anthropic/claude-3-sonnet");
        assert_eq!(metadata.cost, 0.0024);
    }

    #[tokio::test]
    async fn drain_post_terminal_events_captures_late_llm_response() {
        use ploke_llm::response::FinishReason;
        use ploke_tui::llm::{ChatEvt, LlmEvent};

        let state = Arc::new(create_mock_app_state());
        let temp = tempdir().expect("tempdir");
        let trace_path = temp.path().join("turn-trace.json");
        let mut artifact = AgentTurnArtifact {
            task_id: "test".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "test".to_string(),
            user_message_id: Uuid::new_v4().to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: Some(TurnFinishedRecord {
                session_id: Uuid::new_v4().to_string(),
                request_id: Uuid::new_v4().to_string(),
                parent_id: Uuid::new_v4().to_string(),
                assistant_message_id: Uuid::new_v4().to_string(),
                outcome: "completed".to_string(),
                error_id: None,
                summary: "done".to_string(),
                attempts: 1,
            }),
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };
        write_agent_turn_trace(&trace_path, &artifact).expect("seed trace");

        let (_debug_tx, mut debug_rx) = mpsc::channel(1);
        let (realtime_tx, mut realtime_rx) = broadcast::channel(8);
        let (_background_tx, mut background_rx) = broadcast::channel(8);
        let mut tool_request_started_at = HashMap::new();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            let _ = realtime_tx.send(AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Response {
                request_id: Uuid::new_v4(),
                parent_id: Uuid::new_v4(),
                content: "late final answer".to_string(),
                model: "test-model".to_string(),
                metadata: ploke_llm::types::meta::LLMMetadata {
                    model: "test-model".to_string(),
                    usage: ploke_llm::response::TokenUsage {
                        prompt_tokens: 10,
                        completion_tokens: 5,
                        total_tokens: 15,
                    },
                    finish_reason: FinishReason::Stop,
                    processing_time: std::time::Duration::from_millis(50),
                    cost: 0.0,
                    performance: ploke_llm::types::meta::PerformanceMetrics {
                        tokens_per_second: 100.0,
                        time_to_first_token: std::time::Duration::from_millis(10),
                        queue_time: std::time::Duration::from_millis(0),
                    },
                },
                usage: ploke_llm::manager::events::UsageMetrics {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    latency_ms: 50,
                },
            })));
        });

        drain_post_terminal_events(
            &mut artifact,
            &state,
            &mut debug_rx,
            &mut realtime_rx,
            &mut background_rx,
            &trace_path,
            &mut tool_request_started_at,
            Duration::from_millis(250),
        )
        .await
        .expect("drain succeeds");

        assert_eq!(artifact.llm_response.as_deref(), Some("late final answer"));
        assert!(artifact.events.iter().any(|event| matches!(
            event,
            ObservedTurnEvent::LlmResponse(record) if record.content == "late final answer"
        )));
    }

    #[tokio::test]
    async fn terminal_branch_still_drains_events_when_llm_response_already_present() {
        use ploke_core::tool_types::{FunctionMarker, ToolName};
        use ploke_llm::response::{FunctionCall, ToolCall};

        let mut artifact = AgentTurnArtifact {
            task_id: "test".to_string(),
            selected_model: ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            issue_prompt: "test".to_string(),
            user_message_id: Uuid::new_v4().to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: Some(TurnFinishedRecord {
                session_id: Uuid::new_v4().to_string(),
                request_id: Uuid::new_v4().to_string(),
                parent_id: Uuid::new_v4().to_string(),
                assistant_message_id: Uuid::new_v4().to_string(),
                outcome: "completed".to_string(),
                error_id: None,
                summary: "done".to_string(),
                attempts: 1,
            }),
            final_assistant_message: None,
            patch_artifact: PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: Some("already captured".to_string()),
        };

        let (_debug_tx, mut debug_rx) =
            mpsc::channel::<ploke_tui::app::commands::harness::DebugStateCommand>(1);
        let (realtime_tx, mut realtime_rx) = broadcast::channel(8);
        let (_background_tx, mut background_rx) = broadcast::channel::<AppEvent>(8);
        let state = Arc::new(create_mock_app_state());
        let temp = tempdir().expect("tempdir");
        let trace_path = temp.path().join("turn-trace.json");
        let mut tool_request_started_at = HashMap::new();

        let request_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let _ = realtime_tx.send(AppEvent::System(SystemEvent::ToolCallRequested {
            request_id,
            parent_id,
            tool_call: ToolCall {
                call_id: ploke_core::ArcStr::from("queued-after-terminal"),
                call_type: FunctionMarker,
                function: FunctionCall {
                    name: ToolName::ApplyCodeEdit,
                    arguments: "{}".into(),
                },
                extra_content: None,
            },
        }));

        drain_post_terminal_events(
            &mut artifact,
            &state,
            &mut debug_rx,
            &mut realtime_rx,
            &mut background_rx,
            &trace_path,
            &mut tool_request_started_at,
            Duration::from_millis(250),
        )
        .await
        .expect("drain succeeds");

        assert_eq!(artifact.llm_response.as_deref(), Some("already captured"));
        assert!(artifact.events.iter().any(|event| matches!(
            event,
            ObservedTurnEvent::ToolRequested(record)
                if record.request_id == request_id.to_string()
        )));
        assert!(matches!(
            realtime_rx.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn run_benchmark_turn_keeps_response_when_it_arrives_before_turn_finished() {
        let temp = tempdir().expect("tempdir");
        let prepared = test_prepared_run(&temp);
        let trace_path = temp.path().join("turn-trace.json");
        let state = Arc::new(create_mock_app_state());
        let (mut app, _debug_tx, mut debug_rx, mut realtime_rx, mut background_rx, event_bus) =
            build_benchmark_turn_test_app(Arc::clone(&state)).await;
        let expected_file_baselines = Vec::new();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            event_bus.send(test_chat_response_event("response before terminal"));
            tokio::time::sleep(Duration::from_millis(25)).await;
            event_bus.send(test_turn_finished_event());
        });

        let artifact = run_benchmark_turn(
            &prepared,
            &state,
            &mut app,
            &mut debug_rx,
            &mut realtime_rx,
            &mut background_rx,
            &trace_path,
            ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            &expected_file_baselines,
        )
        .await
        .expect("benchmark turn succeeds");

        assert_eq!(
            artifact.llm_response.as_deref(),
            Some("response before terminal")
        );
        assert_eq!(
            artifact
                .terminal_record
                .as_ref()
                .map(|record| record.summary.as_str()),
            Some("done")
        );
        assert!(artifact.events.iter().any(|event| matches!(
            event,
            ObservedTurnEvent::LlmResponse(record) if record.content == "response before terminal"
        )));
        assert!(artifact.events.iter().any(|event| matches!(
            event,
            ObservedTurnEvent::TurnFinished(record) if record.summary == "done"
        )));
    }

    #[tokio::test]
    async fn run_benchmark_turn_captures_response_when_it_arrives_after_turn_finished() {
        let temp = tempdir().expect("tempdir");
        let prepared = test_prepared_run(&temp);
        let trace_path = temp.path().join("turn-trace.json");
        let state = Arc::new(create_mock_app_state());
        let (mut app, _debug_tx, mut debug_rx, mut realtime_rx, mut background_rx, event_bus) =
            build_benchmark_turn_test_app(Arc::clone(&state)).await;
        let expected_file_baselines = Vec::new();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            event_bus.send(test_turn_finished_event());
            tokio::time::sleep(Duration::from_millis(25)).await;
            event_bus.send(test_chat_response_event("response after terminal"));
        });

        let artifact = run_benchmark_turn(
            &prepared,
            &state,
            &mut app,
            &mut debug_rx,
            &mut realtime_rx,
            &mut background_rx,
            &trace_path,
            ploke_llm::ModelId::from(ploke_llm::ModelKey::default()),
            &expected_file_baselines,
        )
        .await
        .expect("benchmark turn succeeds");

        assert_eq!(
            artifact.llm_response.as_deref(),
            Some("response after terminal")
        );
        assert_eq!(
            artifact
                .terminal_record
                .as_ref()
                .map(|record| record.summary.as_str()),
            Some("done")
        );
        assert!(artifact.events.iter().any(|event| matches!(
            event,
            ObservedTurnEvent::LlmResponse(record) if record.content == "response after terminal"
        )));
        assert!(artifact.events.iter().any(|event| matches!(
            event,
            ObservedTurnEvent::TurnFinished(record) if record.summary == "done"
        )));
    }

    #[test]
    fn embedding_preflight_error_mentions_registry_and_suggestions() {
        let detail = format_embedding_preflight_error(
            &default_eval_embedding_model_id(),
            Some(Path::new("/tmp/embedding-models-openrouter.json")),
            "No successful provider responses",
            &[
                "openai/text-embedding-3-small".to_string(),
                "mistralai/mistral-embed-2312".to_string(),
            ],
        );

        assert!(detail.contains("embedding preflight failed"));
        assert!(detail.contains("mistralai/codestral-embed-2505"));
        assert!(detail.contains("/tmp/embedding-models-openrouter.json"));
        assert!(detail.contains("openai/text-embedding-3-small"));
        assert!(detail.contains("do not mix embedding models"));
    }

    #[cfg(feature = "live_api_tests")]
    #[tokio::test]
    #[ignore = "hits live OpenRouter embeddings; requires OPENROUTER_API_KEY"]
    async fn live_eval_embedding_selection_preflight_uses_openrouter_env() {
        assert!(
            std::env::var_os("OPENROUTER_API_KEY").is_some(),
            "OPENROUTER_API_KEY must be exported for live eval embedding preflight"
        );

        let selection = resolve_eval_embedding_selection(None, None)
            .await
            .expect("eval embedding selection should resolve from exported OPENROUTER_API_KEY");

        assert_eq!(selection.model.id, default_eval_embedding_model_id());
        assert!(selection.dimensions > 0);
    }
}
