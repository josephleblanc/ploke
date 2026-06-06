use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ploke_db::Database;
use ploke_db::bm25_index::bm25_service::Bm25Status;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_embed::indexer::{EmbeddingProcessor, IndexStatus, IndexingStatus};
use ploke_llm::request::models::ResponseItem;
use ploke_llm::router_only::{
    HasEndpoint,
    openrouter::{OpenRouter, OpenRouterModelId},
};
use ploke_llm::{LlmRoute, ModelId, ProviderKey, SupportsTools};
use ploke_tui::AppEvent;
use ploke_tui::app::App;
use ploke_tui::app::commands::harness::{TestRuntime, TestRuntimeActorGuard};
use ploke_tui::app::view::components::model_browser::tool_capable_provider_key;
use ploke_tui::app_state::AppState;
use ploke_tui::app_state::core::RuntimeConfig;
use ploke_tui::parser::{resolve_index_target, run_parse_resolved};
use ploke_tui::user_config::{
    ChatPolicy, ChatTimeoutStrategy, RetrievalStrategyUser, ToolLoopMode,
};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep};
use tracing::info;
use uuid::Uuid;

use crate::inner::core::{RegisteredRunRole, RunIntent};
use crate::inner::registry::RunRegistration;
use crate::provider_prefs::load_provider_for_model;
use crate::run_registry::{persist_registration, register_live_run, storage_roots_for_instance};
use crate::spec::{PrepareError, PreparedSingleRun, RunSource};

mod artifacts;
mod msb_batch;
mod msb_single;
mod replay;

pub(crate) const DEFAULT_PHASE_TIMEOUT_SECS: u64 = 900;
pub(crate) const WAIT_HEARTBEAT_SECS: u64 = 10;
pub(crate) const FINAL_RESPONSE_GRACE_MILLIS: u64 = 750;
pub(crate) const BM25_READY_TIMEOUT_SECS: u64 = 60;
pub(crate) const HEADLESS_TUI_BM25_TIMEOUT_MS: u64 = 10_000;
pub(crate) const HEADLESS_TUI_TOOL_CHAIN_LIMIT: usize = 500;
pub(crate) const HEADLESS_TUI_REPAIR_ATTEMPT_LIMIT: u32 = 128;
pub(crate) const HEADLESS_TUI_LLM_TIMEOUT_SECS: u64 = 900;
pub(crate) const OPENROUTER_CODESTRAL_MODEL: &str = "mistralai/codestral-embed-2505";
pub(crate) const STARTING_DB_CACHE_VERSION: u32 = 2;
pub(crate) const VALIDATION_AUDIT_SCHEMA_V2: &str = "agent-validation-audit.v2";
pub(crate) const VALIDATION_AUDIT_FILE: &str = "validation-audit.json";
static EMBEDDING_PREFLIGHT_CACHE: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

pub(crate) fn benchmark_chat_policy() -> ChatPolicy {
    let mut policy = ChatPolicy::default();
    policy.tool_call_timeout_secs = 60;
    policy.timeout_strategy = ChatTimeoutStrategy::Backoff { attempts: Some(3) };
    policy.timeout_base_secs = 5;
    policy.error_retry_limit = 3;
    policy.validated()
}

pub(crate) fn configure_eval_model_runtime(cfg: &mut RuntimeConfig, route: &LlmRoute) {
    cfg.llm_timeout_secs = ploke_llm::LLM_TIMEOUT_SECS;
    cfg.active_model = route.model().clone();
    cfg.active_router = route.router();
    if let Some(provider) = route.provider_key() {
        cfg.model_registry
            .select_model_provider(route.model(), Some(provider));
    }
}

pub(crate) fn configure_headless_benchmark_chat(cfg: &mut RuntimeConfig, route: &LlmRoute) {
    cfg.editing.auto_confirm_edits = true;
    cfg.chat_policy = benchmark_chat_policy();
    configure_eval_model_runtime(cfg, route);
}

pub(crate) fn artifact_runs_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join("runs")
}

