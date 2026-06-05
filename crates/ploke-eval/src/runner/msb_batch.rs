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
use super::msb_single::*;
use super::*;

impl RunMsbBatchRequest {
    pub async fn run(self) -> Result<BatchRunArtifactPaths, PrepareError> {
        run_batch(
            self.batch_manifest,
            self.index_debug_snapshots,
            self.use_default_model,
            self.model_id,
            self.provider,
            self.stop_on_error,
            false,
        )
        .await
    }
}

impl RunMsbAgentBatchRequest {
    pub async fn run(self) -> Result<BatchRunArtifactPaths, PrepareError> {
        run_batch(
            self.batch_manifest,
            self.index_debug_snapshots,
            self.use_default_model,
            self.model_id,
            self.provider,
            self.stop_on_error,
            true,
        )
        .await
    }
}

pub(crate) async fn run_batch(
    batch_manifest: PathBuf,
    index_debug_snapshots: bool,
    use_default_model: bool,
    model_id: Option<String>,
    provider: Option<ProviderKey>,
    stop_on_error: bool,
    agent_mode: bool,
) -> Result<BatchRunArtifactPaths, PrepareError> {
    let run_arm = RunArm::for_agent_mode(agent_mode);
    let (manifest_path, prepared) = load_prepared_batch(batch_manifest)?;
    fs::create_dir_all(&prepared.output_dir).map_err(|source| PrepareError::CreateOutputDir {
        path: prepared.output_dir.clone(),
        source,
    })?;

    let requested_model = parse_requested_model_id(model_id.as_deref())?;
    let selected_model = resolve_model_for_run(requested_model.as_ref(), use_default_model)?;
    let selected_model_id = selected_model.id.clone();
    let preferred_provider =
        load_provider_preference_for_selected_model(&selected_model, provider.as_ref())?;
    let requested_provider = provider_request_for_selected_model(
        &selected_model,
        provider.as_ref(),
        preferred_provider.as_ref(),
    );
    let route = resolve_route_for_model(&selected_model, requested_provider).await?;
    let selected_provider = route.selected_provider_slug();

    let summary_path = prepared.output_dir.join("batch-run-summary.json");
    let submission_path = prepared.output_dir.join("multi-swe-bench-submission.jsonl");
    fs::write(&submission_path, "").map_err(|source| PrepareError::WriteManifest {
        path: submission_path.clone(),
        source,
    })?;

    let mut stopped_early = false;
    let mut instance_results = Vec::with_capacity(prepared.instances.len());

    for task_id in &prepared.instances {
        let run_manifest = prepared.instances_root.join(task_id).join("run.json");
        if agent_mode {
            match (RunMsbAgentSingleRequest {
                run_manifest: run_manifest.clone(),
                batch_id: Some(prepared.batch_id.clone()),
                index_debug_snapshots,
                use_default_model,
                model_id: model_id.clone(),
                provider: provider.clone(),
                embedding_model_id: None,
                embedding_provider: None,
            })
            .run()
            .await
            {
                Ok(artifacts) => {
                    if let Some(path) = artifacts.base.msb_submission.as_ref() {
                        let blob = fs::read_to_string(path).map_err(|source| {
                            PrepareError::ReadManifest {
                                path: path.clone(),
                                source,
                            }
                        })?;
                        if !blob.trim().is_empty() {
                            append_jsonl_blob(&submission_path, &blob)?;
                        }
                    }
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: Some(artifacts.base.execution_log),
                        record_path: artifacts.base.record_path,
                        turn_summary: Some(artifacts.turn_summary),
                        msb_submission: artifacts.base.msb_submission,
                        status: "completed".to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: None,
                        record_path: None,
                        turn_summary: None,
                        msb_submission: None,
                        status: "failed".to_string(),
                        error: Some(err.to_string()),
                    });
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                }
            }
        } else {
            match (RunMsbSingleRequest {
                run_manifest: run_manifest.clone(),
                batch_id: Some(prepared.batch_id.clone()),
                index_debug_snapshots,
                use_default_model,
                model_id: model_id.clone(),
                provider: provider.clone(),
            })
            .run()
            .await
            {
                Ok(artifacts) => {
                    if let Some(path) = artifacts.msb_submission.as_ref() {
                        let blob = fs::read_to_string(path).map_err(|source| {
                            PrepareError::ReadManifest {
                                path: path.clone(),
                                source,
                            }
                        })?;
                        if !blob.trim().is_empty() {
                            append_jsonl_blob(&submission_path, &blob)?;
                        }
                    }
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: Some(artifacts.execution_log),
                        record_path: artifacts.record_path,
                        turn_summary: None,
                        msb_submission: artifacts.msb_submission,
                        status: "completed".to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: None,
                        record_path: None,
                        turn_summary: None,
                        msb_submission: None,
                        status: "failed".to_string(),
                        error: Some(err.to_string()),
                    });
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                }
            }
        }
    }

    let instances_succeeded = instance_results
        .iter()
        .filter(|result| result.status == "completed")
        .count();
    let instances_failed = instance_results
        .iter()
        .filter(|result| result.status == "failed")
        .count();
    let msb_submission = match fs::metadata(&submission_path) {
        Ok(metadata) if metadata.len() > 0 => Some(submission_path.clone()),
        Ok(_) => None,
        Err(_) => None,
    };

    let summary = BatchRunSummary {
        batch_id: prepared.batch_id,
        mode: if agent_mode {
            "msb_agent_batch".to_string()
        } else {
            "msb_batch".to_string()
        },
        run_arm: run_arm.clone(),
        batch_manifest: manifest_path.clone(),
        output_dir: prepared.output_dir,
        dataset_file: prepared.dataset_file,
        repo_cache: prepared.repo_cache,
        instances_root: prepared.instances_root,
        selected_model: Some(selected_model_id),
        selected_provider: Some(selected_provider),
        instances_total: prepared.instances.len(),
        instances_attempted: instance_results.len(),
        instances_succeeded,
        instances_failed,
        stopped_early,
        instance_results,
    };
    write_json(&summary_path, &summary)?;

    Ok(BatchRunArtifactPaths {
        batch_manifest: manifest_path,
        summary: summary_path,
        msb_submission,
    })
}
