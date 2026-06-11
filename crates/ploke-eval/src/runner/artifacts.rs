use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::prelude::*;

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::Database;
use ploke_embed::config::{OpenRouterConfig, TruncatePolicy};
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexingStatus};
use ploke_embed::providers::openrouter::OpenRouterBackend;
use ploke_llm::embeddings::{
    EmbClientConfig, EmbeddingInput, EmbeddingRequest, HasDims, HasEmbeddings,
};
use ploke_llm::manager::RequestMessage;
use ploke_llm::request::{endpoint::Endpoint, models::ResponseItem};
use ploke_llm::router_only::openrouter::{
    EmbeddingProviderPrefs, OpenRouter, ProviderPreferences, embed::OpenRouterEmbeddingFields,
};
use ploke_llm::{LlmRoute, ModelId, ProviderKey, ProviderSlug};
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
use ploke_tui::app_state::AppState;
use ploke_tui::app_state::core::ParseFailure;
use ploke_tui::app_state::core::{DiffPreview, EditProposalStatus};
use ploke_tui::utils::parse_errors::FlattenedParserDiagnostic;
use sha2::{Digest, Sha256};
use tokio::time::Instant;
use tracing::{info, warn};

use crate::LlmResponseRecord;
use crate::inner::registry::{RunLifecyclePhase, RunPhaseStatus};
use crate::layout;
use crate::record::{
    CrateIndexStatus, IndexedCrateSummary, PackagingPhase, ParseErrorSummary, ParseFailureRecord,
    RunRecord, RunTimingSummary, SetupPhase, SubmissionArtifactState,
};
use crate::spec::{PreparedMsbBatch, PreparedSingleRun, RunSource};

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbAgentSingleRequest {
    pub run_manifest: PathBuf,
    #[serde(default)]
    pub batch_id: Option<String>,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub provider: Option<ProviderKey>,
    #[serde(default)]
    pub embedding_model_id: Option<String>,
    #[serde(default)]
    pub embedding_provider: Option<ProviderKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbSingleRequest {
    pub run_manifest: PathBuf,
    #[serde(default)]
    pub batch_id: Option<String>,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub provider: Option<ProviderKey>,
    #[serde(default)]
    pub embedding_model_id: Option<String>,
    #[serde(default)]
    pub embedding_provider: Option<ProviderKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbBatchRequest {
    pub batch_manifest: PathBuf,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub provider: Option<ProviderKey>,
    #[serde(default)]
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbAgentBatchRequest {
    pub batch_manifest: PathBuf,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub provider: Option<ProviderKey>,
    #[serde(default)]
    pub embedding_model_id: Option<String>,
    #[serde(default)]
    pub embedding_provider: Option<ProviderKey>,
    #[serde(default)]
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMsbBatchRequest {
    pub run_manifest: PathBuf,
    /// 1-based batch index to replay.
    pub batch_number: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunArmRole {
    Control,
    Treatment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunArm {
    pub id: String,
    pub role: RunArmRole,
    pub command: String,
    pub execution: String,
}

impl RunArm {
    pub fn shell_only_control() -> Self {
        Self {
            id: "shell-only".to_string(),
            role: RunArmRole::Control,
            command: "run single setup".to_string(),
            execution: "setup-only".to_string(),
        }
    }

    pub fn structured_current_policy_treatment() -> Self {
        Self {
            id: "structured-current-policy".to_string(),
            role: RunArmRole::Treatment,
            command: "run single agent".to_string(),
            execution: "agent-single-turn".to_string(),
        }
    }

    pub fn for_agent_mode(agent_mode: bool) -> Self {
        if agent_mode {
            Self::structured_current_policy_treatment()
        } else {
            Self::shell_only_control()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunArtifactPaths {
    pub run_manifest: PathBuf,
    pub execution_log: PathBuf,
    pub repo_state: PathBuf,
    pub indexing_status: PathBuf,
    pub indexing_checkpoint_db: PathBuf,
    pub indexing_failure_db: PathBuf,
    pub snapshot_status: PathBuf,
    pub msb_submission: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_projection: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_audit: Option<PathBuf>,
    /// Path to the compressed RunRecord (`record.json.gz`).
    /// Added in Phase 1 for comprehensive run persistence and replay.
    pub record_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_response_trace: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunArtifactPaths {
    pub base: RunArtifactPaths,
    pub turn_trace: PathBuf,
    pub turn_summary: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRunArtifactPaths {
    pub batch_manifest: PathBuf,
    pub summary: PathBuf,
    pub msb_submission: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStateArtifact {
    pub repo_root: PathBuf,
    pub requested_base_sha: Option<String>,
    pub checked_out_head_sha: Option<String>,
    pub git_status_porcelain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatusArtifact {
    pub status: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_progress: Option<IndexingProgressArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingProgressArtifact {
    pub raw_status: String,
    pub recent_processed: usize,
    pub num_not_proc: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseFailureArtifact {
    pub target_dir: PathBuf,
    pub message: String,
    pub occurred_at_ms: i64,
    pub diagnostics: Vec<FlattenedParserDiagnostic>,
}

impl From<&IndexingStatus> for IndexingProgressArtifact {
    fn from(status: &IndexingStatus) -> Self {
        Self {
            raw_status: indexing_status_name(&status.status).to_string(),
            recent_processed: status.recent_processed,
            num_not_proc: status.num_not_proc,
            current_file: status.current_file.clone(),
            errors: status.errors.clone(),
            observed_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }
}

pub(crate) fn indexing_status_name(status: &IndexStatus) -> &'static str {
    match status {
        IndexStatus::Idle => "idle",
        IndexStatus::Running => "running",
        IndexStatus::Paused => "paused",
        IndexStatus::Completed => "completed",
        IndexStatus::Failed(_) => "failed",
        IndexStatus::Cancelled => "cancelled",
    }
}

pub(crate) fn indexing_status_artifact_for_error(
    err: &PrepareError,
    last_progress: Option<IndexingProgressArtifact>,
) -> Option<IndexingStatusArtifact> {
    match err {
        PrepareError::IndexingFailed { detail } => Some(IndexingStatusArtifact {
            status: "failed".to_string(),
            detail: detail.clone(),
            last_progress,
        }),
        PrepareError::Timeout { phase, secs } if phase.starts_with("indexing_completed") => {
            Some(IndexingStatusArtifact {
                status: "timed_out".to_string(),
                detail: format!("timed out waiting for '{phase}' after {secs} seconds"),
                last_progress,
            })
        }
        PrepareError::EventStreamClosed { phase } if phase.starts_with("indexing_completed") => {
            Some(IndexingStatusArtifact {
                status: "event_stream_closed".to_string(),
                detail: format!("event stream closed while waiting for '{phase}'"),
                last_progress,
            })
        }
        _ => None,
    }
}

pub(crate) fn persist_indexing_failure_status(
    indexing_status_path: &Path,
    err: &PrepareError,
    last_progress: Option<IndexingProgressArtifact>,
) {
    let Some(artifact) = indexing_status_artifact_for_error(err, last_progress) else {
        return;
    };
    if let Err(write_err) = write_json(indexing_status_path, &artifact) {
        warn!(
            path = %indexing_status_path.display(),
            error = %write_err,
            "runner phase: failed to persist indexing failure status artifact"
        );
    }
}

pub(crate) fn parse_failure_artifact_for_state(
    parse_failure: ParseFailure,
) -> ParseFailureArtifact {
    ParseFailureArtifact {
        target_dir: parse_failure.target_dir,
        message: parse_failure.message,
        occurred_at_ms: parse_failure.occurred_at_ms,
        diagnostics: parse_failure.diagnostics,
    }
}

pub(crate) async fn persist_parse_failure_artifact(
    state: &Arc<AppState>,
    parse_failure_path: &Path,
) {
    let parse_failure = state
        .with_system_read(|sys| sys.last_parse_failure().cloned())
        .await;
    let Some(parse_failure) = parse_failure else {
        return;
    };
    let artifact = parse_failure_artifact_for_state(parse_failure);
    if let Err(write_err) = write_json(parse_failure_path, &artifact) {
        warn!(
            path = %parse_failure_path.display(),
            error = %write_err,
            "runner phase: failed to persist parse failure artifact"
        );
    }
}

/// Build SetupPhase with indexed crate information from the database.
pub(crate) async fn build_setup_phase(
    db: &Database,
    repo_state: &RepoStateArtifact,
    indexing_status: &IndexingStatusArtifact,
    setup_start_time: chrono::DateTime<chrono::Utc>,
    using_cached_db: bool,
    parse_failure_path: &Path,
) -> Result<SetupPhase, PrepareError> {
    // 1. Get crate list from DB
    let crate_rows = db
        .list_crate_context_rows()
        .map_err(|e| PrepareError::DatabaseSetup {
            phase: "list_crate_context_rows",
            detail: format!("Failed to list crate contexts: {e}"),
        })?;

    // 2. Read parse failures from file if it exists (for determining crate status)
    let parse_failure_artifact: Option<ParseFailureArtifact> = if parse_failure_path.exists() {
        match fs::read_to_string(parse_failure_path) {
            Ok(text) => serde_json::from_str(&text).ok(),
            Err(_) => None,
        }
    } else {
        None
    };

    // 3. For each crate, build IndexedCrateSummary
    let mut indexed_crates: Vec<IndexedCrateSummary> = Vec::new();
    for row in crate_rows {
        let node_count = db.count_nodes_for_namespace(row.namespace).map_err(|e| {
            PrepareError::DatabaseSetup {
                phase: "count_nodes_for_namespace",
                detail: format!("Failed to count nodes for namespace {}: {e}", row.namespace),
            }
        })?;

        let embedded_count = db
            .count_embedded_for_namespace(row.namespace)
            .map_err(|e| PrepareError::DatabaseSetup {
                phase: "count_embedded_for_namespace",
                detail: format!(
                    "Failed to count embedded nodes for namespace {}: {e}",
                    row.namespace
                ),
            })?;

        // Determine status based on whether we used cached DB and parse failures
        let status = if using_cached_db {
            CrateIndexStatus::Skipped
        } else if let Some(ref failure) = parse_failure_artifact {
            // Check if this crate's root_path matches the failed target_dir
            let failure_target = &failure.target_dir;
            let crate_root = PathBuf::from(&row.root_path);
            if crate_root == *failure_target || failure_target.to_string_lossy().contains(&row.name)
            {
                CrateIndexStatus::Failed
            } else {
                CrateIndexStatus::Success
            }
        } else {
            CrateIndexStatus::Success
        };

        // Check for parse error specific to this crate
        let parse_error = if using_cached_db {
            None
        } else if let Some(ref failure) = parse_failure_artifact {
            let failure_target = &failure.target_dir;
            let crate_root = PathBuf::from(&row.root_path);
            if crate_root == *failure_target || failure_target.to_string_lossy().contains(&row.name)
            {
                Some(ParseErrorSummary {
                    message: failure.message.clone(),
                    target_dir: failure.target_dir.clone(),
                    occurred_at_ms: failure.occurred_at_ms,
                })
            } else {
                None
            }
        } else {
            None
        };

        indexed_crates.push(IndexedCrateSummary {
            name: row.name,
            version: String::new(), // Version not stored in DB currently
            namespace: row.namespace,
            root_path: PathBuf::from(row.root_path),
            file_count: 0, // Can be queried from DB if needed
            node_count,
            embedded_count,
            status,
            parse_error,
        });
    }

    // 4. Build parse_failures list from artifact
    let parse_failures: Vec<ParseFailureRecord> = if let Some(failure) = parse_failure_artifact {
        vec![ParseFailureRecord {
            target_dir: failure.target_dir,
            message: failure.message,
            occurred_at_ms: failure.occurred_at_ms,
        }]
    } else {
        vec![]
    };

    // 5. Get DB timestamp
    let db_timestamp_micros =
        db.current_validity_micros()
            .map_err(|e| PrepareError::DatabaseSetup {
                phase: "current_validity_micros",
                detail: format!("Failed to get DB timestamp: {e}"),
            })?;

    Ok(SetupPhase {
        started_at: setup_start_time.to_rfc3339(),
        ended_at: chrono::Utc::now().to_rfc3339(),
        repo_state: repo_state.clone(),
        indexing_status: indexing_status.clone(),
        indexed_crates,
        parse_failures,
        db_timestamp_micros,
        tool_schema_version: None, // Can be populated from state.config if needed
    })
}

pub(crate) fn finalize_run_timing(
    run_record: &mut RunRecord,
    started_at: chrono::DateTime<chrono::Utc>,
    run_start_instant: Instant,
    setup_wall_clock_secs: Option<f64>,
    agent_wall_clock_secs: Option<f64>,
) {
    run_record.timing = Some(RunTimingSummary {
        started_at: started_at.to_rfc3339(),
        ended_at: chrono::Utc::now().to_rfc3339(),
        total_wall_clock_secs: run_start_instant.elapsed().as_secs_f64(),
        setup_wall_clock_secs,
        agent_wall_clock_secs,
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotStatusArtifact {
    pub status: String,
    pub snapshot_file: Option<PathBuf>,
    pub registry_file: PathBuf,
    pub config_home: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartingDbCacheMetadata {
    pub version: u32,
    pub task_id: String,
    #[serde(default)]
    pub repo_root: PathBuf,
    pub checkout_sha: Option<String>,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub embedding_dimensions: u32,
    pub embedding_dtype: String,
    #[serde(default)]
    pub typed_type_graph: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct EvalEmbeddingSelection {
    pub(crate) model: ResponseItem,
    pub(crate) provider: Option<ProviderKey>,
    pub(crate) dimensions: u32,
}

impl EvalEmbeddingSelection {
    fn cache_key(&self) -> String {
        let provider = self
            .provider
            .as_ref()
            .map(|provider| provider.slug.as_str())
            .unwrap_or("<auto>");
        format!("{}::{provider}", self.model.id)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StartingDbCachePaths {
    pub(crate) snapshot: PathBuf,
    pub(crate) metadata: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub task_id: String,
    pub run_arm: RunArm,
    pub repo_root: PathBuf,
    pub output_dir: PathBuf,
    pub selected_model: ModelId,
    pub selected_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_endpoint: Option<SelectedEndpointProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_response_trace: Option<PathBuf>,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectedEndpointProvenance {
    pub provider_name: String,
    pub provider_slug: String,
    pub endpoint_name: String,
    pub endpoint_model_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
}

impl SelectedEndpointProvenance {
    fn from_endpoint(endpoint: &Endpoint) -> Self {
        Self {
            provider_name: endpoint.provider_name.as_str().to_string(),
            provider_slug: endpoint.tag.provider_name.as_str().to_string(),
            endpoint_name: endpoint.name.as_ref().to_string(),
            endpoint_model_name: endpoint.model_name.as_str().to_string(),
            quantization: endpoint.quantization.map(|q| q.as_str().to_string()),
        }
    }
}

pub(crate) fn selected_endpoint_provenance(route: &LlmRoute) -> Option<SelectedEndpointProvenance> {
    match route {
        LlmRoute::OpenRouter(route) => {
            Some(SelectedEndpointProvenance::from_endpoint(&route.endpoint))
        }
        LlmRoute::Google(_) => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInstanceResult {
    pub task_id: String,
    pub run_arm: RunArm,
    pub run_manifest: PathBuf,
    pub execution_log: Option<PathBuf>,
    pub record_path: Option<PathBuf>,
    pub turn_summary: Option<PathBuf>,
    pub msb_submission: Option<PathBuf>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRunSummary {
    pub batch_id: String,
    pub mode: String,
    pub run_arm: RunArm,
    pub batch_manifest: PathBuf,
    pub output_dir: PathBuf,
    pub dataset_file: PathBuf,
    pub repo_cache: PathBuf,
    #[serde(alias = "runs_root")]
    pub instances_root: PathBuf,
    pub selected_model: Option<ModelId>,
    pub selected_provider: Option<String>,
    pub instances_total: usize,
    pub instances_attempted: usize,
    pub instances_succeeded: usize,
    pub instances_failed: usize,
    pub stopped_early: bool,
    pub instance_results: Vec<BatchInstanceResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBatchArtifact {
    pub batch_number: usize,
    pub run_manifest: PathBuf,
    pub batch_file: PathBuf,
    pub batch: Vec<ploke_db::TypedEmbedData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequestRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: String,
    /// Captured argument text. Persisted readers must deserialize through a
    /// typed tool-argument record, typed enum, or typed parse-failure record;
    /// do not inspect this as anonymous JSON.
    pub arguments: ploke_records::tool_contracts::ToolArgumentsJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCompletedRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: String,
    pub content: String,
    pub ui_payload: Option<ploke_tui::tools::ToolUiPayload>,
    #[serde(default)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFailedRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: Option<String>,
    pub error: String,
    pub ui_payload: Option<ploke_tui::tools::ToolUiPayload>,
    #[serde(default)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSnapshotRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub tool_call_id: Option<String>,
    pub content_len: usize,
    pub content_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnFinishedRecord {
    pub session_id: String,
    pub request_id: String,
    pub parent_id: String,
    pub assistant_message_id: String,
    pub outcome: String,
    pub error_id: Option<String>,
    pub summary: String,
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservedTurnEvent {
    DebugCommand(String),
    LlmEvent(String),
    /// Structured LLM response capture (Phase 1D).
    LlmResponse(LlmResponseRecord),
    ToolRequested(ToolRequestRecord),
    ToolCompleted(ToolCompletedRecord),
    ToolFailed(ToolFailedRecord),
    MessageUpdated(MessageSnapshotRecord),
    TurnFinished(TurnFinishedRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSnapshotRecord {
    pub request_id: String,
    pub call_id: String,
    pub status: String,
    pub files: Vec<String>,
    pub preview_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchArtifact {
    pub edit_proposals: Vec<ProposalSnapshotRecord>,
    pub create_proposals: Vec<ProposalSnapshotRecord>,
    pub applied: bool,
    pub all_proposals_applied: bool,
    pub expected_file_changes: Vec<ExpectedFileChangeRecord>,
    pub any_expected_file_changed: bool,
    pub all_expected_files_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentValidationAudit {
    pub schema_version: String,
    pub checked_at: String,
    pub changed_paths: Vec<String>,
    pub patch_quality: PatchQualityAudit,
    pub cargo_calls: Vec<CargoValidationCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_cargo_call: Option<CargoValidationCall>,
    pub successful_cargo_covering_changed_files: bool,
    pub final_cargo_covers_changed_files: bool,
    pub fmt_check_observed: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchQualityAudit {
    pub changed_path_count: usize,
    pub test_changed_path_count: usize,
    pub production_changed_path_count: usize,
    pub test_only_changed_paths: bool,
    pub edit_request_count: usize,
    pub test_edit_request_count: usize,
    pub production_edit_request_count: usize,
    pub mostly_test_edit_requests: bool,
    pub expected_output_edit_candidates: Vec<ExpectedOutputEditCandidate>,
    pub red_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpectedOutputEditCandidate {
    pub event_index: usize,
    pub call_id: String,
    pub tool: String,
    pub path: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CargoValidationCall {
    pub event_index: usize,
    pub call_id: String,
    pub command: String,
    pub requested_scope: Option<String>,
    pub resolved_scope: String,
    pub package: Option<String>,
    pub manifest_path: String,
    pub ok: bool,
    pub covers_changed_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CargoRequestProjection {
    requested_scope: Option<String>,
    package: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditRequestProjection {
    path: String,
    body: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CargoResultProjection {
    ok: bool,
    command: String,
    scope: String,
    manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpectedFileChangeRecord {
    pub path: String,
    pub existed_before: bool,
    pub exists_after: bool,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub changed: bool,
}

impl ToRecord for ToolRequestRecord {
    type Record = PersistedToolRequestRecord;

    fn to_record(&self) -> Self::Record {
        PersistedToolRequestRecord {
            request_id: self.request_id.clone(),
            parent_id: self.parent_id.clone(),
            call_id: self.call_id.clone(),
            tool: self.tool.clone(),
            arguments: self.arguments.clone(),
        }
    }
}

impl ToRecord for ToolCompletedRecord {
    type Record = PersistedToolCompletedRecord;

    fn to_record(&self) -> Self::Record {
        PersistedToolCompletedRecord {
            request_id: self.request_id.clone(),
            parent_id: self.parent_id.clone(),
            call_id: self.call_id.clone(),
            tool: self.tool.clone(),
            content: self.content.clone(),
            ui_payload: self.ui_payload.as_ref().map(tool_ui_payload_record),
            latency_ms: self.latency_ms,
        }
    }
}

impl ToRecord for ToolFailedRecord {
    type Record = PersistedToolFailedRecord;

    fn to_record(&self) -> Self::Record {
        PersistedToolFailedRecord {
            request_id: self.request_id.clone(),
            parent_id: self.parent_id.clone(),
            call_id: self.call_id.clone(),
            tool: self.tool.clone(),
            error: self.error.clone(),
            ui_payload: self.ui_payload.as_ref().map(tool_ui_payload_record),
            latency_ms: self.latency_ms,
        }
    }
}

impl ToRecord for MessageSnapshotRecord {
    type Record = PersistedMessageSnapshotRecord;

    fn to_record(&self) -> Self::Record {
        PersistedMessageSnapshotRecord {
            id: self.id.clone(),
            kind: self.kind.clone(),
            status: self.status.clone(),
            tool_call_id: self.tool_call_id.clone(),
            content_len: self.content_len,
            content_preview: self.content_preview.clone(),
        }
    }
}

impl ToRecord for TurnFinishedRecord {
    type Record = PersistedTurnFinishedRecord;

    fn to_record(&self) -> Self::Record {
        PersistedTurnFinishedRecord {
            session_id: self.session_id.clone(),
            request_id: self.request_id.clone(),
            parent_id: self.parent_id.clone(),
            assistant_message_id: self.assistant_message_id.clone(),
            outcome: self.outcome.clone(),
            error_id: self.error_id.clone(),
            summary: self.summary.clone(),
            attempts: self.attempts,
        }
    }
}

impl ToRecord for ProposalSnapshotRecord {
    type Record = PersistedProposalSnapshotRecord;

    fn to_record(&self) -> Self::Record {
        PersistedProposalSnapshotRecord {
            request_id: self.request_id.clone(),
            call_id: self.call_id.clone(),
            status: self.status.clone(),
            files: self.files.clone(),
            preview_mode: self.preview_mode.clone(),
        }
    }
}

impl ToRecord for ExpectedFileChangeRecord {
    type Record = PersistedExpectedFileChangeRecord;

    fn to_record(&self) -> Self::Record {
        PersistedExpectedFileChangeRecord {
            path: self.path.clone(),
            existed_before: self.existed_before,
            exists_after: self.exists_after,
            before_sha256: self.before_sha256.clone(),
            after_sha256: self.after_sha256.clone(),
            changed: self.changed,
        }
    }
}

impl ToRecord for PatchArtifact {
    type Record = PersistedPatchArtifactRecord;

    fn to_record(&self) -> Self::Record {
        PersistedPatchArtifactRecord {
            edit_proposals: self
                .edit_proposals
                .iter()
                .map(ToRecord::to_record)
                .collect(),
            create_proposals: self
                .create_proposals
                .iter()
                .map(ToRecord::to_record)
                .collect(),
            applied: self.applied,
            all_proposals_applied: self.all_proposals_applied,
            expected_file_changes: self
                .expected_file_changes
                .iter()
                .map(ToRecord::to_record)
                .collect(),
            any_expected_file_changed: self.any_expected_file_changed,
            all_expected_files_changed: self.all_expected_files_changed,
        }
    }
}

impl ToRecord for ObservedTurnEvent {
    type Record = ObservedTurnEventRecord;

    fn to_record(&self) -> Self::Record {
        match self {
            Self::DebugCommand(value) => ObservedTurnEventRecord::DebugCommand(value.clone()),
            Self::LlmEvent(value) => ObservedTurnEventRecord::LlmEvent(value.clone()),
            Self::LlmResponse(record) => {
                ObservedTurnEventRecord::LlmResponse(llm_response_record(record))
            }
            Self::ToolRequested(record) => {
                ObservedTurnEventRecord::ToolRequested(record.to_record())
            }
            Self::ToolCompleted(record) => {
                ObservedTurnEventRecord::ToolCompleted(record.to_record())
            }
            Self::ToolFailed(record) => ObservedTurnEventRecord::ToolFailed(record.to_record()),
            Self::MessageUpdated(record) => {
                ObservedTurnEventRecord::MessageUpdated(record.to_record())
            }
            Self::TurnFinished(record) => ObservedTurnEventRecord::TurnFinished(record.to_record()),
        }
    }
}

impl ToRecord for AgentTurnArtifact {
    type Record = PersistedAgentTurnArtifactRecord;

    fn to_record(&self) -> Self::Record {
        PersistedAgentTurnArtifactRecord {
            task_id: self.task_id.clone(),
            selected_model: self.selected_model.to_string(),
            model_route: None,
            issue_prompt: self.issue_prompt.clone(),
            user_message_id: self.user_message_id.clone(),
            events: self.events.iter().map(ToRecord::to_record).collect(),
            prompt_debug: self.prompt_debug.clone(),
            terminal_record: self.terminal_record.as_ref().map(ToRecord::to_record),
            final_assistant_message: self
                .final_assistant_message
                .as_ref()
                .map(ToRecord::to_record),
            patch_artifact: self.patch_artifact.to_record(),
            llm_prompt: self.llm_prompt.iter().map(request_message_record).collect(),
            llm_response: self.llm_response.clone(),
        }
    }
}

pub(crate) fn llm_response_record(record: &LlmResponseRecord) -> PersistedLlmResponseRecord {
    PersistedLlmResponseRecord {
        content: record.content.clone(),
        model: record.model.clone(),
        usage: record.usage.map(token_usage_record),
        finish_reason: record.finish_reason.as_ref().map(finish_reason_record),
        metadata: record.metadata.as_ref().map(llm_metadata_record),
    }
}

pub(crate) fn request_message_record(message: &RequestMessage) -> PersistedRequestMessageRecord {
    PersistedRequestMessageRecord {
        role: match message.role {
            ploke_llm::manager::Role::User => RequestRoleRecord::User,
            ploke_llm::manager::Role::Assistant => RequestRoleRecord::Assistant,
            ploke_llm::manager::Role::System => RequestRoleRecord::System,
            ploke_llm::manager::Role::Tool => RequestRoleRecord::Tool,
        },
        content: message.content.clone(),
        tool_call_id: message.tool_call_id.as_ref().map(ToString::to_string),
        tool_calls: message
            .tool_calls
            .as_ref()
            .map(|calls| calls.iter().map(provider_tool_call_record).collect()),
    }
}

pub(crate) fn provider_tool_call_record(
    call: &ploke_llm::response::ToolCall,
) -> ProviderToolCallRecord {
    ProviderToolCallRecord {
        call_id: call.call_id.to_string(),
        call_type: FunctionCallMarker,
        function: ProviderFunctionCallRecord {
            name: call.function.name,
            arguments: call.function.arguments.as_str().into(),
        },
    }
}

pub(crate) fn token_usage_record(
    usage: ploke_llm::response::TokenUsage,
) -> PersistedTokenUsageRecord {
    PersistedTokenUsageRecord {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
    }
}

pub(crate) fn finish_reason_record(
    reason: &ploke_llm::response::FinishReason,
) -> FinishReasonRecord {
    match reason {
        ploke_llm::response::FinishReason::Stop => FinishReasonRecord::Stop,
        ploke_llm::response::FinishReason::Length => FinishReasonRecord::Length,
        ploke_llm::response::FinishReason::ContentFilter => FinishReasonRecord::ContentFilter,
        ploke_llm::response::FinishReason::ToolCalls => FinishReasonRecord::ToolCalls,
        ploke_llm::response::FinishReason::MalformedFunctionCall => {
            FinishReasonRecord::MalformedFunctionCall
        }
        ploke_llm::response::FinishReason::UnexpectedToolCall => {
            FinishReasonRecord::UnexpectedToolCall
        }
        ploke_llm::response::FinishReason::Timeout => FinishReasonRecord::Timeout,
        ploke_llm::response::FinishReason::Error(message) => {
            FinishReasonRecord::Error(message.clone())
        }
    }
}

pub(crate) fn llm_metadata_record(
    metadata: &ploke_llm::types::meta::LLMMetadata,
) -> PersistedLlmMetadataRecord {
    PersistedLlmMetadataRecord {
        model: metadata.model.clone(),
        usage: token_usage_record(metadata.usage),
        finish_reason: finish_reason_record(&metadata.finish_reason),
        processing_time: metadata.processing_time,
        cost: metadata.cost,
        performance: PersistedPerformanceMetricsRecord {
            tokens_per_second: metadata.performance.tokens_per_second,
            time_to_first_token: metadata.performance.time_to_first_token,
            queue_time: metadata.performance.queue_time,
        },
    }
}

pub(crate) fn tool_ui_payload_record(
    payload: &ploke_tui::tools::ToolUiPayload,
) -> ToolUiPayloadRecord {
    ToolUiPayloadRecord {
        tool: payload.tool,
        call_id: payload.call_id.to_string(),
        request_id: payload.request_id.map(|value| value.to_string()),
        proposal_id: payload.proposal_id.map(|value| value.to_string()),
        summary: payload.summary.clone(),
        fields: payload
            .fields
            .iter()
            .map(|field| ToolUiFieldRecord {
                name: field.name.to_string(),
                value: field.value.to_string(),
            })
            .collect(),
        details: payload.details.clone(),
        verbosity: match payload.verbosity {
            ploke_tui::tools::ToolVerbosity::Minimal => ToolVerbosityRecord::Minimal,
            ploke_tui::tools::ToolVerbosity::Normal => ToolVerbosityRecord::Normal,
            ploke_tui::tools::ToolVerbosity::Verbose => ToolVerbosityRecord::Verbose,
        },
        error: payload.error.as_ref().map(tool_error_wire_record),
        error_code: payload.error_code.map(tool_error_code_record),
    }
}

pub(crate) fn tool_error_wire_record(
    error: &ploke_tui::tools::ToolErrorWire,
) -> ToolErrorWireRecord {
    ToolErrorWireRecord {
        user: error.user.clone(),
        llm: ToolLlmErrorPayloadRecord {
            ok: error.llm.ok,
            tool: error.llm.tool,
            code: tool_error_code_record(error.llm.code),
            field: error.llm.field.clone(),
            expected: error.llm.expected.clone(),
            received: error.llm.received.clone(),
            message: error.llm.message.clone(),
            snippet: error.llm.snippet.clone(),
            retry_hint: error.llm.retry_hint.clone(),
            retry_context: error
                .llm
                .retry_context
                .as_ref()
                .map(tool_retry_context_record),
        },
        system: error.system.clone(),
    }
}

pub(crate) fn tool_retry_context_record(
    context: &ploke_tui::tools::ToolRetryContext,
) -> ToolRetryContextRecord {
    ToolRetryContextRecord {
        fields: context
            .fields
            .iter()
            .map(|field| ToolRetryContextFieldRecord {
                name: field.name.clone(),
                value: match &field.value {
                    ploke_tui::tools::ToolRetryContextValue::Null => {
                        ToolRetryContextValueRecord::Null
                    }
                    ploke_tui::tools::ToolRetryContextValue::Bool(value) => {
                        ToolRetryContextValueRecord::Bool(*value)
                    }
                    ploke_tui::tools::ToolRetryContextValue::Number(value) => {
                        ToolRetryContextValueRecord::Number(value.clone())
                    }
                    ploke_tui::tools::ToolRetryContextValue::String(value) => {
                        ToolRetryContextValueRecord::String(value.clone())
                    }
                    ploke_tui::tools::ToolRetryContextValue::StringList(value) => {
                        ToolRetryContextValueRecord::StringList(value.clone())
                    }
                },
            })
            .collect(),
    }
}

pub(crate) fn tool_error_code_record(code: ploke_tui::tools::ToolErrorCode) -> ToolErrorCodeRecord {
    match code {
        ploke_tui::tools::ToolErrorCode::FieldTooLarge => ToolErrorCodeRecord::FieldTooLarge,
        ploke_tui::tools::ToolErrorCode::WrongType => ToolErrorCodeRecord::WrongType,
        ploke_tui::tools::ToolErrorCode::MissingField => ToolErrorCodeRecord::MissingField,
        ploke_tui::tools::ToolErrorCode::MalformedDiff => ToolErrorCodeRecord::MalformedDiff,
        ploke_tui::tools::ToolErrorCode::InvalidFormat => ToolErrorCodeRecord::InvalidFormat,
        ploke_tui::tools::ToolErrorCode::Io => ToolErrorCodeRecord::Io,
        ploke_tui::tools::ToolErrorCode::Timeout => ToolErrorCodeRecord::Timeout,
        ploke_tui::tools::ToolErrorCode::Internal => ToolErrorCodeRecord::Internal,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurnArtifact {
    pub task_id: String,
    pub selected_model: ModelId,
    pub issue_prompt: String,
    pub user_message_id: String,
    pub events: Vec<ObservedTurnEvent>,
    pub prompt_debug: Option<String>,
    pub terminal_record: Option<TurnFinishedRecord>,
    pub final_assistant_message: Option<MessageSnapshotRecord>,
    pub patch_artifact: PatchArtifact,
    /// The prompt sent to the LLM (captured from PromptConstructed event).
    /// This is what the LLM actually sees, including conversation history and context.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub llm_prompt: Vec<RequestMessage>,
    /// The LLM's response content (captured from Response event).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub llm_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MultiSweBenchSubmissionRecord {
    pub org: String,
    pub repo: String,
    pub number: u64,
    pub fix_patch: String,
}

pub(crate) fn truncate_preview(input: &str, max_chars: usize) -> String {
    let total = input.chars().count();
    if total <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}...<truncated {} chars>", total - max_chars)
}

pub(crate) fn build_agent_issue_prompt(prepared: &PreparedSingleRun) -> String {
    let mut out = String::new();
    out.push_str("Solve the following benchmark issue.\n\n");
    out.push_str(&format!("Task id: {}\n", prepared.task_id));
    out.push_str(&format!(
        "Repository root: {}\n",
        prepared.repo_root.display()
    ));
    if let Some(base_sha) = &prepared.base_sha {
        out.push_str(&format!("Base SHA: {}\n", base_sha));
    }
    if let Some(title) = &prepared.issue.title {
        out.push_str(&format!("Title: {}\n", title));
    }
    out.push('\n');
    if let Some(body) = &prepared.issue.body {
        out.push_str(body.trim());
        out.push('\n');
    }
    out.push_str("\nUse the repository tools to inspect the code, make the minimal fix, and finish by producing the patch output.");
    out.push_str("\n\nValidation reporting rules:\n");
    out.push_str("- Prefer package-qualified cargo validation for the package(s) touched by your patch. A bare cargo command may run against the focused crate and is not workspace-wide evidence.\n");
    out.push_str("- In the final response, name the exact cargo command and manifest/package shown by the tool result. Do not call a run workspace-wide unless the tool result used the intended workspace/package target.\n");
    out.push_str("- Do not claim formatting or cargo fmt was checked unless a formatting tool result exists; the cargo tool only runs check/test.");
    out
}

pub(crate) fn snapshot_message(
    state: &Arc<AppState>,
    message_event: ploke_tui::app_state::events::MessageUpdatedEvent,
) -> Option<MessageSnapshotRecord> {
    let chat = state.chat.0.try_read().ok()?;
    let msg = chat.messages.get(&message_event.0)?;
    Some(MessageSnapshotRecord {
        id: message_event.0.to_string(),
        kind: msg.kind.to_string(),
        status: msg.status.to_string(),
        tool_call_id: msg.tool_call_id.as_ref().map(ToString::to_string),
        content_len: msg.content.chars().count(),
        content_preview: truncate_preview(&msg.content, 1_500),
    })
}

pub(crate) async fn collect_patch_artifact(state: &Arc<AppState>) -> PatchArtifact {
    collect_patch_artifact_with_expected(state, &[])
        .await
        .unwrap_or_else(|_| PatchArtifact {
            edit_proposals: Vec::new(),
            create_proposals: Vec::new(),
            applied: false,
            all_proposals_applied: false,
            expected_file_changes: Vec::new(),
            any_expected_file_changed: false,
            all_expected_files_changed: false,
        })
}

#[derive(Debug, Clone)]
pub(crate) struct ExpectedFileBaseline {
    path: PathBuf,
    absolute_path: PathBuf,
    existed_before: bool,
    before_sha256: Option<String>,
}

pub(crate) async fn collect_patch_artifact_with_expected(
    state: &Arc<AppState>,
    expected_file_baselines: &[ExpectedFileBaseline],
) -> Result<PatchArtifact, PrepareError> {
    let edit_proposals = {
        let proposals = state.proposals.read().await;
        proposals
            .values()
            .map(|proposal| ProposalSnapshotRecord {
                request_id: proposal.request_id.to_string(),
                call_id: proposal.call_id.to_string(),
                status: proposal_status_label(&proposal.status).to_string(),
                files: proposal
                    .files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
                preview_mode: match &proposal.preview {
                    DiffPreview::CodeBlocks { .. } => "codeblock".to_string(),
                    DiffPreview::UnifiedDiff { .. } => "diff".to_string(),
                },
            })
            .collect::<Vec<_>>()
    };

    let create_proposals = {
        let proposals = state.create_proposals.read().await;
        proposals
            .values()
            .map(|proposal| ProposalSnapshotRecord {
                request_id: proposal.request_id.to_string(),
                call_id: proposal.call_id.to_string(),
                status: proposal_status_label(&proposal.status).to_string(),
                files: proposal
                    .files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
                preview_mode: match &proposal.preview {
                    DiffPreview::CodeBlocks { .. } => "codeblock".to_string(),
                    DiffPreview::UnifiedDiff { .. } => "diff".to_string(),
                },
            })
            .collect::<Vec<_>>()
    };

    let has_any_proposals = !edit_proposals.is_empty() || !create_proposals.is_empty();
    let applied = edit_proposals.iter().any(proposal_status_is_mutation)
        || create_proposals.iter().any(proposal_status_is_mutation);
    let all_proposals_applied = has_any_proposals
        && edit_proposals.iter().all(|p| p.status == "Applied")
        && create_proposals.iter().all(|p| p.status == "Applied");
    let expected_file_changes = collect_expected_file_changes(expected_file_baselines)?;
    let any_expected_file_changed = expected_file_changes.iter().any(|record| record.changed);
    let all_expected_files_changed = !expected_file_changes.is_empty()
        && expected_file_changes.iter().all(|record| record.changed);

    Ok(PatchArtifact {
        edit_proposals,
        create_proposals,
        applied,
        all_proposals_applied,
        expected_file_changes,
        any_expected_file_changed,
        all_expected_files_changed,
    })
}

pub(crate) fn proposal_status_label(status: &EditProposalStatus) -> &'static str {
    match status {
        EditProposalStatus::Pending => "Pending",
        EditProposalStatus::Approved => "Approved",
        EditProposalStatus::Denied => "Denied",
        EditProposalStatus::Applied => "Applied",
        EditProposalStatus::PartiallyApplied(_) => "PartiallyApplied",
        EditProposalStatus::Failed(_) => "Failed",
        EditProposalStatus::Stale(_) => "Stale",
    }
}

pub(crate) fn proposal_status_is_mutation(proposal: &ProposalSnapshotRecord) -> bool {
    matches!(proposal.status.as_str(), "Applied" | "PartiallyApplied")
}

pub(crate) fn expected_patch_files(prepared: &PreparedSingleRun) -> Vec<PathBuf> {
    match &prepared.source {
        Some(RunSource::MultiSweBench(source)) => source.expected_patch_files.clone(),
        None => Vec::new(),
    }
}

pub(crate) fn maybe_build_msb_submission_record(
    prepared: &PreparedSingleRun,
    run_arm: &RunArm,
) -> Result<Option<MultiSweBenchSubmissionRecord>, PrepareError> {
    if run_arm.role != RunArmRole::Treatment {
        return Ok(None);
    }
    let fix_patch = collect_submission_fix_patch(prepared)?;
    match &prepared.source {
        Some(RunSource::MultiSweBench(source)) => Ok(Some(MultiSweBenchSubmissionRecord {
            org: source.org.clone(),
            repo: source.repo.clone(),
            number: source.number,
            fix_patch,
        })),
        None => Ok(None),
    }
}

pub(crate) fn ensure_msb_submission_patch_evidence(
    fix_patch: &str,
    patch_artifact: Option<&PatchArtifact>,
) -> Result<(), PrepareError> {
    if fix_patch.trim().is_empty() {
        return Ok(());
    }

    let Some(patch_artifact) = patch_artifact else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "non-empty MBE fix_patch requires same-run patch artifact evidence".to_string(),
        });
    };

    if !patch_artifact.applied {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "non-empty MBE fix_patch requires an applied proposal in the same run"
                .to_string(),
        });
    }

    if patch_artifact.expected_file_changes.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "non-empty MBE fix_patch requires expected benchmark file evidence".to_string(),
        });
    }

    if !patch_artifact.any_expected_file_changed {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "non-empty MBE fix_patch requires at least one expected benchmark file change"
                .to_string(),
        });
    }

    Ok(())
}

pub(crate) fn record_packaging_failure(
    run_record: &mut RunRecord,
    registration: &mut crate::inner::registry::RunRegistration,
    packaging_started_at: String,
    detail: String,
) {
    run_record.phases.packaging = Some(PackagingPhase {
        started_at: packaging_started_at,
        ended_at: chrono::Utc::now().to_rfc3339(),
        submission_artifact_state: SubmissionArtifactState::Missing,
        msb_submission_path: None,
        patch_projection_path: None,
        patch_projection_check_state: PatchProjectionCheckState::NotApplicable,
    });
    registration.update_submission_status(None);
    registration.update_phase(
        RunLifecyclePhase::Packaging,
        RunPhaseStatus::InProgress,
        Some(detail),
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenMsbSubmissionArtifact {
    pub(crate) path: PathBuf,
    pub(crate) fix_patch: String,
    pub(crate) patch_projection_path: PathBuf,
    pub(crate) patch_projection_check_state: PatchProjectionCheckState,
}

pub(crate) fn write_msb_submission_artifact(
    prepared: &PreparedSingleRun,
    run_arm: &RunArm,
    run_output_dir: &Path,
    run_manifest_path: &Path,
    record_path: &Path,
    patch_artifact: Option<&PatchArtifact>,
) -> Result<Option<WrittenMsbSubmissionArtifact>, PrepareError> {
    let Some(record) = maybe_build_msb_submission_record(prepared, run_arm)? else {
        return Ok(None);
    };
    ensure_msb_submission_patch_evidence(&record.fix_patch, patch_artifact)?;

    let path = run_output_dir.join("multi-swe-bench-submission.jsonl");
    write_jsonl_line(&path, &record)?;
    let patch_projection_path = write_benchmark_patch_projection(
        prepared,
        &path,
        &record.fix_patch,
        run_output_dir,
        run_manifest_path,
        record_path,
    )?;
    let patch_projection_check_state = PatchProjectionCheckState::Passed;
    Ok(Some(WrittenMsbSubmissionArtifact {
        path,
        fix_patch: record.fix_patch,
        patch_projection_path,
        patch_projection_check_state,
    }))
}

pub(crate) fn write_benchmark_patch_projection(
    prepared: &PreparedSingleRun,
    submission_path: &Path,
    fix_patch: &str,
    run_output_dir: &Path,
    run_manifest_path: &Path,
    record_path: &Path,
) -> Result<PathBuf, PrepareError> {
    let Some(RunSource::MultiSweBench(source)) = &prepared.source else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "benchmark patch projection requires an MBE source".to_string(),
        });
    };
    let projection_path = run_output_dir.join("benchmark-patch-projection.json");
    let patch_sha = hex_sha256(fix_patch.as_bytes());
    let line_count = fix_patch.lines().count() as u64;
    let diff_base = prepared
        .base_sha
        .clone()
        .unwrap_or_else(|| "HEAD".to_string());
    let record = BenchmarkPatchProjectionRecord {
        schema_version: BENCHMARK_PATCH_PROJECTION_SCHEMA_V1.to_string(),
        benchmark: MultiSweBenchTarget {
            org: source.org.clone(),
            repo: source.repo.clone(),
            number: source.number,
            instance_id: source.instance_id.clone(),
            base_sha: prepared.base_sha.clone(),
        },
        run: RunArtifactRef {
            run_manifest: run_manifest_path.to_path_buf(),
            run_root: run_output_dir.to_path_buf(),
            record_path: record_path.to_path_buf(),
        },
        candidate: None,
        checkout: BenchmarkCheckoutRef {
            cwd: prepared.repo_root.clone(),
            head_sha: prepared.head_sha.clone(),
        },
        submission: SubmissionPatchRef {
            path: submission_path.to_path_buf(),
            sha256: patch_sha,
            byte_len: fix_patch.len() as u64,
            line_count,
            diff_base,
        },
        check: PatchProjectionCheck::Passed {
            checked_at: chrono::Utc::now().to_rfc3339(),
            detail: "fix_patch exported from the recorded checkout cwd".to_string(),
        },
    };
    write_json(&projection_path, &record)?;
    Ok(projection_path)
}

pub(crate) fn collect_submission_fix_patch(
    prepared: &PreparedSingleRun,
) -> Result<String, PrepareError> {
    let args = if let Some(base_sha) = prepared.base_sha.as_deref() {
        vec!["diff", "--no-ext-diff", "--binary", base_sha, "--"]
    } else {
        vec!["diff", "--no-ext-diff", "--binary", "HEAD", "--"]
    };
    Ok(git_stdout(
        &prepared.repo_root,
        &args,
        format!("git {}", args.join(" ")),
    )?
    .unwrap_or_default())
}

pub(crate) fn snapshot_expected_files(
    repo_root: &Path,
    expected_files: &[PathBuf],
) -> Result<Vec<ExpectedFileBaseline>, PrepareError> {
    expected_files
        .iter()
        .map(|relative_path| {
            let absolute_path = repo_root.join(relative_path);
            let existed_before = absolute_path.exists();
            let before_sha256 = hash_file_contents(&absolute_path)?;
            Ok(ExpectedFileBaseline {
                path: relative_path.clone(),
                absolute_path,
                existed_before,
                before_sha256,
            })
        })
        .collect()
}

pub(crate) fn collect_expected_file_changes(
    expected_file_baselines: &[ExpectedFileBaseline],
) -> Result<Vec<ExpectedFileChangeRecord>, PrepareError> {
    expected_file_baselines
        .iter()
        .map(|baseline| {
            let after_sha256 = hash_file_contents(&baseline.absolute_path)?;
            let exists_after = baseline.absolute_path.exists();
            Ok(ExpectedFileChangeRecord {
                path: baseline.path.display().to_string(),
                existed_before: baseline.existed_before,
                exists_after,
                before_sha256: baseline.before_sha256.clone(),
                after_sha256: after_sha256.clone(),
                changed: baseline.before_sha256 != after_sha256,
            })
        })
        .collect()
}

pub(crate) fn default_eval_embedding_model_id() -> ModelId {
    OPENROUTER_CODESTRAL_MODEL
        .parse()
        .expect("eval embedding model id must parse")
}

pub(crate) fn eval_embedding_registry_path() -> Result<PathBuf, PrepareError> {
    layout::embedding_model_registry_file()
}

pub(crate) fn embedding_preflight_cache() -> &'static Mutex<HashMap<String, u32>> {
    EMBEDDING_PREFLIGHT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn eval_embedding_provider_prefs(
    provider: Option<&ProviderKey>,
) -> Option<EmbeddingProviderPrefs> {
    provider.map(|provider| {
        EmbeddingProviderPrefs::from_base_provider_prefs(
            ProviderPreferences::default()
                .with_order(std::iter::once(ProviderSlug::new(provider.slug.as_str())))
                .with_allow_fallbacks(true),
        )
    })
}

pub(crate) fn eval_embedding_provider_order(provider: Option<&ProviderKey>) -> Option<Vec<String>> {
    provider.map(|provider| vec![provider.slug.as_str().to_string()])
}

pub(crate) fn eval_embedding_preflight_request(
    model: &ResponseItem,
    provider: Option<&ProviderKey>,
) -> EmbeddingRequest<OpenRouter> {
    EmbeddingRequest::<OpenRouter> {
        model: model.id.clone(),
        input: EmbeddingInput::Single("ploke eval embedding preflight".to_string()),
        router: OpenRouterEmbeddingFields {
            input_type: Some("code-snippet".into()),
            provider: eval_embedding_provider_prefs(provider),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) async fn load_eval_embedding_registry(
    client: &reqwest::Client,
    registry_path: &Path,
) -> Result<ploke_llm::request::models::Response, PrepareError> {
    match OpenRouter::fetch_and_write_embedding_models_registry(
        client,
        EmbClientConfig::new().with_timeout(Duration::from_secs(15)),
        registry_path,
    )
    .await
    {
        Ok(registry) => Ok(registry),
        Err(fetch_err) => OpenRouter::load_embedding_models_registry(registry_path).map_err(
            |load_err| PrepareError::DatabaseSetup {
                phase: "load_embedding_model_registry",
                detail: format!(
                    "failed to refresh embedding registry '{}': {fetch_err}; failed to load cached snapshot: {load_err}",
                    registry_path.display()
                ),
            },
        ),
    }
}

pub(crate) fn resolve_embedding_model_from_registry(
    registry: &ploke_llm::request::models::Response,
    registry_path: &Path,
    requested_model: &ModelId,
) -> Result<ResponseItem, PrepareError> {
    registry
        .data
        .iter()
        .find(|item| item.id == *requested_model)
        .cloned()
        .ok_or_else(|| PrepareError::UnknownModelInRegistry {
            model: requested_model.to_string(),
            path: registry_path.to_path_buf(),
        })
}

pub(crate) fn format_embedding_preflight_error(
    failing_model: &ModelId,
    registry_path: Option<&Path>,
    source: &str,
    suggestions: &[String],
) -> String {
    let mut detail = format!(
        "embedding preflight failed for '{}': {}",
        failing_model, source
    );
    if let Some(path) = registry_path {
        detail.push_str(&format!(
            ". embedding registry snapshot path: '{}'",
            path.display()
        ));
    }
    if !suggestions.is_empty() {
        detail.push_str(&format!(
            ". suggested alternatives: {}",
            suggestions.join(", ")
        ));
    }
    detail.push_str(
        ". choose one embedding model and rerun; do not mix embedding models within the same run or target crate",
    );
    detail
}

pub(crate) async fn resolve_eval_embedding_selection(
    requested_model_id: Option<&str>,
    requested_provider: Option<&ProviderKey>,
) -> Result<EvalEmbeddingSelection, PrepareError> {
    let registry_path = eval_embedding_registry_path()?;
    let client = reqwest::Client::new();
    let registry = load_eval_embedding_registry(&client, &registry_path).await?;
    if requested_model_id.is_none() && requested_provider.is_none() {
        if let Ok(Some(resolved)) = OpenRouter::resolve_live_text_embedding_model(
            &client,
            EmbClientConfig::new().with_timeout(Duration::from_secs(15)),
            Some(OPENROUTER_CODESTRAL_MODEL),
            "fn smoke_eval_probe() {}",
        )
        .await
        {
            let model = resolve_embedding_model_from_registry(
                &registry,
                &registry_path,
                &resolved.model_id,
            )?;
            let cache_key = format!("{}::<auto>", model.id);
            embedding_preflight_cache()
                .lock()
                .expect("embedding preflight cache poisoned")
                .insert(cache_key, resolved.dims);
            return Ok(EvalEmbeddingSelection {
                model,
                provider: None,
                dimensions: resolved.dims,
            });
        }
    }

    let requested_model = parse_requested_model_id(requested_model_id)?
        .unwrap_or_else(default_eval_embedding_model_id);
    let model = resolve_embedding_model_from_registry(&registry, &registry_path, &requested_model)?;

    let cache_key = format!(
        "{}::{}",
        model.id,
        requested_provider
            .map(|provider| provider.slug.as_str())
            .unwrap_or("<auto>")
    );
    if let Some(dimensions) = embedding_preflight_cache()
        .lock()
        .expect("embedding preflight cache poisoned")
        .get(&cache_key)
        .copied()
    {
        return Ok(EvalEmbeddingSelection {
            model,
            provider: requested_provider.cloned(),
            dimensions,
        });
    }

    let request = eval_embedding_preflight_request(&model, requested_provider);
    let preflight = <OpenRouter as HasEmbeddings>::fetch_embeddings(&client, &request).await;
    match preflight {
        Ok(response) => {
            let dimensions = response.dims().ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "embedding_model_preflight",
                detail: format!("embedding preflight returned no vectors for '{}'", model.id),
            })? as u32;
            embedding_preflight_cache()
                .lock()
                .expect("embedding preflight cache poisoned")
                .insert(cache_key, dimensions);
            Ok(EvalEmbeddingSelection {
                model,
                provider: requested_provider.cloned(),
                dimensions,
            })
        }
        Err(err) => {
            let suggestions =
                OpenRouter::suggest_embedding_model_alternatives(&registry, &request.model, 5)
                    .into_iter()
                    .map(|item| item.id.to_string())
                    .collect::<Vec<_>>();

            Err(PrepareError::DatabaseSetup {
                phase: "embedding_model_preflight",
                detail: format_embedding_preflight_error(
                    &request.model,
                    Some(&registry_path),
                    &err.to_string(),
                    &suggestions,
                ),
            })
        }
    }
}

pub(crate) fn eval_embedding_config(selection: &EvalEmbeddingSelection) -> OpenRouterConfig {
    OpenRouterConfig {
        model: selection.model.id.to_string(),
        dimensions: Some(selection.dimensions as usize),
        request_dimensions: None,
        snippet_batch_size: 100,
        max_in_flight: 1,
        requests_per_second: Some(1),
        max_attempts: 3,
        initial_backoff_ms: 250,
        max_backoff_ms: 10_000,
        input_type: Some("code-snippet".into()),
        provider_order: eval_embedding_provider_order(selection.provider.as_ref()),
        allow_fallbacks: selection.provider.as_ref().map(|_| true),
        timeout_secs: 30,
        truncate_policy: TruncatePolicy::Truncate,
    }
}

pub(crate) fn eval_embedding_processor(
    selection: &EvalEmbeddingSelection,
) -> Result<EmbeddingProcessor, PrepareError> {
    info!(
        model = %selection.model.id,
        dimensions = selection.dimensions,
        provider = ?selection.provider.as_ref().map(|provider| provider.slug.as_str()),
        "building eval embedding processor"
    );
    let backend = OpenRouterBackend::new(&eval_embedding_config(selection)).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "init_codestral_embedder",
            detail: err.to_string(),
        }
    })?;
    info!("eval embedding processor initialized");
    Ok(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(
        backend,
    )))
}

pub(crate) fn eval_embedding_set(selection: &EvalEmbeddingSelection) -> EmbeddingSet {
    EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(&selection.model.id.to_string()),
        EmbeddingShape::new_dims_default(selection.dimensions),
    )
}

pub(crate) fn activate_eval_embedding_runtime(
    state: &Arc<AppState>,
    selection: &EvalEmbeddingSelection,
) -> Result<(), PrepareError> {
    info!(model = %selection.model.id, "activating eval embedding set");
    let processor = Arc::new(eval_embedding_processor(selection)?);
    state
        .embedder
        .activate(&state.db, eval_embedding_set(selection), processor)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "activate_codestral_embedding_set",
            detail: err.to_string(),
        })?;
    info!("eval embedding set activated");
    Ok(())
}

pub(crate) fn hash_file_contents(path: &Path) -> Result<Option<String>, PrepareError> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut digest = Sha256::new();
            digest.update(bytes);
            Ok(Some(format!("{:x}", digest.finalize())))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(PrepareError::DatabaseSetup {
            phase: "hash_expected_file",
            detail: format!("failed to hash '{}': {err}", path.display()),
        }),
    }
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn build_agent_validation_audit(
    prepared: &PreparedSingleRun,
    artifact: &AgentTurnArtifact,
) -> AgentValidationAudit {
    let changed_paths = validation_changed_paths(&prepared.repo_root, &artifact.patch_artifact);
    let patch_quality = build_patch_quality_audit(artifact, &changed_paths);
    let mut cargo_requests: HashMap<String, CargoRequestProjection> = HashMap::new();
    let mut cargo_calls = Vec::new();

    for (event_index, event) in artifact.events.iter().enumerate() {
        match event {
            ObservedTurnEvent::ToolRequested(record) if record.tool == "cargo" => {
                cargo_requests.insert(
                    record.call_id.clone(),
                    decode_cargo_request_projection(&record.arguments),
                );
            }
            ObservedTurnEvent::ToolCompleted(record) if record.tool == "cargo" => {
                let Ok(result) = serde_json::from_str::<CargoResultProjection>(&record.content)
                else {
                    continue;
                };
                let request = cargo_requests.get(&record.call_id);
                let covers_changed_files = cargo_manifest_covers_changed_paths(
                    &prepared.repo_root,
                    &result.manifest_path,
                    &changed_paths,
                );
                cargo_calls.push(CargoValidationCall {
                    event_index,
                    call_id: record.call_id.clone(),
                    command: result.command,
                    requested_scope: request.and_then(|req| req.requested_scope.clone()),
                    resolved_scope: result.scope,
                    package: request.and_then(|req| req.package.clone()),
                    manifest_path: result.manifest_path,
                    ok: result.ok,
                    covers_changed_files,
                });
            }
            _ => {}
        }
    }

    let final_cargo_call = cargo_calls.last().cloned();
    let successful_cargo_covering_changed_files = cargo_calls
        .iter()
        .any(|call| call.ok && call.covers_changed_files);
    let final_cargo_covers_changed_files = final_cargo_call
        .as_ref()
        .is_some_and(|call| call.covers_changed_files);
    let fmt_check_observed = false;
    let mut warnings = Vec::new();
    if !changed_paths.is_empty() {
        if cargo_calls.is_empty() {
            warnings.push("no cargo check/test tool result recorded for changed files".to_string());
        } else if !successful_cargo_covering_changed_files {
            warnings.push(
                "no successful cargo check/test result covered the changed files".to_string(),
            );
        }
        if let Some(call) = final_cargo_call.as_ref()
            && !call.covers_changed_files
        {
            warnings.push(format!(
                "final cargo {} resolved to manifest '{}' which does not cover changed paths: {}",
                call.command,
                call.manifest_path,
                changed_paths.join(", ")
            ));
        }
    }
    if !fmt_check_observed {
        warnings.push(
            "no formatting check evidence recorded; do not claim cargo fmt/rustfmt passed"
                .to_string(),
        );
    }
    warnings.extend(patch_quality.red_flags.iter().cloned());

    AgentValidationAudit {
        schema_version: VALIDATION_AUDIT_SCHEMA_V2.to_string(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        changed_paths,
        patch_quality,
        cargo_calls,
        final_cargo_call,
        successful_cargo_covering_changed_files,
        final_cargo_covers_changed_files,
        fmt_check_observed,
        warnings,
    }
}

pub(crate) fn validation_changed_paths(
    repo_root: &Path,
    patch_artifact: &PatchArtifact,
) -> Vec<String> {
    let mut changed_paths = Vec::new();
    for path in patch_artifact
        .expected_file_changes
        .iter()
        .filter(|change| change.changed)
        .map(|change| change.path.as_str())
    {
        push_unique_changed_path(&mut changed_paths, path.to_string());
    }
    for proposal in patch_artifact
        .edit_proposals
        .iter()
        .chain(patch_artifact.create_proposals.iter())
        .filter(|proposal| proposal_status_is_mutation(proposal))
    {
        for path in &proposal.files {
            push_unique_changed_path(
                &mut changed_paths,
                normalize_validation_changed_path(repo_root, path),
            );
        }
    }
    changed_paths
}

pub(crate) fn normalize_validation_changed_path(repo_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    if path.is_absolute()
        && let Ok(relative) = path.strip_prefix(repo_root)
    {
        return relative.display().to_string();
    }
    path.display().to_string()
}

pub(crate) fn push_unique_changed_path(changed_paths: &mut Vec<String>, path: String) {
    if !changed_paths.contains(&path) {
        changed_paths.push(path);
    }
}

pub(crate) fn build_patch_quality_audit(
    artifact: &AgentTurnArtifact,
    changed_paths: &[String],
) -> PatchQualityAudit {
    let changed_path_count = changed_paths.len();
    let test_changed_path_count = changed_paths
        .iter()
        .filter(|path| path_looks_test_scoped(path))
        .count();
    let production_changed_path_count = changed_path_count.saturating_sub(test_changed_path_count);

    let mut edit_request_count = 0;
    let mut test_edit_request_count = 0;
    let mut production_edit_request_count = 0;
    let mut expected_output_edit_candidates = Vec::new();

    for (event_index, event) in artifact.events.iter().enumerate() {
        let ObservedTurnEvent::ToolRequested(record) = event else {
            continue;
        };
        for edit in decode_edit_request_projections(&record.tool, &record.arguments) {
            edit_request_count += 1;
            let test_scoped =
                path_looks_test_scoped(&edit.path) || body_looks_test_scoped(&edit.body);
            if test_scoped {
                test_edit_request_count += 1;
            } else {
                production_edit_request_count += 1;
            }

            if edit_looks_like_expected_output_change(&edit.body) {
                expected_output_edit_candidates.push(ExpectedOutputEditCandidate {
                    event_index,
                    call_id: record.call_id.clone(),
                    tool: record.tool.clone(),
                    path: edit.path,
                    evidence: expected_output_evidence(&edit.body),
                });
            }
        }
    }

    let test_only_changed_paths = changed_path_count > 0 && production_changed_path_count == 0;
    let mostly_test_edit_requests =
        edit_request_count > 0 && test_edit_request_count > production_edit_request_count;
    let mut red_flags = Vec::new();
    if test_only_changed_paths {
        red_flags.push("patch changed only test-scoped paths".to_string());
    }
    if mostly_test_edit_requests {
        red_flags.push(format!(
            "edit requests were mostly test-scoped ({test_edit_request_count}/{edit_request_count})"
        ));
    }
    if !expected_output_edit_candidates.is_empty() {
        red_flags.push(
            "edit request looks like a test expected-output/assertion change; inspect whether behavior changed"
                .to_string(),
        );
    }

    PatchQualityAudit {
        changed_path_count,
        test_changed_path_count,
        production_changed_path_count,
        test_only_changed_paths,
        edit_request_count,
        test_edit_request_count,
        production_edit_request_count,
        mostly_test_edit_requests,
        expected_output_edit_candidates,
        red_flags,
    }
}

pub(crate) fn decode_edit_request_projections(
    tool: &str,
    arguments: &ploke_records::tool_contracts::ToolArgumentsJson,
) -> Vec<EditRequestProjection> {
    match arguments.decode_for_tool(tool) {
        PersistedToolCallArguments::Decoded(ToolCallArguments::ApplyCodeEdit(args)) => args
            .edits
            .into_iter()
            .map(|edit| EditRequestProjection {
                path: edit.file,
                body: edit.code,
            })
            .collect(),
        PersistedToolCallArguments::Decoded(ToolCallArguments::InsertRustItem(args)) => {
            vec![EditRequestProjection {
                path: args.file,
                body: args.code,
            }]
        }
        PersistedToolCallArguments::Decoded(ToolCallArguments::CreateFile(args)) => {
            vec![EditRequestProjection {
                path: args.file_path,
                body: args.content,
            }]
        }
        PersistedToolCallArguments::Decoded(ToolCallArguments::NsPatch(args)) => args
            .patches
            .into_iter()
            .map(|patch| EditRequestProjection {
                path: patch.file,
                body: format!("{}\n{}", patch.reasoning, patch.diff),
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn path_looks_test_scoped(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    lower.contains("/tests/")
        || lower.contains("/test/")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_tests.rs")
        || lower.ends_with(".test.rs")
        || lower.ends_with(".tests.rs")
}

pub(crate) fn body_looks_test_scoped(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("#[test]")
        || lower.contains("assert_eq!")
        || lower.contains("assert_ne!")
        || lower.contains("expected")
        || lower.contains("insta::")
}

pub(crate) fn edit_looks_like_expected_output_change(body: &str) -> bool {
    if !body_looks_test_scoped(body) {
        return false;
    }
    let removed_expected = body.lines().any(diff_removed_line_looks_expected);
    let added_expected = body.lines().any(diff_added_line_looks_expected);
    removed_expected && added_expected
}

pub(crate) fn diff_removed_line_looks_expected(line: &str) -> bool {
    line.starts_with('-') && !line.starts_with("---") && line_looks_expected_value(line)
}

pub(crate) fn diff_added_line_looks_expected(line: &str) -> bool {
    line.starts_with('+') && !line.starts_with("+++") && line_looks_expected_value(line)
}

pub(crate) fn line_looks_expected_value(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains('"')
        || lower.contains("expected")
        || lower.contains("assert_eq!")
        || lower.contains("snapshot")
}

pub(crate) fn expected_output_evidence(body: &str) -> String {
    body.lines()
        .filter(|line| {
            diff_removed_line_looks_expected(line) || diff_added_line_looks_expected(line)
        })
        .take(4)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn decode_cargo_request_projection(
    arguments: &ploke_records::tool_contracts::ToolArgumentsJson,
) -> CargoRequestProjection {
    match arguments.decode_for_tool("cargo") {
        PersistedToolCallArguments::Decoded(ToolCallArguments::Cargo(args)) => {
            CargoRequestProjection {
                requested_scope: Some(cargo_scope_label(args.scope).to_string()),
                package: args.package,
            }
        }
        _ => CargoRequestProjection {
            requested_scope: None,
            package: None,
        },
    }
}

pub(crate) fn cargo_scope_label(scope: ploke_tui::tools::cargo::CargoScope) -> &'static str {
    match scope {
        ploke_tui::tools::cargo::CargoScope::Focused => "focused",
        ploke_tui::tools::cargo::CargoScope::Workspace => "workspace",
    }
}

pub(crate) fn cargo_manifest_covers_changed_paths(
    repo_root: &Path,
    manifest_path: &str,
    changed_paths: &[String],
) -> bool {
    if changed_paths.is_empty() {
        return false;
    }
    let manifest_path = Path::new(manifest_path);
    let workspace_manifest = repo_root.join("Cargo.toml");
    if manifest_path == workspace_manifest {
        return true;
    }
    let Some(manifest_dir) = manifest_path.parent() else {
        return false;
    };
    changed_paths.iter().any(|changed_path| {
        let changed_path = Path::new(changed_path);
        let absolute_changed_path = if changed_path.is_absolute() {
            changed_path.to_path_buf()
        } else {
            repo_root.join(changed_path)
        };
        absolute_changed_path.starts_with(manifest_dir)
    })
}

pub(crate) fn starting_db_cache_metadata(
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
) -> StartingDbCacheMetadata {
    let embedding = eval_embedding_set(embedding_selection);
    StartingDbCacheMetadata {
        version: STARTING_DB_CACHE_VERSION,
        task_id: prepared.task_id.clone(),
        repo_root: prepared.repo_root.clone(),
        checkout_sha: prepared
            .base_sha
            .clone()
            .or_else(|| prepared.head_sha.clone()),
        embedding_provider: embedding.provider.to_string(),
        embedding_model: embedding.model.to_string(),
        embedding_dimensions: embedding.dims(),
        embedding_dtype: embedding.shape.dtype_tag().to_string(),
        typed_type_graph: cfg!(feature = "typed_type_graph"),
    }
}

pub(crate) fn starting_db_cache_key_for_metadata(metadata: &StartingDbCacheMetadata) -> String {
    let payload = format!(
        "{version}:{task_id}:{repo_root}:{checkout_sha}:{provider}:{model}:{dims}:{dtype}:{typed_type_graph}",
        version = metadata.version,
        task_id = metadata.task_id,
        repo_root = metadata.repo_root.display(),
        checkout_sha = metadata.checkout_sha.as_deref().unwrap_or("<none>"),
        provider = metadata.embedding_provider,
        model = metadata.embedding_model,
        dims = metadata.embedding_dimensions,
        dtype = metadata.embedding_dtype,
        typed_type_graph = metadata.typed_type_graph,
    );
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

pub(crate) fn starting_db_cache_key(
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
) -> String {
    starting_db_cache_key_for_metadata(&starting_db_cache_metadata(prepared, embedding_selection))
}

pub(crate) fn starting_db_cache_paths_at(
    eval_home: impl AsRef<Path>,
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
) -> StartingDbCachePaths {
    let key = starting_db_cache_key(prepared, embedding_selection);
    let base = eval_home.as_ref().join("cache").join("starting-dbs");
    StartingDbCachePaths {
        snapshot: base.join(format!("{key}.sqlite")),
        metadata: base.join(format!("{key}.json")),
    }
}

pub(crate) fn load_cached_starting_db_at(
    eval_home: impl AsRef<Path>,
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
) -> Result<Option<StartingDbCachePaths>, PrepareError> {
    let paths = starting_db_cache_paths_at(eval_home, prepared, embedding_selection);
    if !paths.snapshot.exists() || !paths.metadata.exists() {
        return Ok(None);
    }

    let text = match fs::read_to_string(&paths.metadata) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PrepareError::ReadStartingDbCacheMetadata {
                path: paths.metadata.clone(),
                source,
            });
        }
    };
    let metadata: StartingDbCacheMetadata = serde_json::from_str(&text).map_err(|source| {
        PrepareError::ParseStartingDbCacheMetadata {
            path: paths.metadata.clone(),
            source,
        }
    })?;
    if metadata != starting_db_cache_metadata(prepared, embedding_selection) {
        return Ok(None);
    }

    Ok(Some(paths))
}

pub(crate) fn load_cached_starting_db(
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
) -> Result<Option<StartingDbCachePaths>, PrepareError> {
    load_cached_starting_db_at(layout::ploke_eval_home()?, prepared, embedding_selection)
}

pub(crate) async fn persist_starting_db_cache_at(
    eval_home: impl AsRef<Path>,
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
    snapshot_path: &Path,
) -> Result<StartingDbCachePaths, PrepareError> {
    let paths = starting_db_cache_paths_at(eval_home, prepared, embedding_selection);
    if let Some(parent) = paths.snapshot.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            PrepareError::WriteStartingDbCacheSnapshot {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    fs::copy(snapshot_path, &paths.snapshot).map_err(|source| {
        PrepareError::WriteStartingDbCacheSnapshot {
            path: paths.snapshot.clone(),
            source,
        }
    })?;

    let metadata = starting_db_cache_metadata(prepared, embedding_selection);
    let json = serde_json::to_string_pretty(&metadata)
        .map_err(PrepareError::SerializeStartingDbCacheMetadata)?;
    fs::write(&paths.metadata, json).map_err(|source| {
        PrepareError::WriteStartingDbCacheMetadata {
            path: paths.metadata.clone(),
            source,
        }
    })?;

    Ok(paths)
}

pub(crate) async fn persist_starting_db_cache(
    prepared: &PreparedSingleRun,
    embedding_selection: &EvalEmbeddingSelection,
    snapshot_path: &Path,
) -> Result<StartingDbCachePaths, PrepareError> {
    persist_starting_db_cache_at(
        layout::ploke_eval_home()?,
        prepared,
        embedding_selection,
        snapshot_path,
    )
    .await
}
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    let json = serde_json::to_string_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, json).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn write_agent_turn_trace(
    path: &Path,
    artifact: &AgentTurnArtifact,
) -> Result<(), PrepareError> {
    let record = AgentTurnTraceRecord(artifact.to_record());
    write_json(path, &record)
}

pub(crate) fn write_agent_turn_summary(
    path: &Path,
    artifact: &AgentTurnArtifact,
) -> Result<(), PrepareError> {
    let record = AgentTurnSummaryRecord(artifact.to_record());
    write_json(path, &record)
}

pub(crate) fn write_jsonl_line<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    let mut json = serde_json::to_string(value).map_err(PrepareError::Serialize)?;
    json.push('\n');
    fs::write(path, json).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn append_jsonl_blob(path: &Path, blob: &str) -> Result<(), PrepareError> {
    let mut existing = if path.exists() {
        fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?
    } else {
        String::new()
    };
    existing.push_str(blob.trim_end());
    existing.push('\n');
    fs::write(path, existing).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn current_trace_file_offset(path: &Path) -> Result<u64, PrepareError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(source) => Err(PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        }),
    }
}
pub(crate) fn load_prepared_batch(
    batch_manifest: PathBuf,
) -> Result<(PathBuf, PreparedMsbBatch), PrepareError> {
    let manifest_path = canonicalize_batch_file(&batch_manifest)?;
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|source| PrepareError::ReadBatchManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let prepared: PreparedMsbBatch = serde_json::from_str(&manifest_text).map_err(|source| {
        PrepareError::ParseBatchManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;

    Ok((manifest_path, prepared))
}

pub(crate) fn canonicalize_batch_file(path: &Path) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingBatchManifest(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}