pub(crate) fn allocate_run_output_dir(
    instance_dir: &Path,
    run_arm: &RunArm,
) -> Result<PathBuf, PrepareError> {
    let parent = artifact_runs_dir(instance_dir);
    fs::create_dir_all(&parent).map_err(|source| PrepareError::CreateOutputDir {
        path: parent.clone(),
        source,
    })?;
    let run_id = format!(
        "run-{}-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        run_arm.id,
        &Uuid::new_v4().simple().to_string()[..8]
    );
    Ok(parent.join(run_id))
}

pub(crate) fn registered_run_role(run_arm: &RunArm) -> RegisteredRunRole {
    match run_arm.role {
        RunArmRole::Control => RegisteredRunRole::Control,
        RunArmRole::Treatment => RegisteredRunRole::Treatment,
    }
}

pub(crate) fn build_run_intent(
    prepared: &PreparedSingleRun,
    run_arm: &RunArm,
    selected_model_id: &impl ToString,
    selected_provider_slug: &str,
    batch_id: Option<String>,
) -> Result<RunIntent, PrepareError> {
    Ok(RunIntent {
        task_id: prepared.task_id.clone(),
        repo_root: prepared.repo_root.clone(),
        storage_roots: storage_roots_for_instance(&prepared.output_dir)?,
        base_sha: prepared.base_sha.clone(),
        budget: prepared.budget.clone(),
        model_id: Some(selected_model_id.to_string()),
        provider_slug: Some(selected_provider_slug.to_string()),
        campaign_id: prepared
            .campaign
            .as_ref()
            .map(|campaign| campaign.campaign_id.clone()),
        batch_id,
        run_arm_id: run_arm.id.clone(),
        run_role: registered_run_role(run_arm),
    })
}

pub(crate) fn register_run_attempt(
    prepared: &PreparedSingleRun,
    manifest_path: &Path,
    run_arm: &RunArm,
    run_output_dir: &Path,
    selected_model_id: &impl ToString,
    selected_provider_slug: &str,
    batch_id: Option<String>,
) -> Result<RunRegistration, PrepareError> {
    let run_id = run_output_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| PrepareError::MissingRunManifest(run_output_dir.to_path_buf()))?;
    let intent = build_run_intent(
        prepared,
        run_arm,
        selected_model_id,
        selected_provider_slug,
        batch_id,
    )?;
    let mut registration = register_live_run(intent, run_id)?;
    registration.artifacts.run_manifest = manifest_path.to_path_buf();
    if matches!(prepared.source, Some(RunSource::MultiSweBench(_)))
        && run_arm.role == RunArmRole::Treatment
    {
        registration.artifacts.msb_submission =
            Some(run_output_dir.join("multi-swe-bench-submission.jsonl"));
        registration.artifacts.patch_projection =
            Some(run_output_dir.join("benchmark-patch-projection.json"));
    }
    persist_registration(&registration)?;
    Ok(registration)
}
pub(crate) async fn resolve_route_for_model(
    selected_model: &ResponseItem,
    requested_provider: Option<&ProviderKey>,
) -> Result<LlmRoute, PrepareError> {
    if !selected_model.supports_tools() {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_model_route",
            detail: format!(
                "model '{}' does not advertise tool-call support",
                selected_model.id
            ),
        });
    }

    if selected_model.route_source.is_direct_google() {
        if let Some(provider) = requested_provider {
            let requested_slug = provider.slug.as_str();
            if requested_slug != "google" {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_model_route",
                    detail: format!(
                        "requested provider '{requested_slug}' is not valid for direct Google model '{}'",
                        selected_model.id
                    ),
                });
            }
        }

        return Ok(LlmRoute::direct_google_model(selected_model));
    }

    let client = reqwest::Client::new();
    let typed_model = OpenRouterModelId::from(selected_model.id.clone());
    let endpoints = OpenRouter::fetch_model_endpoints(&client, typed_model)
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "fetch_model_endpoints",
            detail: err.to_string(),
        })?;

    if let Some(requested_provider) = requested_provider {
        let requested_slug = requested_provider.slug.as_str();
        let endpoint = endpoints
            .data
            .endpoints
            .iter()
            .find(|ep| ep.tag.provider_name.as_str() == requested_slug)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "resolve_model_provider",
                detail: format!(
                    "requested provider '{requested_slug}' was not returned for model '{}'",
                    selected_model.id
                ),
            })?;

        if !endpoint.supports_tools() {
            return Err(PrepareError::DatabaseSetup {
                phase: "resolve_model_provider",
                detail: format!(
                    "requested provider '{requested_slug}' for model '{}' does not support tool calls",
                    selected_model.id
                ),
            });
        }

        return Ok(LlmRoute::openrouter(
            selected_model.id.clone(),
            requested_provider.clone(),
            endpoint.clone(),
        ));
    }

    let provider = tool_capable_provider_key(&endpoints.data.endpoints).ok_or_else(|| {
        PrepareError::DatabaseSetup {
            phase: "resolve_model_provider",
            detail: format!(
                "no tool-capable provider endpoints returned for model '{}'",
                selected_model.id
            ),
        }
    })?;

    let endpoint = endpoints
        .data
        .endpoints
        .iter()
        .find(|ep| ep.tag.provider_name.as_str() == provider.slug.as_str())
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_model_provider",
            detail: format!(
                "selected provider '{}' was chosen but its endpoint metadata was unavailable for model '{}'",
                provider.slug.as_str(),
                selected_model.id
            ),
        })?;

    Ok(LlmRoute::openrouter(
        selected_model.id.clone(),
        provider,
        endpoint.clone(),
    ))
}

