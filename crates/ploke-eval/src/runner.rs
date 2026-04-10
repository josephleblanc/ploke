use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::Database;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_embed::config::{OpenRouterConfig, TruncatePolicy};
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexingStatus};
use ploke_embed::providers::openrouter::OpenRouterBackend;
use ploke_llm::request::models::ResponseItem;
use ploke_llm::router_only::{
    HasEndpoint,
    openrouter::{OpenRouter, OpenRouterModelId},
};
use ploke_llm::{ModelId, ProviderKey, SupportsTools};
use ploke_llm::manager::RequestMessage;
use ploke_tui::AppEvent;
use ploke_tui::llm::{ChatEvt, LlmEvent};
use ploke_tui::app::App;
use ploke_tui::app::commands::harness::TestAppAccessor;
use ploke_tui::app::commands::harness::TestRuntime;
use ploke_tui::app::view::components::model_browser::tool_capable_provider_key;
use ploke_tui::app_state::AppState;
use ploke_tui::app_state::core::ParseFailure;
use ploke_tui::app_state::core::{DiffPreview, EditProposalStatus};
use ploke_tui::app_state::events::SystemEvent;
use ploke_tui::parser::{resolve_index_target, run_parse_resolved};
use ploke_tui::user_config::{ChatPolicy, ChatTimeoutStrategy};
use ploke_tui::utils::parse_errors::FlattenedParserDiagnostic;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep};
use tracing::{info, warn};

use crate::layout;
use crate::model_registry::resolve_model_for_run;
use crate::provider_prefs::load_provider_for_model;
use crate::record::{write_compressed_record, RunRecord, RunRecordBuilder};
use crate::run_history::record_last_run;
use crate::spec::{PrepareError, PreparedMsbBatch, PreparedSingleRun, RunSource};
use crate::LlmResponseRecord;

const DEFAULT_PHASE_TIMEOUT_SECS: u64 = 300;
const WAIT_HEARTBEAT_SECS: u64 = 10;
const OPENROUTER_CODESTRAL_MODEL: &str = "mistralai/codestral-embed-2505";
const OPENROUTER_CODESTRAL_DIMS: usize = 1536;
const STARTING_DB_CACHE_VERSION: u32 = 1;

