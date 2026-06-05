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

use super::artifacts::*;
use super::*;

impl ReplayMsbBatchRequest {
    pub async fn run(self) -> Result<PathBuf, PrepareError> {
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
        let (_app, state, _config_guard) = setup_replay_runtime(&prepared).await?;
        let replay_batch_path = prepared
            .output_dir
            .join(format!("replay-batch-{:03}.json", self.batch_number));

        let indexer_task =
            state
                .indexer_task
                .as_ref()
                .cloned()
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "replay_batch_indexer_task",
                    detail: "missing indexer task in app state".to_string(),
                })?;

        let batch = indexer_task
            .replay_batch(self.batch_number)
            .await
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "replay_batch",
                detail: err.to_string(),
            })?
            .ok_or_else(|| PrepareError::IndexingFailed {
                detail: format!(
                    "batch {} was not available in manifest {}",
                    self.batch_number,
                    manifest_path.display()
                ),
            })?;

        log_replay_batch_context(self.batch_number, &batch);
        let replay_artifact = ReplayBatchArtifact {
            batch_number: self.batch_number,
            run_manifest: manifest_path.clone(),
            batch_file: replay_batch_path.clone(),
            batch: batch.clone(),
        };
        write_json(&replay_batch_path, &replay_artifact)?;
        info!(
            batch_file = %replay_batch_path.display(),
            batch_number = self.batch_number,
            "wrote replay batch artifact"
        );
        indexer_task
            .process_batch(batch, |current, total| {
                info!(
                    batch_number = self.batch_number,
                    current, total, "replay batch progress"
                )
            })
            .await
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "replay_batch_process",
                detail: err.to_string(),
            })?;

        Ok(replay_batch_path)
    }
}
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
pub(crate) async fn prepare_workspace_for_replay(
    state: &Arc<AppState>,
    prepared: &PreparedSingleRun,
) -> Result<(), PrepareError> {
    info!(
        repo_root = %prepared.repo_root.display(),
        "preparing replay workspace for batch selection"
    );
    let resolved = resolve_index_target(Some(prepared.repo_root.clone()), &prepared.repo_root)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "replay_resolve_index_target",
            detail: err.to_string(),
        })?;

    run_parse_resolved(Arc::clone(&state.db), &resolved).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "replay_run_parse_resolved",
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
    info!(
        workspace_root = %resolved.workspace_root.display(),
        "replay workspace prepared"
    );
    Ok(())
}
pub(crate) fn log_replay_batch_context(batch_number: usize, batch: &[ploke_db::TypedEmbedData]) {
    tracing::info!(
        batch_number,
        relation_count = batch.len(),
        "replaying selected batch"
    );

    for (relation_index, relation) in batch.iter().enumerate() {
        tracing::trace!(
            target: "embed-pipeline",
            batch_number,
            relation_index,
            relation = %relation.ty.relation_str(),
            node_count = relation.v.len(),
            "replay batch relation"
        );

        for (node_index, node) in relation.v.iter().enumerate() {
            tracing::trace!(
                target: "embed-pipeline",
                batch_number,
                relation_index,
                node_index,
                node_id = %node.id,
                node_name = %node.name,
                file_path = %node.file_path.display(),
                start_byte = node.start_byte,
                end_byte = node.end_byte,
                "replay batch node"
            );
        }
    }
}