pub(crate) fn parse_requested_model_id(
    model_id: Option<&str>,
) -> Result<Option<ModelId>, PrepareError> {
    model_id
        .map(|model_id| {
            model_id
                .parse()
                .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                    phase: "resolve_run_model_id",
                    detail: err.to_string(),
                })
        })
        .transpose()
}

pub(crate) fn provider_request_for_selected_model<'a>(
    selected_model: &ResponseItem,
    explicit_provider: Option<&'a ProviderKey>,
    preferred_provider: Option<&'a ProviderKey>,
) -> Option<&'a ProviderKey> {
    explicit_provider.or_else(|| {
        if selected_model.route_source.is_direct_google() {
            None
        } else {
            preferred_provider
        }
    })
}

pub(crate) fn load_provider_preference_for_selected_model(
    selected_model: &ResponseItem,
    explicit_provider: Option<&ProviderKey>,
) -> Result<Option<ProviderKey>, PrepareError> {
    if explicit_provider.is_some() || selected_model.route_source.is_direct_google() {
        Ok(None)
    } else {
        load_provider_for_model(&selected_model.id)
    }
}
pub(crate) fn init_runtime_db() -> Result<Arc<Database>, PrepareError> {
    info!("initializing eval runtime database");
    let db = Arc::new(
        Database::init_with_schema().map_err(|err| PrepareError::DatabaseSetup {
            phase: "init_with_schema",
            detail: err.to_string(),
        })?,
    );
    db.setup_multi_embedding()
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "setup_multi_embedding",
            detail: err.to_string(),
        })?;
    info!("eval runtime database initialized");
    Ok(db)
}

#[cfg(test)]
pub(crate) async fn setup_replay_runtime(
    prepared: &PreparedSingleRun,
) -> Result<(App, Arc<AppState>, XdgConfigHomeGuard), PrepareError> {
    let runtime_db = init_runtime_db()?;
    let embedding_selection = resolve_eval_embedding_selection(None, None).await?;

    let config_home = prepared.output_dir.join("config");
    fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
        path: config_home.clone(),
        source,
    })?;
    let config_guard = XdgConfigHomeGuard::set_to(&config_home);

    let embedding_processor = eval_embedding_processor(&embedding_selection)?;
    let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus();
    let state = runtime.state_arc();

    activate_eval_embedding_runtime(&state, &embedding_selection)?;

    prepare_workspace_for_replay(&state, prepared).await?;

    let app = runtime
        .into_app_with_state_pwd(prepared.repo_root.clone())
        .await;

    Ok((app, state, config_guard))
}