fn benchmark_chat_policy() -> ChatPolicy {
    let mut policy = ChatPolicy::default();
    policy.tool_call_timeout_secs = 60;
    policy.timeout_strategy = ChatTimeoutStrategy::Backoff { attempts: Some(3) };
    policy.timeout_base_secs = 5;
    policy.error_retry_limit = 3;
    policy.validated()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbAgentSingleRequest {
    pub run_manifest: PathBuf,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
    #[serde(default)]
    pub provider: Option<ProviderKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbSingleRequest {
    pub run_manifest: PathBuf,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
    #[serde(default)]
    pub provider: Option<ProviderKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbBatchRequest {
    pub batch_manifest: PathBuf,
    pub index_debug_snapshots: bool,
    #[serde(default)]
    pub use_default_model: bool,
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
    pub provider: Option<ProviderKey>,
    #[serde(default)]
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMsbBatchRequest {
    pub run_manifest: PathBuf,
    /// 1-based batch index to replay.
    pub batch_number: usize,
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
    /// Path to the compressed RunRecord (`record.json.gz`).
    /// Added in Phase 1 for comprehensive run persistence and replay.
    pub record_path: Option<PathBuf>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseFailureArtifact {
    pub target_dir: PathBuf,
    pub message: String,
    pub occurred_at_ms: i64,
    pub diagnostics: Vec<FlattenedParserDiagnostic>,
}

fn indexing_status_artifact_for_error(err: &PrepareError) -> Option<IndexingStatusArtifact> {
    match err {
        PrepareError::IndexingFailed { detail } => Some(IndexingStatusArtifact {
            status: "failed".to_string(),
            detail: detail.clone(),
        }),
        PrepareError::Timeout { phase, secs } if phase.starts_with("indexing_completed") => {
            Some(IndexingStatusArtifact {
                status: "timed_out".to_string(),
                detail: format!("timed out waiting for '{phase}' after {secs} seconds"),
            })
        }
        PrepareError::EventStreamClosed { phase } if phase.starts_with("indexing_completed") => {
            Some(IndexingStatusArtifact {
                status: "event_stream_closed".to_string(),
                detail: format!("event stream closed while waiting for '{phase}'"),
            })
        }
        _ => None,
    }
}

fn persist_indexing_failure_status(indexing_status_path: &Path, err: &PrepareError) {
    let Some(artifact) = indexing_status_artifact_for_error(err) else {
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

fn parse_failure_artifact_for_state(parse_failure: ParseFailure) -> ParseFailureArtifact {
    ParseFailureArtifact {
        target_dir: parse_failure.target_dir,
        message: parse_failure.message,
        occurred_at_ms: parse_failure.occurred_at_ms,
        diagnostics: parse_failure.diagnostics,
    }
}

async fn persist_parse_failure_artifact(state: &Arc<AppState>, parse_failure_path: &Path) {
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
    pub checkout_sha: Option<String>,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub embedding_dimensions: u32,
    pub embedding_dtype: String,
}

#[derive(Debug, Clone)]
struct StartingDbCachePaths {
    snapshot: PathBuf,
    metadata: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub task_id: String,
    pub repo_root: PathBuf,
    pub output_dir: PathBuf,
    pub selected_model: ModelId,
    pub selected_provider: Option<String>,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInstanceResult {
    pub task_id: String,
    pub run_manifest: PathBuf,
    pub execution_log: Option<PathBuf>,
    pub turn_summary: Option<PathBuf>,
    pub msb_submission: Option<PathBuf>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRunSummary {
    pub batch_id: String,
    pub mode: String,
    pub batch_manifest: PathBuf,
    pub output_dir: PathBuf,
    pub dataset_file: PathBuf,
    pub repo_cache: PathBuf,
    pub runs_root: PathBuf,
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
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCompletedRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: String,
    pub content: String,
    pub ui_payload: Option<ploke_tui::tools::ToolUiPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFailedRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: Option<String>,
    pub error: String,
    pub ui_payload: Option<ploke_tui::tools::ToolUiPayload>,
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
pub struct ExpectedFileChangeRecord {
    pub path: String,
    pub existed_before: bool,
    pub exists_after: bool,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub changed: bool,
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

fn truncate_preview(input: &str, max_chars: usize) -> String {
    let total = input.chars().count();
    if total <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}...<truncated {} chars>", total - max_chars)
}

fn build_agent_issue_prompt(prepared: &PreparedSingleRun) -> String {
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
    out
}

fn snapshot_message(
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

async fn collect_patch_artifact(state: &Arc<AppState>) -> PatchArtifact {
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
struct ExpectedFileBaseline {
    path: PathBuf,
    absolute_path: PathBuf,
    existed_before: bool,
    before_sha256: Option<String>,
}

async fn collect_patch_artifact_with_expected(
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
    let applied = edit_proposals.iter().any(|p| p.status == "Applied")
        || create_proposals.iter().any(|p| p.status == "Applied");
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

fn proposal_status_label(status: &EditProposalStatus) -> &'static str {
    match status {
        EditProposalStatus::Pending => "Pending",
        EditProposalStatus::Approved => "Approved",
        EditProposalStatus::Denied => "Denied",
        EditProposalStatus::Applied => "Applied",
        EditProposalStatus::Failed(_) => "Failed",
        EditProposalStatus::Stale(_) => "Stale",
    }
}

fn expected_patch_files(prepared: &PreparedSingleRun) -> Vec<PathBuf> {
    match &prepared.source {
        Some(RunSource::MultiSweBench(source)) => source.expected_patch_files.clone(),
        None => Vec::new(),
    }
}

fn build_msb_submission_record(
    prepared: &PreparedSingleRun,
    fix_patch: String,
) -> Option<MultiSweBenchSubmissionRecord> {
    match &prepared.source {
        Some(RunSource::MultiSweBench(source)) => Some(MultiSweBenchSubmissionRecord {
            org: source.org.clone(),
            repo: source.repo.clone(),
            number: source.number,
            fix_patch,
        }),
        None => None,
    }
}

fn collect_submission_fix_patch(prepared: &PreparedSingleRun) -> Result<String, PrepareError> {
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

fn snapshot_expected_files(
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

fn collect_expected_file_changes(
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

fn hash_file_contents(path: &Path) -> Result<Option<String>, PrepareError> {
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

fn starting_db_cache_metadata(prepared: &PreparedSingleRun) -> StartingDbCacheMetadata {
    let embedding = codestral_embedding_set();
    StartingDbCacheMetadata {
        version: STARTING_DB_CACHE_VERSION,
        task_id: prepared.task_id.clone(),
        checkout_sha: prepared
            .base_sha
            .clone()
            .or_else(|| prepared.head_sha.clone()),
        embedding_provider: embedding.provider.to_string(),
        embedding_model: embedding.model.to_string(),
        embedding_dimensions: embedding.dims(),
        embedding_dtype: embedding.shape.dtype_tag().to_string(),
    }
}

fn starting_db_cache_key(prepared: &PreparedSingleRun) -> String {
    let metadata = starting_db_cache_metadata(prepared);
    let payload = format!(
        "{version}:{task_id}:{checkout_sha}:{provider}:{model}:{dims}:{dtype}",
        version = metadata.version,
        task_id = metadata.task_id,
        checkout_sha = metadata.checkout_sha.as_deref().unwrap_or("<none>"),
        provider = metadata.embedding_provider,
        model = metadata.embedding_model,
        dims = metadata.embedding_dimensions,
        dtype = metadata.embedding_dtype,
    );
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

fn starting_db_cache_paths_at(
    eval_home: impl AsRef<Path>,
    prepared: &PreparedSingleRun,
) -> StartingDbCachePaths {
    let key = starting_db_cache_key(prepared);
    let base = eval_home.as_ref().join("cache").join("starting-dbs");
    StartingDbCachePaths {
        snapshot: base.join(format!("{key}.sqlite")),
        metadata: base.join(format!("{key}.json")),
    }
}

fn load_cached_starting_db_at(
    eval_home: impl AsRef<Path>,
    prepared: &PreparedSingleRun,
) -> Result<Option<StartingDbCachePaths>, PrepareError> {
    let paths = starting_db_cache_paths_at(eval_home, prepared);
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
    if metadata != starting_db_cache_metadata(prepared) {
        return Ok(None);
    }

    Ok(Some(paths))
}

fn load_cached_starting_db(
    prepared: &PreparedSingleRun,
) -> Result<Option<StartingDbCachePaths>, PrepareError> {
    load_cached_starting_db_at(layout::ploke_eval_home()?, prepared)
}

async fn persist_starting_db_cache_at(
    eval_home: impl AsRef<Path>,
    prepared: &PreparedSingleRun,
    snapshot_path: &Path,
) -> Result<StartingDbCachePaths, PrepareError> {
    let paths = starting_db_cache_paths_at(eval_home, prepared);
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

    let metadata = starting_db_cache_metadata(prepared);
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

async fn persist_starting_db_cache(
    prepared: &PreparedSingleRun,
    snapshot_path: &Path,
) -> Result<StartingDbCachePaths, PrepareError> {
    persist_starting_db_cache_at(layout::ploke_eval_home()?, prepared, snapshot_path).await
}

pub(crate) async fn resolve_provider_for_model(
    selected_model: &ResponseItem,
    requested_provider: Option<&ProviderKey>,
) -> Result<ProviderKey, PrepareError> {
    if !selected_model.supports_tools() {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_model_provider",
            detail: format!(
                "model '{}' does not advertise tool-call support",
                selected_model.id
            ),
        });
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

        return Ok(requested_provider.clone());
    }

    tool_capable_provider_key(&endpoints.data.endpoints).ok_or_else(|| {
        PrepareError::DatabaseSetup {
            phase: "resolve_model_provider",
            detail: format!(
                "no tool-capable provider endpoints returned for model '{}'",
                selected_model.id
            ),
        }
    })
}

impl RunMsbSingleRequest {
    pub async fn run(self) -> Result<RunArtifactPaths, PrepareError> {
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        let selected_model = resolve_model_for_run(self.use_default_model)?;
        let selected_model_id = selected_model.id.clone();
        let preferred_provider = load_provider_for_model(&selected_model_id)?;
        let selected_provider = resolve_provider_for_model(
            &selected_model,
            self.provider.as_ref().or(preferred_provider.as_ref()),
        )
        .await?;

        fs::create_dir_all(&prepared.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: prepared.output_dir.clone(),
                source,
            }
        })?;

        let execution_log_path = prepared.output_dir.join("execution-log.json");
        let repo_state_path = prepared.output_dir.join("repo-state.json");
        let indexing_status_path = prepared.output_dir.join("indexing-status.json");
        let parse_failure_path = prepared.output_dir.join("parse-failure.json");
        let snapshot_status_path = prepared.output_dir.join("snapshot-status.json");
        let indexing_checkpoint_db = prepared.output_dir.join("indexing-checkpoint.db");
        let indexing_failure_db = prepared.output_dir.join("indexing-failure.db");

        let mut steps = vec!["load_manifest".to_string()];
        checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
        steps.push("checkout_base_sha".to_string());

        let repo_state = RepoStateArtifact {
            repo_root: prepared.repo_root.clone(),
            requested_base_sha: prepared.base_sha.clone(),
            checked_out_head_sha: git_stdout(
                &prepared.repo_root,
                &["rev-parse", "HEAD"],
                "git rev-parse HEAD",
            )?
            .map(|s| s.trim().to_string()),
            git_status_porcelain: git_stdout(
                &prepared.repo_root,
                &["status", "--short"],
                "git status --short",
            )?
            .unwrap_or_default(),
        };
        write_json(&repo_state_path, &repo_state)?;
        steps.push("write_repo_state".to_string());

        let cached_starting_db = match load_cached_starting_db(&prepared) {
            Ok(cached) => cached,
            Err(err) => {
                warn!(error = %err, "runner phase: starting db cache lookup failed; falling back to fresh indexing");
                None
            }
        };
        let mut using_cached_starting_db = false;
        let runtime_db = if let Some(cache_paths) = cached_starting_db.as_ref() {
            match Database::create_new_backup_default(&cache_paths.snapshot).await {
                Ok(db) => {
                    info!(
                        snapshot = %cache_paths.snapshot.display(),
                        "runner phase: restoring cached starting db snapshot"
                    );
                    using_cached_starting_db = true;
                    steps.push("restore_cached_starting_db".to_string());
                    Arc::new(db)
                }
                Err(err) => {
                    warn!(
                        snapshot = %cache_paths.snapshot.display(),
                        error = %err,
                        "runner phase: cached starting db restore failed; falling back to fresh indexing"
                    );
                    let db = init_runtime_db()?;
                    steps.push("init_runtime_db".to_string());
                    db
                }
            }
        } else {
            let db = init_runtime_db()?;
            steps.push("init_runtime_db".to_string());
            db
        };

        let config_home = prepared.output_dir.join("config");
        fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
            path: config_home.clone(),
            source,
        })?;
        let _config_guard = XdgConfigHomeGuard::set_to(&config_home);
        steps.push("sandbox_config_home".to_string());

        let embedding_processor = codestral_embedding_processor()?;
        steps.push("init_codestral_embedder".to_string());

        let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
            .spawn_file_manager()
            .spawn_state_manager()
            .spawn_event_bus();
        let events = runtime
            .events_builder()
            .build_event_bus_only()
            .event_bus_events;
        let mut realtime_rx = events.realtime_tx_rx;
        let mut background_rx = events.background_tx_rx;
        let mut index_rx =
            Arc::try_unwrap(events.index_tx_rx).map_err(|_| PrepareError::DatabaseSetup {
                phase: "subscribe_index_status",
                detail: "index receiver unexpectedly shared".to_string(),
            })?;
        let state = runtime.state_arc();
        {
            let mut cfg = state.config.write().await;
            cfg.active_model = selected_model_id.clone();
            cfg.model_registry
                .select_model_provider(&selected_model_id, Some(&selected_provider));
        }
        info!("runner phase: inspect active embedding set before activation");
        let currently_active_set: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(
            ?currently_active_set,
            "active embedding set before activation"
        );

        activate_codestral_runtime(&state)?;

        let currently_active_set: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(
            ?currently_active_set,
            "active embedding set after activation"
        );
        steps.push("activate_codestral_embedding_set".to_string());
        let mut app = runtime
            .into_app_with_state_pwd(prepared.repo_root.clone())
            .await;
        steps.push("bootstrap_headless_runtime".to_string());

        if using_cached_starting_db {
            info!(
                task_id = %prepared.task_id,
                repo_root = %prepared.repo_root.display(),
                "runner phase: cached starting db present, skipping indexing"
            );
            seed_loaded_workspace_from_repo(&state, &prepared).await?;
            steps.push("seed_loaded_workspace_from_repo".to_string());
            app.pump_pending_events().await;
            steps.push("pump_post_cached_workspace_events".to_string());
        } else {
            info!(task_id = %prepared.task_id, repo_root = %prepared.repo_root.display(), "runner phase: start indexing");
            app.run_command_text("/index").await;
            steps.push("run_index_command".to_string());

            info!("runner phase: waiting for indexing completion");
            if let Err(err) = wait_for_indexing_completion(
                &mut app,
                &mut realtime_rx,
                &mut background_rx,
                &mut index_rx,
                Arc::clone(&state.db),
                indexing_checkpoint_db.clone(),
                indexing_failure_db.clone(),
                self.index_debug_snapshots,
            )
            .await
            {
                persist_indexing_failure_status(&indexing_status_path, &err);
                persist_parse_failure_artifact(&state, &parse_failure_path).await;
                return Err(err);
            }
            steps.push("indexing_completed".to_string());
            info!("runner phase: indexing completed");
            app.pump_pending_events().await;
            steps.push("pump_post_index_events".to_string());
        }

        let indexing_status = IndexingStatusArtifact {
            status: "completed".to_string(),
            detail: if using_cached_starting_db {
                "Loaded cached starting db snapshot and skipped reindexing.".to_string()
            } else {
                "Indexing completed through the full app command path.".to_string()
            },
        };
        write_json(&indexing_status_path, &indexing_status)?;
        steps.push("write_indexing_status".to_string());

        persist_db_snapshot(
            Arc::clone(&state.db),
            indexing_checkpoint_db.clone(),
            "starting snapshot checkpoint",
        )
        .await?;
        steps.push("write_indexing_checkpoint".to_string());

        if !using_cached_starting_db {
            if let Err(err) = persist_starting_db_cache(&prepared, &indexing_checkpoint_db).await {
                warn!(
                    snapshot = %indexing_checkpoint_db.display(),
                    error = %err,
                    "runner phase: failed to refresh starting db cache"
                );
            }
            steps.push("refresh_starting_db_cache".to_string());
        }

        let snapshot_file = prepared.output_dir.join("final-snapshot.db");
        info!(
            snapshot = %snapshot_file.display(),
            "runner phase: persisting final eval snapshot"
        );
        persist_db_snapshot(
            Arc::clone(&state.db),
            snapshot_file.clone(),
            "final snapshot",
        )
        .await?;
        steps.push("snapshot_completed".to_string());
        info!(snapshot_file = %snapshot_file.display(), "runner phase: snapshot completed");

        let snapshot_status = SnapshotStatusArtifact {
            status: "completed".to_string(),
            snapshot_file: Some(snapshot_file.clone()),
            registry_file: snapshot_file.clone(),
            config_home,
        };
        write_json(&snapshot_status_path, &snapshot_status)?;
        steps.push("write_snapshot_status".to_string());

        let msb_submission_path = if let Some(record) =
            build_msb_submission_record(&prepared, collect_submission_fix_patch(&prepared)?)
        {
            let path = prepared.output_dir.join("multi-swe-bench-submission.jsonl");
            write_jsonl_line(&path, &record)?;
            steps.push("write_msb_submission".to_string());
            Some(path)
        } else {
            None
        };

        let execution_log = ExecutionLog {
            task_id: prepared.task_id,
            repo_root: prepared.repo_root,
            output_dir: prepared.output_dir,
            selected_model: selected_model_id,
            selected_provider: Some(selected_provider.slug.as_str().to_string()),
            steps,
        };
        write_json(&execution_log_path, &execution_log)?;
        info!(
            execution_log = %execution_log_path.display(),
            repo_state = %repo_state_path.display(),
            indexing_status = %indexing_status_path.display(),
            indexing_checkpoint_db = %indexing_checkpoint_db.display(),
            indexing_failure_db = %indexing_failure_db.display(),
            snapshot_status = %snapshot_status_path.display(),
            msb_submission = msb_submission_path.as_ref().map(|p| p.display().to_string()),
            "runner phase: wrote run artifacts"
        );
        record_last_run(&execution_log.output_dir)?;

        Ok(RunArtifactPaths {
            run_manifest: manifest_path,
            execution_log: execution_log_path,
            repo_state: repo_state_path,
            indexing_status: indexing_status_path,
            indexing_checkpoint_db,
            indexing_failure_db,
            snapshot_status: snapshot_status_path,
            msb_submission: msb_submission_path,
            record_path: None, // Will be populated when RunRecord emission is implemented
        })
    }
}

impl RunMsbAgentSingleRequest {
    pub async fn run(self) -> Result<AgentRunArtifactPaths, PrepareError> {
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        let selected_model = resolve_model_for_run(self.use_default_model)?;
        let selected_model_id = selected_model.id.clone();
        let preferred_provider = load_provider_for_model(&selected_model_id)?;
        let selected_provider = resolve_provider_for_model(
            &selected_model,
            self.provider.as_ref().or(preferred_provider.as_ref()),
        )
        .await?;

        fs::create_dir_all(&prepared.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: prepared.output_dir.clone(),
                source,
            }
        })?;

        let execution_log_path = prepared.output_dir.join("execution-log.json");
        let repo_state_path = prepared.output_dir.join("repo-state.json");
        let indexing_status_path = prepared.output_dir.join("indexing-status.json");
        let parse_failure_path = prepared.output_dir.join("parse-failure.json");
        let snapshot_status_path = prepared.output_dir.join("snapshot-status.json");
        let indexing_checkpoint_db = prepared.output_dir.join("indexing-checkpoint.db");
        let indexing_failure_db = prepared.output_dir.join("indexing-failure.db");
        let turn_trace_path = prepared.output_dir.join("agent-turn-trace.json");
        let turn_summary_path = prepared.output_dir.join("agent-turn-summary.json");
        let record_path = prepared.output_dir.join("record.json.gz");

        // Initialize RunRecord for comprehensive run persistence (Phase 1E)
        let mut run_record = RunRecord::new(&prepared);

        let mut steps = vec!["load_manifest".to_string()];
        checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
        steps.push("checkout_base_sha".to_string());
        let expected_file_baselines =
            snapshot_expected_files(&prepared.repo_root, &expected_patch_files(&prepared))?;
        steps.push("snapshot_expected_files_before_turn".to_string());

        let repo_state = RepoStateArtifact {
            repo_root: prepared.repo_root.clone(),
            requested_base_sha: prepared.base_sha.clone(),
            checked_out_head_sha: git_stdout(
                &prepared.repo_root,
                &["rev-parse", "HEAD"],
                "git rev-parse HEAD",
            )?
            .map(|s| s.trim().to_string()),
            git_status_porcelain: git_stdout(
                &prepared.repo_root,
                &["status", "--short"],
                "git status --short",
            )?
            .unwrap_or_default(),
        };
        write_json(&repo_state_path, &repo_state)?;
        steps.push("write_repo_state".to_string());

        let cached_starting_db = match load_cached_starting_db(&prepared) {
            Ok(cached) => cached,
            Err(err) => {
                warn!(error = %err, "runner phase: starting db cache lookup failed; falling back to fresh indexing");
                None
            }
        };
        let mut using_cached_starting_db = false;
        let runtime_db = if let Some(cache_paths) = cached_starting_db.as_ref() {
            match Database::create_new_backup_default(&cache_paths.snapshot).await {
                Ok(db) => {
                    info!(
                        snapshot = %cache_paths.snapshot.display(),
                        "runner phase: restoring cached starting db snapshot"
                    );
                    using_cached_starting_db = true;
                    steps.push("restore_cached_starting_db".to_string());
                    Arc::new(db)
                }
                Err(err) => {
                    warn!(
                        snapshot = %cache_paths.snapshot.display(),
                        error = %err,
                        "runner phase: cached starting db restore failed; falling back to fresh indexing"
                    );
                    let db = init_runtime_db()?;
                    steps.push("init_runtime_db".to_string());
                    db
                }
            }
        } else {
            let db = init_runtime_db()?;
            steps.push("init_runtime_db".to_string());
            db
        };

        let config_home = prepared.output_dir.join("config");
        fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
            path: config_home.clone(),
            source,
        })?;
        let _config_guard = XdgConfigHomeGuard::set_to(&config_home);
        steps.push("sandbox_config_home".to_string());

        let embedding_processor = codestral_embedding_processor()?;
        steps.push("init_codestral_embedder".to_string());

        let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
            .spawn_file_manager()
            .spawn_state_manager()
            .spawn_event_bus()
            .spawn_llm_manager()
            .spawn_observability();
        let events = runtime.events_builder().build_all();
        let mut realtime_rx = events.event_bus_events.realtime_tx_rx;
        let mut background_rx = events.event_bus_events.background_tx_rx;
        let mut index_rx = Arc::try_unwrap(events.event_bus_events.index_tx_rx).map_err(|_| {
            PrepareError::DatabaseSetup {
                phase: "subscribe_index_status",
                detail: "index receiver unexpectedly shared".to_string(),
            }
        })?;
        let mut debug_rx =
            events
                .app_actor_events
                .debug_string_rx
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "subscribe_debug_string",
                    detail: "missing debug string receiver".to_string(),
                })?;
        let state = runtime.state_arc();
        {
            let mut cfg = state.config.write().await;
            cfg.editing.auto_confirm_edits = true;
            cfg.chat_policy = benchmark_chat_policy();
            cfg.active_model = selected_model_id.clone();
            cfg.model_registry
                .select_model_provider(&selected_model_id, Some(&selected_provider));
        }
        info!("runner phase: inspect active embedding set before activation");
        let currently_active_set: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(
            ?currently_active_set,
            "active embedding set before activation"
        );

        activate_codestral_runtime(&state)?;

        let currently_active_set: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(
            ?currently_active_set,
            "active embedding set after activation"
        );
        steps.push("activate_codestral_embedding_set".to_string());
        let mut app = runtime
            .into_app_with_state_pwd(prepared.repo_root.clone())
            .await;
        steps.push("bootstrap_headless_runtime".to_string());

        if using_cached_starting_db {
            info!(
                task_id = %prepared.task_id,
                repo_root = %prepared.repo_root.display(),
                "runner phase: cached starting db present, skipping indexing"
            );
            seed_loaded_workspace_from_repo(&state, &prepared).await?;
            steps.push("seed_loaded_workspace_from_repo".to_string());
            app.pump_pending_events().await;
            steps.push("pump_post_cached_workspace_events".to_string());
        } else {
            info!(task_id = %prepared.task_id, repo_root = %prepared.repo_root.display(), "runner phase: start indexing");
            app.run_command_text("/index").await;
            steps.push("run_index_command".to_string());

            info!("runner phase: waiting for indexing completion");
            if let Err(err) = wait_for_indexing_completion(
                &mut app,
                &mut realtime_rx,
                &mut background_rx,
                &mut index_rx,
                Arc::clone(&state.db),
                indexing_checkpoint_db.clone(),
                indexing_failure_db.clone(),
                self.index_debug_snapshots,
            )
            .await
            {
                persist_indexing_failure_status(&indexing_status_path, &err);
                persist_parse_failure_artifact(&state, &parse_failure_path).await;
                return Err(err);
            }
            steps.push("indexing_completed".to_string());
            info!("runner phase: indexing completed");
            app.pump_pending_events().await;
            steps.push("pump_post_index_events".to_string());
        }

        let indexing_status = IndexingStatusArtifact {
            status: "completed".to_string(),
            detail: if using_cached_starting_db {
                "Loaded cached starting db snapshot and skipped reindexing.".to_string()
            } else {
                "Indexing completed through the full app command path.".to_string()
            },
        };
        write_json(&indexing_status_path, &indexing_status)?;
        steps.push("write_indexing_status".to_string());

        persist_db_snapshot(
            Arc::clone(&state.db),
            indexing_checkpoint_db.clone(),
            "starting snapshot checkpoint",
        )
        .await?;
        steps.push("write_indexing_checkpoint".to_string());

        if !using_cached_starting_db {
            if let Err(err) = persist_starting_db_cache(&prepared, &indexing_checkpoint_db).await {
                warn!(
                    snapshot = %indexing_checkpoint_db.display(),
                    error = %err,
                    "runner phase: failed to refresh starting db cache"
                );
            }
            steps.push("refresh_starting_db_cache".to_string());
        }

        let turn_artifact = run_benchmark_turn(
            &prepared,
            &state,
            &mut app,
            &mut debug_rx,
            &mut realtime_rx,
            &mut background_rx,
            &turn_trace_path,
            selected_model_id.clone(),
            &expected_file_baselines,
        )
        .await?;
        write_json(&turn_summary_path, &turn_artifact)?;
        steps.push("benchmark_turn_completed".to_string());

        // Capture Cozo timestamp for time-travel queries and add turn to RunRecord
        let db_timestamp = state
            .db
            .current_validity_micros()
            .map_err(|e| PrepareError::DatabaseSetup {
                phase: "get_db_timestamp",
                detail: format!("Failed to get Cozo timestamp: {}", e),
            })?;
        run_record.mark_time_travel(1, db_timestamp, "turn_complete");
        run_record.add_turn_from_artifact(turn_artifact, db_timestamp);

        let snapshot_file = prepared.output_dir.join("final-snapshot.db");
        info!(
            snapshot = %snapshot_file.display(),
            "runner phase: persisting final eval snapshot"
        );
        persist_db_snapshot(
            Arc::clone(&state.db),
            snapshot_file.clone(),
            "final snapshot",
        )
        .await?;
        steps.push("snapshot_completed".to_string());
        info!(snapshot_file = %snapshot_file.display(), "runner phase: snapshot completed");

        let snapshot_status = SnapshotStatusArtifact {
            status: "completed".to_string(),
            snapshot_file: Some(snapshot_file.clone()),
            registry_file: snapshot_file.clone(),
            config_home,
        };
        write_json(&snapshot_status_path, &snapshot_status)?;
        steps.push("write_snapshot_status".to_string());

        let msb_submission_path = if let Some(record) =
            build_msb_submission_record(&prepared, collect_submission_fix_patch(&prepared)?)
        {
            let path = prepared.output_dir.join("multi-swe-bench-submission.jsonl");
            write_jsonl_line(&path, &record)?;
            steps.push("write_msb_submission".to_string());
            Some(path)
        } else {
            None
        };

        let execution_log = ExecutionLog {
            task_id: prepared.task_id,
            repo_root: prepared.repo_root,
            output_dir: prepared.output_dir,
            selected_model: selected_model_id,
            selected_provider: Some(selected_provider.slug.as_str().to_string()),
            steps,
        };
        write_json(&execution_log_path, &execution_log)?;
        info!(
            execution_log = %execution_log_path.display(),
            repo_state = %repo_state_path.display(),
            indexing_status = %indexing_status_path.display(),
            indexing_checkpoint_db = %indexing_checkpoint_db.display(),
            indexing_failure_db = %indexing_failure_db.display(),
            snapshot_status = %snapshot_status_path.display(),
            turn_trace = %turn_trace_path.display(),
            turn_summary = %turn_summary_path.display(),
            msb_submission = msb_submission_path.as_ref().map(|p| p.display().to_string()),
            "runner phase: wrote run artifacts"
        );
        record_last_run(&execution_log.output_dir)?;

        // Emit compressed RunRecord (Phase 1E)
        if let Err(e) = write_compressed_record(&record_path, &run_record) {
            warn!(
                path = %record_path.display(),
                error = %e,
                "runner phase: failed to write compressed run record"
            );
        } else {
            info!(path = %record_path.display(), "runner phase: wrote compressed run record");
        }

        Ok(AgentRunArtifactPaths {
            base: RunArtifactPaths {
                run_manifest: manifest_path,
                execution_log: execution_log_path,
                repo_state: repo_state_path,
                indexing_status: indexing_status_path,
                indexing_checkpoint_db,
                indexing_failure_db,
                snapshot_status: snapshot_status_path,
                msb_submission: msb_submission_path,
                record_path: Some(record_path),
            },
            turn_trace: turn_trace_path,
            turn_summary: turn_summary_path,
        })
    }
}

impl RunMsbBatchRequest {
    pub async fn run(self) -> Result<BatchRunArtifactPaths, PrepareError> {
        run_batch(
            self.batch_manifest,
            self.index_debug_snapshots,
            self.use_default_model,
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
            self.provider,
            self.stop_on_error,
            true,
        )
        .await
    }
}

async fn run_batch(
    batch_manifest: PathBuf,
    index_debug_snapshots: bool,
    use_default_model: bool,
    provider: Option<ProviderKey>,
    stop_on_error: bool,
    agent_mode: bool,
) -> Result<BatchRunArtifactPaths, PrepareError> {
    let (manifest_path, prepared) = load_prepared_batch(batch_manifest)?;
    fs::create_dir_all(&prepared.output_dir).map_err(|source| PrepareError::CreateOutputDir {
        path: prepared.output_dir.clone(),
        source,
    })?;

    let selected_model = resolve_model_for_run(use_default_model)?;
    let selected_model_id = selected_model.id.clone();
    let preferred_provider = load_provider_for_model(&selected_model_id)?;
    let selected_provider = resolve_provider_for_model(
        &selected_model,
        provider.as_ref().or(preferred_provider.as_ref()),
    )
    .await?;

    let summary_path = prepared.output_dir.join("batch-run-summary.json");
    let submission_path = prepared.output_dir.join("multi-swe-bench-submission.jsonl");
    fs::write(&submission_path, "").map_err(|source| PrepareError::WriteManifest {
        path: submission_path.clone(),
        source,
    })?;

    let mut stopped_early = false;
    let mut instance_results = Vec::with_capacity(prepared.instances.len());

    for task_id in &prepared.instances {
        let run_manifest = prepared.runs_root.join(task_id).join("run.json");
        if agent_mode {
            match (RunMsbAgentSingleRequest {
                run_manifest: run_manifest.clone(),
                index_debug_snapshots,
                use_default_model,
                provider: provider.clone(),
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
                        run_manifest,
                        execution_log: Some(artifacts.base.execution_log),
                        turn_summary: Some(artifacts.turn_summary),
                        msb_submission: artifacts.base.msb_submission,
                        status: "completed".to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_manifest,
                        execution_log: None,
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
                index_debug_snapshots,
                use_default_model,
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
                        run_manifest,
                        execution_log: Some(artifacts.execution_log),
                        turn_summary: None,
                        msb_submission: artifacts.msb_submission,
                        status: "completed".to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_manifest,
                        execution_log: None,
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
        batch_manifest: manifest_path.clone(),
        output_dir: prepared.output_dir,
        dataset_file: prepared.dataset_file,
        repo_cache: prepared.repo_cache,
        runs_root: prepared.runs_root,
        selected_model: Some(selected_model_id),
        selected_provider: Some(selected_provider.slug.as_str().to_string()),
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

fn init_runtime_db() -> Result<Arc<Database>, PrepareError> {
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

async fn setup_replay_runtime(
    prepared: &PreparedSingleRun,
) -> Result<(App, Arc<AppState>, XdgConfigHomeGuard), PrepareError> {
    let runtime_db = init_runtime_db()?;

    let config_home = prepared.output_dir.join("config");
    fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
        path: config_home.clone(),
        source,
    })?;
    let config_guard = XdgConfigHomeGuard::set_to(&config_home);

    let embedding_processor = codestral_embedding_processor()?;
    let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus();
    let state = runtime.state_arc();

    activate_codestral_runtime(&state)?;

    prepare_workspace_for_replay(&state, prepared).await?;

    let app = runtime
        .into_app_with_state_pwd(prepared.repo_root.clone())
        .await;

    Ok((app, state, config_guard))
}

async fn seed_loaded_workspace_from_repo(
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

async fn prepare_workspace_for_replay(
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

fn codestral_embedding_config() -> OpenRouterConfig {
    OpenRouterConfig {
        model: OPENROUTER_CODESTRAL_MODEL.to_string(),
        dimensions: Some(OPENROUTER_CODESTRAL_DIMS),
        request_dimensions: None,
        snippet_batch_size: 100,
        max_in_flight: 1,
        requests_per_second: Some(1),
        max_attempts: 3,
        initial_backoff_ms: 250,
        max_backoff_ms: 10_000,
        input_type: Some("code-snippet".into()),
        timeout_secs: 30,
        truncate_policy: TruncatePolicy::Truncate,
    }
}

fn codestral_embedding_processor() -> Result<EmbeddingProcessor, PrepareError> {
    info!(
        model = OPENROUTER_CODESTRAL_MODEL,
        dimensions = OPENROUTER_CODESTRAL_DIMS,
        "building codestral embedding processor"
    );
    let backend = OpenRouterBackend::new(&codestral_embedding_config()).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "init_codestral_embedder",
            detail: err.to_string(),
        }
    })?;
    info!("codestral embedding processor initialized");
    Ok(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(
        backend,
    )))
}

fn codestral_embedding_set() -> EmbeddingSet {
    EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(OPENROUTER_CODESTRAL_MODEL),
        EmbeddingShape::new_dims_default(OPENROUTER_CODESTRAL_DIMS as u32),
    )
}

fn activate_codestral_runtime(state: &Arc<AppState>) -> Result<(), PrepareError> {
    info!("activating codestral embedding set");
    let processor = Arc::new(codestral_embedding_processor()?);
    state
        .embedder
        .activate(&state.db, codestral_embedding_set(), processor)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "activate_codestral_embedding_set",
            detail: err.to_string(),
        })?;
    info!("codestral embedding set activated");
    Ok(())
}

async fn wait_for_indexing_completion(
    app: &mut App,
    realtime_rx: &mut broadcast::Receiver<AppEvent>,
    background_rx: &mut broadcast::Receiver<AppEvent>,
    index_rx: &mut broadcast::Receiver<IndexingStatus>,
    db: Arc<Database>,
    checkpoint_snapshot: PathBuf,
    failure_snapshot: PathBuf,
    persist_debug_snapshots: bool,
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
                    Ok(IndexingStatus { status: IndexStatus::Running, .. }) => {
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
                    Ok(IndexingStatus { status: IndexStatus::Completed, .. }) => {
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
                    Ok(IndexingStatus { status: IndexStatus::Failed(err), .. }) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failure",
                            )
                            .await?;
                        }
                        return Err(PrepareError::IndexingFailed { detail: err });
                    }
                    Ok(IndexingStatus { status: IndexStatus::Cancelled, .. }) => {
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
                    Ok(_) => continue,
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

async fn persist_db_snapshot(
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

fn canonicalize_file(path: &Path) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingRunManifest(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}

fn checkout_repo_to_base(repo_root: &Path, base_sha: Option<&str>) -> Result<(), PrepareError> {
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

fn run_git(
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

async fn run_benchmark_turn(
    prepared: &PreparedSingleRun,
    state: &Arc<AppState>,
    app: &mut App,
    debug_rx: &mut mpsc::Receiver<ploke_tui::app::commands::harness::DebugStateCommand>,
    realtime_rx: &mut broadcast::Receiver<AppEvent>,
    background_rx: &mut broadcast::Receiver<AppEvent>,
    trace_path: &Path,
    selected_model: ModelId,
    expected_file_baselines: &[ExpectedFileBaseline],
) -> Result<AgentTurnArtifact, PrepareError> {
    let deadline = Instant::now() + Duration::from_secs(prepared.budget.wall_clock_secs as u64);
    let issue_prompt = build_agent_issue_prompt(prepared);
    let user_message_id = submit_benchmark_prompt(app, &issue_prompt).await?;
    let mut artifact = AgentTurnArtifact {
        task_id: prepared.task_id.clone(),
        selected_model,
        issue_prompt,
        user_message_id,
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
    write_json(trace_path, &artifact)?;

    loop {
        app.pump_pending_events().await;

        if artifact.terminal_record.is_some() {
            break;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            artifact.patch_artifact =
                collect_patch_artifact_with_expected(state, expected_file_baselines).await?;
            write_json(trace_path, &artifact)?;
            return Err(PrepareError::Timeout {
                phase: "benchmark_turn",
                secs: prepared.budget.wall_clock_secs as u64,
            });
        }

        let wait_for = remaining.min(Duration::from_millis(250));
        tokio::select! {
            debug = debug_rx.recv() => {
                match debug {
                    Some(debug) => {
                        artifact.events.push(ObservedTurnEvent::DebugCommand(debug.as_str().to_string()));
                        write_json(trace_path, &artifact)?;
                    }
                    None => {
                        return Err(PrepareError::EventStreamClosed { phase: "benchmark_turn_debug" });
                    }
                }
            }
            realtime = realtime_rx.recv() => {
                match realtime {
                    Ok(event) => {
                        handle_benchmark_event(&mut artifact, state, event).await;
                        write_json(trace_path, &artifact)?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed { phase: "benchmark_turn_realtime" });
                    }
                }
            }
            background = background_rx.recv() => {
                match background {
                    Ok(event) => {
                        handle_benchmark_event(&mut artifact, state, event).await;
                        write_json(trace_path, &artifact)?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed { phase: "benchmark_turn_background" });
                    }
                }
            }
            _ = sleep(wait_for) => {
                continue;
            }
        }
    }

    app.pump_pending_events().await;
    artifact.final_assistant_message = None;
    artifact.patch_artifact =
        collect_patch_artifact_with_expected(state, expected_file_baselines).await?;
    
    // Note: llm_prompt and llm_response are now captured via events in handle_benchmark_event
    // This avoids the need for mutable state access and TTL mutation side effects
    
    write_json(trace_path, &artifact)?;
    Ok(artifact)
}

async fn handle_benchmark_event(
    artifact: &mut AgentTurnArtifact,
    state: &Arc<AppState>,
    event: AppEvent,
) {
    match event {
        AppEvent::Llm(llm_event) => {
            // Capture structured LLM events for RunRecord (Phase 1C/1D)
            match &llm_event {
                LlmEvent::ChatCompletion(ChatEvt::PromptConstructed { formatted_prompt, .. }) => {
                    // Capture the exact prompt sent to the LLM (what LLM sees)
                    artifact.llm_prompt = formatted_prompt.clone();
                    artifact.prompt_debug = Some(format!("{:?}", llm_event));
                    // Still log as debug string for other events
                    let rendered = format!("{:?}", llm_event);
                    artifact.events.push(ObservedTurnEvent::LlmEvent(rendered));
                }
                LlmEvent::ChatCompletion(ChatEvt::Response { 
                    content, 
                    model, 
                    metadata, 
                    usage,
                    ..
                }) => {
                    // Capture the LLM's response content (for backward compat)
                    artifact.llm_response = Some(content.clone());
                    
                    // Phase 1D: Structured LLM response capture
                    let record = LlmResponseRecord {
                        content: content.clone(),
                        model: model.clone(),
                        usage: Some(ploke_llm::response::TokenUsage {
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            total_tokens: usage.total_tokens,
                        }),
                        finish_reason: Some(metadata.finish_reason.clone()),
                        metadata: Some(metadata.clone()),
                    };
                    artifact.events.push(ObservedTurnEvent::LlmResponse(record));
                }
                _ => {
                    // Other LLM events: log as debug string
                    let rendered = format!("{:?}", llm_event);
                    artifact.events.push(ObservedTurnEvent::LlmEvent(rendered));
                }
            }
        }
        AppEvent::System(SystemEvent::ToolCallRequested {
            request_id,
            parent_id,
            tool_call,
        }) => {
            let record = ToolRequestRecord {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: tool_call.call_id.to_string(),
                tool: tool_call.function.name.as_str().to_string(),
                arguments: tool_call.function.arguments.clone(),
            };
            artifact
                .events
                .push(ObservedTurnEvent::ToolRequested(record));
        }
        AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id,
            call_id,
            content,
            ui_payload,
        }) => {
            let tool = artifact
                .events
                .iter()
                .rev()
                .find_map(|ev| match ev {
                    ObservedTurnEvent::ToolRequested(req) if req.call_id == call_id.to_string() => {
                        Some(req.tool.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| "unknown".to_string());
            let record = ToolCompletedRecord {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: call_id.to_string(),
                tool,
                content,
                ui_payload,
            };
            artifact
                .events
                .push(ObservedTurnEvent::ToolCompleted(record));
        }
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id,
            error,
            ui_payload,
        }) => {
            let tool = artifact.events.iter().rev().find_map(|ev| match ev {
                ObservedTurnEvent::ToolRequested(req) if req.call_id == call_id.to_string() => {
                    Some(req.tool.clone())
                }
                _ => None,
            });
            let record = ToolFailedRecord {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: call_id.to_string(),
                tool,
                error,
                ui_payload,
            };
            artifact.events.push(ObservedTurnEvent::ToolFailed(record));
        }
        AppEvent::MessageUpdated(message_event) => {
            if let Some(snapshot) = snapshot_message(state, message_event) {
                artifact
                    .events
                    .push(ObservedTurnEvent::MessageUpdated(snapshot));
            }
        }
        AppEvent::System(SystemEvent::ChatTurnFinished {
            session_id,
            request_id,
            parent_id,
            assistant_message_id,
            outcome,
            error_id,
            summary,
            attempts,
            ..
        }) => {
            let record = TurnFinishedRecord {
                session_id: session_id.to_string(),
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                assistant_message_id: assistant_message_id.to_string(),
                outcome,
                error_id: error_id.map(|id| id.to_string()),
                summary,
                attempts,
            };
            artifact.terminal_record = Some(record.clone());
            artifact
                .events
                .push(ObservedTurnEvent::TurnFinished(record));
        }
        _ => {}
    }
}

async fn submit_benchmark_prompt(app: &mut App, prompt: &str) -> Result<String, PrepareError> {
    let state_cmd_tx = app.state_cmd_tx();
    let user_message_id = ploke_core::PROJECT_NAMESPACE_UUID;
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();

    state_cmd_tx
        .send(
            ploke_tui::app_state::commands::StateCommand::AddUserMessage {
                content: prompt.to_string(),
                new_user_msg_id: user_message_id,
                completion_tx,
            },
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "submit_benchmark_prompt_add_user",
            detail: err.to_string(),
        })?;
    state_cmd_tx
        .send(ploke_tui::app_state::commands::StateCommand::ScanForChange { scan_tx })
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "submit_benchmark_prompt_scan",
            detail: err.to_string(),
        })?;
    state_cmd_tx
        .send(ploke_tui::app_state::commands::StateCommand::EmbedMessage {
            new_msg_id: user_message_id,
            completion_rx,
            scan_rx,
        })
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "submit_benchmark_prompt_embed",
            detail: err.to_string(),
        })?;

    Ok(user_message_id.to_string())
}

fn git_stdout(
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

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    let json = serde_json::to_string_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, json).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn write_jsonl_line<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    let mut json = serde_json::to_string(value).map_err(PrepareError::Serialize)?;
    json.push('\n');
    fs::write(path, json).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn append_jsonl_blob(path: &Path, blob: &str) -> Result<(), PrepareError> {
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

fn load_prepared_batch(
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

fn load_prepared_run(run_manifest: PathBuf) -> Result<(PathBuf, PreparedSingleRun), PrepareError> {
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

fn canonicalize_batch_file(path: &Path) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingBatchManifest(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}

fn log_replay_batch_context(batch_number: usize, batch: &[ploke_db::TypedEmbedData]) {
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

struct XdgConfigHomeGuard {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::EvalBudget;
    use ploke_db::multi_embedding::schema::EmbeddingSetExt;
    use ploke_llm::response::FunctionCall;
    use ploke_tui::app_state::core::{CreateProposal, EditProposal};
    use ploke_tui::app_state::events::MessageUpdatedEvent;
    use ploke_tui::test_utils::mock::create_mock_app_state;
    use ploke_tui::tools::{FunctionMarker, ToolCall, ToolName};
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tracing_subscriber::fmt::SubscriberBuilder;
    use uuid::Uuid;

    fn init_tracing() {
        let _ = SubscriberBuilder::default()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_test_writer()
            .try_init();
    }

    fn run_git_test(repo_root: &Path, args: &[&str], label: &str) {
        let status = Command::new("git")
            .current_dir(repo_root)
            .args(args)
            .status()
            .unwrap_or_else(|err| panic!("{label} failed to start: {err}"));
        assert!(status.success(), "{label} failed with status {status}");
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

        let _processor = codestral_embedding_processor().expect("init embedding processor");
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
        let processor = codestral_embedding_processor().expect("init embedding processor");
        let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, processor);
        let state = runtime.state_arc();

        let before: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set before activation");
        info!(?before, "active embedding set before activation");

        activate_codestral_runtime(&state).expect("activate codestral runtime");

        let after: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set after activation");
        info!(?after, "active embedding set after activation");

        assert_ne!(before.hash_id(), after.hash_id());
        assert_eq!(after.hash_id(), codestral_embedding_set().hash_id());
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

        let paths = persist_starting_db_cache_at(cache_root.path(), &prepared, &snapshot_path)
            .await
            .expect("persist cache");
        assert!(paths.snapshot.exists());
        assert!(paths.metadata.exists());

        let loaded =
            load_cached_starting_db_at(cache_root.path(), &prepared).expect("load cache hit");
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
        };
        let prepared_b = PreparedSingleRun {
            base_sha: Some("different".to_string()),
            ..prepared_a.clone()
        };

        let paths = starting_db_cache_paths_at(cache_root.path(), &prepared_a);
        assert_ne!(
            paths.snapshot,
            starting_db_cache_paths_at(cache_root.path(), &prepared_b).snapshot
        );
        assert!(
            load_cached_starting_db_at(cache_root.path(), &prepared_a)
                .expect("empty cache should not error")
                .is_none()
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

        let artifact = indexing_status_artifact_for_error(&err).expect("indexing artifact");
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

        let artifact = indexing_status_artifact_for_error(&err).expect("timeout artifact");
        assert_eq!(artifact.status, "timed_out");
        assert!(artifact.detail.contains("300 seconds"));
    }

    #[test]
    fn indexing_status_artifact_ignores_non_indexing_errors() {
        let err = PrepareError::Timeout {
            phase: "benchmark_turn",
            secs: 10,
        };

        assert!(indexing_status_artifact_for_error(&err).is_none());
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
        };

        let prompt = build_agent_issue_prompt(&prepared);
        assert!(prompt.contains("case-123"));
        assert!(prompt.contains("/tmp/repo"));
        assert!(prompt.contains("abc123"));
        assert!(prompt.contains("Fix the thing"));
        assert!(prompt.contains("The body text."));
    }

    #[test]
    fn build_msb_submission_record_uses_benchmark_identity() {
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
        };

        let record = build_msb_submission_record(&prepared, "diff --git a/foo b/foo\n".to_string())
            .expect("submission record");
        assert_eq!(record.org, "BurntSushi");
        assert_eq!(record.repo, "ripgrep");
        assert_eq!(record.number, 2209);
        assert!(record.fix_patch.starts_with("diff --git"));
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
        };

        let fix_patch = collect_submission_fix_patch(&prepared).expect("submission patch");
        assert!(fix_patch.contains("diff --git a/src/lib.rs b/src/lib.rs"));
        assert!(fix_patch.contains("+    println!(\"hi\");"));

        let record = build_msb_submission_record(&prepared, fix_patch).expect("submission");
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

    #[tokio::test]
    async fn collect_patch_artifact_snapshots_applied_proposals() {
        let state = create_mock_app_state();
        let state = Arc::new(state);
        {
            let mut proposals = state.proposals.write().await;
            proposals.insert(
                ploke_core::PROJECT_NAMESPACE_UUID,
                EditProposal {
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
            proposals.insert(
                ploke_core::PROJECT_NAMESPACE_UUID,
                EditProposal {
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
            proposals.insert(
                uuid::Uuid::new_v4(),
                EditProposal {
                    request_id: uuid::Uuid::new_v4(),
                    parent_id: ploke_core::PROJECT_NAMESPACE_UUID,
                    call_id: ploke_core::ArcStr::from("edit-call-failed"),
                    proposed_at_ms: 2,
                    edits: Vec::new(),
                    files: vec![PathBuf::from("src/lib.rs")],
                    edits_ns: Vec::new(),
                    preview: DiffPreview::UnifiedDiff {
                        text: "--- a/src/lib.rs\n+++ b/src/lib.rs\n".to_string(),
                    },
                    status: EditProposalStatus::Failed("boom".to_string()),
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

        handle_benchmark_event(
            &mut artifact,
            &state,
            AppEvent::MessageUpdated(MessageUpdatedEvent::new(user_id)),
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
                        arguments: "{}".to_string(),
                    },
                },
            }),
        )
        .await;

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

        // Handle the event
        handle_benchmark_event(&mut artifact, &state, event).await;

        // Verify the prompt was captured
        assert_eq!(artifact.llm_prompt.len(), 2, "expected 2 messages in llm_prompt");
        assert_eq!(artifact.llm_prompt[0].role, Role::User);
        assert_eq!(artifact.llm_prompt[0].content, "Hello, fix this bug");
        assert_eq!(artifact.llm_prompt[1].role, Role::Assistant);
        assert_eq!(artifact.llm_prompt[1].content, "I'll help you fix it");
        assert!(artifact.prompt_debug.is_some(), "prompt_debug should be set");
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

        // Handle the event
        handle_benchmark_event(&mut artifact, &state, event).await;

        // Verify the response was captured
        assert_eq!(artifact.llm_response, Some("Here is the fix you requested".to_string()));
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

        // Handle the event
        handle_benchmark_event(&mut artifact, &state, event).await;

        // Verify a structured LlmResponse event was captured (Phase 1D)
        let llm_response_events: Vec<_> = artifact
            .events
            .iter()
            .filter_map(|ev| match ev {
                ObservedTurnEvent::LlmResponse(record) => Some(record),
                _ => None,
            })
            .collect();

        assert_eq!(llm_response_events.len(), 1, "expected exactly one LlmResponse event");
        
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
}