pub(crate) struct WorkspaceTuiRuntime {
    _actor_guard: TestRuntimeActorGuard,
    pub(crate) app: App,
    pub(crate) state: Arc<AppState>,
    pub(crate) debug_rx: mpsc::Receiver<ploke_tui::app::commands::harness::DebugStateCommand>,
    pub(crate) realtime_rx: broadcast::Receiver<AppEvent>,
    pub(crate) background_rx: broadcast::Receiver<AppEvent>,
    _config_home: tempfile::TempDir,
    _config_guard: XdgConfigHomeGuard,
}

#[cfg(test)]
pub(crate) async fn setup_workspace_tui_runtime(
    workspace_root: &Path,
) -> Result<WorkspaceTuiRuntime, PrepareError> {
    setup_workspace_tui_runtime_with_read_roots(workspace_root, &[]).await
}

pub(crate) async fn setup_workspace_tui_runtime_with_read_roots(
    workspace_root: &Path,
    extra_read_roots: &[PathBuf],
) -> Result<WorkspaceTuiRuntime, PrepareError> {
    let runtime_db = init_runtime_db()?;

    let config_home = tempfile::tempdir().map_err(|source| PrepareError::CreateOutputDir {
        path: PathBuf::from("<temporary xdg config home>"),
        source,
    })?;
    let config_guard = XdgConfigHomeGuard::set_to(config_home.path());

    let embedding_processor = sparse_headless_embedding_processor();
    let runtime = TestRuntime::new_with_embedding_processor_and_bm25_timeout(
        &runtime_db,
        embedding_processor,
        HEADLESS_TUI_BM25_TIMEOUT_MS,
    )
    .spawn_file_manager()
    .spawn_state_manager()
    .spawn_event_bus()
    .spawn_llm_manager()
    .spawn_observability();
    let events = runtime.events_builder().build_all();
    let realtime_rx = events.event_bus_events.realtime_tx_rx;
    let background_rx = events.event_bus_events.background_tx_rx;
    let index_rx = Arc::try_unwrap(events.event_bus_events.index_tx_rx).map_err(|_| {
        PrepareError::DatabaseSetup {
            phase: "subscribe_index_status",
            detail: "index receiver unexpectedly shared".to_string(),
        }
    })?;
    drop(index_rx);
    let debug_rx =
        events
            .app_actor_events
            .debug_string_rx
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "subscribe_debug_string",
                detail: "missing debug string receiver".to_string(),
            })?;
    let state = runtime.state_arc();

    configure_sparse_strict_rag(&state).await;
    prepare_sparse_workspace(&state, workspace_root, extra_read_roots).await?;

    let (mut app, actor_guard) = runtime
        .into_app_with_state_pwd_and_actor_guard(workspace_root.to_path_buf())
        .await;
    wait_for_bm25_ready(&mut app, Arc::clone(&state)).await?;
    app.pump_pending_events().await;

    Ok(WorkspaceTuiRuntime {
        _actor_guard: actor_guard,
        app,
        state,
        debug_rx,
        realtime_rx,
        background_rx,
        _config_home: config_home,
        _config_guard: config_guard,
    })
}
#[cfg(test)]
pub(crate) async fn setup_workspace_tui_prompt_runtime(
    workspace_root: &Path,
) -> Result<WorkspaceTuiRuntime, PrepareError> {
    let runtime_db = init_runtime_db()?;

    let config_home = tempfile::tempdir().map_err(|source| PrepareError::CreateOutputDir {
        path: PathBuf::from("<temporary xdg config home>"),
        source,
    })?;
    let config_guard = XdgConfigHomeGuard::set_to(config_home.path());

    let embedding_processor = sparse_headless_embedding_processor();
    let runtime = TestRuntime::new_with_embedding_processor_and_bm25_timeout(
        &runtime_db,
        embedding_processor,
        HEADLESS_TUI_BM25_TIMEOUT_MS,
    )
    .spawn_file_manager()
    .spawn_state_manager()
    .spawn_event_bus()
    .spawn_observability();
    let events = runtime.events_builder().build_all();
    let realtime_rx = events.event_bus_events.realtime_tx_rx;
    let background_rx = events.event_bus_events.background_tx_rx;
    let index_rx = Arc::try_unwrap(events.event_bus_events.index_tx_rx).map_err(|_| {
        PrepareError::DatabaseSetup {
            phase: "subscribe_index_status",
            detail: "index receiver unexpectedly shared".to_string(),
        }
    })?;
    drop(index_rx);
    let debug_rx =
        events
            .app_actor_events
            .debug_string_rx
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "subscribe_debug_string",
                detail: "missing debug string receiver".to_string(),
            })?;
    let state = runtime.state_arc();

    configure_sparse_strict_rag(&state).await;
    prepare_sparse_workspace(&state, workspace_root, &[]).await?;

    let (mut app, actor_guard) = runtime
        .into_app_with_state_pwd_and_actor_guard(workspace_root.to_path_buf())
        .await;
    wait_for_bm25_ready(&mut app, Arc::clone(&state)).await?;
    app.pump_pending_events().await;

    Ok(WorkspaceTuiRuntime {
        _actor_guard: actor_guard,
        app,
        state,
        debug_rx,
        realtime_rx,
        background_rx,
        _config_home: config_home,
        _config_guard: config_guard,
    })
}

pub(crate) async fn configure_sparse_strict_rag(state: &Arc<AppState>) {
    let mut cfg = state.config.write().await;
    cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
    cfg.rag.strict_bm25_by_default = true;
    cfg.rag.bm25_timeout_ms = HEADLESS_TUI_BM25_TIMEOUT_MS;
    cfg.llm_timeout_secs = HEADLESS_TUI_LLM_TIMEOUT_SECS;
    cfg.chat_policy.tool_call_timeout_secs = HEADLESS_TUI_LLM_TIMEOUT_SECS;
    cfg.chat_policy.tool_call_chain_limit = HEADLESS_TUI_TOOL_CHAIN_LIMIT;
    cfg.chat_policy.tool_loop_mode = ToolLoopMode::Gated;
    cfg.chat_policy.repair_attempt_limit = HEADLESS_TUI_REPAIR_ATTEMPT_LIMIT;
    cfg.chat_policy.error_retry_limit = 10;
    cfg.chat_policy.length_retry_limit = 5;
    cfg.chat_policy.timeout_base_secs = HEADLESS_TUI_LLM_TIMEOUT_SECS;
    cfg.chat_policy.timeout_strategy = ChatTimeoutStrategy::Backoff { attempts: Some(10) };
}

pub(crate) fn sparse_headless_embedding_processor() -> EmbeddingProcessor {
    // The Prototype 1 headless TUI adapter forces sparse-strict retrieval below.
    // It still needs an EmbeddingProcessor to construct the TUI runtime, but it
    // should not spend remote embedding quota before a sparse-only splice run.
    EmbeddingProcessor::new_mock()
}

pub(crate) async fn prepare_sparse_workspace(
    state: &Arc<AppState>,
    workspace_root: &Path,
    extra_read_roots: &[PathBuf],
) -> Result<(), PrepareError> {
    let resolved = resolve_index_target(Some(workspace_root.to_path_buf()), workspace_root)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "sparse_workspace_resolve_index_target",
            detail: err.to_string(),
        })?;

    run_parse_resolved(Arc::clone(&state.db), &resolved).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "sparse_workspace_run_parse_resolved",
            detail: err.to_string(),
        }
    })?;

    let outcome = state
        .with_system_txn(|txn| {
            txn.set_loaded_workspace(
                resolved.workspace_root.clone(),
                resolved.member_roots.clone(),
                Some(resolved.focused_root.clone()),
            );
            txn.set_extra_read_roots(extra_read_roots.to_vec());
            txn.record_parse_success();
            txn.derive_path_policy(&[])
        })
        .await;
    if let Some(policy) = outcome.result {
        state
            .io_handle
            .update_roots(Some(policy.roots), Some(policy.symlink_policy))
            .await;
    }

    Ok(())
}

pub(crate) async fn wait_for_bm25_ready(
    app: &mut App,
    state: Arc<AppState>,
) -> Result<(), PrepareError> {
    let Some(rag) = state.rag.as_ref().cloned() else {
        return Err(PrepareError::DatabaseSetup {
            phase: "bm25_ready",
            detail: "RAG service is unavailable".to_string(),
        });
    };

    rag.bm25_rebuild()
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "bm25_rebuild",
            detail: err.to_string(),
        })?;

    let deadline = Instant::now() + Duration::from_secs(BM25_READY_TIMEOUT_SECS);
    loop {
        app.pump_pending_events().await;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(PrepareError::Timeout {
                phase: "bm25_ready",
                secs: BM25_READY_TIMEOUT_SECS,
            });
        }
        let status = rag
            .bm25_status_with_timeout(remaining)
            .await
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "bm25_status",
                detail: err.to_string(),
            })?;
        match status {
            Bm25Status::Ready { docs } => {
                if docs > 0 {
                    return Ok(());
                }
            }
            Bm25Status::Error(detail) => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "bm25_ready",
                    detail,
                });
            }
            Bm25Status::Empty => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "bm25_ready",
                    detail: "BM25 rebuild completed with no indexed documents".to_string(),
                });
            }
            Bm25Status::Uninitialized | Bm25Status::Building => {}
        }

        if Instant::now() >= deadline {
            return Err(PrepareError::Timeout {
                phase: "bm25_ready",
                secs: BM25_READY_TIMEOUT_SECS,
            });
        }
        sleep(Duration::from_millis(100)).await;
    }
}

pub(crate) async fn seed_loaded_workspace_from_repo(
    state: &Arc<AppState>,
    prepared: &PreparedSingleRun,
) -> Result<(), PrepareError> {
    let resolved = resolve_index_target(Some(prepared.repo_root.clone()), &prepared.repo_root)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "seed_loaded_workspace_resolve_index_target",
            detail: err.to_string(),
        })?;

    let policy = state
        .with_system_txn(|txn| {
            txn.set_loaded_workspace(
                resolved.workspace_root.clone(),
                resolved.member_roots.clone(),
                Some(resolved.focused_root.clone()),
            );
            txn.derive_path_policy(&[])
        })
        .await;

    if let Some(policy) = policy.result {
        state
            .io_handle
            .update_roots(Some(policy.roots), Some(policy.symlink_policy))
            .await;
    }

    Ok(())
}
pub(crate) async fn wait_for_indexing_completion(
    app: &mut App,
    realtime_rx: &mut broadcast::Receiver<AppEvent>,
    background_rx: &mut broadcast::Receiver<AppEvent>,
    index_rx: &mut broadcast::Receiver<IndexingStatus>,
    db: Arc<Database>,
    checkpoint_snapshot: PathBuf,
    failure_snapshot: PathBuf,
    persist_debug_snapshots: bool,
    last_progress: &mut Option<IndexingProgressArtifact>,
) -> Result<(), PrepareError> {
    let deadline = Instant::now() + Duration::from_secs(DEFAULT_PHASE_TIMEOUT_SECS);
    let mut last_heartbeat = Instant::now();
    loop {
        app.pump_pending_events().await;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            if persist_debug_snapshots {
                persist_db_snapshot(
                    Arc::clone(&db),
                    failure_snapshot.clone(),
                    "indexing timeout",
                )
                .await?;
            }
            return Err(PrepareError::Timeout {
                phase: "indexing_completed",
                secs: DEFAULT_PHASE_TIMEOUT_SECS,
            });
        }

        if last_heartbeat.elapsed() >= Duration::from_secs(WAIT_HEARTBEAT_SECS) {
            info!(
                remaining_secs = remaining.as_secs(),
                "waiting for indexing completion"
            );
            last_heartbeat = Instant::now();
        }

        let wait_for = remaining.min(Duration::from_millis(250));
        tokio::select! {
            realtime = realtime_rx.recv() => {
                match realtime {
                    Ok(AppEvent::IndexingCompleted) => {
                        app.pump_pending_events().await;
                        return Ok(());
                    }
                    Ok(AppEvent::IndexingFailed) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failed realtime",
                            )
                            .await?;
                        }
                        app.pump_pending_events().await;
                        return Err(PrepareError::IndexingFailed {
                            detail: "received AppEvent::IndexingFailed".to_string(),
                        });
                    }
                    Ok(AppEvent::Error(error)) if error.message.contains("Indexing failed") => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failed realtime error",
                            )
                            .await?;
                        }
                        app.pump_pending_events().await;
                        return Err(PrepareError::IndexingFailed {
                            detail: error.message,
                        });
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed {
                            phase: "indexing_completed",
                        });
                    }
                }
            }
            background = background_rx.recv() => {
                match background {
                    Ok(AppEvent::Error(error)) if error.message.contains("Indexing failed") => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failed background error",
                            )
                            .await?;
                        }
                        return Err(PrepareError::IndexingFailed {
                            detail: error.message,
                        });
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed {
                            phase: "indexing_completed_background",
                        });
                    }
                }
            }
            raw = index_rx.recv() => {
                match raw {
                    Ok(status) => {
                        *last_progress = Some(IndexingProgressArtifact::from(&status));
                        match &status.status {
                            IndexStatus::Running => {
                                if persist_debug_snapshots {
                                    persist_db_snapshot(
                                        Arc::clone(&db),
                                        checkpoint_snapshot.clone(),
                                        "indexing checkpoint",
                                    )
                                    .await?;
                                }
                                continue;
                            }
                            IndexStatus::Completed => {
                                if persist_debug_snapshots {
                                    persist_db_snapshot(
                                        Arc::clone(&db),
                                        checkpoint_snapshot.clone(),
                                        "completed checkpoint",
                                    )
                                    .await?;
                                }
                                return Ok(());
                            }
                            IndexStatus::Failed(err) => {
                                if persist_debug_snapshots {
                                    persist_db_snapshot(
                                        Arc::clone(&db),
                                        failure_snapshot.clone(),
                                        "indexing failure",
                                    )
                                    .await?;
                                }
                                return Err(PrepareError::IndexingFailed {
                                    detail: err.clone(),
                                });
                            }
                            IndexStatus::Cancelled => {
                                if persist_debug_snapshots {
                                    persist_db_snapshot(
                                        Arc::clone(&db),
                                        failure_snapshot.clone(),
                                        "indexing cancelled",
                                    )
                                    .await?;
                                }
                                return Err(PrepareError::IndexingFailed {
                                    detail: "indexing cancelled".to_string(),
                                });
                            }
                            _ => continue,
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed {
                            phase: "indexing_completed_raw",
                        });
                    }
                }
            }
            _ = sleep(wait_for) => continue,
        }
    }
}

pub(crate) async fn persist_db_snapshot(
    db: Arc<Database>,
    snapshot_path: PathBuf,
    label: &'static str,
) -> Result<(), PrepareError> {
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    info!(snapshot = %snapshot_path.display(), label, "persisting eval db snapshot");
    tokio::task::spawn_blocking(move || {
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path).map_err(|source| PrepareError::WriteManifest {
                path: snapshot_path.clone(),
                source,
            })?;
        }
        db.backup_db(snapshot_path.clone())
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "backup_db",
                detail: source.to_string(),
            })
    })
    .await
    .map_err(|join_err| PrepareError::DatabaseSetup {
        phase: "backup_db_join",
        detail: join_err.to_string(),
    })?
}

pub(crate) fn canonicalize_file(path: &Path) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingRunManifest(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}

pub(crate) fn checkout_repo_to_base(
    repo_root: &Path,
    base_sha: Option<&str>,
) -> Result<(), PrepareError> {
    run_git(repo_root, &["reset", "--hard"], "git reset --hard")?;
    if let Some(base_sha) = base_sha {
        run_git(
            repo_root,
            &["checkout", "--detach", base_sha],
            format!("git checkout --detach {base_sha}"),
        )?;
    }
    Ok(())
}

pub(crate) fn run_git(
    repo_root: &Path,
    args: &[&str],
    command_label: impl Into<String>,
) -> Result<(), PrepareError> {
    let command_label = command_label.into();
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .status()
        .map_err(|source| PrepareError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(PrepareError::GitCommandStatus {
            command: command_label,
            status: status.code().unwrap_or(-1),
        })
    }
}
pub(crate) fn git_stdout(
    repo_root: &Path,
    args: &[&str],
    command_label: impl Into<String>,
) -> Result<Option<String>, PrepareError> {
    let command_label = command_label.into();
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .map_err(|source| PrepareError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if !output.status.success() {
        return Err(PrepareError::GitCommandStatus {
            command: command_label,
            status: output.status.code().unwrap_or(-1),
        });
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}
pub(crate) async fn persist_full_response_trace_slice(
    source_path: &Path,
    start_offset: u64,
    destination_path: &Path,
) -> Result<bool, PrepareError> {
    use std::io::{Read, Seek, SeekFrom};

    let mut attempt = 0usize;
    let slice = loop {
        match fs::File::open(source_path) {
            Ok(mut file) => {
                let len = file
                    .metadata()
                    .map_err(|source| PrepareError::ReadManifest {
                        path: source_path.to_path_buf(),
                        source,
                    })?
                    .len();
                if len <= start_offset {
                    if attempt >= 5 {
                        break None;
                    }
                } else {
                    file.seek(SeekFrom::Start(start_offset)).map_err(|source| {
                        PrepareError::ReadManifest {
                            path: source_path.to_path_buf(),
                            source,
                        }
                    })?;
                    let mut buf = String::new();
                    file.read_to_string(&mut buf)
                        .map_err(|source| PrepareError::ReadManifest {
                            path: source_path.to_path_buf(),
                            source,
                        })?;
                    if !buf.trim().is_empty() {
                        break Some(buf);
                    }
                    if attempt >= 5 {
                        break None;
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if attempt >= 5 {
                    break None;
                }
            }
            Err(source) => {
                return Err(PrepareError::ReadManifest {
                    path: source_path.to_path_buf(),
                    source,
                });
            }
        }

        attempt += 1;
        sleep(Duration::from_millis(50)).await;
    };

    if let Some(blob) = slice {
        append_jsonl_blob(destination_path, &blob)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
pub(crate) fn load_prepared_run(
    run_manifest: PathBuf,
) -> Result<(PathBuf, PreparedSingleRun), PrepareError> {
    let manifest_path = canonicalize_file(&run_manifest)?;
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|source| PrepareError::ReadManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let prepared: PreparedSingleRun =
        serde_json::from_str(&manifest_text).map_err(|source| PrepareError::ParseManifest {
            path: manifest_path.clone(),
            source,
        })?;

    Ok((manifest_path, prepared))
}
pub(crate) struct XdgConfigHomeGuard {
    old_xdg: Option<String>,
}

impl XdgConfigHomeGuard {
    fn set_to(path: &Path) -> Self {
        let old_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", path);
        }
        Self { old_xdg }
    }
}

impl Drop for XdgConfigHomeGuard {
    fn drop(&mut self) {
        if let Some(old_xdg) = self.old_xdg.take() {
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", old_xdg);
            }
        } else {
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }
}

pub use artifacts::*;
#[cfg(test)]
pub use msb_batch::*;
#[cfg(test)]
pub use msb_single::*;
pub(crate) use replay::*;

#[cfg(test)]
mod tests;
