use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use chrono::Utc;
use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand};
use ploke_llm::Router;
use ploke_llm::request::{endpoint::Endpoint, models::ModelRouteSource};
use ploke_llm::router_only::HasEndpoint;
use ploke_llm::router_only::google::Google;
use ploke_llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use ploke_llm::{ModelId, ProviderKey, SupportsTools};
use ploke_protocol::procedure::{
    ProcedureDebugEvent, ProcedureDebugEventKind, ProcedureDebugSink, set_procedure_debug_sink,
};
use ploke_protocol::tool_calls::trace::NeighborhoodSource;
use ploke_protocol::tool_calls::{review, segment, trace};
use ploke_protocol::{JsonAdjudicator, JsonLlmConfig, Procedure, ProtocolReasoningPolicy};
use ploke_records::llm_response::{FULL_RESPONSE_TRACE_FILE, RawFullResponseRecord};
use ploke_records::protocol::{InterventionApplyArtifact, InterventionIssueDetectionArtifact};
use ploke_records::tool_contracts::{
    PersistedToolCallArguments, ToolArgumentDecodeError, ToolArgumentParseFailure,
    ToolArgumentsJson, ToolCallArguments,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

mod args;
mod dispatch;
mod format;
mod handlers;

pub(crate) use handlers::registry::registry_dataset_view;
pub(crate) use handlers::run::{default_batch_id, resolve_batch_manifest, yes_no};
mod prototype1_process;
/// Prototype 1 typed state model and persisted artifact map.
///
/// See `prototype1_state::mod` for the implementation split and the on-disk
/// campaign layout under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`.
pub(crate) mod prototype1_state;
mod provider;

pub use args::*;

pub(crate) use format::{
    display_context_length, display_price_per_million, extract_model_size, model_size_string,
};

pub(crate) use provider::{
    current_provider_for_model, headless_model_selection,
    headless_model_selection_from_provider_preference, load_parent_patcher_model_selection,
    parse_provider_key, registry_route_source, resolve_provider_model_id,
};

#[cfg(test)]
#[path = "cli/tests.rs"]
mod tests;

const PROTOCOL_HTTP_MAX_ATTEMPTS: u32 = 1;
const PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS: usize = 3;
const TOOL_CALL_REVIEW_TIMEOUT_SECS: u64 = ploke_llm::LLM_TIMEOUT_SECS;

use crate::campaign::{
    CampaignManifest, CampaignOverrides, CampaignValidationCheck, EvalCampaignPolicy,
    ProtocolCampaignPolicy, ResolvedCampaignConfig, adopt_campaign_manifest_from_closure_state,
    adopt_campaign_manifest_from_registry, apply_campaign_overrides, campaign_closure_state_path,
    campaign_manifest_path, dataset_files_from_sources, dataset_keys_from_sources,
    default_protocol_max_tokens, list_campaigns, render_resolved_campaign_config,
    resolve_campaign_config, save_campaign_manifest, validate_campaign_config,
};
use crate::closure::{
    ClosureClass, ClosureRecomputeRequest, closure_state_path, load_closure_state,
    recompute_closure_state, render_closure_status,
};
use crate::inner::registry::RunRegistration;
use crate::intervention::{
    INTERVENTION_APPLY_PROCEDURE, INTERVENTION_ISSUE_DETECTION_PROCEDURE,
    INTERVENTION_SYNTHESIS_PROCEDURE, InterventionApplyInput, InterventionApplyOutput,
    InterventionSynthesisInput, IssueCase, IssueDetectionInput, IssueDetectionOutput,
    detect_issue_cases, execute_intervention_apply, issue_detection_artifact_input,
    operation_target_artifact_id, select_primary_issue, synthesize_intervention_with_llm,
};
use crate::intervention_issue_aggregate::{
    IssueDetectionAggregate, IssueDetectionAggregateError, load_issue_detection_aggregate,
};
use crate::layout::{
    active_model_file, batches_dir, cache_dir, datasets_dir, instances_dir, model_registry_file,
    models_dir, parent_patcher_model_file, repos_dir, starting_db_cache_dir,
    workspace_root_for_key,
};
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, load_parent_patcher_model,
    refresh_model_registry, registry_has_model, save_active_model, save_parent_patcher_model,
};
use crate::msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
use crate::projection::OperatorProjectionRead;
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolAggregateError, ProtocolCallReviewRow, load_protocol_aggregate,
};
use crate::protocol_artifacts::{
    StoredProtocolArtifactFile, list_protocol_artifact_load_results, list_protocol_artifacts,
    load_protocol_artifact, protocol_artifact_preview, protocol_artifact_summary,
    write_protocol_artifact,
};
use crate::protocol_report::{
    ProtocolAggregateCallIssueRow, ProtocolAggregateCoverage, ProtocolAggregateReport,
    ProtocolAggregateSegmentRow, ProtocolColorProfile, ProtocolReportRenderOptions,
    render_protocol_aggregate_report_with_options,
};
use crate::protocol_triage_report::{
    ProtocolCampaignCountRow, ProtocolCampaignEvidence, ProtocolCampaignExemplarRow,
    ProtocolCampaignFamilyRow, ProtocolCampaignSummary, ProtocolCampaignTriageReport,
    render_protocol_campaign_triage_report, sort_count_rows,
};
use crate::provider_prefs::{
    clear_provider_for_model, load_provider_for_model, set_provider_for_model,
};
use crate::record::read_compressed_record;
use crate::registry::{builtin_dataset_registry_entries, builtin_dataset_registry_entry};
use crate::run_history::{
    RunDirPreference, list_finished_record_paths_in_instances_root, preferred_run_dir_for_instance,
    print_assistant_messages_from_record_path,
};
use crate::run_registry::list_registrations_for_instance;
use crate::runner::{
    BatchRunArtifactPaths, BatchRunSummary, MultiSweBenchSubmissionRecord, ReplayMsbBatchRequest,
    RunMsbAgentBatchRequest, RunMsbAgentSingleRequest, RunMsbBatchRequest, RunMsbSingleRequest,
    resolve_route_for_model,
};
use crate::selection::{
    ActiveSelection, ActiveSelectionSlot, clear_active_selection, load_active_selection,
    load_active_selection_at, render_selection_warnings, save_active_selection,
    unset_active_selection_slot,
};
use crate::spec::{
    EvalBudget, IssueInput, OutputMode, PrepareError, PrepareSingleRunRequest, PrepareWrite,
    PreparedCampaignContext,
};
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, target_registry_path,
};

impl Cli {
    pub async fn run(self) -> ExitCode {
        dispatch::run(self).await
    }
}

impl LoopCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            LoopSubcommand::Prototype1(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Setup(cmd) => cmd.run_setup().await,
            LoopSubcommand::Prototype1Doctor(cmd) => prototype1_state::run::doctor(cmd).await,
            LoopSubcommand::Prototype1Prompt(cmd) => prototype1_state::run::prompt(cmd).await,
            LoopSubcommand::Prototype1Continue(cmd) => prototype1_state::run::resume(cmd).await,
            LoopSubcommand::Prototype1Step(cmd) => prototype1_state::run::step(cmd).await,
            LoopSubcommand::Prototype1State(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Runner(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Harness(cmd) => cmd.run().await,
        }
    }
}

impl Prototype1RunnerCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let Some(invocation) = self.invocation else {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "prototype1-runner requires --invocation".to_string(),
            });
        };
        if !self.execute {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "prototype1-runner currently requires --execute".to_string(),
            });
        }
        let _ = self.campaign;
        let _ = self.node_id;
        let _ = self.stop_on_error;
        let _ = self.format;
        prototype1_process::execute_prototype1_runner_invocation(&invocation)
            .await
            .map(|_| ())
    }
}

impl Prototype1HarnessCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            Prototype1HarnessSubcommand::Attempt(cmd) => cmd.run().await,
            Prototype1HarnessSubcommand::Sweep(cmd) => cmd.run().await,
        }
    }
}

impl Prototype1HarnessAttemptCommand {
    async fn run(self) -> Result<(), PrepareError> {
        let options = prototype1_state::cli_facing::BroadTuiAttemptOptions::from_cli(
            self.model_id,
            self.provider,
            self.max_attempts,
            self.timeout_secs,
        )?;
        let row = prototype1_state::cli_facing::run_broad_harness_attempt_from_request_path(
            self.request,
            options,
        )
        .await?;
        print_broad_harness_attempt_rows(std::slice::from_ref(&row), self.format)
    }
}

impl Prototype1HarnessSweepCommand {
    async fn run(self) -> Result<(), PrepareError> {
        let requests = collect_broad_harness_request_paths(self.requests, self.requests_dir)?;
        let lanes = broad_harness_sweep_lanes(
            requests,
            self.model_ids,
            self.provider,
            self.max_attempts,
            self.timeout_secs,
        )?;
        let rows =
            prototype1_state::cli_facing::run_broad_harness_attempt_sweep(lanes, self.parallel)
                .await?;
        print_broad_harness_attempt_rows(&rows, self.format)
    }
}

fn collect_broad_harness_request_paths(
    mut requests: Vec<PathBuf>,
    requests_dir: Option<PathBuf>,
) -> Result<Vec<PathBuf>, PrepareError> {
    if let Some(dir) = requests_dir {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not read requests directory '{}': {source}",
                dir.display()
            ),
        })? {
            let entry = entry.map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "could not read entry in requests directory '{}': {source}",
                    dir.display()
                ),
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                entries.push(path);
            }
        }
        entries.sort();
        requests.extend(entries);
    }

    if requests.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "prototype1-harness sweep requires --request or --requests-dir".to_string(),
        });
    }

    let mut seen = BTreeSet::new();
    for path in &requests {
        if !seen.insert(path.clone()) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "prototype1-harness sweep received duplicate request path '{}'",
                    path.display()
                ),
            });
        }
    }

    Ok(requests)
}

fn broad_harness_sweep_lanes(
    requests: Vec<PathBuf>,
    model_ids: Vec<String>,
    provider: Option<String>,
    max_attempts: Option<u32>,
    timeout_secs: Option<u64>,
) -> Result<
    Vec<(
        PathBuf,
        prototype1_state::cli_facing::BroadTuiAttemptOptions,
    )>,
    PrepareError,
> {
    let mut lanes = Vec::with_capacity(requests.len());
    if model_ids.is_empty() {
        for request in requests {
            lanes.push((
                request,
                prototype1_state::cli_facing::BroadTuiAttemptOptions::from_cli(
                    None,
                    provider.clone(),
                    max_attempts,
                    timeout_secs,
                )?,
            ));
        }
        return Ok(lanes);
    }

    if model_ids.len() != 1 && model_ids.len() != requests.len() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1-harness sweep requires one --model-id for all requests or one per request; got {} model(s) for {} request(s)",
                model_ids.len(),
                requests.len()
            ),
        });
    }

    for (index, request) in requests.into_iter().enumerate() {
        let model_id = if model_ids.len() == 1 {
            model_ids[0].clone()
        } else {
            model_ids[index].clone()
        };
        lanes.push((
            request,
            prototype1_state::cli_facing::BroadTuiAttemptOptions::from_cli(
                Some(model_id),
                provider.clone(),
                max_attempts,
                timeout_secs,
            )?,
        ));
    }
    Ok(lanes)
}

fn print_broad_harness_attempt_rows(
    rows: &[prototype1_state::cli_facing::BroadHarnessAttemptProjection],
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(rows).map_err(|source| {
                    PrepareError::InvalidBatchSelection {
                        detail: format!("could not serialize broad harness attempt rows: {source}"),
                    }
                })?
            );
        }
        InspectOutputFormat::Table => {
            println!(
                "{:<8} {:<42} {:<28} {:>8} {:>8} {:>7} {}",
                "status", "request_id", "model", "timeout", "elapsed", "paths", "request"
            );
            for row in rows {
                let model = row.model_id.as_deref().unwrap_or("default");
                let changed = row.changed_paths.len();
                println!(
                    "{:<8} {:<42} {:<28} {:>8} {:>8} {:>7} {}",
                    row.status,
                    row.request_id,
                    model,
                    row.timeout_secs,
                    row.elapsed_ms,
                    changed,
                    row.request_path.display()
                );
                if let Some(error) = row.error.as_deref() {
                    println!("  error: {error}");
                }
            }
        }
    }
    Ok(())
}

struct TimingTrace;

struct TimingScope {
    label: String,
    started_at: Instant,
}

impl TimingTrace {
    fn mark(label: &str) {
        #[cfg(not(feature = "demo"))]
        eprintln!("{} {}", Utc::now().format("%H:%M:%S"), label);
        #[cfg(feature = "demo")]
        let _ = label;
    }

    fn scope(label: impl Into<String>) -> TimingScope {
        let label = label.into();
        Self::mark(&format!("{label}.start"));
        TimingScope {
            label,
            started_at: Instant::now(),
        }
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        #[cfg(not(feature = "demo"))]
        eprintln!(
            "{} {}.end +{:.3}s",
            Utc::now().format("%H:%M:%S"),
            self.label,
            self.started_at.elapsed().as_secs_f64()
        );
        #[cfg(feature = "demo")]
        let _ = (&self.label, self.started_at);
    }
}

pub(crate) fn print_builtin_dataset_entries() {
    for entry in builtin_dataset_registry_entries() {
        println!("{}\t{}\t{}", entry.key, entry.language, entry.url);
    }
}

async fn execute_batch_eval_for_manifest(
    batch_manifest: PathBuf,
    index_debug_snapshots: bool,
    use_default_model: bool,
    model_id: Option<String>,
    provider: Option<ProviderKey>,
    stop_on_error: bool,
) -> Result<BatchRunArtifactPaths, PrepareError> {
    RunMsbAgentBatchRequest {
        batch_manifest,
        index_debug_snapshots,
        use_default_model,
        model_id,
        provider,
        stop_on_error,
    }
    .run()
    .await
}

#[derive(Debug)]
struct ProtocolBatchExecution {
    executions: Vec<ProtocolRunExecution>,
    failures: Vec<String>,
}

async fn execute_protocol_run_tasks(
    tasks: Vec<ProtocolRunTask>,
    model_id: String,
    route_source: Option<ModelRouteSource>,
    provider_slug: Option<String>,
    max_concurrency: usize,
    tool_review_parallelism: usize,
    stop_on_error: bool,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) -> Result<ProtocolBatchExecution, PrepareError> {
    let mut executions = Vec::new();
    let mut failures = Vec::new();
    let max_concurrency = max_concurrency.max(1);
    let tool_review_parallelism = tool_review_parallelism.max(1);
    let mut pending = tasks.into_iter().collect::<VecDeque<_>>();
    let mut join_set = JoinSet::new();
    let review_permits = Arc::new(Semaphore::new(tool_review_parallelism));

    while join_set.len() < max_concurrency {
        let Some(task) = pending.pop_front() else {
            break;
        };
        spawn_protocol_run_task(
            &mut join_set,
            task,
            model_id.clone(),
            route_source,
            provider_slug.clone(),
            review_permits.clone(),
            max_tokens,
            reasoning,
        );
    }

    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(Ok(execution)) => {
                executions.push(execution);
                if let Some(task) = pending.pop_front() {
                    spawn_protocol_run_task(
                        &mut join_set,
                        task,
                        model_id.clone(),
                        route_source,
                        provider_slug.clone(),
                        review_permits.clone(),
                        max_tokens,
                        reasoning,
                    );
                }
            }
            Ok(Err(err)) => {
                failures.push(err.to_string());
                if stop_on_error {
                    join_set.abort_all();
                    return Err(err);
                }
                if let Some(task) = pending.pop_front() {
                    spawn_protocol_run_task(
                        &mut join_set,
                        task,
                        model_id.clone(),
                        route_source,
                        provider_slug.clone(),
                        review_permits.clone(),
                        max_tokens,
                        reasoning,
                    );
                }
            }
            Err(err) => {
                let detail = PrepareError::DatabaseSetup {
                    phase: "protocol_run_tasks",
                    detail: format!("protocol worker task failed: {err}"),
                };
                failures.push(detail.to_string());
                if stop_on_error {
                    join_set.abort_all();
                    return Err(detail);
                }
                if let Some(task) = pending.pop_front() {
                    spawn_protocol_run_task(
                        &mut join_set,
                        task,
                        model_id.clone(),
                        route_source,
                        provider_slug.clone(),
                        review_permits.clone(),
                        max_tokens,
                        reasoning,
                    );
                }
            }
        }
    }

    Ok(ProtocolBatchExecution {
        executions,
        failures,
    })
}

fn persist_issue_detection_for_record(
    record_path: &Path,
) -> Result<IssueDetectionOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let protocol_aggregate = load_protocol_aggregate(record_path).ok();
    let detection_input = IssueDetectionInput::from_record(record, protocol_aggregate);
    let persisted_input = issue_detection_artifact_input(&detection_input);
    let output = detect_issue_cases(&detection_input);
    let artifact = build_issue_detection_artifact(&output);
    write_protocol_artifact(
        record_path,
        INTERVENTION_ISSUE_DETECTION_PROCEDURE,
        &subject_id,
        None,
        None,
        &persisted_input,
        &output,
        &artifact,
    )?;
    Ok(output)
}

async fn persist_intervention_synthesis_for_record(
    record_path: &Path,
    issue: IssueCase,
    source_state_id: String,
    model_id: Option<String>,
    provider: Option<String>,
) -> Result<crate::intervention::InterventionSynthesisOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let target_relpath = PathBuf::from(issue.target_tool.description_artifact_relpath());
    let source_content =
        fs::read_to_string(&target_relpath).map_err(|source| PrepareError::ReadManifest {
            path: target_relpath.clone(),
            source,
        })?;
    let input = InterventionSynthesisInput {
        issue,
        source_state_id,
        source_content,
        // The generic CLI path does not yet know the durable Artifact target
        // for this record. Downstream layers will preserve a fallback text-file
        // surface id, but future backend-aware callers should pass an
        // OperationTarget here instead of relying on that fallback.
        operation_target: None,
    };
    let cfg = protocol_llm_config(
        model_id,
        None,
        provider,
        120,
        PROTOCOL_HTTP_MAX_ATTEMPTS,
        3200,
        ProtocolReasoningPolicy::default(),
    )?;
    let run = synthesize_intervention_with_llm(input.clone(), cfg.clone())
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "intervention_synthesis",
            detail: err.to_string(),
        })?;
    write_protocol_artifact(
        record_path,
        INTERVENTION_SYNTHESIS_PROCEDURE,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &input,
        &run.output,
        &run.artifact,
    )?;
    Ok(run.output)
}

fn persist_intervention_apply_for_record(
    record_path: &Path,
    synthesis: &crate::intervention::InterventionSynthesisOutput,
    candidate_id: &str,
    repo_root: &Path,
) -> Result<InterventionApplyOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let candidate = synthesis
        .candidate_set
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "intervention apply candidate '{}' not found for subject '{}'",
                candidate_id, subject_id
            ),
        })?;
    let base_artifact_id = synthesis
        .candidate_set
        .operation_target
        .as_ref()
        .and_then(operation_target_artifact_id)
        .cloned();
    let patch_id = candidate.patch_id.clone();
    let input = InterventionApplyInput {
        source_state_id: synthesis.candidate_set.source_state_id.clone(),
        candidate,
        target_relpath: synthesis.candidate_set.target_relpath.clone(),
        expected_source_content: synthesis.candidate_set.source_content.clone(),
        repo_root: repo_root.to_path_buf(),
        base_artifact_id,
        patch_id,
    };
    let output =
        execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
            phase: "intervention_apply",
            detail: source.to_string(),
        })?;
    let artifact = InterventionApplyArtifact(&output);
    write_protocol_artifact(
        record_path,
        INTERVENTION_APPLY_PROCEDURE,
        &subject_id,
        None,
        None,
        &input,
        &output,
        &artifact,
    )?;
    Ok(output)
}

fn pending_prototype1_stages(stage_reached: Prototype1LoopStopAfter) -> Vec<&'static str> {
    match stage_reached {
        Prototype1LoopStopAfter::BaselineEval => {
            vec![
                "baseline protocol",
                "target selection",
                "intervention apply",
                "treatment arm",
                "compare",
            ]
        }
        Prototype1LoopStopAfter::BaselineProtocol => {
            vec![
                "target selection",
                "intervention apply",
                "treatment arm",
                "compare",
            ]
        }
        Prototype1LoopStopAfter::TargetSelection => {
            vec!["intervention apply", "treatment arm", "compare"]
        }
        Prototype1LoopStopAfter::InterventionApply => vec!["treatment arm", "compare"],
        Prototype1LoopStopAfter::Compare => Vec::new(),
    }
}

pub(crate) fn write_json_file_pretty<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

impl MbeRequestArgs {
    fn into_request(self) -> Result<crate::mbe::Request, PrepareError> {
        let options = mbe_options_with_workers(self.workers);

        match (self.run, self.instance) {
            (Some(run), None) => {
                if self.attempt.is_some() {
                    return Err(PrepareError::InvalidMbeRequest {
                        detail: "--attempt requires --instance, not --run".to_string(),
                    });
                }
                crate::mbe::Request::from_manifest(
                    run,
                    self.submission,
                    self.output_dir,
                    self.repo_dir,
                    options,
                )
            }
            (None, Some(instance)) => crate::mbe::Request::from_instance(
                &instance,
                self.attempt,
                self.submission,
                self.output_dir,
                self.repo_dir,
                options,
            ),
            (Some(_), Some(_)) => Err(PrepareError::InvalidMbeRequest {
                detail: "specify only one of --run or --instance".to_string(),
            }),
            (None, None) => Err(PrepareError::InvalidMbeRequest {
                detail: "specify --run <path> or --instance <id>".to_string(),
            }),
        }
    }
}

fn mbe_options_with_workers(workers: u32) -> crate::mbe::Options {
    let mut options = crate::mbe::Options::default();
    options.workers = crate::mbe::Workers {
        general: workers,
        build_image: workers,
        run_instance: workers,
    };
    options
}

impl MbeRunsCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        for candidate in crate::mbe::run_candidates(&self.instance)? {
            let latest = if candidate.latest { "latest" } else { "" };
            let submission = candidate
                .submission_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}",
                candidate.attempt,
                latest,
                candidate.execution_status,
                candidate.submission_status,
                candidate.run_id,
                candidate.run_manifest.display(),
                submission
            );
        }
        Ok(())
    }
}

impl MbeCampaignCandidatesCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        println!(
            "generation\tnode\tparent\tprimary_instance\tcohort\tnonempty\toracle\tbytes\tlines\tinstances\ttreatment_campaign"
        );
        for candidate in crate::mbe::campaign_candidates(&self.campaign, self.nonempty_only)? {
            let oracle = candidate.oracle_eligible_instance_count()?;
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                candidate.generation(),
                candidate.node_id(),
                candidate.parent_node_id().unwrap_or("-"),
                candidate.primary_instance_id(),
                candidate.cohort_size(),
                candidate.nonempty_instance_count(),
                oracle,
                candidate.total_fix_patch_bytes(),
                candidate.total_fix_patch_lines(),
                candidate.instance_ids().join(","),
                candidate.treatment_campaign_id()?,
            );
        }
        Ok(())
    }
}

impl MbeRunCampaignCandidateCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let candidate = crate::mbe::campaign_candidate_by_node(&self.campaign, &self.node)?;
        let request = crate::mbe::CohortRequest::from_campaign_candidate(
            &candidate,
            self.output_dir,
            self.repo_dir,
            mbe_options_with_workers(self.workers),
        )?;
        let run = request.run_harness(self.python)?;
        println!("node: {}", candidate.node_id());
        println!("cohort_instances: {}", candidate.cohort_size());
        println!("config: {}", run.written.path.display());
        println!("report: {}", run.written.report_path.display());
        println!("command: {}", run.invocation.command_line());
        println!(
            "submitted/completed/resolved/unresolved: {}/{}/{}/{}",
            run.report.submitted_instances,
            run.report.completed_instances,
            run.report.resolved_instances,
            run.report.unresolved_instances
        );
        println!("instance\tverdict\tdiagnostic\tusable\tinstance_report");
        for evaluation in run.evaluations {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                evaluation.evidence.instance_id,
                evaluation.evidence.verdict,
                evaluation.diagnostic,
                evaluation.usable_for_selection,
                evaluation.instance_report_path.display()
            );
        }
        Ok(())
    }
}

impl MbeRunCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let request = self.request.into_request()?;
        let run = request.run_harness(self.python)?;
        println!("config: {}", run.written.path.display());
        println!("report: {}", run.written.report_path.display());
        println!("command: {}", run.invocation.command_line());
        println!(
            "verdict: {}\t{}",
            run.evidence.report_id, run.evidence.verdict
        );
        println!(
            "instance_report: {}",
            run.evaluation.instance_report_path.display()
        );
        println!("diagnostic: {}", run.evaluation.diagnostic);
        println!(
            "usable_for_selection: {}",
            run.evaluation.usable_for_selection
        );
        Ok(())
    }
}

impl MbeWriteConfigCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let request = self.request.into_request()?;
        let written = request.write_config()?;
        let invocation = written.harness_invocation(self.python);
        println!("config: {}", written.path.display());
        println!("report: {}", written.report_path.display());
        println!("command: {}", invocation.command_line());
        Ok(())
    }
}

impl MbeVerdictCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let prepared = crate::spec::PreparedSingleRun::load_manifest(self.run)?;
        let report = crate::mbe::FinalReport::load(&self.report)?;
        let evidence = crate::mbe::OracleEvidence::from_report(&prepared, self.report, &report)?;
        let layout = crate::mbe::Layout::under(
            evidence
                .report_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            PathBuf::new(),
        );
        let evaluation = crate::mbe::OracleEvaluation::from_evidence(&prepared, evidence, &layout)?;
        println!(
            "{}\t{}\t{}\t{}",
            evaluation.evidence.report_id,
            evaluation.evidence.verdict,
            evaluation.diagnostic,
            evaluation.usable_for_selection
        );
        Ok(())
    }
}

impl InspectCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            InspectSubcommand::Conversations(cmd) => cmd.run().await,
            InspectSubcommand::ToolCalls(cmd) => cmd.run().await,
            InspectSubcommand::ToolOverview(cmd) => cmd.run().await,
            InspectSubcommand::DbSnapshots(cmd) => cmd.run().await,
            InspectSubcommand::Failures(cmd) => cmd.run().await,
            InspectSubcommand::Config(cmd) => cmd.run().await,
            InspectSubcommand::Operational(cmd) => cmd.run().await,
            InspectSubcommand::Turn(cmd) => cmd.run().await,
            InspectSubcommand::Query(cmd) => cmd.run().await,
            InspectSubcommand::ProtocolArtifacts(cmd) => cmd.run().await,
            InspectSubcommand::ProtocolOverview(cmd) => cmd.run().await,
            InspectSubcommand::IssueOverview(cmd) => cmd.run().await,
        }
    }
}

impl ProtocolCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ProtocolSubcommand::Status(cmd) => cmd.run().await,
            ProtocolSubcommand::Run(cmd) => cmd.run().await,
            ProtocolSubcommand::IssueDetection(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallReview(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallIntentSegments(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallSegmentReview(cmd) => cmd.run().await,
        }
    }
}

impl CampaignCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            CampaignSubcommand::List(cmd) => cmd.run().await,
            CampaignSubcommand::Init(cmd) => cmd.run().await,
            CampaignSubcommand::Show(cmd) => cmd.run().await,
            CampaignSubcommand::Validate(cmd) => cmd.run().await,
            CampaignSubcommand::ExportSubmissions(cmd) => cmd.run().await,
        }
    }
}

impl ClosureCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ClosureSubcommand::Recompute(cmd) => cmd.run().await,
            ClosureSubcommand::Status(cmd) => cmd.run().await,
            ClosureSubcommand::Advance(cmd) => cmd.run().await,
        }
    }
}

impl ClosureAdvanceCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ClosureAdvanceSubcommand::Eval(cmd) => cmd.run().await,
            ClosureAdvanceSubcommand::Protocol(cmd) => cmd.run().await,
            ClosureAdvanceSubcommand::All(cmd) => cmd.run().await,
        }
    }
}

impl CampaignOverrideArgs {
    fn into_overrides(self) -> CampaignOverrides {
        CampaignOverrides {
            dataset_keys: self.dataset_key,
            dataset_files: self.dataset,
            model_id: self.model_id,
            provider_slug: self.provider,
            route_source: self.route_source,
            required_procedures: self.required_procedure,
            instances_root: self.instances_root,
            batches_root: self.batches_root,
        }
    }
}

impl CampaignInitCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let path = campaign_manifest_path(&self.campaign)?;
        if path.exists() && !self.force {
            return Err(PrepareError::DatabaseSetup {
                phase: "campaign_init",
                detail: format!(
                    "campaign manifest already exists at '{}' (pass --force to overwrite)",
                    path.display()
                ),
            });
        }

        let overrides = self.overrides.into_overrides();
        let mut manifest = if self.from_closure_state {
            adopt_campaign_manifest_from_closure_state(&self.campaign)?
        } else if self.from_registry {
            adopt_campaign_manifest_from_registry(&self.campaign)?
        } else {
            let closure_path = campaign_closure_state_path(&self.campaign)?;
            if closure_path.exists() && overrides.is_empty() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "campaign_init",
                    detail: format!(
                        "closure state exists at '{}' but no manifest exists; pass --from-closure-state to adopt it",
                        closure_path.display()
                    ),
                });
            }
            CampaignManifest::new(self.campaign.clone())
        };
        apply_campaign_overrides(&mut manifest, &overrides)?;
        let saved_path = save_campaign_manifest(&manifest)?;
        let resolved = resolve_campaign_config(&self.campaign, &CampaignOverrides::default())?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!("manifest: {}", saved_path.display());
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "manifest_path": saved_path,
                    "config": resolved,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignListCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let campaigns = list_campaigns()?;
        match self.format {
            InspectOutputFormat::Table => {
                if campaigns.is_empty() {
                    println!("campaigns: none");
                } else {
                    println!("campaigns");
                    for campaign in &campaigns {
                        let status = match (campaign.has_manifest, campaign.has_closure_state) {
                            (true, true) => "manifest+closure",
                            (true, false) => "manifest-only",
                            (false, true) => "closure-only",
                            (false, false) => "empty",
                        };
                        println!("  - {} | {}", campaign.campaign_id, status);
                    }
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&campaigns).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignShowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!(
                    "manifest: {}",
                    campaign_manifest_path(&self.campaign)?.display()
                );
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&resolved).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignValidateCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let checks = validate_campaign_config(&resolved).await?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!("\nvalidation");
                for check in &checks {
                    println!("  - {}: {}", check.label, check.detail);
                }
            }
            InspectOutputFormat::Json => {
                let payload = CampaignValidationView {
                    config: resolved,
                    checks,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignExportSubmissionsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let state = load_closure_state(&self.campaign)?;
        let records = collect_campaign_submission_records(&state, self.nonempty_only)?;
        let complete_eval_rows = state
            .instances
            .iter()
            .filter(|row| row.eval_status == ClosureClass::Complete)
            .count();
        let empty_patch_rows = count_campaign_empty_patch_rows(&state)?;
        let output_path = self
            .output
            .unwrap_or(default_campaign_submission_export_path(
                &self.campaign,
                self.nonempty_only,
            )?);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut jsonl = String::new();
        for record in &records {
            let line = serde_json::to_string(record).map_err(PrepareError::Serialize)?;
            jsonl.push_str(&line);
            jsonl.push('\n');
        }
        fs::write(&output_path, jsonl).map_err(|source| PrepareError::WriteManifest {
            path: output_path.clone(),
            source,
        })?;

        let summary = CampaignSubmissionExportSummary {
            campaign_id: self.campaign,
            closure_state_path: closure_state_path(&state.campaign_id)?,
            output_path,
            exported_records: records.len(),
            nonempty_only: self.nonempty_only,
            complete_eval_rows,
            empty_patch_rows_skipped: if self.nonempty_only {
                empty_patch_rows
            } else {
                0
            },
            failed_eval_rows: state
                .instances
                .iter()
                .filter(|row| row.eval_status == ClosureClass::Failed)
                .count(),
        };

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "campaign {} | exported {} submission records{}",
                    summary.campaign_id,
                    summary.exported_records,
                    if summary.nonempty_only {
                        " (non-empty only)"
                    } else {
                        ""
                    }
                );
                println!("closure: {}", summary.closure_state_path.display());
                println!("output: {}", summary.output_path.display());
                println!("complete eval rows: {}", summary.complete_eval_rows);
                println!(
                    "empty patch rows skipped: {}",
                    summary.empty_patch_rows_skipped
                );
                println!("failed eval rows: {}", summary.failed_eval_rows);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl ClosureRecomputeCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let (path, state) = recompute_closure_state(closure_request_from_campaign(&resolved))?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_closure_status(&state));
                println!("state: {}", path.display());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl ClosureStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let state = load_closure_state(&self.campaign)?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_closure_status(&state));
                println!("state: {}", closure_state_path(&self.campaign)?.display());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl ProtocolToolCallReviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_review_subject(&record, self.index)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();

        let client = reqwest::Client::new();
        let cfg = protocol_llm_config(
            self.model_id,
            self.route_source,
            self.provider,
            TOOL_CALL_REVIEW_TIMEOUT_SECS,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            default_protocol_max_tokens(),
            ProtocolReasoningPolicy::default(),
        )?;
        let reviewed =
            run_tool_call_review_with_json_retries(subject, &cfg, &client, self.index).await?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &reviewed.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &reviewed.output,
            &reviewed.artifact,
        )?;
        let review = &reviewed.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", reviewed.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!("Provider: {}", cfg.provider_display());
                println!("Artifact: {}", persisted_path.display());
                println!(
                    "Target: {:?} {}",
                    review.packet.target_kind, review.packet.target_id
                );
                println!("Scope: {}", review.packet.scope_summary);
                println!("Turns: {}", join_turns(&review.packet.turn_span));
                println!("Calls in scope: {}", review.packet.total_calls_in_scope);
                if let Some(focal_index) = review.packet.focal_call_index {
                    println!("Focal call index: {}", focal_index);
                }
                println!("Calls:");
                for call in &review.packet.calls {
                    let marker = if Some(call.index) == review.packet.focal_call_index {
                        "focal"
                    } else {
                        "scope"
                    };
                    println!(
                        "  {:<6} [{}] {} | {}",
                        marker, call.index, call.tool_name, call.summary
                    );
                }
                println!();
                println!("Signals");
                println!("{}", "-".repeat(40));
                println!(
                    "Repeated tool calls: {}",
                    review.signals.repeated_tool_name_count
                );
                println!("Distinct tools: {}", review.signals.distinct_tool_count);
                println!(
                    "Similar searches: {}",
                    review.signals.similar_search_neighbors
                );
                println!("Directory pivots: {}", review.signals.directory_pivots);
                println!("Search calls: {}", review.signals.search_calls_in_scope);
                println!("Read calls: {}", review.signals.read_calls_in_scope);
                println!("Browse calls: {}", review.signals.browse_calls_in_scope);
                println!(
                    "Candidate concerns: {:?}",
                    review.signals.candidate_concerns
                );
                println!();
                println!("Assessments");
                println!("{}", "-".repeat(40));
                println!(
                    "Usefulness: {:?} ({:?})",
                    review.usefulness.verdict, review.usefulness.confidence
                );
                println!("  {}", review.usefulness.rationale);
                println!(
                    "Redundancy: {:?} ({:?})",
                    review.redundancy.verdict, review.redundancy.confidence
                );
                println!("  {}", review.redundancy.rationale);
                println!(
                    "Recoverability: {:?} ({:?})",
                    review.recoverability.verdict, review.recoverability.confidence
                );
                println!("  {}", review.recoverability.rationale);
                println!();
                println!(
                    "Overall: {:?} ({:?})",
                    review.overall, review.overall_confidence
                );
                println!("Synthesis: {}", review.synthesis_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": reviewed.procedure_name,
                    "persisted_artifact_path": persisted_path,
                    "output": reviewed.output,
                    "artifact": reviewed.artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let mut state = protocol_state_for_run(&instance_id, &record_path)?;
        state.next_command =
            protocol_next_command(&instance_id, &record_path, &state.next_step, true, false);

        match self.format {
            InspectOutputFormat::Table => {
                print_protocol_state_table(&state);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolRunCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let mut before = protocol_state_for_run(&instance_id, &record_path)?;
        before.next_command =
            protocol_next_command(&instance_id, &record_path, &before.next_step, true, false);

        let progress_guard = match self.format {
            InspectOutputFormat::Table => Some(install_protocol_progress_printer()),
            InspectOutputFormat::Json => None,
        };

        let executed = match before.next_step.clone() {
            ProtocolNextStep::Ineligible => None,
            ProtocolNextStep::IntentSegmentation => {
                execute_protocol_intent_segments_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.route_source,
                    self.provider.clone(),
                    default_protocol_max_tokens(),
                    ProtocolReasoningPolicy::default(),
                )
                .await?;
                Some("tool_call_intent_segmentation".to_string())
            }
            ProtocolNextStep::ToolCallReview { index } => {
                execute_protocol_tool_call_review_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.route_source,
                    self.provider.clone(),
                    index,
                    default_protocol_max_tokens(),
                    ProtocolReasoningPolicy::default(),
                )
                .await?;
                Some(format!("tool_call_review[{index}]"))
            }
            ProtocolNextStep::ToolCallSegmentReview { segment_index } => {
                execute_protocol_tool_call_segment_review_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.route_source,
                    self.provider.clone(),
                    segment_index,
                    default_protocol_max_tokens(),
                    ProtocolReasoningPolicy::default(),
                )
                .await?;
                Some(format!("tool_call_segment_review[{segment_index}]"))
            }
            ProtocolNextStep::Complete | ProtocolNextStep::Blocked => None,
        };

        let mut after = protocol_state_for_run(&instance_id, &record_path)?;
        after.next_command =
            protocol_next_command(&instance_id, &record_path, &after.next_step, true, false);

        drop(progress_guard);

        match self.format {
            InspectOutputFormat::Table => {
                if let Some(executed) = &executed {
                    println!("protocol run");
                    println!("{}", "-".repeat(40));
                    println!("executed: {}", executed);
                    println!();
                } else {
                    println!("protocol run");
                    println!("{}", "-".repeat(40));
                    println!("executed: (none)");
                    println!();
                }
                print_protocol_state_table(&after);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "executed": executed,
                    "before": before,
                    "after": after,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolIssueDetectionCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject_id = record.metadata.benchmark.instance_id.clone();
        let protocol_aggregate = load_protocol_aggregate(&record_path).ok();
        let detection_input = IssueDetectionInput::from_record(record, protocol_aggregate);
        let persisted_input = issue_detection_artifact_input(&detection_input);
        let output = detect_issue_cases(&detection_input);
        let artifact = build_issue_detection_artifact(&output);
        let persisted_path = write_protocol_artifact(
            &record_path,
            INTERVENTION_ISSUE_DETECTION_PROCEDURE,
            &subject_id,
            None,
            None,
            &persisted_input,
            &output,
            &artifact,
        )?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", INTERVENTION_ISSUE_DETECTION_PROCEDURE);
                println!("{}", "-".repeat(40));
                println!("Artifact: {}", persisted_path.display());
                println!("Cases: {}", output.cases.len());
                if let Some(primary) = select_primary_issue(&output) {
                    print_issue_case_block("Primary issue", &primary);
                } else {
                    println!("Primary issue: (none)");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": INTERVENTION_ISSUE_DETECTION_PROCEDURE,
                    "persisted_artifact_path": persisted_path,
                    "output": output,
                    "artifact": artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

struct ProtocolProgressGuard;

impl Drop for ProtocolProgressGuard {
    fn drop(&mut self) {
        set_procedure_debug_sink(None);
    }
}

fn install_protocol_progress_printer() -> ProtocolProgressGuard {
    let sink: ProcedureDebugSink = Arc::new(|event: &ProcedureDebugEvent| match event.event {
        ProcedureDebugEventKind::ProcedureStarted => {
            if event.request_label.is_none() {
                eprintln!("protocol progress: {} started", event.procedure_name);
            }
        }
        ProcedureDebugEventKind::ProcedureFinished => {
            if event.request_label.is_none() {
                if let Some(elapsed_ms) = event.elapsed_ms {
                    eprintln!(
                        "protocol progress: {} finished ({} ms)",
                        event.procedure_name, elapsed_ms
                    );
                } else {
                    eprintln!("protocol progress: {} finished", event.procedure_name);
                }
            }
        }
        ProcedureDebugEventKind::ProcedureFailed => {
            let detail = event.detail.as_deref().unwrap_or("unknown error");
            if let Some(elapsed_ms) = event.elapsed_ms {
                eprintln!(
                    "protocol progress: {} failed after {} ms: {}",
                    event.procedure_name, elapsed_ms, detail
                );
            } else {
                eprintln!(
                    "protocol progress: {} failed: {}",
                    event.procedure_name, detail
                );
            }
        }
        ProcedureDebugEventKind::SubrequestStarted => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            match (event.request_index, event.request_total) {
                (Some(index), Some(total)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) sent",
                        index, total, label
                    );
                }
                _ => eprintln!("protocol progress: model request ({}) sent", label),
            }
        }
        ProcedureDebugEventKind::SubrequestFinished => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            match (event.request_index, event.request_total, event.elapsed_ms) {
                (Some(index), Some(total), Some(elapsed_ms)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) received ({} ms)",
                        index, total, label, elapsed_ms
                    );
                }
                (Some(index), Some(total), None) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) received",
                        index, total, label
                    );
                }
                _ => eprintln!("protocol progress: model request ({}) received", label),
            }
        }
        ProcedureDebugEventKind::SubrequestFailed => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            let detail = event.detail.as_deref().unwrap_or("unknown error");
            match (event.request_index, event.request_total, event.elapsed_ms) {
                (Some(index), Some(total), Some(elapsed_ms)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) failed after {} ms: {}",
                        index, total, label, elapsed_ms, detail
                    );
                }
                (Some(index), Some(total), None) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) failed: {}",
                        index, total, label, detail
                    );
                }
                _ => eprintln!(
                    "protocol progress: model request ({}) failed: {}",
                    label, detail
                ),
            }
        }
    });

    set_procedure_debug_sink(Some(sink));
    ProtocolProgressGuard
}

impl ClosureAdvanceEvalCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let mut policy = resolved.eval.clone();
        if let Some(limit) = self.limit {
            policy.limit = Some(limit);
        }
        if self.stop_on_error {
            policy.stop_on_error = true;
        }
        let report = advance_eval_closure(&resolved, &policy, self.dry_run, None).await?;
        render_advance_eval_report(report, self.format)
    }
}

impl ClosureAdvanceProtocolCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let mut policy = resolved.protocol.clone();
        if let Some(limit_runs) = self.limit_runs {
            policy.limit_runs = Some(limit_runs);
        }
        if let Some(max_concurrency) = self.max_concurrency {
            policy.max_concurrency = max_concurrency.max(1);
        }
        if self.stop_on_error {
            policy.stop_on_error = true;
        }
        let report = advance_protocol_closure(&resolved, &policy, self.dry_run).await?;
        render_advance_protocol_report(report, self.format)
    }
}

impl ClosureAdvanceAllCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let mut eval_policy = resolved.eval.clone();
        if let Some(limit) = self.eval_limit {
            eval_policy.limit = Some(limit);
        }
        if self.stop_on_error {
            eval_policy.stop_on_error = true;
        }
        let mut protocol_policy = resolved.protocol.clone();
        if let Some(limit_runs) = self.protocol_limit_runs {
            protocol_policy.limit_runs = Some(limit_runs);
        }
        if let Some(max_concurrency) = self.protocol_max_concurrency {
            protocol_policy.max_concurrency = max_concurrency.max(1);
        }
        if self.stop_on_error {
            protocol_policy.stop_on_error = true;
        }

        let eval_report = advance_eval_closure(&resolved, &eval_policy, self.dry_run, None).await?;
        let protocol_report =
            advance_protocol_closure(&resolved, &protocol_policy, self.dry_run).await?;
        render_advance_all_report(
            ClosureAdvanceAllReport {
                campaign_id: resolved.campaign_id,
                dry_run: self.dry_run,
                eval: eval_report,
                protocol: protocol_report,
            },
            self.format,
        )
    }
}

#[derive(Debug, Serialize)]
struct CampaignValidationView {
    config: ResolvedCampaignConfig,
    checks: Vec<CampaignValidationCheck>,
}

#[derive(Debug, Clone, Serialize)]
struct CampaignSubmissionExportSummary {
    campaign_id: String,
    closure_state_path: PathBuf,
    output_path: PathBuf,
    exported_records: usize,
    complete_eval_rows: usize,
    empty_patch_rows_skipped: usize,
    failed_eval_rows: usize,
    nonempty_only: bool,
}

#[derive(Debug, Clone, Serialize)]
struct EvalBatchPlan {
    batch_id: String,
    dataset_label: String,
    dataset_path: PathBuf,
    instances: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ClosureAdvanceEvalReport {
    campaign_id: String,
    dry_run: bool,
    before: crate::closure::EvalClosureSummary,
    after: crate::closure::EvalClosureSummary,
    selected_instances: Vec<String>,
    selected_batches: Vec<EvalBatchPlan>,
    executed_batches: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunPlan {
    instance_id: String,
    segmentation_needed: bool,
    missing_call_indices: Vec<usize>,
    missing_segment_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunState {
    instance_id: String,
    tool_calls_total: usize,
    protocol_eligible: bool,
    artifact_count: usize,
    segmentation_present: bool,
    call_review_count: usize,
    segment_review_count: usize,
    aggregate_available: bool,
    missing_call_indices: Vec<usize>,
    missing_segment_indices: Vec<usize>,
    next_step: ProtocolNextStep,
    next_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aggregate_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ProtocolNextStep {
    Ineligible,
    IntentSegmentation,
    ToolCallReview { index: usize },
    ToolCallSegmentReview { segment_index: usize },
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunTask {
    instance_id: String,
    record_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunExecution {
    instance_id: String,
    plan: ProtocolRunPlan,
    segmentations_created: usize,
    call_reviews_created: usize,
    segment_reviews_created: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ClosureAdvanceProtocolReport {
    campaign_id: String,
    dry_run: bool,
    before: crate::closure::ProtocolClosureSummary,
    after: crate::closure::ProtocolClosureSummary,
    selected_runs: Vec<ProtocolRunPlan>,
    executed_runs: usize,
    segmentations_created: usize,
    call_reviews_created: usize,
    segment_reviews_created: usize,
    failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ClosureAdvanceAllReport {
    campaign_id: String,
    dry_run: bool,
    eval: ClosureAdvanceEvalReport,
    protocol: ClosureAdvanceProtocolReport,
}

fn closure_request_from_campaign(config: &ResolvedCampaignConfig) -> ClosureRecomputeRequest {
    config.closure_recompute_request()
}

fn campaign_context_from_config(config: &ResolvedCampaignConfig) -> PreparedCampaignContext {
    PreparedCampaignContext {
        campaign_id: config.campaign_id.clone(),
        model_id: Some(config.model_id.clone()),
        provider_slug: config.provider_slug.clone(),
        framework: config.framework.clone(),
    }
}

fn default_campaign_submission_export_path(
    campaign_id: &str,
    nonempty_only: bool,
) -> Result<PathBuf, PrepareError> {
    let file_name = if nonempty_only {
        "multi-swe-bench-submission.nonempty.jsonl"
    } else {
        "multi-swe-bench-submission.jsonl"
    };
    Ok(crate::layout::campaigns_dir()?
        .join(campaign_id)
        .join(file_name))
}

fn collect_campaign_submission_records(
    state: &crate::closure::ClosureState,
    nonempty_only: bool,
) -> Result<Vec<MultiSweBenchSubmissionRecord>, PrepareError> {
    let mut records = Vec::new();
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if nonempty_only && record.fix_patch.trim().is_empty() {
            continue;
        }
        records.push(record);
    }
    Ok(records)
}

fn count_campaign_empty_patch_rows(
    state: &crate::closure::ClosureState,
) -> Result<usize, PrepareError> {
    let mut count = 0;
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if record.fix_patch.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

fn load_submission_record_for_row(
    state: &crate::closure::ClosureState,
    row: &crate::closure::ClosureInstanceRow,
) -> Result<MultiSweBenchSubmissionRecord, PrepareError> {
    let path = if let Some(path) = row.artifacts.msb_submission.as_ref() {
        path.clone()
    } else {
        let instance_root = state.config.instances_root.join(&row.instance_id);
        let run_dir = preferred_run_dir_for_instance(
            &state.config.instances_root,
            &row.instance_id,
            RunDirPreference::PreferTreatmentWithSubmission,
        )?
        .unwrap_or(instance_root);
        run_dir.join("multi-swe-bench-submission.jsonl")
    };
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(text.trim()).map_err(|source| PrepareError::ParseManifest { path, source })
}

fn select_eval_rows<'a>(
    state: &'a crate::closure::ClosureState,
    policy: &EvalCampaignPolicy,
) -> Vec<&'a crate::closure::ClosureInstanceRow> {
    let mut selected = state
        .instances
        .iter()
        .filter(|row| match row.eval_status {
            crate::closure::ClosureClass::Missing => true,
            crate::closure::ClosureClass::Partial => policy.include_partial,
            _ => false,
        })
        .filter(|row| {
            (policy.include_dataset_labels.is_empty()
                || policy
                    .include_dataset_labels
                    .iter()
                    .any(|label| label == &row.dataset_label))
                && !policy
                    .exclude_dataset_labels
                    .iter()
                    .any(|label| label == &row.dataset_label)
        })
        .collect::<Vec<_>>();
    if let Some(limit) = policy.limit {
        selected.truncate(limit);
    }
    selected
}

fn select_protocol_rows<'a>(
    state: &'a crate::closure::ClosureState,
    policy: &ProtocolCampaignPolicy,
) -> Vec<&'a crate::closure::ClosureInstanceRow> {
    let mut selected = state
        .instances
        .iter()
        .filter(|row| row.eval_status == crate::closure::ClosureClass::Complete)
        .filter(|row| match row.protocol_status {
            crate::closure::ClosureClass::Missing => true,
            crate::closure::ClosureClass::Partial => policy.include_partial,
            crate::closure::ClosureClass::Incompatible => policy.include_incompatible,
            crate::closure::ClosureClass::Failed => policy.include_failed,
            _ => false,
        })
        .collect::<Vec<_>>();
    if let Some(limit) = policy.limit_runs {
        selected.truncate(limit);
    }
    selected
}

fn build_eval_batch_plans(
    registry: &TargetRegistry,
    selected_rows: &[&crate::closure::ClosureInstanceRow],
    campaign_id: &str,
    batch_prefix: Option<&str>,
) -> Result<Vec<EvalBatchPlan>, PrepareError> {
    let mut by_instance = BTreeMap::<String, RegistryEntry>::new();
    for entry in &registry.entries {
        by_instance.insert(entry.instance_id.clone(), entry.clone());
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let prefix = batch_prefix.unwrap_or(campaign_id);
    let mut grouped = BTreeMap::<(String, PathBuf), Vec<String>>::new();
    for row in selected_rows {
        let entry =
            by_instance
                .get(&row.instance_id)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "closure_advance_eval",
                    detail: format!("registry entry missing for '{}'", row.instance_id),
                })?;
        grouped
            .entry((row.dataset_label.clone(), entry.source.dataset_path.clone()))
            .or_default()
            .push(row.instance_id.clone());
    }

    let mut plans = Vec::new();
    for ((dataset_label, dataset_path), mut instances) in grouped {
        instances.sort();
        let batch_id = format!(
            "{}-eval-{}-{}",
            sanitize_batch_component(prefix),
            sanitize_batch_component(&dataset_label),
            timestamp
        );
        plans.push(EvalBatchPlan {
            batch_id,
            dataset_label,
            dataset_path,
            instances,
        });
    }
    plans.sort_by(|left, right| left.batch_id.cmp(&right.batch_id));
    Ok(plans)
}

fn ensure_eval_plan_repo_cache(
    registry: &TargetRegistry,
    plan: &EvalBatchPlan,
    repo_cache: &Path,
) -> Result<(), PrepareError> {
    let mut by_instance = BTreeMap::<String, RegistryEntry>::new();
    for entry in &registry.entries {
        by_instance.insert(entry.instance_id.clone(), entry.clone());
    }

    let mut cloned = BTreeSet::<(String, String)>::new();
    for instance_id in &plan.instances {
        let entry = by_instance
            .get(instance_id)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "closure_eval_repo_cache",
                detail: format!("registry entry missing for '{instance_id}'"),
            })?;
        let key = (entry.source.org.clone(), entry.source.repo.clone());
        if !cloned.insert(key.clone()) {
            continue;
        }
        clone_repo_into_cache(&key.0, &key.1, repo_cache)?;
    }
    Ok(())
}

fn clone_repo_into_cache(org: &str, repo: &str, repo_cache: &Path) -> Result<(), PrepareError> {
    let source = repos_dir()?.join(org).join(repo);
    if !source.is_dir() {
        return Err(PrepareError::MissingRepoRoot(source));
    }
    let target = ensure_repo_cache_clone_preflight(org, repo, &source, repo_cache)?;
    let org_dir = target
        .parent()
        .expect("repo cache clone target should include org directory");
    fs::create_dir_all(org_dir).map_err(|source| PrepareError::CreateOutputDir {
        path: org_dir.to_path_buf(),
        source,
    })?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|source| PrepareError::WriteManifest {
            path: target.clone(),
            source,
        })?;
    }
    let command_label = format!(
        "git clone --no-hardlinks {} {}",
        source.display(),
        target.display()
    );
    let output = ProcessCommand::new("git")
        .args(["clone", "--no-hardlinks"])
        .arg(&source)
        .arg(&target)
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
    Ok(())
}

fn ensure_repo_cache_clone_preflight(
    org: &str,
    repo: &str,
    source: &Path,
    repo_cache: &Path,
) -> Result<PathBuf, PrepareError> {
    ensure_repo_cache_component("org", org)?;
    ensure_repo_cache_component("repo", repo)?;

    let source_root = source
        .canonicalize()
        .map_err(|err| PrepareError::Canonicalize {
            path: source.to_path_buf(),
            source: err,
        })?;
    let cache_root = repo_cache
        .canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: repo_cache.to_path_buf(),
            source,
        })?;

    if source_root.starts_with(&cache_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "repo cache override '{}' contains shared source repo '{}'",
                cache_root.display(),
                source_root.display()
            ),
        });
    }

    let target = cache_root.join(org).join(repo);
    if target.exists() {
        let target_root = target
            .canonicalize()
            .map_err(|source| PrepareError::Canonicalize {
                path: target.clone(),
                source,
            })?;
        if target_root == source_root
            || target_root.starts_with(&source_root)
            || source_root.starts_with(&target_root)
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "repo cache clone target '{}' overlaps shared source repo '{}'",
                    target_root.display(),
                    source_root.display()
                ),
            });
        }
    }

    Ok(target)
}

fn ensure_repo_cache_component(label: &str, value: &str) -> Result<(), PrepareError> {
    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(()),
        _ => Err(PrepareError::InvalidBatchSelection {
            detail: format!("invalid {label} component for repo cache clone: '{value}'"),
        }),
    }
}

fn ensure_prepared_runs_under_repo_cache(
    runs: &[crate::spec::PreparedSingleRun],
    repo_cache: &Path,
) -> Result<(), PrepareError> {
    let cache_root = repo_cache
        .canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: repo_cache.to_path_buf(),
            source,
        })?;
    for run in runs {
        let run_root =
            run.repo_root
                .canonicalize()
                .map_err(|source| PrepareError::Canonicalize {
                    path: run.repo_root.clone(),
                    source,
                })?;
        if run_root.starts_with(&cache_root) {
            continue;
        }
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prepared MBE run '{}' repo root '{}' escaped repo cache override '{}'",
                run.task_id,
                run_root.display(),
                cache_root.display()
            ),
        });
    }
    Ok(())
}

pub(crate) async fn advance_eval_closure(
    config: &ResolvedCampaignConfig,
    policy: &EvalCampaignPolicy,
    dry_run: bool,
    repo_cache_override: Option<&Path>,
) -> Result<ClosureAdvanceEvalReport, PrepareError> {
    let before_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    let selected_rows = select_eval_rows(&before_state, policy);
    let registry = recompute_target_registry(RegistryRecomputeRequest {
        benchmark_family: config.benchmark_family,
        dataset_keys: dataset_keys_from_sources(&config.dataset_sources),
        dataset_files: dataset_files_from_sources(&config.dataset_sources),
    })?
    .1;
    let plans = build_eval_batch_plans(
        &registry,
        &selected_rows,
        &config.campaign_id,
        policy.batch_prefix.as_deref(),
    )?;
    let selected_instances = plans
        .iter()
        .flat_map(|plan| plan.instances.iter().cloned())
        .collect::<Vec<_>>();

    let mut executed_batches = 0usize;
    if !dry_run {
        let campaign_context = campaign_context_from_config(config);
        let provider = parse_provider_key(config.provider_slug.clone())?;
        for plan in &plans {
            let repo_cache = repo_cache_override
                .map(Path::to_path_buf)
                .unwrap_or(repos_dir()?);
            if repo_cache_override.is_some() {
                ensure_eval_plan_repo_cache(&registry, plan, &repo_cache)?;
            }
            let mut prepared = PrepareMsbBatchRequest {
                dataset_file: Some(plan.dataset_path.clone()),
                dataset_key: None,
                batch_id: plan.batch_id.clone(),
                select_all: false,
                instance_ids: plan.instances.clone(),
                specifics: Vec::new(),
                limit: None,
                repo_cache,
                instances_root: config.instances_root.clone(),
                batches_root: config.batches_root.clone(),
                budget: policy.budget.clone(),
            }
            .prepare()?;

            if let Some(repo_cache) = repo_cache_override {
                ensure_prepared_runs_under_repo_cache(&prepared.runs, repo_cache)?;
            }

            for run in &mut prepared.runs {
                run.campaign = Some(campaign_context.clone());
                run.write_manifest(OutputMode::Pretty, PrepareWrite::File(run.manifest_path()))?;
            }
            prepared.batch.campaign = Some(campaign_context.clone());
            prepared.batch.write_manifest(OutputMode::Pretty)?;

            let artifacts = execute_batch_eval_for_manifest(
                prepared.batch.manifest_path(),
                true,
                false,
                Some(config.model_id.clone()),
                provider.clone(),
                policy.stop_on_error,
            )
            .await?;
            executed_batches += 1;

            if policy.stop_on_error && batch_summary_has_failure(&artifacts.summary)? {
                break;
            }
        }
    }

    let after_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    Ok(ClosureAdvanceEvalReport {
        campaign_id: config.campaign_id.clone(),
        dry_run,
        before: before_state.eval,
        after: after_state.eval,
        selected_instances,
        selected_batches: plans,
        executed_batches,
    })
}

pub(crate) async fn advance_protocol_closure(
    config: &ResolvedCampaignConfig,
    policy: &ProtocolCampaignPolicy,
    dry_run: bool,
) -> Result<ClosureAdvanceProtocolReport, PrepareError> {
    let before_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    let selected_rows = select_protocol_rows(&before_state, policy);
    let selected_order = selected_rows
        .iter()
        .map(|row| row.instance_id.clone())
        .collect::<Vec<_>>();
    let tasks =
        selected_rows
            .iter()
            .map(|row| {
                let record_path = row.artifacts.record_path.clone().ok_or_else(|| {
                    PrepareError::DatabaseSetup {
                        phase: "closure_advance_protocol",
                        detail: format!("record path missing for '{}'", row.instance_id),
                    }
                })?;
                Ok(ProtocolRunTask {
                    instance_id: row.instance_id.clone(),
                    record_path,
                })
            })
            .collect::<Result<Vec<_>, PrepareError>>()?;
    let mut plans_by_instance = BTreeMap::<String, ProtocolRunPlan>::new();
    let mut executed_runs = 0usize;
    let mut segmentations_created = 0usize;
    let mut call_reviews_created = 0usize;
    let mut segment_reviews_created = 0usize;
    let mut failures = Vec::new();

    if dry_run {
        for task in tasks {
            let plan = protocol_run_plan(&task.instance_id, &task.record_path)?;
            plans_by_instance.insert(task.instance_id, plan);
        }
    } else {
        for task in &tasks {
            if let Ok(plan) = protocol_run_plan(&task.instance_id, &task.record_path) {
                plans_by_instance.insert(task.instance_id.clone(), plan);
            }
        }
        let execution = execute_protocol_run_tasks(
            tasks,
            policy.model_id_for(&config.model_id),
            policy.route_source_for(config.route_source),
            policy.provider_slug_for(config.provider_slug.as_deref()),
            policy.max_concurrency,
            policy.tool_review_parallelism,
            policy.stop_on_error,
            policy.max_tokens,
            policy.reasoning,
        )
        .await?;
        failures = execution.failures;
        for run in execution.executions {
            executed_runs += 1;
            segmentations_created += run.segmentations_created;
            call_reviews_created += run.call_reviews_created;
            segment_reviews_created += run.segment_reviews_created;
            plans_by_instance.insert(run.instance_id.clone(), run.plan);
        }
    }

    let plans = selected_order
        .into_iter()
        .filter_map(|instance_id| plans_by_instance.remove(&instance_id))
        .collect::<Vec<_>>();

    let after_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    Ok(ClosureAdvanceProtocolReport {
        campaign_id: config.campaign_id.clone(),
        dry_run,
        before: before_state.protocol,
        after: after_state.protocol,
        selected_runs: plans,
        executed_runs,
        segmentations_created,
        call_reviews_created,
        segment_reviews_created,
        failures,
    })
}

pub(crate) async fn advance_protocol_or_block(
    config: &ResolvedCampaignConfig,
    policy: &ProtocolCampaignPolicy,
) -> Result<(), PrepareError> {
    let report = advance_protocol_closure(config, policy, false).await?;
    if protocol_report_allows_continue(&report) {
        return Ok(());
    }

    Err(protocol_no_progress_error(config, policy, &report))
}

fn protocol_report_allows_continue(report: &ClosureAdvanceProtocolReport) -> bool {
    protocol_report_made_progress(report) || report.after.status == ClosureClass::Complete
}

fn protocol_report_made_progress(report: &ClosureAdvanceProtocolReport) -> bool {
    report.segmentations_created > 0
        || report.call_reviews_created > 0
        || report.segment_reviews_created > 0
        || protocol_summary_changed(&report.before, &report.after)
}

fn protocol_summary_changed(
    before: &crate::closure::ProtocolClosureSummary,
    after: &crate::closure::ProtocolClosureSummary,
) -> bool {
    before.expected_total != after.expected_total
        || before.full_total != after.full_total
        || before.partial_total != after.partial_total
        || before.failed_total != after.failed_total
        || before.missing_total != after.missing_total
        || before.incompatible_total != after.incompatible_total
        || before.ineligible_total != after.ineligible_total
        || before.in_progress_total != after.in_progress_total
        || before.status != after.status
        || before.required_procedures != after.required_procedures
        || before.last_transition_at != after.last_transition_at
        || before.status_by_procedure.len() != after.status_by_procedure.len()
        || before
            .status_by_procedure
            .iter()
            .any(|(procedure, before)| {
                after
                    .status_by_procedure
                    .get(procedure)
                    .is_none_or(|after| procedure_summary_changed(before, after))
            })
}

fn procedure_summary_changed(
    before: &crate::closure::ProcedureClosureSummary,
    after: &crate::closure::ProcedureClosureSummary,
) -> bool {
    before.expected_total != after.expected_total
        || before.complete_total != after.complete_total
        || before.failed_total != after.failed_total
        || before.missing_total != after.missing_total
        || before.incompatible_total != after.incompatible_total
        || before.partial_total != after.partial_total
        || before.ineligible_total != after.ineligible_total
}

fn protocol_no_progress_error(
    config: &ResolvedCampaignConfig,
    policy: &ProtocolCampaignPolicy,
    report: &ClosureAdvanceProtocolReport,
) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: format!(
            "baseline_protocol blocked: campaign {} made no protocol progress; model {}; route {}; selected_runs={}; remaining={}; created={{segmentations:{}, call_reviews:{}, segment_reviews:{}}}; failures={}",
            report.campaign_id,
            policy.model_id_for(&config.model_id),
            protocol_route_detail(config, policy),
            format_protocol_selected_runs(&report.selected_runs),
            format_remaining_protocol_work(&report.after),
            report.segmentations_created,
            report.call_reviews_created,
            report.segment_reviews_created,
            format_protocol_failures(&report.failures),
        ),
    }
}

fn protocol_route_detail(
    config: &ResolvedCampaignConfig,
    policy: &ProtocolCampaignPolicy,
) -> String {
    let model_id = policy.model_id_for(&config.model_id);
    let route = model_id.parse::<ModelId>().ok().and_then(|model_id| {
        resolve_protocol_route(
            &model_id,
            policy.route_source_for(config.route_source),
            policy.provider_slug_for(config.provider_slug.as_deref()),
        )
        .ok()
    });
    match route {
        Some((route_source, provider_slug)) => match provider_slug {
            Some(provider) => format!("{route_source:?}/{provider}"),
            None => format!("{route_source:?}"),
        },
        None => policy
            .provider_slug_for(config.provider_slug.as_deref())
            .as_deref()
            .unwrap_or("unresolved")
            .to_string(),
    }
}

fn format_protocol_selected_runs(runs: &[ProtocolRunPlan]) -> String {
    if runs.is_empty() {
        return "none".to_string();
    }

    let mut parts = runs
        .iter()
        .take(3)
        .map(|run| {
            format!(
                "{}(segmentation_needed={}, missing_calls={}, missing_segments={})",
                run.instance_id,
                run.segmentation_needed,
                run.missing_call_indices.len(),
                run.missing_segment_indices.len()
            )
        })
        .collect::<Vec<_>>();
    if runs.len() > parts.len() {
        parts.push(format!("+{} more", runs.len() - parts.len()));
    }
    parts.join(", ")
}

fn format_remaining_protocol_work(summary: &crate::closure::ProtocolClosureSummary) -> String {
    let mut remaining = summary
        .status_by_procedure
        .iter()
        .filter_map(|(procedure, status)| {
            let incomplete = status.missing_total
                + status.partial_total
                + status.failed_total
                + status.incompatible_total;
            (incomplete > 0).then(|| {
                format!(
                    "{}(missing={}, partial={}, failed={}, incompatible={})",
                    procedure,
                    status.missing_total,
                    status.partial_total,
                    status.failed_total,
                    status.incompatible_total
                )
            })
        })
        .collect::<Vec<_>>();
    if remaining.is_empty() {
        remaining.push("none".to_string());
    }
    remaining.join(", ")
}

fn format_protocol_failures(failures: &[String]) -> String {
    if failures.is_empty() {
        return "none".to_string();
    }

    let mut parts = failures.iter().take(3).cloned().collect::<Vec<_>>();
    if failures.len() > parts.len() {
        parts.push(format!("+{} more", failures.len() - parts.len()));
    }
    parts.join(" | ")
}

fn spawn_protocol_run_task(
    join_set: &mut JoinSet<Result<ProtocolRunExecution, PrepareError>>,
    task: ProtocolRunTask,
    model_id: String,
    route_source: Option<ModelRouteSource>,
    provider_slug: Option<String>,
    review_permits: Arc<Semaphore>,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) {
    join_set.spawn(async move {
        execute_protocol_run_task(
            task,
            model_id,
            route_source,
            provider_slug,
            review_permits,
            max_tokens,
            reasoning,
        )
        .await
    });
}

async fn execute_protocol_run_task(
    task: ProtocolRunTask,
    model_id: String,
    route_source: Option<ModelRouteSource>,
    provider_slug: Option<String>,
    review_permits: Arc<Semaphore>,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) -> Result<ProtocolRunExecution, PrepareError> {
    let mut plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        }
    })?;
    let mut segmentations_created = 0usize;
    let mut call_reviews_created = 0usize;
    let mut segment_reviews_created = 0usize;

    if plan.segmentation_needed {
        execute_protocol_intent_segments_quiet(
            &task.record_path,
            Some(model_id.clone()),
            route_source,
            provider_slug.clone(),
            max_tokens,
            reasoning,
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        })?;
        segmentations_created += 1;
        plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
            PrepareError::DatabaseSetup {
                phase: "closure_advance_protocol",
                detail: format!("{}: {err}", task.instance_id),
            }
        })?;
    }

    let call_subjects = call_review_subjects(&task.record_path, &plan.missing_call_indices)?;
    let call_reviews = review_calls(
        call_subjects,
        protocol_llm_config(
            Some(model_id.clone()),
            route_source,
            provider_slug.clone(),
            TOOL_CALL_REVIEW_TIMEOUT_SECS,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            max_tokens,
            reasoning,
        )?,
        review_permits.clone(),
    )
    .await
    .map_err(|err| PrepareError::DatabaseSetup {
        phase: "closure_advance_protocol",
        detail: format!("{}: {err}", task.instance_id),
    })?;
    for review in call_reviews {
        write_call_review(&task.record_path, review).map_err(|err| {
            PrepareError::DatabaseSetup {
                phase: "closure_advance_protocol",
                detail: format!("{}: {err}", task.instance_id),
            }
        })?;
        call_reviews_created += 1;
    }

    plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        }
    })?;
    for segment_index in plan.missing_segment_indices.clone() {
        execute_protocol_tool_call_segment_review_quiet(
            &task.record_path,
            Some(model_id.clone()),
            route_source,
            provider_slug.clone(),
            segment_index,
            max_tokens,
            reasoning,
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        })?;
        segment_reviews_created += 1;
    }

    let plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        }
    })?;
    Ok(ProtocolRunExecution {
        instance_id: task.instance_id,
        plan,
        segmentations_created,
        call_reviews_created,
        segment_reviews_created,
    })
}

fn protocol_run_plan(
    instance_id: &str,
    record_path: &Path,
) -> Result<ProtocolRunPlan, PrepareError> {
    match load_protocol_aggregate(record_path) {
        Ok(aggregate) => Ok(ProtocolRunPlan {
            instance_id: instance_id.to_string(),
            segmentation_needed: false,
            missing_call_indices: aggregate.coverage.missing_call_indices,
            missing_segment_indices: aggregate.coverage.missing_segment_indices,
        }),
        Err(err) => match err {
            crate::protocol::protocol_aggregate::ProtocolAggregateError::MissingAnchor {
                ..
            } => Ok(ProtocolRunPlan {
                instance_id: instance_id.to_string(),
                segmentation_needed: true,
                missing_call_indices: Vec::new(),
                missing_segment_indices: Vec::new(),
            }),
            other => Err(PrepareError::DatabaseSetup {
                phase: "closure_advance_protocol",
                detail: format!("{}: {other}", instance_id),
            }),
        },
    }
}

fn protocol_state_for_run(
    instance_id: &str,
    record_path: &Path,
) -> Result<ProtocolRunState, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let tool_calls_total = record.tool_calls().len();
    let protocol_eligible = tool_calls_total > 0;
    let artifacts = list_protocol_artifacts(record_path)?;
    let artifact_count = artifacts.len();
    let mut call_review_count = 0usize;
    let mut segment_review_count = 0usize;
    let mut segmentation_present = false;
    for artifact in &artifacts {
        match artifact.stored.procedure_name.as_str() {
            "tool_call_intent_segmentation" => segmentation_present = true,
            "tool_call_review" => call_review_count += 1,
            "tool_call_segment_review" => segment_review_count += 1,
            _ => {}
        }
    }

    if !protocol_eligible {
        return Ok(ProtocolRunState {
            instance_id: instance_id.to_string(),
            tool_calls_total,
            protocol_eligible,
            artifact_count,
            segmentation_present,
            call_review_count,
            segment_review_count,
            aggregate_available: false,
            missing_call_indices: Vec::new(),
            missing_segment_indices: Vec::new(),
            next_step: ProtocolNextStep::Ineligible,
            next_command: None,
            aggregate_error: None,
        });
    }

    match load_protocol_aggregate(record_path) {
        Ok(aggregate) => {
            let next_step = if let Some(index) = aggregate.coverage.missing_call_indices.first() {
                ProtocolNextStep::ToolCallReview { index: *index }
            } else if let Some(segment_index) = aggregate.coverage.missing_segment_indices.first() {
                ProtocolNextStep::ToolCallSegmentReview {
                    segment_index: *segment_index,
                }
            } else {
                ProtocolNextStep::Complete
            };
            let next_command =
                protocol_next_command(instance_id, record_path, &next_step, false, false);
            Ok(ProtocolRunState {
                instance_id: instance_id.to_string(),
                tool_calls_total,
                protocol_eligible,
                artifact_count,
                segmentation_present: true,
                call_review_count,
                segment_review_count,
                aggregate_available: true,
                missing_call_indices: aggregate.coverage.missing_call_indices,
                missing_segment_indices: aggregate.coverage.missing_segment_indices,
                next_step,
                next_command,
                aggregate_error: None,
            })
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) => {
            let next_step = ProtocolNextStep::IntentSegmentation;
            let next_command =
                protocol_next_command(instance_id, record_path, &next_step, false, false);
            Ok(ProtocolRunState {
                instance_id: instance_id.to_string(),
                tool_calls_total,
                protocol_eligible,
                artifact_count,
                segmentation_present,
                call_review_count,
                segment_review_count,
                aggregate_available: false,
                missing_call_indices: Vec::new(),
                missing_segment_indices: Vec::new(),
                next_step,
                next_command,
                aggregate_error: None,
            })
        }
        Err(err) => {
            let next_step = ProtocolNextStep::Blocked;
            let next_command =
                protocol_next_command(instance_id, record_path, &next_step, false, false);
            Ok(ProtocolRunState {
                instance_id: instance_id.to_string(),
                tool_calls_total,
                protocol_eligible,
                artifact_count,
                segmentation_present,
                call_review_count,
                segment_review_count,
                aggregate_available: false,
                missing_call_indices: Vec::new(),
                missing_segment_indices: Vec::new(),
                next_step,
                next_command,
                aggregate_error: Some(err.to_string()),
            })
        }
    }
}

fn protocol_next_command(
    _instance_id: &str,
    _record_path: &Path,
    next_step: &ProtocolNextStep,
    prefer_run_wrapper: bool,
    for_expert_command: bool,
) -> Option<String> {
    match next_step {
        ProtocolNextStep::Ineligible => None,
        ProtocolNextStep::IntentSegmentation => {
            if prefer_run_wrapper {
                Some("ploke-eval protocol run".to_string())
            } else {
                Some("ploke-eval protocol tool-call-intent-segments".to_string())
            }
        }
        ProtocolNextStep::ToolCallReview { index } => {
            if prefer_run_wrapper {
                Some("ploke-eval protocol run".to_string())
            } else {
                Some(format!("ploke-eval protocol tool-call-review {}", index))
            }
        }
        ProtocolNextStep::ToolCallSegmentReview { segment_index } => {
            if prefer_run_wrapper {
                Some("ploke-eval protocol run".to_string())
            } else {
                Some(format!(
                    "ploke-eval protocol tool-call-segment-review {}",
                    segment_index
                ))
            }
        }
        ProtocolNextStep::Complete => None,
        ProtocolNextStep::Blocked => {
            if for_expert_command {
                Some("ploke-eval inspect protocol-artifacts --full".to_string())
            } else {
                None
            }
        }
    }
}

fn print_protocol_state_table(state: &ProtocolRunState) {
    println!("protocol state");
    println!("{}", "-".repeat(40));
    println!("instance: {}", state.instance_id);
    println!(
        "eligible: {}",
        if state.protocol_eligible { "yes" } else { "no" }
    );
    println!("tool calls: {}", state.tool_calls_total);
    println!("protocol artifacts: {}", state.artifact_count);
    println!(
        "segmentation: {}",
        if state.segmentation_present {
            "present"
        } else {
            "(none found)"
        }
    );
    println!("review_calls: {}", state.call_review_count);
    println!("review_segments: {}", state.segment_review_count);
    println!(
        "aggregate overview: {}",
        if state.aggregate_available {
            "available"
        } else {
            "unavailable"
        }
    );
    if !state.missing_call_indices.is_empty() {
        println!(
            "missing call reviews: {}",
            join_indices(&state.missing_call_indices)
        );
    }
    if !state.missing_segment_indices.is_empty() {
        println!(
            "missing segment reviews: {}",
            join_indices(&state.missing_segment_indices)
        );
    }
    if let Some(error) = &state.aggregate_error {
        println!("aggregate error: {}", error);
    }
    if let Some(next_command) = &state.next_command {
        println!();
        println!("next command to advance:");
        println!("  {next_command}");
    }
}

fn batch_summary_has_failure(path: &Path) -> Result<bool, PrepareError> {
    let text = std::fs::read_to_string(path).map_err(|source| PrepareError::ReadBatchManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let summary: BatchRunSummary =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseBatchManifest {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(summary.instances_failed > 0)
}

async fn execute_protocol_intent_segments_quiet(
    record_path: &Path,
    model_id: Option<String>,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) -> Result<segment::SegmentedToolCallSequence, PrepareError> {
    const MAX_SEGMENTATION_ATTEMPTS: usize = 3;

    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject = build_tool_call_sequence_subject(&record)?;
    let subject_id = subject.subject_id.clone();
    let persisted_input = subject.clone();
    let client = reqwest::Client::new();
    let cfg = protocol_llm_config(
        model_id,
        route_source,
        provider,
        120,
        PROTOCOL_HTTP_MAX_ATTEMPTS,
        max_tokens,
        reasoning,
    )?;
    let segmented = 'retry: loop {
        for attempt in 1..=MAX_SEGMENTATION_ATTEMPTS {
            let protocol = segment::ToolCallIntentSegmentation::new(JsonAdjudicator::new(
                client.clone(),
                cfg.clone(),
            ));
            match protocol.run(subject.clone()).await {
                Ok(segmented) => break 'retry segmented,
                Err(err)
                    if is_retryable_intent_segmentation_error(&err)
                        && attempt < MAX_SEGMENTATION_ATTEMPTS =>
                {
                    eprintln!(
                        "protocol progress: retrying tool_call_intent_segmentation attempt {}/{} after invalid adjudicated segmentation: {}",
                        attempt + 1,
                        MAX_SEGMENTATION_ATTEMPTS,
                        err
                    );
                }
                Err(err) => {
                    return Err(tool_call_intent_segmentation_error_to_prepare(err));
                }
            }
        }
        unreachable!();
    };
    write_protocol_artifact(
        record_path,
        &segmented.procedure_name,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &persisted_input,
        &segmented.output,
        &segmented.artifact,
    )?;
    Ok(segmented.output)
}

async fn execute_protocol_tool_call_review_quiet(
    record_path: &Path,
    model_id: Option<String>,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
    index: usize,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) -> Result<(), PrepareError> {
    let subject = call_review_subjects(record_path, &[index])?
        .into_iter()
        .next()
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "protocol_tool_call_review",
            detail: format!("tool call index {index} was not selected"),
        })?;
    let review = review_call(
        subject,
        protocol_llm_config(
            model_id,
            route_source,
            provider,
            TOOL_CALL_REVIEW_TIMEOUT_SECS,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            max_tokens,
            reasoning,
        )?,
        Arc::new(Semaphore::new(1)),
    )
    .await?;
    write_call_review(record_path, review)?;
    Ok(())
}

#[derive(Debug)]
struct CallReview {
    index: usize,
    subject_id: String,
    input: trace::ToolCallNeighborhood,
    config: JsonLlmConfig,
    reviewed: ploke_protocol::ProcedureRun<
        review::LocalAnalysisAssessment,
        review::ToolCallReviewArtifact,
    >,
}

fn call_review_subjects(
    record_path: &Path,
    indices: &[usize],
) -> Result<Vec<trace::ToolCallNeighborhood>, PrepareError> {
    if indices.is_empty() {
        return Ok(Vec::new());
    }

    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    indices
        .iter()
        .map(|index| build_tool_call_review_subject(&record, *index))
        .collect()
}

pub(crate) fn protocol_llm_config(
    model_id: Option<String>,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
    timeout_secs: u64,
    max_attempts: u32,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) -> Result<JsonLlmConfig, PrepareError> {
    let model_id = resolve_protocol_model_id(model_id)?;
    let (route_source, provider_slug) = resolve_protocol_route(&model_id, route_source, provider)?;
    let reasoning = effective_protocol_reasoning(route_source, reasoning);
    Ok(JsonLlmConfig {
        model_id: model_id.to_string(),
        route_source,
        provider_slug,
        timeout_secs,
        max_attempts,
        max_tokens,
        reasoning,
    })
}

fn effective_protocol_reasoning(
    route_source: ModelRouteSource,
    reasoning: ProtocolReasoningPolicy,
) -> ProtocolReasoningPolicy {
    if route_source.is_direct_google() && reasoning == ProtocolReasoningPolicy::auto() {
        ProtocolReasoningPolicy::disabled()
    } else {
        reasoning
    }
}

async fn review_calls(
    subjects: Vec<trace::ToolCallNeighborhood>,
    config: JsonLlmConfig,
    permits: Arc<Semaphore>,
) -> Result<Vec<CallReview>, PrepareError> {
    let mut join_set = JoinSet::new();
    for subject in subjects {
        let config = config.clone();
        let permits = permits.clone();
        join_set.spawn(async move { review_call(subject, config, permits).await });
    }

    let mut reviews = Vec::new();
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(Ok(review)) => reviews.push(review),
            Ok(Err(err)) => {
                join_set.abort_all();
                return Err(err);
            }
            Err(err) => {
                join_set.abort_all();
                return Err(PrepareError::DatabaseSetup {
                    phase: "protocol_tool_call_review",
                    detail: format!("tool review task failed: {err}"),
                });
            }
        }
    }

    reviews.sort_by_key(|review| review.index);
    Ok(reviews)
}

async fn review_call(
    subject: trace::ToolCallNeighborhood,
    config: JsonLlmConfig,
    permits: Arc<Semaphore>,
) -> Result<CallReview, PrepareError> {
    let _permit = permits
        .acquire_owned()
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_tool_call_review",
            detail: format!("tool review permit closed: {err}"),
        })?;
    let index = subject.focal.index;
    let subject_id = subject.subject_id.clone();
    let input = subject.clone();
    let client = reqwest::Client::new();
    let reviewed =
        run_tool_call_review_with_json_retries(subject.clone(), &config, &client, index).await?;
    Ok(CallReview {
        index,
        subject_id,
        input,
        config,
        reviewed,
    })
}

async fn run_tool_call_review_with_json_retries(
    subject: trace::ToolCallNeighborhood,
    config: &JsonLlmConfig,
    client: &reqwest::Client,
    index: usize,
) -> Result<
    ploke_protocol::ProcedureRun<review::LocalAnalysisAssessment, review::ToolCallReviewArtifact>,
    PrepareError,
> {
    for attempt in 1..=PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS {
        let protocol =
            review::ToolCallReview::new(JsonAdjudicator::new(client.clone(), config.clone()));
        match protocol.run(subject.clone()).await {
            Ok(reviewed) => return Ok(reviewed),
            Err(err)
                if is_retryable_local_analysis_review_error(&err)
                    && attempt < PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS =>
            {
                eprintln!(
                    "protocol progress: retrying tool_call_review[{index}] attempt {}/{} after malformed adjudication JSON: {}",
                    attempt + 1,
                    PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS,
                    err
                );
            }
            Err(err) => return Err(tool_call_review_error_to_prepare(err)),
        }
    }
    unreachable!("bounded review retry loop always returns");
}

async fn run_tool_call_segment_review_with_json_retries(
    subject: review::SegmentReviewSubject,
    config: &JsonLlmConfig,
    client: &reqwest::Client,
    segment_index: usize,
) -> Result<
    ploke_protocol::ProcedureRun<
        review::LocalAnalysisAssessment,
        review::ToolCallSegmentReviewArtifact,
    >,
    PrepareError,
> {
    for attempt in 1..=PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS {
        let protocol = review::ToolCallSegmentReview::new(JsonAdjudicator::new(
            client.clone(),
            config.clone(),
        ));
        match protocol.run(subject.clone()).await {
            Ok(reviewed) => return Ok(reviewed),
            Err(err)
                if is_retryable_local_analysis_review_error(&err)
                    && attempt < PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS =>
            {
                eprintln!(
                    "protocol progress: retrying tool_call_segment_review[{segment_index}] attempt {}/{} after malformed adjudication JSON: {}",
                    attempt + 1,
                    PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS,
                    err
                );
            }
            Err(err) => return Err(tool_call_segment_review_error_to_prepare(err)),
        }
    }
    unreachable!("bounded review retry loop always returns");
}

fn is_retryable_local_analysis_review_error(err: &review::ToolCallReviewError) -> bool {
    matches!(
        local_analysis_review_protocol_error(err),
        Some(ploke_protocol::ProtocolLlmError::ParseJson { .. })
    )
}

fn local_analysis_review_protocol_error(
    err: &review::ToolCallReviewError,
) -> Option<&ploke_protocol::ProtocolLlmError> {
    match err {
        ploke_protocol::SequenceError::Second(ploke_protocol::MergeError::Branches(
            ploke_protocol::FanOutError::Left(ploke_protocol::FanOutError::Left(err))
            | ploke_protocol::FanOutError::Left(ploke_protocol::FanOutError::Right(err))
            | ploke_protocol::FanOutError::Right(err),
        )) => Some(err),
        _ => None,
    }
}

fn write_call_review(record_path: &Path, review: CallReview) -> Result<(), PrepareError> {
    write_protocol_artifact(
        record_path,
        &review.reviewed.procedure_name,
        &review.subject_id,
        Some(review.config.model_id.as_str()),
        review.config.provider_slug.as_deref(),
        &review.input,
        &review.reviewed.output,
        &review.reviewed.artifact,
    )?;
    Ok(())
}

fn load_latest_segmented_sequence(
    record_path: &Path,
) -> Result<Option<segment::SegmentedToolCallSequence>, PrepareError> {
    let artifacts = list_protocol_artifacts(record_path)?;
    let latest = artifacts
        .into_iter()
        .filter(|entry| entry.stored.procedure_name == "tool_call_intent_segmentation")
        .max_by_key(|entry| entry.stored.created_at_ms);
    latest
        .map(|entry| {
            serde_json::from_value(entry.stored.output).map_err(|source| {
                PrepareError::DatabaseSetup {
                    phase: "protocol_load_segmented_sequence",
                    detail: source.to_string(),
                }
            })
        })
        .transpose()
}

async fn execute_protocol_tool_call_segment_review_quiet(
    record_path: &Path,
    model_id: Option<String>,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
    segment_index: usize,
    max_tokens: u32,
    reasoning: ProtocolReasoningPolicy,
) -> Result<(), PrepareError> {
    let segmented = match load_latest_segmented_sequence(record_path)? {
        Some(segmented) => segmented,
        None => {
            execute_protocol_intent_segments_quiet(
                record_path,
                model_id.clone(),
                route_source,
                provider.clone(),
                max_tokens,
                reasoning,
            )
            .await?
        }
    };
    let subject = build_segment_review_subject(&segmented, segment_index)?;
    let subject_id = subject.subject_id.clone();
    let persisted_input = subject.clone();
    let cfg = protocol_llm_config(
        model_id,
        route_source,
        provider,
        120,
        PROTOCOL_HTTP_MAX_ATTEMPTS,
        max_tokens,
        reasoning,
    )?;
    let client = reqwest::Client::new();
    let reviewed = run_tool_call_segment_review_with_json_retries(
        subject.clone(),
        &cfg,
        &client,
        segment_index,
    )
    .await?;
    write_protocol_artifact(
        record_path,
        &reviewed.procedure_name,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &persisted_input,
        &reviewed.output,
        &reviewed.artifact,
    )?;
    Ok(())
}

fn build_issue_detection_artifact(
    output: &IssueDetectionOutput,
) -> InterventionIssueDetectionArtifact<IssueCase> {
    InterventionIssueDetectionArtifact {
        case_count: output.cases.len(),
        primary_issue: select_primary_issue(output),
    }
}

fn issue_aggregate_error_to_prepare(err: IssueDetectionAggregateError) -> PrepareError {
    match err {
        IssueDetectionAggregateError::Source(source) => source,
        IssueDetectionAggregateError::MissingArtifact { record_path } => {
            PrepareError::DatabaseSetup {
                phase: "inspect_issue_overview",
                detail: format!(
                    "no persisted issue-detection artifact found for '{}'; run `ploke-eval protocol issue-detection --record {}` first",
                    record_path.display(),
                    record_path.display()
                ),
            }
        }
        IssueDetectionAggregateError::DeserializeOutput { path, detail } => {
            PrepareError::DatabaseSetup {
                phase: "inspect_issue_overview",
                detail: format!(
                    "failed to deserialize issue-detection output from '{}': {}",
                    path.display(),
                    detail
                ),
            }
        }
        IssueDetectionAggregateError::DeserializeInput { path, detail } => {
            PrepareError::DatabaseSetup {
                phase: "inspect_issue_overview",
                detail: format!(
                    "failed to deserialize issue-detection input from '{}': {}",
                    path.display(),
                    detail
                ),
            }
        }
    }
}

fn serde_name<T>(value: &T) -> String
where
    T: Serialize,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn print_issue_case_block(label: &str, issue: &IssueCase) {
    println!("{}:", label);
    println!("  selection_basis: {}", serde_name(&issue.selection_basis));
    println!("  target_tool: {}", issue.target_tool.as_str());
    println!(
        "  target_file: {}",
        issue.target_tool.description_artifact_relpath()
    );
    println!(
        "  evidence: reviewed_calls={} reviewed_issue_calls={}",
        issue.evidence.reviewed_call_count, issue.evidence.reviewed_issue_call_count
    );
    let protocol = &issue.evidence.protocol;
    if !protocol.reviewed_call_indices.is_empty() {
        println!("  reviewed_calls: {:?}", protocol.reviewed_call_indices);
    }
    if !protocol.reviewed_segment_indices.is_empty() {
        println!(
            "  reviewed_segments: {:?}",
            protocol.reviewed_segment_indices
        );
    }
    if !protocol.nearby_segment_labels.is_empty() {
        println!(
            "  nearby_segment_labels: {:?}",
            protocol.nearby_segment_labels
        );
    }
    if !protocol.candidate_concerns.is_empty() {
        println!("  candidate_concerns:");
        for concern in &protocol.candidate_concerns {
            println!("    - {}", concern);
        }
    }
}

fn print_issue_detection_aggregate(aggregate: &IssueDetectionAggregate) {
    println!("issue overview");
    println!("{}", "-".repeat(40));
    println!("Procedure: {}", aggregate.artifact.procedure_name);
    println!("Artifact: {}", aggregate.artifact.path.display());
    println!("Run: {}", aggregate.run.run_id);
    println!("Subject: {}", aggregate.run.subject_id);
    println!("Cases: {}", aggregate.output.cases.len());
    println!(
        "Protocol coverage: total_calls={} anchor_segments={} reviewed_calls={} reviewed_segments={} scanned_artifacts={}",
        aggregate.input.total_calls_in_run,
        aggregate.input.anchor_segment_count,
        aggregate.input.protocol_reviewed_call_count,
        aggregate.input.protocol_reviewed_segment_count,
        aggregate.input.protocol_artifact_count
    );

    if let Some(primary) = &aggregate.primary_issue {
        print_issue_case_block("Primary issue", primary);
    } else {
        println!("Primary issue: (none)");
    }

    if aggregate.output.cases.len() > 1 {
        println!("Other cases:");
        for issue in aggregate.output.cases.iter().skip(1) {
            println!(
                "  - {} ({})",
                issue.target_tool.as_str(),
                serde_name(&issue.selection_basis)
            );
        }
    }
}

fn render_advance_eval_report(
    report: ClosureAdvanceEvalReport,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Table => {
            println!(
                "campaign {} | eval before {} complete / {} fail / {} partial / {} missing",
                report.campaign_id,
                report.before.complete_total,
                report.before.failed_total,
                report.before.partial_total,
                report.before.missing_total
            );
            println!(
                "selected {} instance(s) in {} batch(es){}",
                report.selected_instances.len(),
                report.selected_batches.len(),
                if report.dry_run { " [dry-run]" } else { "" }
            );
            for batch in &report.selected_batches {
                println!(
                    "  - {} | {} | {} instance(s)",
                    batch.batch_id,
                    batch.dataset_label,
                    batch.instances.len()
                );
            }
            println!(
                "eval after {} complete / {} fail / {} partial / {} missing",
                report.after.complete_total,
                report.after.failed_total,
                report.after.partial_total,
                report.after.missing_total
            );
        }
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn render_advance_protocol_report(
    report: ClosureAdvanceProtocolReport,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Table => {
            println!(
                "campaign {} | protocol before {} full / {} partial / {} incompatible / {} fail / {} missing / {} ineligible",
                report.campaign_id,
                report.before.full_total,
                report.before.partial_total,
                report.before.incompatible_total,
                report.before.failed_total,
                report.before.missing_total,
                report.before.ineligible_total
            );
            println!(
                "selected {} run(s){}",
                report.selected_runs.len(),
                if report.dry_run { " [dry-run]" } else { "" }
            );
            for run in &report.selected_runs {
                println!(
                    "  - {} | segmentation {} | missing calls {} | missing segments {}",
                    run.instance_id,
                    if run.segmentation_needed { "yes" } else { "no" },
                    run.missing_call_indices.len(),
                    run.missing_segment_indices.len()
                );
            }
            println!(
                "protocol after {} full / {} partial / {} incompatible / {} fail / {} missing / {} ineligible",
                report.after.full_total,
                report.after.partial_total,
                report.after.incompatible_total,
                report.after.failed_total,
                report.after.missing_total,
                report.after.ineligible_total
            );
            if !report.failures.is_empty() {
                println!("\nfailures");
                for failure in &report.failures {
                    println!("  - {}", failure);
                }
            }
        }
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn render_advance_all_report(
    report: ClosureAdvanceAllReport,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Table => {
            println!(
                "campaign {}{}",
                report.campaign_id,
                if report.dry_run { " [dry-run]" } else { "" }
            );
            println!(
                "eval: selected {} instance(s), missing now {}",
                report.eval.selected_instances.len(),
                report.eval.after.missing_total
            );
            println!(
                "protocol: selected {} run(s), missing now {}",
                report.protocol.selected_runs.len(),
                report.protocol.after.missing_total
            );
        }
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

impl ProtocolToolCallSegmentReviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let sequence_subject = build_tool_call_sequence_subject(&record)?;
        let client = reqwest::Client::new();
        let cfg = protocol_llm_config(
            self.model_id,
            self.route_source,
            self.provider,
            120,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            default_protocol_max_tokens(),
            ProtocolReasoningPolicy::default(),
        )?;
        let adjudicator = JsonAdjudicator::new(client.clone(), cfg.clone());
        let segmentation = segment::ToolCallIntentSegmentation::new(adjudicator.clone())
            .run(sequence_subject)
            .await
            .map_err(tool_call_intent_segmentation_error_to_prepare)?;

        let subject = build_segment_review_subject(&segmentation.output, self.segment_index)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();
        let reviewed = run_tool_call_segment_review_with_json_retries(
            subject,
            &cfg,
            &client,
            self.segment_index,
        )
        .await?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &reviewed.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &reviewed.output,
            &reviewed.artifact,
        )?;
        let review = &reviewed.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", reviewed.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!("Provider: {}", cfg.provider_display());
                println!("Artifact: {}", persisted_path.display());
                println!(
                    "Target: {:?} {}",
                    review.packet.target_kind, review.packet.target_id
                );
                println!("Scope: {}", review.packet.scope_summary);
                println!("Turns: {}", join_turns(&review.packet.turn_span));
                println!("Calls in scope: {}", review.packet.total_calls_in_scope);
                println!("Calls:");
                for call in &review.packet.calls {
                    println!("  [{}] {} | {}", call.index, call.tool_name, call.summary);
                }
                println!();
                println!("Signals");
                println!("{}", "-".repeat(40));
                println!(
                    "Repeated tool calls: {}",
                    review.signals.repeated_tool_name_count
                );
                println!("Distinct tools: {}", review.signals.distinct_tool_count);
                println!("Directory pivots: {}", review.signals.directory_pivots);
                println!("Search calls: {}", review.signals.search_calls_in_scope);
                println!("Read calls: {}", review.signals.read_calls_in_scope);
                println!("Browse calls: {}", review.signals.browse_calls_in_scope);
                println!(
                    "Source labeled segments: {}",
                    review.signals.labeled_segments_in_source.unwrap_or(0)
                );
                println!(
                    "Source ambiguous segments: {}",
                    review.signals.ambiguous_segments_in_source.unwrap_or(0)
                );
                println!(
                    "Source uncovered calls: {}",
                    review.signals.uncovered_calls_in_source.unwrap_or(0)
                );
                println!(
                    "Candidate concerns: {:?}",
                    review.signals.candidate_concerns
                );
                println!();
                println!("Assessments");
                println!("{}", "-".repeat(40));
                println!(
                    "Usefulness: {:?} ({:?})",
                    review.usefulness.verdict, review.usefulness.confidence
                );
                println!("  {}", review.usefulness.rationale);
                println!(
                    "Redundancy: {:?} ({:?})",
                    review.redundancy.verdict, review.redundancy.confidence
                );
                println!("  {}", review.redundancy.rationale);
                println!(
                    "Recoverability: {:?} ({:?})",
                    review.recoverability.verdict, review.recoverability.confidence
                );
                println!("  {}", review.recoverability.rationale);
                println!();
                println!(
                    "Overall: {:?} ({:?})",
                    review.overall, review.overall_confidence
                );
                println!("Synthesis: {}", review.synthesis_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": reviewed.procedure_name,
                    "persisted_artifact_path": persisted_path,
                    "output": reviewed.output,
                    "artifact": reviewed.artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolToolCallIntentSegmentsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_sequence_subject(&record)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();

        let client = reqwest::Client::new();
        let cfg = protocol_llm_config(
            self.model_id,
            self.route_source,
            self.provider,
            120,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            default_protocol_max_tokens(),
            ProtocolReasoningPolicy::default(),
        )?;
        let protocol =
            segment::ToolCallIntentSegmentation::new(JsonAdjudicator::new(client, cfg.clone()));
        let segmented = protocol
            .run(subject)
            .await
            .map_err(tool_call_intent_segmentation_error_to_prepare)?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &segmented.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &segmented.output,
            &segmented.artifact,
        )?;
        let output = &segmented.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", segmented.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!("Provider: {}", cfg.provider_display());
                println!("Artifact: {}", persisted_path.display());
                println!("Turns: {}", output.sequence.total_turns);
                println!("Tool calls: {}", output.sequence.total_calls_in_run);
                println!("Segments: {}", output.segments.len());
                println!("Labeled segments: {}", output.coverage.labeled_segments);
                println!("Ambiguous segments: {}", output.coverage.ambiguous_segments);
                println!("Labeled calls: {}", output.coverage.labeled_calls);
                println!("Ambiguous calls: {}", output.coverage.ambiguous_calls);
                println!("Uncovered calls: {}", output.coverage.uncovered_calls);
                if !output.uncovered_call_indices.is_empty() {
                    println!(
                        "Uncovered indices: {}",
                        join_indices(&output.uncovered_call_indices)
                    );
                }
                if !output.uncovered_spans.is_empty() {
                    println!(
                        "Uncovered spans: {}",
                        output
                            .uncovered_spans
                            .iter()
                            .map(|span| format!("{}..={}", span.start_index, span.end_index))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                println!();
                println!("Sequence Signals");
                println!("{}", "-".repeat(40));
                println!("Search calls: {}", output.signals.search_calls);
                println!("Read calls: {}", output.signals.read_calls);
                println!("Browse calls: {}", output.signals.browse_calls);
                println!("Edit calls: {}", output.signals.edit_calls);
                println!("Failed calls: {}", output.signals.failed_calls);
                println!(
                    "Repeated search runs: {}",
                    output.signals.repeated_search_runs
                );
                println!("Directory pivots: {}", output.signals.directory_pivots);
                println!();
                println!("Intent Segments");
                println!("{}", "-".repeat(40));
                for segment in &output.segments {
                    println!(
                        "[{}] {} {}..={} confidence={:?}",
                        segment.segment_index,
                        format_segment_descriptor(segment.status, segment.label),
                        segment.start_index,
                        segment.end_index,
                        segment.confidence
                    );
                    println!("  turns ......... {}", join_turns(&segment.turns));
                    println!("  rationale ..... {}", segment.rationale);
                    for call in &segment.calls {
                        println!(
                            "  call .......... [{}] {} | {}",
                            call.index, call.tool_name, call.summary
                        );
                    }
                    println!();
                }
                if !output.uncovered_spans.is_empty() {
                    println!("Uncovered Regions");
                    println!("{}", "-".repeat(40));
                    for span in &output.uncovered_spans {
                        println!(
                            "{}..={} calls={}",
                            span.start_index,
                            span.end_index,
                            join_indices(&span.call_indices)
                        );
                        println!("  rationale ..... {}", span.rationale);
                    }
                    println!();
                }
                println!("Overall rationale");
                println!("{}", "-".repeat(40));
                println!("{}", output.overall_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "persisted_artifact_path": persisted_path,
                    "run": segmented,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectConversationsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        if matches!(self.format, InspectOutputFormat::Json) && !self.full {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_conversations",
                detail: "refusing to emit full conversation JSON without --full; this output can be very large. Re-run with `--format json --full` and pipe to `jq` or `rg`.".to_string(),
            });
        }

        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        match self.format {
            InspectOutputFormat::Table => {
                let summary = record.outcome_summary();
                let raw_usage = if summary.total_token_usage.total_tokens == 0 {
                    load_full_response_usage_totals(&record_path, &record)
                        .ok()
                        .flatten()
                } else {
                    None
                };
                let display_usage = raw_usage.as_ref().unwrap_or(&summary.total_token_usage);
                println!("Run summary");
                println!("{}", "-".repeat(40));
                println!("Turns: {}", summary.turn_count);
                println!(
                    "Token usage: {}",
                    format_usage_triplet(
                        display_usage.prompt_tokens,
                        display_usage.completion_tokens,
                        display_usage.total_tokens,
                    )
                );
                println!("Token cost: ${:.6}", summary.total_token_cost);
                if raw_usage.is_some() {
                    println!("Usage source: raw response sidecar");
                    println!("Usage note: may undercount if final stop response was not captured");
                }
                println!("Wall time: {:.3}s", summary.wall_clock_secs);
                println!();
                println!("{:<6} {:<6} {:<7} {}", "Turn", "Tools", "Failed", "Outcome");
                println!("{}", "-".repeat(40));
                for turn in record.conversations() {
                    let tool_count = turn.tool_calls().len();
                    println!(
                        "{:<6} {:<6} {:<7} {}",
                        turn.turn_number,
                        tool_count,
                        failed_tool_count(turn),
                        truncate_for_table(&summarize_turn_outcome(turn), 18),
                    );
                }
                println!("\nTotal turns: {}", record.conversations().count());
                if let Some(first_turn) = record.conversations().next() {
                    println!("Next:");
                    println!("  ploke-eval inspect turn {}", first_turn.turn_number);
                    println!(
                        "  ploke-eval inspect turn {} --show tool-calls",
                        first_turn.turn_number
                    );
                    println!(
                        "  ploke-eval inspect turn {} --show messages",
                        first_turn.turn_number
                    );
                }
            }
            InspectOutputFormat::Json => {
                let turns: Vec<_> = record.conversations().collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&turns).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectToolCallsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let indexed_tool_calls = indexed_tool_calls(&record);

        if let Some(index) = self.index {
            match self.format {
                InspectOutputFormat::Table => match indexed_tool_calls.get(index) {
                    Some((_, turn, call)) => {
                        print_tool_call_detail(*turn, index, call, self.full);
                    }
                    None => {
                        println!(
                            "Error: Index {} out of range. Run has {} tool call{} (valid indices: 0..{}).",
                            index,
                            indexed_tool_calls.len(),
                            if indexed_tool_calls.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            indexed_tool_calls.len().saturating_sub(1)
                        );
                    }
                },
                InspectOutputFormat::Json => match indexed_tool_calls.get(index) {
                    Some((_, turn, call)) => {
                        let payload = serde_json::json!({
                            "index": index,
                            "turn": turn,
                            "tool_call": call,
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&payload)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                    None => {
                        println!(
                            "Error: Index {} out of range. Run has {} tool call{} (valid indices: 0..{}).",
                            index,
                            indexed_tool_calls.len(),
                            if indexed_tool_calls.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            indexed_tool_calls.len().saturating_sub(1)
                        );
                    }
                },
            }
            print_record_resolution_footer(&resolution);
            return Ok(());
        }

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "{:<5} {:<6} {:<20} {:<48} {}",
                    "Idx", "Turn", "Tool", "Context", "Result"
                );
                println!("{}", "-".repeat(120));
                for (index, turn, call) in &indexed_tool_calls {
                    println!(
                        "{:<5} {:<6} {:<20} {:<48} {}",
                        index,
                        turn,
                        truncate_for_table(&call.request.tool, 18),
                        truncate_for_table(
                            &summarize_tool_inputs(&call.request.tool, &call.request.arguments),
                            46,
                        ),
                        truncate_for_table(&summarize_tool_result(&call.result), 28),
                    );
                }
                println!("\nTotal tool calls: {}", indexed_tool_calls.len());
                if !indexed_tool_calls.is_empty() {
                    println!("Next:");
                    let next_index = tool_call_next_step_index(indexed_tool_calls.len())
                        .expect("non-empty tool call list should have a next-step index");
                    println!("  ploke-eval inspect tool-calls {}", next_index);
                    println!("  ploke-eval inspect tool-calls --full {}", next_index);
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &indexed_tool_calls
                            .iter()
                            .map(|(index, turn, call)| {
                                serde_json::json!({
                                    "index": index,
                                    "turn": turn,
                                    "tool_call": call,
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectToolOverviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let report = collect_tool_campaign_overview(&self)?;

        match self.format {
            InspectOutputFormat::Table => print_tool_campaign_overview(&report, self.limit.max(1)),
            InspectOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            ),
        }

        Ok(())
    }
}

impl InspectDbSnapshotsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let snapshots = record.db_snapshots();

        match self.format {
            InspectOutputFormat::Table => {
                println!("{:<6} {}", "Turn", "DB Timestamp (micros)");
                println!("{}", "-".repeat(40));
                for (i, snapshot) in snapshots.iter().enumerate() {
                    println!("{:<6} {}", i + 1, snapshot.timestamp_micros());
                }
                println!("\nTotal snapshots: {}", snapshots.len());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snapshots).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectFailuresCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let failures = record.failures();

        match self.format {
            InspectOutputFormat::Table => {
                if failures.is_empty() {
                    println!("No failures found.");
                } else {
                    println!("{:<6} {:<24} {}", "Turn", "Started", "Error");
                    println!("{}", "-".repeat(80));
                    for turn in &failures {
                        let error_str = match &turn.outcome {
                            crate::record::TurnOutcome::Error { message } => {
                                message.chars().take(50).collect::<String>()
                            }
                            _ => "unknown".to_string(),
                        };
                        println!(
                            "{:<6} {:<24} {}",
                            turn.turn_number,
                            turn.started_at.chars().take(23).collect::<String>(),
                            error_str
                        );
                    }
                    println!("\nTotal failures: {}", failures.len());
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&failures).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectConfigCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let config = record.config();

        match self.format {
            InspectOutputFormat::Table => {
                println!("Run Configuration");
                println!("{}", "-".repeat(40));
                println!("Instance ID: {}", config.benchmark.instance_id);
                println!("Repository: {}", config.benchmark.repo_root.display());
                if let Some(sha) = &config.benchmark.base_sha {
                    println!("Base SHA: {}", sha);
                }
                println!("Max Turns: {}", config.budget.max_turns);
                println!("Max Tool Calls: {}", config.budget.max_tool_calls);
                println!("Wall Clock (secs): {}", config.budget.wall_clock_secs);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(config).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectOperationalCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let metrics = record.operational_metrics();

        match self.format {
            InspectOutputFormat::Table => {
                println!("Operational Metrics");
                println!("{}", "-".repeat(40));
                println!("Instance ID: {}", record.metadata.benchmark.instance_id);
                println!("Run arm: {}", record.metadata.run_arm.id);
                println!(
                    "Role: {}",
                    format!("{:?}", record.metadata.run_arm.role).to_lowercase()
                );
                println!("Patch apply state: {}", metrics.patch_apply_state.as_str());
                println!(
                    "Submission artifact: {}",
                    metrics.submission_artifact_state.as_str()
                );
                println!("Patch attempted: {}", yes_no(metrics.patch_attempted));
                println!("Aborted: {}", yes_no(metrics.aborted));
                println!(
                    "Aborted repair loop: {}",
                    yes_no(metrics.aborted_repair_loop)
                );
                println!(
                    "Nonempty valid patch: {}",
                    yes_no(metrics.nonempty_valid_patch)
                );
                println!("Convergence: {}", yes_no(metrics.convergence));
                println!("Oracle eligible: {}", yes_no(metrics.oracle_eligible));
                println!();
                println!("Counts");
                println!("{}", "-".repeat(40));
                println!("Tool calls total: {}", metrics.tool_calls_total);
                println!("Tool calls failed: {}", metrics.tool_calls_failed);
                println!("Partial patch failures: {}", metrics.partial_patch_failures);
                println!(
                    "Same-file patch retries: {}",
                    metrics.same_file_patch_retry_count
                );
                println!(
                    "Same-file max streak: {}",
                    metrics.same_file_patch_max_streak
                );
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "instance_id": record.metadata.benchmark.instance_id,
                    "run_arm": {
                        "id": record.metadata.run_arm.id,
                        "role": record.metadata.run_arm.role,
                        "command": record.metadata.run_arm.command,
                        "execution": record.metadata.run_arm.execution,
                    },
                    "metrics": metrics,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectTurnCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let turn_number =
            self.turn
                .or(self.turn_flag)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "inspect_turn",
                    detail: "Turn number is required (for example: `inspect turn 1`)".to_string(),
                })?;

        let turn = record
            .turn_record(turn_number)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "inspect_turn",
                detail: format!("Turn {} not found", turn_number),
            })?;

        match self.show {
            TurnShowOption::All => {
                print_turn_summary(turn);
                println!();
                println!("Next:");
                println!(
                    "  ploke-eval inspect turn {} --show responses",
                    turn.turn_number
                );
                println!("  ploke-eval inspect turn {} --show loop", turn.turn_number);
                println!(
                    "  ploke-eval inspect turn {} --show tool-calls",
                    turn.turn_number
                );
                println!(
                    "  ploke-eval inspect turn {} --show messages",
                    turn.turn_number
                );
                println!(
                    "  ploke-eval inspect turn {} --show messages --exclude-roles system,user",
                    turn.turn_number
                );
            }
            TurnShowOption::Messages => {
                let messages = filter_messages(turn.messages(), &self.roles, &self.exclude_roles);
                match self.format {
                    InspectOutputFormat::Table => {
                        println!("{}", render_messages_table(&messages));
                        println!(
                            "\nNext:\n  ploke-eval inspect turn {} --show messages --exclude-roles system,user",
                            turn.turn_number
                        );
                    }
                    InspectOutputFormat::Json => {
                        println!("{}", render_messages_json(&messages)?);
                    }
                }
            }
            TurnShowOption::Responses => {
                let trace_path = resolve_full_response_trace_path(&record_path, &record)?;
                let assistant_message_id =
                    assistant_message_id_for_turn(turn).ok_or_else(|| {
                        PrepareError::DatabaseSetup {
                            phase: "inspect_turn",
                            detail: format!(
                                "Turn {} does not expose an assistant_message_id in its artifact",
                                turn.turn_number
                            ),
                        }
                    })?;
                let responses =
                    load_full_response_records_for_turn(&trace_path, assistant_message_id)?;
                if responses.is_empty() {
                    println!("No raw full responses captured for this turn.");
                } else {
                    match self.format {
                        InspectOutputFormat::Table => {
                            println!(
                                "{}",
                                render_full_response_table(turn.turn_number, &responses)
                            );
                        }
                        InspectOutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&responses)
                                    .map_err(PrepareError::Serialize)?
                            );
                        }
                    }
                }
            }
            TurnShowOption::Loop => {
                let tool_calls = turn.tool_calls();
                match self.format {
                    InspectOutputFormat::Table => {
                        println!("{}", render_tool_loop_table(turn.turn_number, &tool_calls));
                        if !tool_calls.is_empty() {
                            println!("\nNext:");
                            println!(
                                "  ploke-eval inspect turn {} --show tool-call --index 0",
                                turn.turn_number
                            );
                            println!(
                                "  ploke-eval inspect turn {} --show tool-result --index 0",
                                turn.turn_number
                            );
                        }
                    }
                    InspectOutputFormat::Json => {
                        println!("{}", render_tool_loop_json(turn.turn_number, &tool_calls)?);
                    }
                }
            }
            TurnShowOption::ToolCalls => {
                let tool_calls = turn.tool_calls();
                if tool_calls.is_empty() {
                    println!("No tool calls in this turn.");
                } else {
                    match self.format {
                        InspectOutputFormat::Table => {
                            println!("{:<5} {:<20} {:<48} {}", "Idx", "Tool", "Context", "Result");
                            println!("{}", "-".repeat(110));
                            for (index, call) in tool_calls.iter().enumerate() {
                                println!(
                                    "{:<5} {:<20} {:<48} {}",
                                    index,
                                    truncate_for_table(&call.request.tool, 18),
                                    truncate_for_table(
                                        &summarize_tool_inputs(
                                            &call.request.tool,
                                            &call.request.arguments,
                                        ),
                                        46
                                    ),
                                    truncate_for_table(&summarize_tool_result(&call.result), 28),
                                );
                            }
                            println!("\nNext:");
                            println!(
                                "  ploke-eval inspect turn {} --show tool-call --index 0",
                                turn.turn_number
                            );
                            println!(
                                "  ploke-eval inspect turn {} --show tool-result --index 0",
                                turn.turn_number
                            );
                        }
                        InspectOutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&tool_calls)
                                    .map_err(PrepareError::Serialize)?
                            );
                        }
                    }
                }
            }
            TurnShowOption::ToolCall => {
                let tool_calls = turn.tool_calls();
                match (self.index, tool_calls.len()) {
                    (Some(idx), len) if idx < len => {
                        let tool_call = &tool_calls[idx];
                        match self.format {
                            InspectOutputFormat::Table => {
                                print_tool_call_detail(turn.turn_number, idx, tool_call, false);
                            }
                            InspectOutputFormat::Json => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&tool_call)
                                        .map_err(PrepareError::Serialize)?
                                );
                            }
                        }
                    }
                    (Some(idx), len) => {
                        println!(
                            "Error: Index {} out of range. Turn has {} tool call{} (valid indices: 0..{}).",
                            idx,
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                    (None, 1) => {
                        let tool_call = &tool_calls[0];
                        match self.format {
                            InspectOutputFormat::Table => {
                                print_tool_call_detail(turn.turn_number, 0, tool_call, false);
                            }
                            InspectOutputFormat::Json => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&tool_call)
                                        .map_err(PrepareError::Serialize)?
                                );
                            }
                        }
                    }
                    (None, len) => {
                        println!(
                            "Turn has {} tool call{}. Use --index 0..{} to select one.",
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                }
            }
            TurnShowOption::ToolResult => {
                let tool_calls = turn.tool_calls();
                match (self.index, tool_calls.len()) {
                    (Some(idx), len) if idx < len => {
                        let tool_call = &tool_calls[idx];
                        match self.format {
                            InspectOutputFormat::Table => {
                                print_tool_result_detail(idx, &tool_call.result, false);
                            }
                            InspectOutputFormat::Json => {
                                let result = &tool_calls[idx].result;
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&result)
                                        .map_err(PrepareError::Serialize)?
                                );
                            }
                        }
                    }
                    (Some(idx), len) => {
                        println!(
                            "Error: Index {} out of range. Turn has {} tool call{} (valid indices: 0..{}).",
                            idx,
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                    (None, 1) => match self.format {
                        InspectOutputFormat::Table => {
                            print_tool_result_detail(0, &tool_calls[0].result, false);
                        }
                        InspectOutputFormat::Json => {
                            let result = &tool_calls[0].result;
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&result)
                                    .map_err(PrepareError::Serialize)?
                            );
                        }
                    },
                    (None, len) => {
                        println!(
                            "Turn has {} tool call{}. Use --index 0..{} to select one.",
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                }
            }
            TurnShowOption::DbState => {
                let db_state = turn.db_state();
                println!("DB State for Turn {}", turn.turn_number);
                println!("Timestamp (micros): {}", db_state.timestamp_micros());
                println!(
                    "\nUse this timestamp with 'inspect query' to run queries against this state."
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectQueryCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        // Validate that we have either --turn or --timestamp
        if self.turn.is_none() && self.timestamp.is_none() {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: "Either --turn or --timestamp must be provided".to_string(),
            });
        }

        // Validate that we have either --lookup or query string
        if self.lookup.is_none() && self.query.is_none() {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: "Either --lookup <name> or a query string must be provided".to_string(),
            });
        }

        // Load the record
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        // Resolve timestamp
        let timestamp_micros = if let Some(turn) = self.turn {
            record
                .timestamp_for_turn(turn)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "inspect_query",
                    detail: format!("Turn {} not found in record", turn),
                })?
        } else {
            self.timestamp.unwrap() // Safe because we validated above
        };

        // Find DB path: look in run directory for final-snapshot.db first, fallback to indexing-checkpoint.db
        let run_dir = record_path
            .parent()
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: "Could not determine run directory from record path".to_string(),
            })?;

        let final_snapshot = run_dir.join("final-snapshot.db");
        let checkpoint_db = run_dir.join("indexing-checkpoint.db");

        let db_path = if final_snapshot.exists() {
            final_snapshot
        } else if checkpoint_db.exists() {
            checkpoint_db
        } else {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: format!(
                    "No DB snapshot found in {}. Looked for final-snapshot.db and indexing-checkpoint.db",
                    run_dir.display()
                ),
            });
        };

        // Open the database (async method)
        let db = ploke_db::Database::create_new_backup_default(&db_path)
            .await
            .map_err(|e| PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: format!("Failed to open database at {}: {}", db_path.display(), e),
            })?;

        // Create DbState with the timestamp
        let db_state = crate::record::DbState::new(timestamp_micros);

        // Execute query or lookup
        if let Some(name) = self.lookup {
            // Use db_state.lookup() for name-based lookup
            match db_state.lookup(&db, &name) {
                Ok(Some(node_info)) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&node_info)
                            .map_err(PrepareError::Serialize)?
                    );
                }
                Ok(None) => {
                    println!(
                        "Symbol '{}' not found at timestamp {}",
                        name, timestamp_micros
                    );
                }
                Err(e) => {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "inspect_query",
                        detail: format!("Lookup failed: {}", e),
                    });
                }
            }
        } else if let Some(query) = self.query {
            // Use db_state.query() for raw Cozo queries
            match db_state.query(&db, &query) {
                Ok(result) => {
                    // Convert QueryResult to JSON-serializable format
                    let rows: Vec<Vec<serde_json::Value>> = result
                        .rows
                        .iter()
                        .map(|row| row.iter().map(|val| cozo_data_to_json(val)).collect())
                        .collect();

                    let output = serde_json::json!({
                        "headers": result.headers,
                        "rows": rows,
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&output).map_err(PrepareError::Serialize)?
                    );
                }
                Err(e) => {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "inspect_query",
                        detail: format!("Query failed: {}", e),
                    });
                }
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectProtocolArtifactsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let artifacts = list_protocol_artifacts(&record_path)?;

        if let Some(index) = self.index {
            let selected = artifacts
                .get(index)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "inspect_protocol_artifacts",
                    detail: format!(
                        "protocol artifact index {} out of range ({} artifact{})",
                        index,
                        artifacts.len(),
                        if artifacts.len() == 1 { "" } else { "s" }
                    ),
                })?;
            match self.format {
                InspectOutputFormat::Table => {
                    print_protocol_artifact_detail(index, selected, self.full);
                }
                InspectOutputFormat::Json => {
                    if self.full {
                        let payload = serde_json::json!({
                            "index": index,
                            "path": selected.path,
                            "artifact": selected.stored,
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&payload)
                                .map_err(PrepareError::Serialize)?
                        );
                    } else {
                        let payload = serde_json::json!({
                            "index": index,
                            "path": selected.path,
                            "procedure_name": selected.stored.procedure_name,
                            "subject_id": selected.stored.subject_id,
                            "run_id": selected.stored.run_id,
                            "created_at_ms": selected.stored.created_at_ms,
                            "model_id": selected.stored.model_id,
                            "provider_slug": selected.stored.provider_slug,
                            "summary": protocol_artifact_summary(selected),
                            "input_preview": protocol_artifact_preview(&selected.stored.input),
                            "output_preview": protocol_artifact_preview(&selected.stored.output),
                            "artifact_preview": protocol_artifact_preview(&selected.stored.artifact),
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&payload)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                }
            }
            print_record_resolution_footer(&resolution);
            return Ok(());
        }

        match self.format {
            InspectOutputFormat::Table => {
                if artifacts.is_empty() {
                    println!("No persisted protocol artifacts for this run.");
                } else {
                    println!(
                        "{:<5} {:<30} {:<28} {:<16} Summary",
                        "Idx", "Procedure", "Subject", "Model"
                    );
                    println!("{}", "-".repeat(120));
                    for (index, entry) in artifacts.iter().enumerate() {
                        println!(
                            "{:<5} {:<30} {:<28} {:<16} {}",
                            index,
                            truncate_for_table(&entry.stored.procedure_name, 28),
                            truncate_for_table(&entry.stored.subject_id, 26),
                            truncate_for_table(entry.stored.model_id.as_deref().unwrap_or("-"), 14),
                            truncate_for_table(&protocol_artifact_summary(entry), 42),
                        );
                    }
                    println!("\nTotal protocol artifacts: {}", artifacts.len());
                    println!("Next:");
                    println!("  ploke-eval inspect protocol-artifacts 0");
                    println!("  ploke-eval inspect protocol-artifacts 0 --full");
                }
            }
            InspectOutputFormat::Json => {
                let payload: Vec<_> = artifacts
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        serde_json::json!({
                            "index": index,
                            "path": entry.path,
                            "procedure_name": entry.stored.procedure_name,
                            "subject_id": entry.stored.subject_id,
                            "created_at_ms": entry.stored.created_at_ms,
                            "model_id": entry.stored.model_id,
                            "provider_slug": entry.stored.provider_slug,
                            "summary": protocol_artifact_summary(entry),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectIssueOverviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let aggregate = load_issue_detection_aggregate(&record_path)
            .map_err(issue_aggregate_error_to_prepare)?;

        match self.format {
            InspectOutputFormat::Table => {
                print_issue_detection_aggregate(&aggregate);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&aggregate).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectProtocolOverviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        if let Some(campaign_id) = self.campaign.clone() {
            let report = collect_protocol_campaign_triage_report(&campaign_id, &self)?;
            return match self.format {
                InspectOutputFormat::Table => {
                    print!(
                        "{}",
                        render_protocol_campaign_triage_report(&report, self.width.max(88),)
                    );
                    Ok(())
                }
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                    );
                    Ok(())
                }
            };
        }

        if self.all_runs {
            let summaries = collect_protocol_run_summaries()?;
            return match self.format {
                InspectOutputFormat::Table => {
                    print_protocol_run_summaries(&summaries, &self);
                    Ok(())
                }
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&summaries)
                            .map_err(PrepareError::Serialize)?
                    );
                    Ok(())
                }
            };
        }

        let resolution = resolve_record_path(self.record.clone(), self.instance.clone(), None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let aggregate = match load_protocol_aggregate(&record_path) {
            Ok(aggregate) => aggregate,
            Err(ProtocolAggregateError::MissingAnchor { .. }) => {
                let mut state = protocol_state_for_run(&instance_id, &record_path)?;
                state.next_command = protocol_next_command(
                    &instance_id,
                    &record_path,
                    &state.next_step,
                    true,
                    false,
                );
                match self.format {
                    InspectOutputFormat::Table => {
                        print_protocol_state_table(&state);
                    }
                    InspectOutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&state)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                }
                print_record_resolution_footer(&resolution);
                return Ok(());
            }
            Err(err) => {
                let mut state = protocol_state_for_run(&instance_id, &record_path)?;
                state.next_command = protocol_next_command(
                    &instance_id,
                    &record_path,
                    &state.next_step,
                    true,
                    false,
                );
                state.aggregate_error = Some(err.to_string());
                match self.format {
                    InspectOutputFormat::Table => {
                        print_protocol_state_table(&state);
                    }
                    InspectOutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&state)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                }
                print_record_resolution_footer(&resolution);
                return Ok(());
            }
        };
        let mut report = build_protocol_report(&aggregate)?;
        apply_protocol_report_filters(&mut report, &self);

        match self.format {
            InspectOutputFormat::Table => {
                let use_color = match self.color {
                    ProtocolColorMode::Always => true,
                    ProtocolColorMode::Never => false,
                    ProtocolColorMode::Auto => std::io::stdout().is_terminal(),
                };
                let rendered = render_protocol_aggregate_report_with_options(
                    &report,
                    ProtocolReportRenderOptions {
                        width: self.width,
                        use_color,
                        top_call_issues: self.limit,
                        color_profile: self.color_profile.into(),
                    },
                );
                print!("{rendered}");
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct ProtocolCampaignQuery {
    issue: Option<String>,
    tool: Option<String>,
    status: Option<String>,
}

impl ProtocolCampaignQuery {
    fn from_command(command: &InspectProtocolOverviewCommand) -> Self {
        Self {
            issue: command.issue.as_deref().map(normalize_filter_value),
            tool: command.tool.as_deref().map(normalize_filter_value),
            status: command.status.as_deref().map(normalize_filter_value),
        }
    }

    fn matches_entry(&self, entry: &ProtocolRunSummaryRecord) -> bool {
        if let Some(status) = self.status.as_deref() {
            if entry.summary.protocol_status != status {
                return false;
            }
        }

        if self.issue.is_none() && self.tool.is_none() {
            return true;
        }

        entry
            .report
            .as_ref()
            .map(|report| {
                report
                    .call_issues
                    .iter()
                    .any(|row| self.matches_call_issue(row))
            })
            .unwrap_or(false)
    }

    fn matches_call_issue(&self, row: &ProtocolAggregateCallIssueRow) -> bool {
        if let Some(issue) = self.issue.as_deref() {
            let row_issue = row
                .issue
                .as_deref()
                .map(normalize_filter_value)
                .unwrap_or_default();
            if row_issue != issue {
                return false;
            }
        }

        if let Some(tool) = self.tool.as_deref() {
            let row_tool = row
                .tool_name
                .as_deref()
                .map(normalize_filter_value)
                .unwrap_or_default();
            if !row_tool.contains(tool) {
                return false;
            }
        }

        true
    }

    fn scope_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(status) = self.status.as_deref() {
            parts.push(format!("status={status}"));
        }
        if let Some(issue) = self.issue.as_deref() {
            parts.push(format!("issue={issue}"));
        }
        if let Some(tool) = self.tool.as_deref() {
            parts.push(format!("tool={tool}"));
        }
        if parts.is_empty() {
            "campaign-wide protocol triage".to_string()
        } else {
            format!("campaign protocol triage filtered by {}", parts.join(", "))
        }
    }
}

#[derive(Debug, Clone)]
struct ProtocolRunSummaryRecord {
    summary: ProtocolRunSummaryRow,
    report: Option<ProtocolAggregateReport>,
}

fn collect_protocol_campaign_triage_report(
    campaign_id: &str,
    command: &InspectProtocolOverviewCommand,
) -> Result<ProtocolCampaignTriageReport, PrepareError> {
    let state = load_closure_state(campaign_id)?;
    let query = ProtocolCampaignQuery::from_command(command);

    let mut entries = Vec::new();
    for row in state
        .instances
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Complete)
    {
        entries.push(collect_protocol_campaign_summary_record(row)?);
    }

    let campaign_runs = entries.len();
    let selected_entries = entries
        .iter()
        .filter(|entry| query.matches_entry(entry))
        .collect::<Vec<_>>();

    let mut issue_kind_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut issue_tool_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut segment_label_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut segment_status_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut summary = ProtocolCampaignSummary::default();
    let mut evidence = ProtocolCampaignEvidence::default();
    let mut problem_families = Vec::new();
    let mut exemplars = Vec::new();
    let mut filtered_matching_runs = BTreeSet::new();

    let mut artifact_error_runs = Vec::new();
    let mut missing_coverage_runs = Vec::new();
    let mut ineligible_runs = Vec::new();
    let mut high_issue_runs = Vec::new();

    for entry in selected_entries.iter().copied() {
        match entry.summary.protocol_status.as_str() {
            "ineligible" => summary.ineligible_runs += 1,
            _ => {
                summary.eligible_runs += 1;
                match entry.summary.protocol_status.as_str() {
                    "full" => summary.full_runs += 1,
                    "partial" => summary.partial_runs += 1,
                    "error" => summary.error_runs += 1,
                    "missing" => summary.missing_runs += 1,
                    _ => {}
                }
            }
        }

        evidence.total_tool_calls += entry.summary.tool_calls_total;
        evidence.missing_tool_call_reviews += entry.summary.missing_call_reviews;
        if entry.summary.protocol_status == "error" {
            evidence.artifact_failure_runs += 1;
        }

        if let Some(report) = entry.report.as_ref() {
            evidence.reviewed_tool_calls += report.coverage.reviewed_tool_calls;
            evidence.known_segments += report.coverage.total_segments;
            evidence.usable_segment_reviews += report.coverage.usable_segment_reviews;
            evidence.mismatched_segment_reviews += report.coverage.mismatched_segment_reviews;
            evidence.missing_segment_reviews += report.coverage.missing_segment_indices.len();
            evidence.duplicate_artifacts += report.coverage.duplicate_artifacts.unwrap_or(0);

            let matching_issues = report
                .call_issues
                .iter()
                .filter(|row| query.matches_call_issue(row))
                .collect::<Vec<_>>();

            if !matching_issues.is_empty() {
                filtered_matching_runs.insert(entry.summary.run_id.clone());
                let segment_rows = report
                    .segments
                    .iter()
                    .map(|row| (row.index, row))
                    .collect::<BTreeMap<_, _>>();
                for row in matching_issues {
                    if let Some(issue) = row.issue.as_deref() {
                        let (count, runs) = issue_kind_counts
                            .entry(issue.to_string())
                            .or_insert_with(|| (0usize, BTreeSet::new()));
                        *count += 1;
                        runs.insert(entry.summary.run_id.clone());
                    }
                    if let Some(tool) = row.tool_name.as_deref() {
                        let (count, runs) = issue_tool_counts
                            .entry(tool.to_string())
                            .or_insert_with(|| (0usize, BTreeSet::new()));
                        *count += 1;
                        runs.insert(entry.summary.run_id.clone());
                    }
                    if let Some(segment_index) = row.segment_index {
                        if let Some(segment) = segment_rows.get(&segment_index) {
                            if let Some(label) = segment.label.as_deref() {
                                let (count, runs) = segment_label_counts
                                    .entry(label.to_string())
                                    .or_insert_with(|| (0usize, BTreeSet::new()));
                                *count += 1;
                                runs.insert(entry.summary.run_id.clone());
                            }
                            if let Some(status) = segment.status.as_deref() {
                                let (count, runs) = segment_status_counts
                                    .entry(status.to_string())
                                    .or_insert_with(|| (0usize, BTreeSet::new()));
                                *count += 1;
                                runs.insert(entry.summary.run_id.clone());
                            }
                        }
                    }
                }
            }

            if entry.summary.call_issues > 0 {
                high_issue_runs.push(entry);
            }
        }

        if entry.summary.protocol_status == "error" {
            artifact_error_runs.push(entry);
        } else if entry.summary.protocol_status == "partial"
            || entry.summary.protocol_status == "missing"
        {
            missing_coverage_runs.push(entry);
        } else if entry.summary.protocol_status == "ineligible" {
            ineligible_runs.push(entry);
        }
    }

    summary.runs_with_issue_calls = if query.issue.is_some() || query.tool.is_some() {
        filtered_matching_runs.len()
    } else {
        selected_entries
            .iter()
            .filter(|entry| entry.summary.call_issues > 0)
            .count()
    };

    let mut issue_kinds = count_rows_from_map(issue_kind_counts);
    let mut issue_tools = count_rows_from_map(issue_tool_counts);
    let nearby_segment_labels = count_rows_from_map(segment_label_counts);
    let nearby_segment_statuses = count_rows_from_map(segment_status_counts);

    if query.issue.is_none() && query.tool.is_none() {
        if !artifact_error_runs.is_empty() {
            problem_families.push(problem_family_for_status_group(
                "artifact/schema failures",
                "artifact_schema_failure",
                &artifact_error_runs,
                "ploke-protocol / ploke-eval",
                "protocol artifact compatibility is blocking interpretation",
                "reduce error-status runs to zero",
            ));
        }
        if !missing_coverage_runs.is_empty() {
            problem_families.push(problem_family_for_status_group(
                "missing review coverage",
                "missing_review_coverage",
                &missing_coverage_runs,
                "ploke-eval protocol execution",
                "reviews are incomplete, so the campaign cannot fully explain tool behavior yet",
                "reduce partial+missing protocol rows",
            ));
        }
        for row in issue_kinds.iter().take(3) {
            let exemplars_for_issue = selected_entries
                .iter()
                .filter(|entry| {
                    entry
                        .report
                        .as_ref()
                        .map(|report| {
                            report.call_issues.iter().any(|issue| {
                                issue
                                    .issue
                                    .as_deref()
                                    .map(|value| value == row.label)
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            if !exemplars_for_issue.is_empty() {
                problem_families.push(ProtocolCampaignFamilyRow {
                    label: format!("issue: {}", row.label),
                    family_kind: "issue_family".to_string(),
                    affected_runs: row.affected_runs,
                    affected_calls: row.count,
                    likely_owner: "ploke-tui tool harness".to_string(),
                    exemplar_run: exemplars_for_issue
                        .first()
                        .map(|entry| entry.summary.run_id.clone()),
                    note: Some(
                        "completed protocol data still shows friction in this call family"
                            .to_string(),
                    ),
                    success_metric: Some(format!(
                        "lower {} issue-call count and affected runs",
                        row.label
                    )),
                });
            }
        }
        if !ineligible_runs.is_empty() {
            problem_families.push(problem_family_for_status_group(
                "ineligible zero-tool runs",
                "ineligible",
                &ineligible_runs,
                "campaign/selection hygiene",
                "these rows do not speak to tool friction and should stay outside the main frontier",
                "keep ineligible rows out of runnable protocol work",
            ));
        }
    } else {
        let label = describe_selected_family(&query);
        let affected_calls = selected_entries
            .iter()
            .map(|entry| matching_issue_count(entry, &query))
            .sum();
        problem_families.push(ProtocolCampaignFamilyRow {
            label,
            family_kind: "filtered_family".to_string(),
            affected_runs: selected_entries.len(),
            affected_calls,
            likely_owner: "ploke-tui tool harness".to_string(),
            exemplar_run: selected_entries
                .first()
                .map(|entry| entry.summary.run_id.clone()),
            note: Some("this slice is the current drilldown target".to_string()),
            success_metric: Some(
                "reduce matching calls and affected runs after the next tool/harness change"
                    .to_string(),
            ),
        });
    }

    sort_problem_families(&mut problem_families);

    if query.issue.is_none() && query.tool.is_none() {
        high_issue_runs.sort_by(|left, right| {
            right
                .summary
                .call_issues
                .cmp(&left.summary.call_issues)
                .then_with(|| {
                    right
                        .summary
                        .tool_calls_total
                        .cmp(&left.summary.tool_calls_total)
                })
        });
        exemplars.extend(
            artifact_error_runs
                .iter()
                .take(1)
                .chain(missing_coverage_runs.iter().take(1))
                .chain(
                    high_issue_runs
                        .iter()
                        .take(command.limit.max(4).saturating_sub(2)),
                )
                .map(|entry| exemplar_row_for_entry(entry, None)),
        );
    } else {
        let mut filtered_entries = selected_entries.clone();
        filtered_entries.sort_by(|left, right| {
            matching_issue_count(right, &query)
                .cmp(&matching_issue_count(left, &query))
                .then_with(|| right.summary.call_issues.cmp(&left.summary.call_issues))
                .then_with(|| {
                    right
                        .summary
                        .tool_calls_total
                        .cmp(&left.summary.tool_calls_total)
                })
        });
        exemplars.extend(
            filtered_entries
                .into_iter()
                .take(if command.examples {
                    command.limit.max(6)
                } else {
                    command.limit.min(5).max(3)
                })
                .map(|entry| exemplar_row_for_entry(entry, Some(&query))),
        );
    }

    dedupe_exemplars(&mut exemplars);
    issue_kinds.truncate(command.limit.max(3));
    issue_tools.truncate(command.limit.max(3));
    let mut nearby_segment_labels = nearby_segment_labels;
    let mut nearby_segment_statuses = nearby_segment_statuses;
    nearby_segment_labels.truncate(command.limit.max(3));
    nearby_segment_statuses.truncate(command.limit.max(3));
    problem_families.truncate(command.limit.max(4));
    if !command.examples {
        exemplars.truncate(command.limit.min(5).max(4));
    }

    let next_steps = build_triage_next_steps(
        campaign_id,
        &query,
        &problem_families,
        &issue_kinds,
        &issue_tools,
        &exemplars,
    );

    Ok(ProtocolCampaignTriageReport {
        campaign_id: campaign_id.to_string(),
        scope: query.scope_label(),
        issue_filter: command.issue.clone(),
        tool_filter: command.tool.clone(),
        status_filter: command.status.clone(),
        selected_runs: selected_entries.len(),
        campaign_runs,
        summary,
        evidence,
        issue_kinds,
        issue_tools,
        nearby_segment_labels,
        nearby_segment_statuses,
        problem_families,
        exemplars,
        next_steps,
    })
}

fn collect_protocol_campaign_summary_record(
    row: &crate::closure::ClosureInstanceRow,
) -> Result<ProtocolRunSummaryRecord, PrepareError> {
    if let Some(record_path) = row.artifacts.record_path.as_deref() {
        if record_path.exists() {
            return collect_protocol_run_summary_record(record_path);
        }
    }
    Ok(ProtocolRunSummaryRecord {
        summary: protocol_summary_row_from_closure_row(row),
        report: None,
    })
}

fn protocol_summary_row_from_closure_row(
    row: &crate::closure::ClosureInstanceRow,
) -> ProtocolRunSummaryRow {
    let protocol_status = match row.protocol_status {
        ClosureClass::Complete => "full",
        ClosureClass::Partial => "partial",
        ClosureClass::Missing => "missing",
        ClosureClass::Ineligible => "ineligible",
        ClosureClass::Failed | ClosureClass::Incompatible => "error",
    };
    let counts = row.protocol_counts.as_ref();
    let total_calls = counts.map(|counts| counts.total_calls).unwrap_or(0);
    let reviewed_calls = counts.map(|counts| counts.reviewed_calls).unwrap_or(0);
    let total_segments = counts.map(|counts| counts.total_segments).unwrap_or(0);
    let usable_segments = counts.map(|counts| counts.usable_segments).unwrap_or(0);
    ProtocolRunSummaryRow {
        run_id: row.instance_id.clone(),
        subject_id: row.instance_id.clone(),
        protocol_status: protocol_status.to_string(),
        call_review_ratio: ratio(reviewed_calls, total_calls),
        usable_segment_ratio: ratio(usable_segments, total_segments),
        tool_calls_total: total_calls,
        segments_total: total_segments,
        call_issues: 0,
        mismatched_segment_reviews: counts.map(|counts| counts.mismatched_segments).unwrap_or(0),
        missing_call_reviews: total_calls.saturating_sub(reviewed_calls),
        missing_segment_reviews: counts.map(|counts| counts.missing_segments).unwrap_or(0),
        note: row.protocol_failure.clone(),
    }
}

fn count_rows_from_map(
    rows: BTreeMap<String, (usize, BTreeSet<String>)>,
) -> Vec<ProtocolCampaignCountRow> {
    let mut counts = rows
        .into_iter()
        .map(|(label, (count, runs))| ProtocolCampaignCountRow {
            label,
            count,
            affected_runs: runs.len(),
        })
        .collect::<Vec<_>>();
    sort_count_rows(&mut counts);
    counts
}

fn collect_tool_campaign_overview(
    command: &InspectToolOverviewCommand,
) -> Result<ToolCampaignOverviewReport, PrepareError> {
    let state = load_closure_state(&command.campaign)?;
    let tool_filter = command.tool.as_deref().map(normalize_filter_value);
    let mut report = ToolCampaignOverviewReport {
        campaign_id: command.campaign.clone(),
        tool_filter: command.tool.clone(),
        scanned_complete_runs: 0,
        runs_with_tool: 0,
        runs_with_failed_calls: 0,
        repeated_failure_runs: 0,
        mixed_outcome_runs: 0,
        total_calls: 0,
        completed_calls: 0,
        failed_calls: 0,
        failure_codes: Vec::new(),
        failure_reasons: Vec::new(),
        exemplar_runs: Vec::new(),
        next_steps: Vec::new(),
    };
    let mut failure_code_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut failure_reason_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut run_rows = Vec::new();

    for row in state
        .instances
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Complete)
    {
        report.scanned_complete_runs += 1;
        let Some(record_path) = row.artifacts.record_path.as_ref() else {
            continue;
        };
        if !record_path.exists() {
            continue;
        }
        let record =
            read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let mut run = ToolRunAccumulator::default();

        for call in record.tool_calls().into_iter().filter(|call| {
            tool_filter.as_deref().map_or(true, |tool| {
                normalize_filter_value(&call.request.tool) == tool
            })
        }) {
            run.total_calls += 1;
            report.total_calls += 1;
            match call.result {
                crate::record::ToolResult::Completed(_) => {
                    run.completed_calls += 1;
                    report.completed_calls += 1;
                }
                crate::record::ToolResult::Failed(failed) => {
                    run.failed_calls += 1;
                    report.failed_calls += 1;

                    let reason = summarize_failure_reason(&failed.error);
                    *run.failure_reasons.entry(reason.clone()).or_insert(0) += 1;
                    let (count, runs) = failure_reason_counts
                        .entry(reason)
                        .or_insert_with(|| (0usize, BTreeSet::new()));
                    *count += 1;
                    runs.insert(row.instance_id.clone());

                    if let Some(code) = tool_failure_code(&failed) {
                        *run.failure_codes.entry(code.clone()).or_insert(0) += 1;
                        let (count, runs) = failure_code_counts
                            .entry(code)
                            .or_insert_with(|| (0usize, BTreeSet::new()));
                        *count += 1;
                        runs.insert(row.instance_id.clone());
                    }
                }
            }
        }

        if run.total_calls == 0 {
            continue;
        }

        report.runs_with_tool += 1;
        if run.failed_calls > 0 {
            report.runs_with_failed_calls += 1;
        }
        if run.failed_calls >= 2 {
            report.repeated_failure_runs += 1;
        }
        if run.failed_calls > 0 && run.completed_calls > 0 {
            report.mixed_outcome_runs += 1;
        }

        run_rows.push(ToolOverviewRunRow {
            run_id: row.instance_id.clone(),
            total_calls: run.total_calls,
            completed_calls: run.completed_calls,
            failed_calls: run.failed_calls,
            top_failure_code: top_failure_label(&run.failure_codes),
            top_failure_reason: top_failure_label(&run.failure_reasons),
        });
    }

    report.failure_codes = count_rows_from_map(failure_code_counts)
        .into_iter()
        .map(|row| ToolOverviewCountRow {
            label: row.label,
            count: row.count,
            affected_runs: row.affected_runs,
        })
        .collect();
    report.failure_reasons = count_rows_from_map(failure_reason_counts)
        .into_iter()
        .map(|row| ToolOverviewCountRow {
            label: row.label,
            count: row.count,
            affected_runs: row.affected_runs,
        })
        .collect();
    run_rows.sort_by(|left, right| {
        right
            .failed_calls
            .cmp(&left.failed_calls)
            .then_with(|| right.total_calls.cmp(&left.total_calls))
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    run_rows.truncate(command.limit.max(3));
    report.exemplar_runs = run_rows;
    report.failure_codes.truncate(command.limit.max(3));
    report.failure_reasons.truncate(command.limit.max(3));
    report.next_steps = build_tool_overview_next_steps(&report);

    Ok(report)
}

fn problem_family_for_status_group(
    label: &str,
    family_kind: &str,
    entries: &[&ProtocolRunSummaryRecord],
    likely_owner: &str,
    note: &str,
    success_metric: &str,
) -> ProtocolCampaignFamilyRow {
    let exemplar_run = entries.first().map(|entry| entry.summary.run_id.clone());
    let affected_calls = entries
        .iter()
        .map(|entry| {
            if family_kind == "issue_family" || family_kind == "filtered_family" {
                entry.summary.call_issues
            } else {
                entry.summary.missing_call_reviews + entry.summary.missing_segment_reviews
            }
        })
        .sum();
    ProtocolCampaignFamilyRow {
        label: label.to_string(),
        family_kind: family_kind.to_string(),
        affected_runs: entries.len(),
        affected_calls,
        likely_owner: likely_owner.to_string(),
        exemplar_run,
        note: Some(note.to_string()),
        success_metric: Some(success_metric.to_string()),
    }
}

fn sort_problem_families(rows: &mut [ProtocolCampaignFamilyRow]) {
    rows.sort_by(|left, right| {
        right
            .affected_runs
            .cmp(&left.affected_runs)
            .then_with(|| right.affected_calls.cmp(&left.affected_calls))
            .then_with(|| left.label.cmp(&right.label))
    });
}

fn matching_issue_count(entry: &ProtocolRunSummaryRecord, query: &ProtocolCampaignQuery) -> usize {
    entry
        .report
        .as_ref()
        .map(|report| {
            report
                .call_issues
                .iter()
                .filter(|row| query.matches_call_issue(row))
                .count()
        })
        .unwrap_or(0)
}

fn exemplar_row_for_entry(
    entry: &ProtocolRunSummaryRecord,
    query: Option<&ProtocolCampaignQuery>,
) -> ProtocolCampaignExemplarRow {
    let matching_calls = query
        .map(|query| matching_issue_count(entry, query))
        .unwrap_or(entry.summary.call_issues);
    let focus = if let Some(query) = query {
        Some(describe_selected_family(query))
    } else if entry.summary.protocol_status == "error" {
        Some("artifact/schema failure".to_string())
    } else if entry.summary.protocol_status == "partial"
        || entry.summary.protocol_status == "missing"
    {
        Some("missing protocol coverage".to_string())
    } else if entry.summary.call_issues > 0 {
        entry.report.as_ref().and_then(|report| {
            report
                .call_issues
                .iter()
                .find_map(|row| row.issue.as_deref().map(|value| value.to_string()))
        })
    } else {
        None
    };
    ProtocolCampaignExemplarRow {
        run_id: entry.summary.run_id.clone(),
        protocol_status: entry.summary.protocol_status.clone(),
        matching_calls,
        total_issues: entry.summary.call_issues,
        tool_calls_total: entry.summary.tool_calls_total,
        focus,
        note: entry.summary.note.clone(),
    }
}

fn dedupe_exemplars(rows: &mut Vec<ProtocolCampaignExemplarRow>) {
    let mut seen = BTreeSet::new();
    rows.retain(|row| seen.insert(row.run_id.clone()));
}

fn build_triage_next_steps(
    campaign_id: &str,
    query: &ProtocolCampaignQuery,
    problem_families: &[ProtocolCampaignFamilyRow],
    issue_kinds: &[ProtocolCampaignCountRow],
    issue_tools: &[ProtocolCampaignCountRow],
    exemplars: &[ProtocolCampaignExemplarRow],
) -> Vec<String> {
    let mut steps = Vec::new();
    if query.issue.is_none() {
        if let Some(top_issue) = issue_kinds.first() {
            steps.push(format!(
                "if you want exemplar runs for the top issue family, try `ploke-eval inspect protocol-overview --campaign {campaign_id} --issue {}`",
                top_issue.label
            ));
        }
    }
    if query.tool.is_none() {
        if let Some(top_tool) = issue_tools.first() {
            steps.push(format!(
                "if you want the most suspicious tool slice next, try `ploke-eval inspect protocol-overview --campaign {campaign_id} --tool {}`",
                top_tool.label
            ));
        }
    }
    if query.status.is_none()
        && problem_families
            .iter()
            .any(|family| family.family_kind == "artifact_schema_failure")
    {
        steps.push(format!(
            "if you want only artifact/schema failures, try `ploke-eval inspect protocol-overview --campaign {campaign_id} --status error`"
        ));
    }
    if let Some(exemplar) = exemplars.first() {
        steps.push(format!(
            "if you want the protocol report for the top exemplar, try `ploke-eval inspect protocol-overview --instance {}`",
            exemplar.run_id
        ));
        if exemplar.protocol_status == "error" {
            steps.push(format!(
                "if you want the raw artifact failure for that exemplar, try `ploke-eval inspect protocol-artifacts --instance {} --full`",
                exemplar.run_id
            ));
        } else {
            steps.push(format!(
                "if you want the local tool trace around that exemplar, try `ploke-eval inspect tool-calls --instance {}`",
                exemplar.run_id
            ));
        }
    }
    steps.truncate(4);
    steps
}

fn describe_selected_family(query: &ProtocolCampaignQuery) -> String {
    let mut parts = Vec::new();
    if let Some(status) = query.status.as_deref() {
        parts.push(format!("status={status}"));
    }
    if let Some(issue) = query.issue.as_deref() {
        parts.push(format!("issue={issue}"));
    }
    if let Some(tool) = query.tool.as_deref() {
        parts.push(format!("tool={tool}"));
    }
    if parts.is_empty() {
        "campaign slice".to_string()
    } else {
        parts.join(" + ")
    }
}

fn build_tool_overview_next_steps(report: &ToolCampaignOverviewReport) -> Vec<String> {
    let tool = report
        .tool_filter
        .clone()
        .unwrap_or_else(|| "apply_code_edit".to_string());
    let mut steps = Vec::new();
    if let Some(run) = report.exemplar_runs.first() {
        steps.push(format!(
            "inspect the worst exemplar with `ploke-eval inspect tool-calls --instance {}`",
            run.run_id
        ));
    }
    steps.push(format!(
        "compare protocol issue context with `ploke-eval inspect protocol-overview --campaign {} --tool {}`",
        report.campaign_id, tool
    ));
    steps
}

fn normalize_filter_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunSummaryRow {
    run_id: String,
    subject_id: String,
    protocol_status: String,
    call_review_ratio: f32,
    usable_segment_ratio: f32,
    tool_calls_total: usize,
    segments_total: usize,
    call_issues: usize,
    mismatched_segment_reviews: usize,
    missing_call_reviews: usize,
    missing_segment_reviews: usize,
    note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolCampaignOverviewReport {
    campaign_id: String,
    tool_filter: Option<String>,
    scanned_complete_runs: usize,
    runs_with_tool: usize,
    runs_with_failed_calls: usize,
    repeated_failure_runs: usize,
    mixed_outcome_runs: usize,
    total_calls: usize,
    completed_calls: usize,
    failed_calls: usize,
    failure_codes: Vec<ToolOverviewCountRow>,
    failure_reasons: Vec<ToolOverviewCountRow>,
    exemplar_runs: Vec<ToolOverviewRunRow>,
    next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolOverviewCountRow {
    label: String,
    count: usize,
    affected_runs: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ToolOverviewRunRow {
    run_id: String,
    total_calls: usize,
    completed_calls: usize,
    failed_calls: usize,
    top_failure_code: Option<String>,
    top_failure_reason: Option<String>,
}

#[derive(Debug, Default)]
struct ToolRunAccumulator {
    total_calls: usize,
    completed_calls: usize,
    failed_calls: usize,
    failure_codes: BTreeMap<String, usize>,
    failure_reasons: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct SegmentEvidenceCounts {
    usable: usize,
    mismatched: usize,
    missing: usize,
    missing_indices: Vec<usize>,
}

fn collect_protocol_run_summaries() -> Result<Vec<ProtocolRunSummaryRow>, PrepareError> {
    let total_start = Instant::now();
    let mut summaries = Vec::new();
    for record_path in collect_finished_record_paths()? {
        let run_start = Instant::now();
        let summary = collect_protocol_run_summary_record(&record_path)?.summary;
        let elapsed = run_start.elapsed();
        if elapsed.as_millis() >= 200 {
            eprintln!(
                "protocol-overview: slow run {} took {} ms",
                summary.run_id,
                elapsed.as_millis()
            );
        }
        summaries.push(summary);
    }
    summaries.sort_by(|left, right| {
        let left_evidence_concerns = left.mismatched_segment_reviews
            + left.missing_segment_reviews
            + left.missing_call_reviews;
        let right_evidence_concerns = right.mismatched_segment_reviews
            + right.missing_segment_reviews
            + right.missing_call_reviews;
        protocol_summary_status_rank(&left.protocol_status)
            .cmp(&protocol_summary_status_rank(&right.protocol_status))
            .then_with(|| right_evidence_concerns.cmp(&left_evidence_concerns))
            .then_with(|| right.call_issues.cmp(&left.call_issues))
            .then_with(|| right.tool_calls_total.cmp(&left.tool_calls_total))
    });
    eprintln!(
        "protocol-overview: scanned {} runs in {:.2}s",
        summaries.len(),
        total_start.elapsed().as_secs_f32()
    );
    Ok(summaries)
}

fn collect_protocol_run_summary_record(
    record_path: &Path,
) -> Result<ProtocolRunSummaryRecord, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let run_id = record_path
        .parent()
        .and_then(|path| path.file_name())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| record.manifest_id.clone());
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let tool_calls_total = record.tool_calls().len();
    let artifacts = list_protocol_artifact_load_results(record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "inspect_protocol_overview",
            detail: err.to_string(),
        }
    })?;

    match crate::protocol::protocol_aggregate::load_protocol_aggregate_from_artifacts(
        record_path,
        artifacts,
    ) {
        Ok(aggregate) => {
            let report = build_protocol_report(&aggregate)?;
            let summary = protocol_summary_row_from_aggregate_with_report(&aggregate, &report);
            Ok(ProtocolRunSummaryRecord {
                summary,
                report: Some(report),
            })
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) if tool_calls_total == 0 => {
            Ok(ProtocolRunSummaryRecord {
                summary: ProtocolRunSummaryRow {
                    run_id,
                    subject_id,
                    protocol_status: "ineligible".to_string(),
                    call_review_ratio: 0.0,
                    usable_segment_ratio: 0.0,
                    tool_calls_total,
                    segments_total: 0,
                    call_issues: 0,
                    mismatched_segment_reviews: 0,
                    missing_call_reviews: 0,
                    missing_segment_reviews: 0,
                    note: Some("zero tool calls".to_string()),
                },
                report: None,
            })
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) => Ok(ProtocolRunSummaryRecord {
            summary: ProtocolRunSummaryRow {
                run_id,
                subject_id,
                protocol_status: "missing".to_string(),
                call_review_ratio: 0.0,
                usable_segment_ratio: 0.0,
                tool_calls_total,
                segments_total: 0,
                call_issues: 0,
                mismatched_segment_reviews: 0,
                missing_call_reviews: tool_calls_total,
                missing_segment_reviews: 0,
                note: Some("no intent segmentation artifact".to_string()),
            },
            report: None,
        }),
        Err(err) => Ok(ProtocolRunSummaryRecord {
            summary: ProtocolRunSummaryRow {
                run_id,
                subject_id,
                protocol_status: "error".to_string(),
                call_review_ratio: 0.0,
                usable_segment_ratio: 0.0,
                tool_calls_total,
                segments_total: 0,
                call_issues: 0,
                mismatched_segment_reviews: 0,
                missing_call_reviews: 0,
                missing_segment_reviews: 0,
                note: Some(err.to_string()),
            },
            report: None,
        }),
    }
}

fn protocol_summary_row_from_aggregate_with_report(
    aggregate: &ProtocolAggregate,
    report: &ProtocolAggregateReport,
) -> ProtocolRunSummaryRow {
    let segment_evidence = segment_evidence_counts(aggregate);
    let missing_call_reviews = aggregate.coverage.missing_call_indices.len();
    let missing_segment_reviews = segment_evidence.missing;
    let mismatched_segment_reviews = segment_evidence.mismatched;
    let protocol_status = if missing_call_reviews == 0
        && missing_segment_reviews == 0
        && mismatched_segment_reviews == 0
    {
        "full"
    } else {
        "partial"
    };

    ProtocolRunSummaryRow {
        run_id: aggregate.run.run_id.clone(),
        subject_id: aggregate.run.subject_id.clone(),
        protocol_status: protocol_status.to_string(),
        call_review_ratio: ratio(
            aggregate.coverage.reviewed_call_count,
            aggregate.coverage.total_calls_in_run,
        ),
        usable_segment_ratio: ratio(
            segment_evidence.usable,
            aggregate.coverage.total_segments_in_anchor,
        ),
        tool_calls_total: aggregate.coverage.total_calls_in_run,
        segments_total: aggregate.coverage.total_segments_in_anchor,
        call_issues: report.call_issues.len(),
        mismatched_segment_reviews,
        missing_call_reviews,
        missing_segment_reviews,
        note: None,
    }
}

fn protocol_summary_status_rank(status: &str) -> u8 {
    match status {
        "error" => 0,
        "missing" => 1,
        "partial" => 2,
        "full" => 3,
        "ineligible" => 4,
        _ => 5,
    }
}

fn segment_evidence_counts(aggregate: &ProtocolAggregate) -> SegmentEvidenceCounts {
    let accepted_segment_indices = aggregate
        .segment_reviews
        .iter()
        .map(|row| row.basis.segment_index)
        .collect::<std::collections::BTreeSet<_>>();
    let mismatched_segment_indices = aggregate
        .skipped_segment_reviews
        .iter()
        .map(|row| row.segment_index)
        .collect::<std::collections::BTreeSet<_>>();

    let mut usable = 0usize;
    let mut mismatched = 0usize;
    let mut missing_indices = Vec::new();

    for basis in &aggregate.segmentation.segments {
        if accepted_segment_indices.contains(&basis.segment_index) {
            usable += 1;
        } else if mismatched_segment_indices.contains(&basis.segment_index) {
            mismatched += 1;
        } else {
            missing_indices.push(basis.segment_index);
        }
    }

    SegmentEvidenceCounts {
        usable,
        mismatched,
        missing: missing_indices.len(),
        missing_indices,
    }
}

fn collect_finished_record_paths() -> Result<Vec<PathBuf>, PrepareError> {
    let root = instances_dir()?;
    list_finished_record_paths_in_instances_root(&root)
}

fn build_protocol_report(
    aggregate: &ProtocolAggregate,
) -> Result<ProtocolAggregateReport, PrepareError> {
    let segment_evidence = segment_evidence_counts(aggregate);
    let call_details = load_anchor_call_details(aggregate)?;
    let segment_review_by_index = aggregate
        .segment_reviews
        .iter()
        .map(|row| (row.basis.segment_index, row))
        .collect::<BTreeMap<_, _>>();

    let segments = aggregate
        .segmentation
        .segments
        .iter()
        .map(|basis| {
            let reviewed_call_count = aggregate
                .call_reviews
                .iter()
                .filter(|row| row.segment_index == Some(basis.segment_index))
                .count();
            let segment_review = segment_review_by_index.get(&basis.segment_index);
            let note = if aggregate
                .skipped_segment_reviews
                .iter()
                .any(|row| row.segment_index == basis.segment_index)
            {
                Some("mismatch with current anchor".to_string())
            } else if segment_review.is_none() {
                Some("missing segment review".to_string())
            } else if reviewed_call_count < basis.call_count {
                Some(format!(
                    "call coverage {reviewed_call_count}/{}",
                    basis.call_count
                ))
            } else {
                None
            };
            ProtocolAggregateSegmentRow {
                index: basis.segment_index,
                label: basis.label.map(intent_label_name),
                call_span: Some(format!("{}..{}", basis.start_index, basis.end_index)),
                status: Some(
                    segment_review
                        .map(|review| review.overall.clone())
                        .unwrap_or_else(|| segment_status_name(basis.status).to_string()),
                ),
                evidence: Some(
                    if aggregate
                        .skipped_segment_reviews
                        .iter()
                        .any(|row| row.segment_index == basis.segment_index)
                    {
                        "mismatched".to_string()
                    } else if segment_review.is_some() {
                        "usable".to_string()
                    } else {
                        "missing".to_string()
                    },
                ),
                confidence: segment_review
                    .and_then(|review| {
                        confidence_fraction(Some(review.overall_confidence.as_str()))
                    })
                    .or_else(|| basis.confidence.map(confidence_fraction_typed)),
                note,
                call_refs: basis.call_indices.clone(),
            }
        })
        .collect::<Vec<_>>();

    let call_issues = aggregate
        .call_reviews
        .iter()
        .filter_map(|row| {
            let detail = call_details.get(&row.focal_call_index);
            let issue = primary_call_issue(row);
            if issue.is_none() {
                return None;
            }
            Some(ProtocolAggregateCallIssueRow {
                index: row.focal_call_index,
                turn: row.turn_span.first().copied().map(|turn| turn as u32),
                segment_index: row.segment_index,
                tool_name: detail.map(|detail| detail.tool_name.clone()),
                issue,
                overall: Some(row.overall.clone()),
                detail: detail
                    .map(|detail| detail.summary.clone())
                    .or_else(|| Some(format!("scope={}", join_indices(&row.scope_call_indices)))),
                severity: Some(call_review_severity(row)),
                confidence: confidence_fraction(Some(row.overall_confidence.as_str())),
            })
        })
        .collect::<Vec<_>>();

    let duplicate_artifacts = Some(
        aggregate
            .coverage
            .artifact_counts
            .get("tool_call_review")
            .copied()
            .unwrap_or(0)
            .saturating_sub(aggregate.coverage.reviewed_call_count)
            + aggregate
                .coverage
                .artifact_counts
                .get("tool_call_segment_review")
                .copied()
                .unwrap_or(0)
                .saturating_sub(aggregate.coverage.reviewed_segment_count),
    );

    Ok(ProtocolAggregateReport {
        run_id: aggregate.run.run_id.clone(),
        subject_id: aggregate.run.subject_id.clone(),
        title: Some("Evidence reliability".to_string()),
        generated_at: None,
        scope: Some("protocol-derived evidence".to_string()),
        provenance: vec![
            format!(
                "anchor={} {}",
                aggregate.segmentation.artifact.created_at_ms,
                truncate_middle(
                    aggregate
                        .segmentation
                        .artifact
                        .path
                        .to_string_lossy()
                        .as_ref(),
                    44
                )
            ),
            "derived from intent segmentation + tool call review + segment review".to_string(),
        ],
        coverage: ProtocolAggregateCoverage {
            total_tool_calls: aggregate.coverage.total_calls_in_run,
            reviewed_tool_calls: aggregate.coverage.reviewed_call_count,
            total_segments: aggregate.coverage.total_segments_in_anchor,
            usable_segment_reviews: segment_evidence.usable,
            mismatched_segment_reviews: segment_evidence.mismatched,
            missing_tool_call_indices: aggregate.coverage.missing_call_indices.clone(),
            missing_segment_indices: segment_evidence.missing_indices.clone(),
            duplicate_artifacts,
        },
        segments,
        call_issues,
        notes: if segment_evidence.mismatched > 0 {
            vec![format!(
                "{} segment review artifacts excluded because they do not match the current anchor",
                segment_evidence.mismatched
            )]
        } else {
            Vec::new()
        },
    })
}

fn apply_protocol_report_filters(
    report: &mut ProtocolAggregateReport,
    command: &InspectProtocolOverviewCommand,
) {
    let overall_filter = command.overall.as_deref().map(str::to_lowercase);
    let label_filter = command.segment_label.as_deref().map(str::to_lowercase);
    let tool_filter = command.tool.as_deref().map(str::to_lowercase);

    if let Some(label) = label_filter.as_deref() {
        report.segments.retain(|row| {
            row.label
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(label))
                .unwrap_or(false)
        });
    }

    if let Some(overall) = overall_filter.as_deref() {
        report.segments.retain(|row| {
            row.status
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(overall))
                .unwrap_or(false)
        });
        report.call_issues.retain(|row| {
            row.overall
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(overall))
                .unwrap_or(false)
        });
    }

    if let Some(tool) = tool_filter.as_deref() {
        report.call_issues.retain(|row| {
            row.tool_name
                .as_deref()
                .map(|value| value.to_ascii_lowercase().contains(tool))
                .unwrap_or(false)
        });
    }

    if command.only_issues {
        report.segments.retain(|row| {
            row.note.is_some()
                || row
                    .status
                    .as_deref()
                    .map(|value| value != "focused_progress")
                    .unwrap_or(true)
                || row
                    .evidence
                    .as_deref()
                    .map(|value| value != "usable")
                    .unwrap_or(true)
        });
    }

    match command.view {
        ProtocolOverviewView::Overview => {}
        ProtocolOverviewView::Segments => {
            report.call_issues.clear();
        }
        ProtocolOverviewView::Calls => {
            report.segments.clear();
        }
    }
}

fn print_protocol_run_summaries(
    summaries: &[ProtocolRunSummaryRow],
    command: &InspectProtocolOverviewCommand,
) {
    let mut rows = summaries.to_vec();
    if command.only_issues {
        rows.retain(|row| {
            row.protocol_status != "full" && row.protocol_status != "ineligible"
                || row.call_issues > 0
                || row.missing_call_reviews > 0
                || row.missing_segment_reviews > 0
                || row.mismatched_segment_reviews > 0
        });
    }
    rows.truncate(command.limit.max(1));

    println!("Segment evidence legend: u=usable, m=mismatched, x=missing");
    println!(
        "Protocol status: full=all expected protocol reviews present; partial=review coverage missing; missing=no segmentation anchor; error=artifact/schema failure; ineligible=zero tool calls"
    );
    println!(
        "{:<28} {:<10} {:<12} {:<18} {:<8} Summary",
        "Run", "Call revs", "Protocol", "Segment evidence", "Issues"
    );
    println!("{}", "─".repeat(command.width.min(120)));
    for row in rows {
        let missing_segments = row.missing_segment_reviews;
        let segment_evidence = if row.segments_total == 0 {
            "-".to_string()
        } else {
            format!(
                "u{} m{} x{}",
                (row.usable_segment_ratio * row.segments_total as f32).round() as usize,
                row.mismatched_segment_reviews,
                missing_segments
            )
        };
        let summary = if row.protocol_status == "full" || row.protocol_status == "partial" {
            format!(
                "{} {}",
                progress_bar(row.call_review_ratio, 6),
                summary_segment_bar(
                    (row.usable_segment_ratio * row.segments_total as f32).round() as usize,
                    row.mismatched_segment_reviews,
                    missing_segments,
                    6,
                )
            )
        } else {
            truncate_for_table(row.note.as_deref().unwrap_or("-"), command.width.min(40))
        };
        println!(
            "{:<28} {:<10} {:<12} {:<18} {:<8} {}",
            truncate_for_table(&row.run_id, 26),
            format!(
                "{}/{}",
                (row.call_review_ratio * row.tool_calls_total as f32).round() as usize,
                row.tool_calls_total
            ),
            row.protocol_status,
            segment_evidence,
            row.call_issues,
            summary,
        );
    }
}

fn print_tool_campaign_overview(report: &ToolCampaignOverviewReport, limit: usize) {
    println!("Campaign: {}", report.campaign_id);
    println!(
        "Tool: {}",
        report.tool_filter.as_deref().unwrap_or("<all tools>")
    );
    println!(
        "Runs: {} complete scanned | {} with tool",
        report.scanned_complete_runs, report.runs_with_tool
    );
    println!(
        "Calls: {} total | {} completed | {} failed",
        report.total_calls, report.completed_calls, report.failed_calls
    );
    println!(
        "Run outcomes: {} with failures | {} repeated-failure runs | {} mixed-outcome runs",
        report.runs_with_failed_calls, report.repeated_failure_runs, report.mixed_outcome_runs
    );

    if !report.failure_codes.is_empty() {
        println!("\nTop failure codes");
        for row in report.failure_codes.iter().take(limit) {
            println!(
                "  - {}: {} calls across {} runs",
                row.label, row.count, row.affected_runs
            );
        }
    }

    if !report.failure_reasons.is_empty() {
        println!("\nTop failure reasons");
        for row in report.failure_reasons.iter().take(limit) {
            println!(
                "  - {}: {} calls across {} runs",
                row.label, row.count, row.affected_runs
            );
        }
    }

    if !report.exemplar_runs.is_empty() {
        println!("\nExemplar runs");
        for row in report.exemplar_runs.iter().take(limit) {
            let code = row.top_failure_code.as_deref().unwrap_or("-");
            let reason = row.top_failure_reason.as_deref().unwrap_or("-");
            println!(
                "  - {}: {} calls | {} completed | {} failed | top code {} | {}",
                row.run_id, row.total_calls, row.completed_calls, row.failed_calls, code, reason
            );
        }
    }

    if !report.next_steps.is_empty() {
        println!("\nNext");
        for step in report.next_steps.iter().take(4) {
            println!("  - {}", step);
        }
    }
}

#[derive(Debug, Clone)]
struct AnchorCallDetail {
    tool_name: String,
    summary: String,
}

fn load_anchor_call_details(
    aggregate: &ProtocolAggregate,
) -> Result<BTreeMap<usize, AnchorCallDetail>, PrepareError> {
    let artifact = load_protocol_artifact(&aggregate.segmentation.artifact.path)?;
    let mut details = BTreeMap::new();
    if let Some(calls) = artifact
        .stored
        .input
        .get("calls")
        .and_then(|value| value.as_array())
    {
        for call in calls {
            if let Some(index) = call.get("index").and_then(|value| value.as_u64()) {
                details.insert(
                    index as usize,
                    AnchorCallDetail {
                        tool_name: call
                            .get("tool_name")
                            .and_then(|value| value.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        summary: call
                            .get("summary")
                            .and_then(|value| value.as_str())
                            .unwrap_or("-")
                            .to_string(),
                    },
                );
            }
        }
    }
    Ok(details)
}

fn primary_call_issue(row: &ProtocolCallReviewRow) -> Option<String> {
    if row.redundancy.verdict == "search_thrash" {
        Some("search_thrash".to_string())
    } else if row.recoverability.verdict == "partial_next_step" {
        Some("partial_next_step".to_string())
    } else if row.overall != "focused_progress" {
        Some(row.overall.clone())
    } else {
        None
    }
}

fn call_review_severity(row: &ProtocolCallReviewRow) -> f32 {
    if row.redundancy.verdict == "search_thrash" {
        0.95
    } else if row.recoverability.verdict == "no_clear_recovery" {
        0.85
    } else if row.recoverability.verdict == "partial_next_step" {
        0.7
    } else if row.overall == "mixed" {
        0.55
    } else {
        0.25
    }
}

fn confidence_fraction(value: Option<&str>) -> Option<f32> {
    match value? {
        "high" | "High" => Some(0.9),
        "medium" | "Medium" => Some(0.6),
        "low" | "Low" => Some(0.3),
        _ => None,
    }
}

fn confidence_fraction_typed(value: ploke_protocol::Confidence) -> f32 {
    match value {
        ploke_protocol::Confidence::High => 0.9,
        ploke_protocol::Confidence::Medium => 0.6,
        ploke_protocol::Confidence::Low => 0.3,
    }
}

fn intent_label_name(label: ploke_protocol::IntentLabel) -> String {
    match label {
        ploke_protocol::IntentLabel::LocateTarget => "locate_target".to_string(),
        ploke_protocol::IntentLabel::InspectCandidate => "inspect_candidate".to_string(),
        ploke_protocol::IntentLabel::RefineSearch => "refine_search".to_string(),
        ploke_protocol::IntentLabel::ValidateHypothesis => "validate_hypothesis".to_string(),
        ploke_protocol::IntentLabel::EditAttempt => "edit_attempt".to_string(),
        ploke_protocol::IntentLabel::Recovery => "recovery".to_string(),
        ploke_protocol::IntentLabel::Other => "other".to_string(),
    }
}

fn segment_status_name(status: ploke_protocol::SegmentStatus) -> &'static str {
    match status {
        ploke_protocol::SegmentStatus::Labeled => "labeled",
        ploke_protocol::SegmentStatus::Ambiguous => "ambiguous",
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn progress_bar(value: f32, width: usize) -> String {
    let width = width.max(3);
    let filled = ((value.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}

fn summary_segment_bar(usable: usize, mismatched: usize, missing: usize, width: usize) -> String {
    let total = usable + mismatched + missing;
    if total == 0 {
        return "[------]".to_string();
    }
    let width = width.max(3);
    let usable_width = ((usable as f32 / total as f32) * width as f32).round() as usize;
    let mismatch_width = ((mismatched as f32 / total as f32) * width as f32).round() as usize;
    let mut missing_width = width.saturating_sub(usable_width + mismatch_width);
    let mut usable_width = usable_width.min(width);
    let mut mismatch_width = mismatch_width.min(width.saturating_sub(usable_width));
    missing_width = missing_width.min(width.saturating_sub(usable_width + mismatch_width));
    while usable_width + mismatch_width + missing_width < width {
        if missing > 0 {
            missing_width += 1;
        } else if mismatched > 0 {
            mismatch_width += 1;
        } else {
            usable_width += 1;
        }
    }
    format!(
        "[{}{}{}]",
        "█".repeat(usable_width),
        "▓".repeat(mismatch_width),
        "░".repeat(missing_width)
    )
}

/// Convert a Cozo DataValue to a JSON Value
fn cozo_data_to_json(val: &cozo::DataValue) -> serde_json::Value {
    use cozo::DataValue;

    match val {
        DataValue::Null => serde_json::Value::Null,
        DataValue::Str(s) => serde_json::Value::String(s.to_string()),
        DataValue::Bytes(b) => serde_json::Value::String(format!("{:?}", b)),
        DataValue::Uuid(u) => serde_json::Value::String(u.0.to_string()),
        DataValue::Num(n) => match n {
            cozo::Num::Int(i) => serde_json::Value::Number((*i).into()),
            cozo::Num::Float(f) => serde_json::Value::Number(
                serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0)),
            ),
        },
        DataValue::Bool(b) => serde_json::Value::Bool(*b),
        DataValue::List(l) => serde_json::Value::Array(l.iter().map(cozo_data_to_json).collect()),
        DataValue::Set(s) => serde_json::Value::Array(s.iter().map(cozo_data_to_json).collect()),
        DataValue::Vec(v) => {
            // Vec is an embedding vector - convert to array of floats
            let vec_values: Vec<serde_json::Value> = match v {
                cozo::Vector::F32(f32_vec) => f32_vec
                    .iter()
                    .map(|f| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(*f as f64)
                                .unwrap_or(serde_json::Number::from(0)),
                        )
                    })
                    .collect(),
                cozo::Vector::F64(f64_vec) => f64_vec
                    .iter()
                    .map(|f| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0)),
                        )
                    })
                    .collect(),
            };
            serde_json::Value::Array(vec_values)
        }
        DataValue::Validity(v) => serde_json::json!({
            "type": "validity",
            "timestamp": v.timestamp,
        }),
        // Handle remaining variants with a catch-all
        other => serde_json::json!({
            "type": "unsupported",
            "debug": format!("{:?}", other),
        }),
    }
}

fn render_messages_json(
    messages: &[crate::record::ConversationMessage],
) -> Result<String, PrepareError> {
    serde_json::to_string_pretty(messages).map_err(PrepareError::Serialize)
}

fn render_messages_table(messages: &[crate::record::ConversationMessage]) -> String {
    if messages.is_empty() {
        return "No messages in this turn.".to_string();
    }

    let mut out = String::new();
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }

        out.push_str(&format!("Message {}\n", index + 1));
        out.push_str(&format!(
            "  role .......... {}\n",
            message_role_label(message_role(message))
        ));
        if let Some(tool_call_id) = &message.tool_call_id {
            out.push_str(&format!("  tool call id ... {}\n", tool_call_id));
        }
        out.push_str(&format!(
            "  content ....... {}\n",
            summarize_message_content(&message.content)
        ));
    }

    out.trim_end().to_string()
}

#[derive(Debug, Clone, Serialize)]
struct ToolLoopDetail {
    label: String,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
struct ToolLoopEntry {
    index: usize,
    tool: String,
    input: String,
    status: String,
    summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    details: Vec<ToolLoopDetail>,
}

fn render_tool_loop_table(turn: u32, tool_calls: &[crate::record::ToolExecutionRecord]) -> String {
    if tool_calls.is_empty() {
        return format!("Turn {}\n  No tool calls in this turn.", turn);
    }

    let mut out = String::new();
    out.push_str(&format!("Turn {}\n", turn));
    for (index, entry) in tool_loop_entries(tool_calls).iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("[{}] {}\n", entry.index, entry.tool));
        out.push_str(&dotted_loop_line("input", &entry.input));
        out.push_str(&dotted_loop_line("status", &entry.status));
        for detail in &entry.details {
            out.push_str(&dotted_loop_line(&detail.label, &detail.value));
        }
        out.push_str(&dotted_loop_line("summary", &entry.summary));
    }

    out.trim_end().to_string()
}

fn render_tool_loop_json(
    turn: u32,
    tool_calls: &[crate::record::ToolExecutionRecord],
) -> Result<String, PrepareError> {
    let payload = serde_json::json!({
        "turn": turn,
        "tool_calls": tool_loop_entries(tool_calls),
    });
    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)
}

fn tool_loop_entries(tool_calls: &[crate::record::ToolExecutionRecord]) -> Vec<ToolLoopEntry> {
    tool_calls
        .iter()
        .enumerate()
        .map(|(index, call)| tool_loop_entry(index, call))
        .collect()
}

fn tool_loop_entry(index: usize, call: &crate::record::ToolExecutionRecord) -> ToolLoopEntry {
    let input = summarize_tool_inputs(&call.request.tool, &call.request.arguments);
    match &call.result {
        crate::record::ToolResult::Completed(completed) => ToolLoopEntry {
            index,
            tool: call.request.tool.clone(),
            input: normalize_loop_input(&input),
            status: "completed".to_string(),
            summary: summarize_loop_success(completed),
            details: summarize_loop_success_details(completed),
        },
        crate::record::ToolResult::Failed(failed) => ToolLoopEntry {
            index,
            tool: call.request.tool.clone(),
            input: normalize_loop_input(&input),
            status: "failed".to_string(),
            summary: summarize_loop_failure(failed),
            details: summarize_loop_failure_details(failed),
        },
    }
}

fn normalize_loop_input(input: &str) -> String {
    if input.trim().is_empty() {
        "(none)".to_string()
    } else {
        truncate_middle(input, 96)
    }
}

fn summarize_loop_success(completed: &crate::runner::ToolCompletedRecord) -> String {
    if let Some(ui_payload) = &completed.ui_payload {
        if !ui_payload.summary.trim().is_empty() {
            return truncate_middle(&ui_payload.summary, 96);
        }
    }

    let first_line = completed.content.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "ok".to_string()
    } else {
        truncate_middle(first_line, 96)
    }
}

fn summarize_loop_failure(failed: &crate::runner::ToolFailedRecord) -> String {
    if let Some(ui_payload) = &failed.ui_payload {
        if !ui_payload.summary.trim().is_empty() {
            return truncate_middle(&ui_payload.summary, 96);
        }
    }

    let first_line = failed.error.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "failed".to_string()
    } else {
        truncate_middle(first_line, 96)
    }
}

fn summarize_loop_success_details(
    completed: &crate::runner::ToolCompletedRecord,
) -> Vec<ToolLoopDetail> {
    let Some(ui_payload) = &completed.ui_payload else {
        return Vec::new();
    };

    let mut details: Vec<ToolLoopDetail> = ui_payload
        .fields
        .iter()
        .filter(|field| field.name.as_ref() != "status")
        .take(2)
        .map(|field| ToolLoopDetail {
            label: field.name.to_string(),
            value: truncate_middle(&prettify_field_value(&field.value), 96),
        })
        .collect();

    if details.is_empty() {
        if let Some(details_text) = &ui_payload.details {
            details.push(ToolLoopDetail {
                label: "details".to_string(),
                value: truncate_middle(details_text, 96),
            });
        }
    }

    details
}

fn summarize_loop_failure_details(failed: &crate::runner::ToolFailedRecord) -> Vec<ToolLoopDetail> {
    let Some(ui_payload) = &failed.ui_payload else {
        return Vec::new();
    };

    let mut details = Vec::new();
    if let Some(error_code) = ui_payload.error_code {
        details.push(ToolLoopDetail {
            label: "code".to_string(),
            value: tool_error_code_label(error_code).to_string(),
        });
    }

    for field in ui_payload
        .fields
        .iter()
        .filter(|field| field.name.as_ref() != "code")
        .take(2)
    {
        details.push(ToolLoopDetail {
            label: field.name.to_string(),
            value: truncate_middle(&prettify_field_value(&field.value), 96),
        });
    }

    details
}

fn dotted_loop_line(label: &str, value: &str) -> String {
    const LABEL_WIDTH: usize = 14;
    let dots = LABEL_WIDTH.saturating_sub(label.chars().count()).max(2);
    format!("  {} {} {}\n", label, ".".repeat(dots), value)
}

fn filter_messages(
    messages: Vec<crate::record::ConversationMessage>,
    roles: &[InspectMessageRole],
    exclude_roles: &[InspectMessageRole],
) -> Vec<crate::record::ConversationMessage> {
    messages
        .into_iter()
        .filter(|message| {
            let role = message_role(message);
            (roles.is_empty() || roles.contains(&role)) && !exclude_roles.contains(&role)
        })
        .collect()
}

fn message_role(message: &crate::record::ConversationMessage) -> InspectMessageRole {
    use ploke_tui::chat_history::MessageKind;

    match message.kind {
        MessageKind::System => InspectMessageRole::System,
        MessageKind::SysInfo => InspectMessageRole::System,
        MessageKind::User => InspectMessageRole::User,
        MessageKind::Assistant => InspectMessageRole::Assistant,
        MessageKind::Tool => InspectMessageRole::Tool,
    }
}

fn message_role_label(role: InspectMessageRole) -> &'static str {
    match role {
        InspectMessageRole::System => "system",
        InspectMessageRole::User => "user",
        InspectMessageRole::Assistant => "assistant",
        InspectMessageRole::Tool => "tool",
    }
}

fn indexed_tool_calls(
    record: &crate::record::RunRecord,
) -> Vec<(usize, u32, crate::record::ToolExecutionRecord)> {
    record
        .conversations()
        .flat_map(|turn| {
            turn.tool_calls()
                .into_iter()
                .map(move |call| (turn.turn_number, call))
        })
        .enumerate()
        .map(|(index, (turn, call))| (index, turn, call))
        .collect()
}

fn build_tool_call_sequence_subject(
    record: &crate::record::RunRecord,
) -> Result<trace::ToolCallSequence, PrepareError> {
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let indexed = indexed_tool_calls(record);

    let turns = record
        .conversations()
        .map(|turn| trace::TurnContext {
            turn: turn.turn_number,
            tool_count: turn.tool_calls().len(),
            failed_tool_count: failed_tool_count(turn),
            patch_proposed: turn
                .tool_calls()
                .iter()
                .any(|call| call.request.tool == "non_semantic_patch"),
            patch_applied: summarize_patch_state(&turn.tool_calls()) == "applied",
        })
        .collect();

    let calls = indexed
        .iter()
        .map(|(index, turn, call)| summarize_neighborhood_call(*index, *turn, call))
        .collect();

    Ok(trace::ToolCallSequence {
        subject_id,
        total_turns: record.conversations().count(),
        total_calls_in_run: indexed.len(),
        turns,
        calls,
    })
}

struct RecordToolCallNeighborhoodAdapter<'a> {
    record: &'a crate::record::RunRecord,
    subject_id: String,
}

#[derive(Debug, thiserror::Error)]
enum RecordToolCallNeighborhoodError {
    #[error("selected tool call index {0} not found")]
    MissingIndex(usize),
    #[error("turn {0} not found for selected tool call")]
    MissingTurn(u32),
}

impl trace::NeighborhoodSource for RecordToolCallNeighborhoodAdapter<'_> {
    type Error = RecordToolCallNeighborhoodError;

    fn neighborhood(
        &self,
        request: &trace::NeighborhoodRequest,
    ) -> Result<trace::ToolCallNeighborhood, Self::Error> {
        let indexed = indexed_tool_calls(self.record);
        let focal_turn = indexed
            .iter()
            .find(|(index, _, _)| *index == request.focal_index)
            .map(|(_, turn, _)| *turn)
            .ok_or(RecordToolCallNeighborhoodError::MissingIndex(
                request.focal_index,
            ))?;

        let turn_record = self
            .record
            .turn_record(focal_turn)
            .ok_or(RecordToolCallNeighborhoodError::MissingTurn(focal_turn))?;
        let turn_calls: Vec<_> = indexed
            .iter()
            .filter(|(_, turn, _)| *turn == focal_turn)
            .cloned()
            .collect();
        let turn_position = turn_calls
            .iter()
            .position(|(index, _, _)| *index == request.focal_index)
            .ok_or(RecordToolCallNeighborhoodError::MissingIndex(
                request.focal_index,
            ))?;
        let start = turn_position.saturating_sub(request.radius_before);
        let end = (turn_position + request.radius_after + 1).min(turn_calls.len());

        let before = turn_calls[start..turn_position]
            .iter()
            .map(|(index, turn, call)| summarize_neighborhood_call(*index, *turn, call))
            .collect();
        let focal = summarize_neighborhood_call(
            turn_calls[turn_position].0,
            turn_calls[turn_position].1,
            &turn_calls[turn_position].2,
        );
        let after = turn_calls[turn_position + 1..end]
            .iter()
            .map(|(index, turn, call)| summarize_neighborhood_call(*index, *turn, call))
            .collect();

        Ok(trace::ToolCallNeighborhood {
            subject_id: self.subject_id.clone(),
            total_calls_in_run: indexed.len(),
            total_calls_in_turn: turn_calls.len(),
            turn: trace::TurnContext {
                turn: focal_turn,
                tool_count: turn_record.tool_calls().len(),
                failed_tool_count: failed_tool_count(turn_record),
                patch_proposed: turn_record
                    .tool_calls()
                    .iter()
                    .any(|call| call.request.tool == "non_semantic_patch"),
                patch_applied: summarize_patch_state(&turn_record.tool_calls()) == "applied",
            },
            before,
            focal,
            after,
        })
    }
}

fn build_tool_call_review_subject(
    record: &crate::record::RunRecord,
    index: usize,
) -> Result<trace::ToolCallNeighborhood, PrepareError> {
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let adapter = RecordToolCallNeighborhoodAdapter { record, subject_id };
    adapter
        .neighborhood(&trace::NeighborhoodRequest::centered(index))
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_tool_call_review",
            detail: err.to_string(),
        })
}

fn build_segment_review_subject(
    segmented: &segment::SegmentedToolCallSequence,
    segment_index: usize,
) -> Result<review::SegmentReviewSubject, PrepareError> {
    let segment = segmented
        .segments
        .iter()
        .find(|segment| segment.segment_index == segment_index)
        .cloned()
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "protocol_tool_call_segment_review",
            detail: format!("segment index {segment_index} not found"),
        })?;

    Ok(review::SegmentReviewSubject {
        subject_id: segmented.sequence.subject_id.clone(),
        sequence: segmented.sequence.clone(),
        segment,
        coverage: segmented.coverage.clone(),
    })
}

fn summarize_neighborhood_call(
    index: usize,
    turn: u32,
    call: &crate::record::ToolExecutionRecord,
) -> trace::NeighborhoodCall {
    trace::NeighborhoodCall {
        index,
        turn,
        tool_name: call.request.tool.clone(),
        tool_kind: classify_tool_kind(&call.request.tool),
        failed: matches!(call.result, crate::record::ToolResult::Failed(_)),
        latency_ms: call.latency_ms,
        summary: tool_call_summary_line(call),
        args_preview: truncate_middle(call.request.arguments.as_str(), 96),
        result_preview: tool_result_preview(call),
        search_term: extract_argument_string(
            &call.request.tool,
            &call.request.arguments,
            &["search_term", "query"],
        ),
        path_hint: extract_argument_string(
            &call.request.tool,
            &call.request.arguments,
            &["file", "dir", "path", "target_dir"],
        ),
    }
}

fn classify_tool_kind(tool_name: &str) -> trace::ToolKind {
    match tool_name {
        "request_code_context" | "search_code" | "search_symbols" | "query_codebase" => {
            trace::ToolKind::Search
        }
        "read_file" => trace::ToolKind::Read,
        "list_dir" => trace::ToolKind::Browse,
        "apply_code_edit" => trace::ToolKind::Edit,
        "run_command" | "shell" => trace::ToolKind::Execute,
        _ => trace::ToolKind::Other,
    }
}

fn tool_result_preview(call: &crate::record::ToolExecutionRecord) -> String {
    match &call.result {
        crate::record::ToolResult::Completed(completed) => truncate_middle(&completed.content, 96),
        crate::record::ToolResult::Failed(failed) => truncate_middle(&failed.error, 96),
    }
}

fn extract_argument_string(
    tool: &str,
    arguments: &ToolArgumentsJson,
    keys: &[&str],
) -> Option<String> {
    let PersistedToolCallArguments::Decoded(arguments) = arguments.decode_for_tool(tool) else {
        return None;
    };
    tool_argument_projection(&arguments)
        .into_iter()
        .find(|field| keys.contains(&field.key))
        .map(|field| field.value)
}

fn tool_call_summary_line(call: &crate::record::ToolExecutionRecord) -> String {
    match &call.result {
        crate::record::ToolResult::Completed(completed) => format!(
            "tool={} status=completed latency_ms={} args={} result={}",
            call.request.tool,
            call.latency_ms,
            truncate_middle(call.request.arguments.as_str(), 96),
            truncate_middle(&completed.content, 96),
        ),
        crate::record::ToolResult::Failed(failed) => format!(
            "tool={} status=failed latency_ms={} args={} error={}",
            call.request.tool,
            call.latency_ms,
            truncate_middle(call.request.arguments.as_str(), 96),
            truncate_middle(&failed.error, 96),
        ),
    }
}

fn resolve_protocol_model_id(model_id: Option<String>) -> Result<ModelId, PrepareError> {
    match model_id {
        Some(model_id) => {
            model_id
                .parse()
                .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                    phase: "protocol_model_id",
                    detail: err.to_string(),
                })
        }
        // Temporary split config: eval/protocol defaults still read the active
        // model selection here, while broad parent patching reads
        // `load_parent_patcher_model_selection()` below. Collapse both onto the
        // admitted profile/campaign config once that plumbing exists.
        None => load_active_model().map(|selection| selection.model_id),
    }
}

fn resolve_protocol_provider_slug(
    model_id: &ModelId,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
) -> Result<Option<String>, PrepareError> {
    resolve_protocol_route(model_id, route_source, provider).map(|(_, provider_slug)| provider_slug)
}

fn resolve_protocol_route(
    model_id: &ModelId,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
) -> Result<(ModelRouteSource, Option<String>), PrepareError> {
    if let Some(route_source) = route_source {
        return match route_source {
            ModelRouteSource::DirectGoogle => {
                if let Some(provider) = provider.as_deref()
                    && provider != "google"
                {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "protocol_route",
                        detail: format!(
                            "direct Google route does not accept OpenRouter provider '{provider}'"
                        ),
                    });
                }
                Ok((ModelRouteSource::DirectGoogle, None))
            }
            ModelRouteSource::OpenRouter => {
                let provider_slug = provider
                    .map(|provider| {
                        ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
                            phase: "protocol_provider_slug",
                            detail: err.to_string(),
                        })
                    })
                    .transpose()?
                    .map(|provider| provider.slug.as_str().to_string());
                Ok((ModelRouteSource::OpenRouter, provider_slug))
            }
        };
    }

    if let Some(provider) = provider {
        let parsed = ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_provider_slug",
            detail: err.to_string(),
        })?;
        if registry_route_source(model_id)?.is_some_and(|source| source.is_direct_google()) {
            if parsed.slug.as_str() == "google" {
                return Ok((ModelRouteSource::DirectGoogle, None));
            }
            return Err(PrepareError::DatabaseSetup {
                phase: "protocol_route",
                detail: format!(
                    "direct Google model '{model_id}' does not accept OpenRouter provider '{}'",
                    parsed.slug.as_str()
                ),
            });
        }
        return Ok((
            ModelRouteSource::OpenRouter,
            Some(parsed.slug.as_str().to_string()),
        ));
    }

    if registry_route_source(model_id)?.is_some_and(|source| source.is_direct_google()) {
        return Ok((ModelRouteSource::DirectGoogle, None));
    }

    let provider = load_provider_for_model(model_id)?;
    Ok((
        ModelRouteSource::OpenRouter,
        provider.map(|provider| provider.slug.as_str().to_string()),
    ))
}

fn print_protocol_artifact_detail(index: usize, entry: &StoredProtocolArtifactFile, full: bool) {
    println!("Protocol Artifact {}", index);
    println!("{}", "-".repeat(40));
    println!("Path: {}", entry.path.display());
    println!("Procedure: {}", entry.stored.procedure_name);
    println!("Subject: {}", entry.stored.subject_id);
    println!("Run: {}", entry.stored.run_id);
    println!("Created (ms): {}", entry.stored.created_at_ms);
    println!(
        "Model: {}",
        entry.stored.model_id.as_deref().unwrap_or("(unknown)")
    );
    println!(
        "Provider: {}",
        entry
            .stored
            .provider_slug
            .as_deref()
            .unwrap_or("auto/openrouter")
    );
    println!("Summary: {}", protocol_artifact_summary(entry));
    println!();
    if full {
        println!("Input");
        println!("{}", "-".repeat(40));
        println!(
            "{}",
            serde_json::to_string_pretty(&entry.stored.input)
                .unwrap_or_else(|_| { protocol_artifact_preview(&entry.stored.input) })
        );
        println!();
        println!("Output");
        println!("{}", "-".repeat(40));
        println!(
            "{}",
            serde_json::to_string_pretty(&entry.stored.output)
                .unwrap_or_else(|_| { protocol_artifact_preview(&entry.stored.output) })
        );
        println!();
        println!("Artifact");
        println!("{}", "-".repeat(40));
        println!(
            "{}",
            serde_json::to_string_pretty(&entry.stored.artifact)
                .unwrap_or_else(|_| { protocol_artifact_preview(&entry.stored.artifact) })
        );
    } else {
        println!("Input: {}", protocol_artifact_preview(&entry.stored.input));
        println!(
            "Output: {}",
            protocol_artifact_preview(&entry.stored.output)
        );
        println!(
            "Artifact: {}",
            protocol_artifact_preview(&entry.stored.artifact)
        );
        println!();
        println!("Tip: rerun with `--full` to print the full nested payloads.");
    }
}

fn tool_call_review_error_to_prepare(err: review::ToolCallReviewError) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_review",
        detail: err.to_string(),
    }
}

fn tool_call_intent_segmentation_error_to_prepare(
    err: segment::IntentSegmentationError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_intent_segmentation",
        detail: err.to_string(),
    }
}

fn is_retryable_intent_segmentation_error(err: &segment::IntentSegmentationError) -> bool {
    match err {
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Branches(
            ploke_protocol::FanOutError::Right(llm_error),
        )) if llm_error.is_truncated_json_parse() => true,
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Join(
            segment::NormalizeSegmentsError::Overlap { .. }
            | segment::NormalizeSegmentsError::InvalidRange { .. }
            | segment::NormalizeSegmentsError::MissingLabel { .. }
            | segment::NormalizeSegmentsError::AmbiguousWithLabel { .. },
        )) => true,
        _ => false,
    }
}

fn tool_call_segment_review_error_to_prepare(
    err: review::ToolCallSegmentReviewError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_segment_review",
        detail: err.to_string(),
    }
}

fn failed_tool_count(turn: &crate::record::TurnRecord) -> usize {
    turn.tool_calls()
        .iter()
        .filter(|call| matches!(call.result, crate::record::ToolResult::Failed(_)))
        .count()
}

fn summarize_turn_outcome(turn: &crate::record::TurnRecord) -> String {
    match &turn.outcome {
        crate::record::TurnOutcome::ToolCalls { .. } => "completed".to_string(),
        crate::record::TurnOutcome::Content => "content".to_string(),
        crate::record::TurnOutcome::Error { message } => {
            format!("error: {}", truncate_middle(message, 16))
        }
        crate::record::TurnOutcome::Timeout { elapsed_secs } => {
            format!("timeout({}s)", elapsed_secs)
        }
    }
}

fn summarize_message_content(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "(empty)".to_string()
    } else {
        truncate_middle(first_line, 72)
    }
}

fn tool_call_next_step_index(tool_call_count: usize) -> Option<usize> {
    if tool_call_count > 0 { Some(0) } else { None }
}

fn join_indices(indices: &[usize]) -> String {
    indices
        .iter()
        .map(|idx| idx.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_turns(turns: &[u32]) -> String {
    turns
        .iter()
        .map(|turn| turn.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_segment_descriptor(
    status: segment::SegmentStatus,
    label: Option<segment::IntentLabel>,
) -> String {
    match (status, label) {
        (segment::SegmentStatus::Labeled, Some(label)) => format!("labeled:{label:?}"),
        (segment::SegmentStatus::Labeled, None) => "labeled:<missing>".to_string(),
        (segment::SegmentStatus::Ambiguous, _) => "ambiguous".to_string(),
    }
}

fn print_turn_summary(turn: &crate::record::TurnRecord) {
    let messages = turn.messages();
    let tool_calls = turn.tool_calls();
    let failed_tools = tool_calls
        .iter()
        .filter(|call| matches!(call.result, crate::record::ToolResult::Failed(_)))
        .count();
    let patch_proposed = tool_calls
        .iter()
        .any(|call| call.request.tool == "non_semantic_patch");
    let patch_applied = summarize_patch_state(&tool_calls);

    println!("Turn {}", turn.turn_number);
    println!("  tools .............. {}", tool_calls.len());
    println!("  failed tools ....... {}", failed_tools);
    println!("  messages ........... {}", messages.len());
    println!(
        "  patch proposed ..... {}",
        if patch_proposed { "yes" } else { "no" }
    );
    println!("  patch applied ...... {}", patch_applied);
}

fn assistant_message_id_for_turn(turn: &crate::record::TurnRecord) -> Option<&str> {
    turn.agent_turn_artifact
        .as_ref()
        .and_then(|artifact| artifact.terminal_record.as_ref())
        .map(|record| record.assistant_message_id.as_str())
}

fn resolve_full_response_trace_path(
    record_path: &std::path::Path,
    record: &crate::record::RunRecord,
) -> Result<PathBuf, PrepareError> {
    let run_dir = record_path
        .parent()
        .ok_or_else(|| PrepareError::MissingRunManifest(record_path.to_path_buf()))?;
    let path = run_dir.join(FULL_RESPONSE_TRACE_FILE);
    if path.exists() {
        Ok(path)
    } else if record.metadata.run_arm.execution == "agent-single-turn" {
        Err(PrepareError::MissingRunManifest(path))
    } else {
        Err(PrepareError::DatabaseSetup {
            phase: "inspect_turn",
            detail: "raw full responses are only captured for agent-mode runs".to_string(),
        })
    }
}

fn load_full_response_records_for_turn(
    path: &std::path::Path,
    assistant_message_id: &str,
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let assistant_message_id = parse_full_response_assistant_message_id(assistant_message_id)?;
    let text = std::fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut responses = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: RawFullResponseRecord =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        if record.matches_assistant_message(assistant_message_id) {
            responses.push(record);
        }
    }
    responses.sort_by_key(|record| record.response_index());
    Ok(responses)
}

fn load_all_full_response_records(
    path: &std::path::Path,
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let text = std::fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut responses = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: RawFullResponseRecord =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        responses.push(record);
    }
    responses.sort_by_key(|record| record.response_index());
    Ok(responses)
}

fn parse_full_response_assistant_message_id(
    assistant_message_id: &str,
) -> Result<Uuid, PrepareError> {
    Uuid::parse_str(assistant_message_id).map_err(|source| PrepareError::DatabaseSetup {
        phase: "inspect_turn",
        detail: format!("invalid assistant_message_id '{assistant_message_id}': {source}"),
    })
}

fn aggregate_full_response_usage(
    responses: &[RawFullResponseRecord],
) -> ploke_llm::response::TokenUsage {
    // Stopgap: this sums the persisted raw-response sidecar as captured today.
    // It is useful for eval introspection, but may undercount a turn when the
    // final non-tool-call/stop response is not yet captured into the sidecar.
    let mut total = ploke_llm::response::TokenUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };
    for record in responses {
        if let Some(usage) = record.response().usage.as_ref() {
            total.prompt_tokens += usage.prompt_tokens;
            total.completion_tokens += usage.completion_tokens;
            total.total_tokens += usage.total_tokens;
        }
    }
    total
}

fn load_full_response_usage_totals(
    record_path: &std::path::Path,
    record: &crate::record::RunRecord,
) -> Result<Option<ploke_llm::response::TokenUsage>, PrepareError> {
    let trace_path = match resolve_full_response_trace_path(record_path, record) {
        Ok(path) => path,
        Err(PrepareError::MissingRunManifest(_)) => return Ok(None),
        Err(err) => return Err(err),
    };
    let responses = load_all_full_response_records(&trace_path)?;
    if responses.is_empty() {
        Ok(None)
    } else {
        Ok(Some(aggregate_full_response_usage(&responses)))
    }
}

fn format_usage_triplet(prompt: u32, completion: u32, total: u32) -> String {
    format!(
        "prompt:{} completion:{} total:{}",
        prompt, completion, total
    )
}

fn render_full_response_table(turn: u32, responses: &[RawFullResponseRecord]) -> String {
    let totals = aggregate_full_response_usage(responses);
    let mut out = String::new();
    out.push_str(&format!("Turn {} Raw Full Responses\n", turn));
    out.push_str(&format!("{}\n", "-".repeat(80)));
    out.push_str(&format!(
        "{:<5} {:<40} {:<14} {}\n",
        "Idx", "Response ID", "Finish", "Usage"
    ));
    out.push_str(&format!("{}\n", "-".repeat(80)));
    for record in responses {
        let response = record.response();
        let response_id = truncate_for_table(&response.id, 38);
        let finish = record
            .response()
            .choices
            .first()
            .and_then(|choice| choice.finish_reason.as_ref())
            .map(|reason| format!("{reason:?}").to_lowercase())
            .unwrap_or_else(|| "-".to_string());
        let usage = record
            .response()
            .usage
            .as_ref()
            .map(|usage| {
                format!(
                    "p:{} c:{} t:{}",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                )
            })
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "{:<5} {:<40} {:<14} {}\n",
            record.response_index(),
            response_id,
            finish,
            usage
        ));
    }
    out.push_str(&format!("{}\n", "-".repeat(80)));
    out.push_str(&format!(
        "Totals: {}\n",
        format_usage_triplet(
            totals.prompt_tokens,
            totals.completion_tokens,
            totals.total_tokens
        )
    ));
    out.push_str(
        "Note: sidecar totals may undercount if the final stop response was not captured.\n",
    );
    out
}

fn summarize_patch_state(tool_calls: &[crate::record::ToolExecutionRecord]) -> &'static str {
    let patch_calls: Vec<_> = tool_calls
        .iter()
        .filter(|call| call.request.tool == "non_semantic_patch")
        .collect();

    if patch_calls.is_empty() {
        return "no";
    }

    let completed_calls: Vec<_> = patch_calls
        .iter()
        .filter_map(|call| match &call.result {
            crate::record::ToolResult::Completed(completed) => Some(completed),
            crate::record::ToolResult::Failed(_) => None,
        })
        .collect();

    if completed_calls.iter().any(|completed| {
        tool_ui_field_usize(completed.ui_payload.as_ref(), "applied").is_some_and(|value| value > 0)
    }) {
        "applied"
    } else if completed_calls.iter().any(|completed| {
        tool_ui_field_usize(completed.ui_payload.as_ref(), "staged").is_some_and(|value| value > 0)
    }) {
        "staged"
    } else if patch_calls
        .iter()
        .any(|call| matches!(&call.result, crate::record::ToolResult::Failed(_)))
    {
        "failed"
    } else {
        "no"
    }
}

fn tool_ui_field_usize(
    ui_payload: Option<&ploke_tui::tools::ToolUiPayload>,
    name: &str,
) -> Option<usize> {
    ui_payload
        .and_then(|ui| {
            ui.fields
                .iter()
                .find(|field| field.name.as_ref() == name)
                .map(|field| field.value.as_ref())
        })
        .and_then(|value| value.parse().ok())
}

fn print_tool_call_detail(
    turn: u32,
    index: usize,
    call: &crate::record::ToolExecutionRecord,
    full: bool,
) {
    println!("Tool Call {}", index);
    println!("{}", "-".repeat(40));
    println!("Turn: {}", turn);
    println!("Tool: {}", call.request.tool);
    println!("Status: {}", tool_status_label(&call.result));
    println!("Latency: {} ms", call.latency_ms);
    println!();
    println!("Parsed Inputs (convenience view from stored raw arguments)");
    println!("{}", "-".repeat(40));
    let rendered_inputs = render_tool_inputs(&call.request.tool, &call.request.arguments);
    if rendered_inputs.is_empty() {
        println!("(none)");
    } else {
        for line in rendered_inputs {
            println!("{}", line);
        }
    }
    println!();
    print_tool_result_detail(index, &call.result, full);
    if full {
        println!();
        println!("Stored Raw Arguments");
        println!("{}", "-".repeat(40));
        println!("{}", call.request.arguments.as_str());
    }
}

fn print_tool_result_detail(index: usize, result: &crate::record::ToolResult, full: bool) {
    println!("Result");
    println!("{}", "-".repeat(40));
    match result {
        crate::record::ToolResult::Completed(completed) => {
            if let Some(ui_payload) = &completed.ui_payload {
                println!("UI Summary (convenience only): {}", ui_payload.summary);
                for field in &ui_payload.fields {
                    println!("ui.{}: {}", field.name, prettify_field_value(&field.value));
                }
                if let Some(details) = &ui_payload.details {
                    let rendered = render_payload_block(details, if full { 1200 } else { 220 });
                    println!("UI Details (convenience only):");
                    println!("{}", rendered.text);
                    if let Some(note) = rendered.inspector_truncation_note() {
                        println!("{}", note);
                    }
                }
                println!();
            }
            println!("Stored Raw Output:");
            if completed.content.is_empty() {
                println!("(empty)");
            } else {
                let rendered =
                    render_payload_block(&completed.content, if full { 2400 } else { 220 });
                println!("{}", rendered.text);
                if let Some(note) = summarize_tool_native_truncation(&completed.content) {
                    println!("{}", note);
                }
                if let Some(note) = rendered.inspector_truncation_note() {
                    println!("{}", note);
                }
                println!("Stored raw bytes: {}", rendered.raw_bytes);
            }
        }
        crate::record::ToolResult::Failed(failed) => {
            if let Some(ui_payload) = &failed.ui_payload {
                println!("UI Summary (convenience only): {}", ui_payload.summary);
                for field in &ui_payload.fields {
                    println!("ui.{}: {}", field.name, prettify_field_value(&field.value));
                }
                println!();
            }
            println!("Stored Raw Error:");
            if failed.error.is_empty() {
                println!("(empty)");
            } else {
                let rendered = render_payload_block(&failed.error, if full { 2400 } else { 220 });
                println!("{}", rendered.text);
                if let Some(note) = rendered.inspector_truncation_note() {
                    println!("{}", note);
                }
                println!("Stored raw bytes: {}", rendered.raw_bytes);
            }
        }
    }
    if !full {
        println!();
        println!("Tip: rerun with `--full {}` for the full payload.", index);
    }
}

#[derive(Debug, Clone)]
struct ToolArgumentField {
    key: &'static str,
    value: String,
}

fn render_tool_inputs(tool: &str, arguments: &ToolArgumentsJson) -> Vec<String> {
    match arguments.decode_for_tool(tool) {
        PersistedToolCallArguments::Decoded(arguments) => {
            render_tool_argument_fields(&tool_argument_projection(&arguments))
        }
        PersistedToolCallArguments::ParseFailure(failure) => {
            render_tool_argument_parse_failure(&failure)
        }
    }
}

fn summarize_tool_inputs(tool: &str, arguments: &ToolArgumentsJson) -> String {
    match arguments.decode_for_tool(tool) {
        PersistedToolCallArguments::Decoded(arguments) => {
            summarize_tool_argument_fields(&tool_argument_projection(&arguments))
        }
        PersistedToolCallArguments::ParseFailure(failure) => {
            truncate_middle(&failure.raw_arguments, 46)
        }
    }
}

fn render_tool_argument_fields(fields: &[ToolArgumentField]) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(line_window) = line_window_from_fields(fields) {
        lines.push(format!(
            "lines (derived from start_line/end_line): {}",
            line_window
        ));
    }
    for field in fields {
        if matches!(field.key, "start_line" | "end_line") {
            continue;
        }
        lines.push(format!(
            "{}: {}",
            field.key,
            summarize_argument_value(field.key, &field.value, true)
        ));
    }
    lines
}

fn summarize_tool_argument_fields(fields: &[ToolArgumentField]) -> String {
    let mut parts = Vec::new();
    if let Some(line_window) = line_window_from_fields(fields) {
        parts.push(format!("lines={line_window}"));
    }
    for field in fields {
        if matches!(field.key, "start_line" | "end_line") {
            continue;
        }
        parts.push(format!(
            "{}={}",
            field.key,
            summarize_argument_value(field.key, &field.value, false)
        ));
        if parts.len() >= 3 {
            break;
        }
    }
    parts.join("; ")
}

fn render_tool_argument_parse_failure(failure: &ToolArgumentParseFailure) -> Vec<String> {
    vec![
        format!(
            "decode_error: {}",
            tool_argument_decode_error(&failure.error)
        ),
        format!(
            "arguments: {}",
            render_payload_block(&failure.raw_arguments, 220).text
        ),
    ]
}

fn tool_argument_decode_error(error: &ToolArgumentDecodeError) -> String {
    match error {
        ToolArgumentDecodeError::UnknownTool { tool } => format!("unknown tool `{tool}`"),
        ToolArgumentDecodeError::InvalidJson { message } => message.clone(),
    }
}

fn line_window_from_fields(fields: &[ToolArgumentField]) -> Option<String> {
    let start = fields
        .iter()
        .find(|field| field.key == "start_line")
        .map(|field| field.value.as_str());
    let end = fields
        .iter()
        .find(|field| field.key == "end_line")
        .map(|field| field.value.as_str());
    match (start, end) {
        (Some(start), Some(end)) if start != end => Some(format!("{start}-{end}")),
        (Some(line), _) | (_, Some(line)) => Some(line.to_string()),
        _ => None,
    }
}

fn tool_argument_projection(arguments: &ToolCallArguments) -> Vec<ToolArgumentField> {
    let mut fields = Vec::new();
    match arguments {
        ToolCallArguments::RequestCodeContext(args) => {
            push_option(
                &mut fields,
                "token_budget_per_result",
                args.token_budget_per_result,
            );
            push_option(&mut fields, "token_budget_total", args.token_budget_total);
            push_option_ref(&mut fields, "search_term", args.search_term.as_deref());
        }
        ToolCallArguments::ApplyCodeEdit(args) => {
            push_count(&mut fields, "edits", args.edits.len());
            push_option(&mut fields, "confidence", args.confidence);
            if let Some(edit) = args.edits.first() {
                push_ref(&mut fields, "file", &edit.file);
                push_ref(&mut fields, "canon", &edit.canon);
                push_value(&mut fields, "node_type", format!("{:?}", edit.node_type));
                push_value(&mut fields, "code", byte_count(&edit.code));
            }
        }
        ToolCallArguments::InsertRustItem(args) => {
            push_ref(&mut fields, "file", &args.file);
            push_value(
                &mut fields,
                "container_kind",
                format!("{:?}", args.container_kind),
            );
            push_option_ref(
                &mut fields,
                "container_canon",
                args.container_canon.as_deref(),
            );
            push_value(&mut fields, "item_kind", format!("{:?}", args.item_kind));
            push_value(&mut fields, "code", byte_count(&args.code));
            push_option(&mut fields, "confidence", args.confidence);
        }
        ToolCallArguments::CreateFile(args) => {
            push_ref(&mut fields, "file_path", &args.file_path);
            push_value(&mut fields, "content", byte_count(&args.content));
            push_option_ref(&mut fields, "on_exists", args.on_exists.as_deref());
            push_value(&mut fields, "create_parents", args.create_parents);
        }
        ToolCallArguments::NsPatch(args) => {
            push_count(&mut fields, "patches", args.patches.len());
            push_option(&mut fields, "confidence", args.confidence);
            if let Some(patch) = args.patches.first() {
                push_ref(&mut fields, "file", &patch.file);
                push_value(&mut fields, "diff", byte_count(&patch.diff));
                push_value(&mut fields, "reasoning", byte_count(&patch.reasoning));
            }
        }
        ToolCallArguments::NsRead(args) => {
            push_ref(&mut fields, "file", &args.file);
            push_option(&mut fields, "start_line", args.start_line);
            push_option(&mut fields, "end_line", args.end_line);
            push_option(&mut fields, "max_bytes", args.max_bytes);
        }
        ToolCallArguments::CodeItemLookup(args) => {
            push_ref(&mut fields, "item_name", &args.item_name);
            push_ref(&mut fields, "file_path", &args.file_path);
            push_ref(&mut fields, "node_kind", &args.node_kind);
            push_ref(&mut fields, "module_path", &args.module_path);
        }
        ToolCallArguments::CodeItemEdges(args) => {
            push_ref(&mut fields, "item_name", &args.item_name);
            push_ref(&mut fields, "file_path", &args.file_path);
            push_ref(&mut fields, "node_kind", &args.node_kind);
            push_ref(&mut fields, "module_path", &args.module_path);
        }
        ToolCallArguments::Cargo(args) => {
            push_value(&mut fields, "command", format!("{:?}", args.command));
            push_value(&mut fields, "scope", format!("{:?}", args.scope));
            push_option_ref(&mut fields, "package", args.package.as_deref());
            push_option_ref(&mut fields, "target", args.target.as_deref());
            push_option_ref(&mut fields, "profile", args.profile.as_deref());
            push_option(
                &mut fields,
                "features",
                args.features.as_ref().map(|v| v.join(",")),
            );
        }
        ToolCallArguments::ListDir(args) => {
            push_ref(&mut fields, "dir", &args.dir);
            push_value(&mut fields, "include_hidden", args.include_hidden);
            push_option_ref(&mut fields, "sort", args.sort.as_deref());
            push_option(&mut fields, "max_entries", args.max_entries);
        }
        ToolCallArguments::SearchCode(args)
        | ToolCallArguments::SearchSymbols(args)
        | ToolCallArguments::QueryCodebase(args) => {
            push_option_ref(&mut fields, "search_term", args.search_term.as_deref());
            push_option_ref(&mut fields, "query", args.query.as_deref());
        }
    }
    fields
}

fn push_ref(fields: &mut Vec<ToolArgumentField>, key: &'static str, value: &str) {
    fields.push(ToolArgumentField {
        key,
        value: value.to_string(),
    });
}

fn push_value(fields: &mut Vec<ToolArgumentField>, key: &'static str, value: impl ToString) {
    fields.push(ToolArgumentField {
        key,
        value: value.to_string(),
    });
}

fn push_option<T>(fields: &mut Vec<ToolArgumentField>, key: &'static str, value: Option<T>)
where
    T: ToString,
{
    if let Some(value) = value {
        push_value(fields, key, value);
    }
}

fn push_option_ref(fields: &mut Vec<ToolArgumentField>, key: &'static str, value: Option<&str>) {
    if let Some(value) = value {
        push_ref(fields, key, value);
    }
}

fn push_count(fields: &mut Vec<ToolArgumentField>, key: &'static str, value: usize) {
    push_value(fields, key, value);
}

fn byte_count(value: &str) -> String {
    format!("{} bytes", value.len())
}

#[derive(Debug, Deserialize)]
struct ToolNativeTruncationProjection {
    #[serde(default, deserialize_with = "optional_bool_projection")]
    truncated: Option<bool>,
    #[serde(default, deserialize_with = "optional_u64_projection")]
    byte_len: Option<u64>,
    #[serde(default, deserialize_with = "optional_string_projection")]
    content: Option<String>,
}

fn summarize_tool_result(result: &crate::record::ToolResult) -> String {
    match result {
        crate::record::ToolResult::Completed(completed) => {
            if let Some(ui_payload) = &completed.ui_payload {
                format!("ok: {}", truncate_middle(&ui_payload.summary, 24))
            } else {
                "ok".to_string()
            }
        }
        crate::record::ToolResult::Failed(failed) => {
            format!("failed: {}", truncate_middle(&failed.error, 24))
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolFailureSummaryProjection {
    #[serde(default, deserialize_with = "optional_string_projection")]
    user: Option<String>,
    #[serde(default, deserialize_with = "optional_string_projection")]
    summary: Option<String>,
    #[serde(default, deserialize_with = "optional_string_projection")]
    message: Option<String>,
    #[serde(default, deserialize_with = "optional_string_projection")]
    error: Option<String>,
}

impl ToolFailureSummaryProjection {
    fn first_message(&self) -> Option<&str> {
        [
            self.user.as_deref(),
            self.summary.as_deref(),
            self.message.as_deref(),
            self.error.as_deref(),
        ]
        .into_iter()
        .flatten()
        .find(|text| !text.split_whitespace().collect::<String>().is_empty())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionalStringProjection {
    String(String),
    Other(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionalBoolProjection {
    Bool(bool),
    Other(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionalU64Projection {
    Number(u64),
    String(String),
    Other(serde::de::IgnoredAny),
}

fn optional_string_projection<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<OptionalStringProjection>::deserialize(deserializer)? {
            Some(OptionalStringProjection::String(value)) => Some(value),
            Some(OptionalStringProjection::Other(_)) | None => None,
        },
    )
}

fn optional_bool_projection<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<OptionalBoolProjection>::deserialize(deserializer)? {
            Some(OptionalBoolProjection::Bool(value)) => Some(value),
            Some(OptionalBoolProjection::Other(_)) | None => None,
        },
    )
}

fn optional_u64_projection<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<OptionalU64Projection>::deserialize(deserializer)? {
            Some(OptionalU64Projection::Number(value)) => Some(value),
            Some(OptionalU64Projection::String(value)) => value.parse().ok(),
            Some(OptionalU64Projection::Other(_)) | None => None,
        },
    )
}

fn summarize_failure_reason(error: &str) -> String {
    if let Ok(projection) = serde_json::from_str::<ToolFailureSummaryProjection>(error) {
        if let Some(text) = projection.first_message() {
            let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
            return truncate_middle(&normalized, 96);
        }
    }
    let normalized = error
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "-".to_string());
    truncate_middle(&normalized, 96)
}

fn tool_failure_code(failed: &crate::runner::ToolFailedRecord) -> Option<String> {
    failed
        .ui_payload
        .as_ref()
        .and_then(|payload| payload.error_code)
        .map(tool_error_code_label)
        .map(str::to_string)
}

fn top_failure_label(counts: &BTreeMap<String, usize>) -> Option<String> {
    counts
        .iter()
        .max_by(|(left_label, left_count), (right_label, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_label.cmp(left_label))
        })
        .map(|(label, _)| label.clone())
}

fn summarize_argument_value(key: &str, value: &str, multiline: bool) -> String {
    if looks_like_path(key, value) {
        abbreviate_path_tail(value, if multiline { 72 } else { 24 })
    } else {
        let limit = if multiline { 120 } else { 24 };
        prettify_field_value(&truncate_middle(value, limit))
    }
}

fn looks_like_path(key: &str, value: &str) -> bool {
    matches!(key, "file" | "file_path" | "dir" | "path" | "root_path")
        || value.starts_with('/')
        || value.contains(std::path::MAIN_SEPARATOR)
}

fn abbreviate_path_tail(path: &str, max_len: usize) -> String {
    if path.chars().count() <= max_len {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return truncate_middle(path, max_len);
    }

    let mut kept = Vec::new();
    let mut len = 3usize;
    for part in parts.iter().rev() {
        let next_len = len + part.len() + if kept.is_empty() { 0 } else { 1 };
        if next_len > max_len {
            break;
        }
        kept.push(*part);
        len = next_len;
    }
    kept.reverse();

    if kept.is_empty() {
        truncate_middle(path, max_len)
    } else {
        format!(".../{}", kept.join("/"))
    }
}

pub(crate) fn truncate_for_table(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!(
            "{}...",
            text.chars()
                .take(max_len.saturating_sub(3))
                .collect::<String>()
        )
    }
}

pub(crate) fn truncate_middle(text: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let front = (max_len - 3) / 2;
    let back = max_len - 3 - front;
    format!(
        "{}...{}",
        chars[..front].iter().collect::<String>(),
        chars[chars.len() - back..].iter().collect::<String>()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedPayloadBlock {
    text: String,
    raw_bytes: usize,
    normalized_chars: usize,
    shown_source_chars: usize,
    inspector_truncated: bool,
}

impl RenderedPayloadBlock {
    fn inspector_truncation_note(&self) -> Option<String> {
        if !self.inspector_truncated {
            return None;
        }

        let omitted = self
            .normalized_chars
            .saturating_sub(self.shown_source_chars);
        Some(format!(
            "Inspector display truncated the normalized payload: {}/{} source chars shown (+ ellipsis), {} elided; stored raw payload is {} bytes.",
            self.shown_source_chars, self.normalized_chars, omitted, self.raw_bytes
        ))
    }
}

fn summarize_tool_native_truncation(content: &str) -> Option<String> {
    let projection: ToolNativeTruncationProjection = serde_json::from_str(content).ok()?;
    let truncated = projection.truncated?;
    if !truncated {
        return None;
    }

    let source_bytes = projection.byte_len?;
    let retained_bytes = projection
        .content
        .as_deref()
        .map(|text| text.len() as u64)
        .unwrap_or(0);
    let omitted_bytes = source_bytes.saturating_sub(retained_bytes);

    Some(format!(
        "Tool-reported truncation: {} / {} source bytes retained, {} omitted before inspector display.",
        retained_bytes, source_bytes, omitted_bytes
    ))
}

fn render_payload_block(text: &str, max_len: usize) -> RenderedPayloadBlock {
    let trimmed = text.trim();
    let rendered = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| trimmed.to_string());

    let normalized_chars = rendered.chars().count();
    let inspector_truncated = normalized_chars > max_len;
    let shown_source_chars = if inspector_truncated {
        max_len.saturating_sub(3).min(normalized_chars)
    } else {
        normalized_chars
    };

    RenderedPayloadBlock {
        text: truncate_middle(&rendered, max_len),
        raw_bytes: trimmed.len(),
        normalized_chars,
        shown_source_chars,
        inspector_truncated,
    }
}

fn prettify_field_value(text: &str) -> String {
    text.replace('\n', "\\n")
}

fn tool_status_label(result: &crate::record::ToolResult) -> &'static str {
    match result {
        crate::record::ToolResult::Completed(_) => "completed",
        crate::record::ToolResult::Failed(_) => "failed",
    }
}

fn tool_error_code_label(code: ploke_tui::tools::ToolErrorCode) -> &'static str {
    use ploke_tui::tools::ToolErrorCode;

    match code {
        ToolErrorCode::FieldTooLarge => "field_too_large",
        ToolErrorCode::WrongType => "wrong_type",
        ToolErrorCode::MissingField => "missing_field",
        ToolErrorCode::MalformedDiff => "malformed_diff",
        ToolErrorCode::InvalidFormat => "invalid_format",
        ToolErrorCode::Io => "io",
        ToolErrorCode::Timeout => "timeout",
        ToolErrorCode::Internal => "internal",
    }
}

#[derive(Debug, Clone)]
struct RecordResolution {
    record_path: PathBuf,
    footer: Option<String>,
    warnings: Vec<String>,
}

pub(crate) fn has_legacy_instance_root_artifacts(instances_root: &Path, instance_id: &str) -> bool {
    let instance_root = instances_root.join(instance_id);
    [
        "record.json.gz",
        "execution-log.json",
        "repo-state.json",
        "indexing-status.json",
        "snapshot-status.json",
        "final-snapshot.db",
        "agent-turn-summary.json",
        "agent-turn-trace.json",
        "llm-full-responses.jsonl",
        "multi-swe-bench-submission.jsonl",
        "protocol-artifacts",
    ]
    .iter()
    .any(|name| instance_root.join(name).exists())
}

pub(crate) fn legacy_instance_root_warning(instance_id: &str) -> String {
    format!(
        "instance {instance_id} still has legacy top-level run artifacts under ~/.ploke-eval/instances/{instance_id}; authoritative attempt data lives under runs/run-* and is selected from registrations"
    )
}

pub(crate) fn resolve_record_path(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
) -> Result<RecordResolution, PrepareError> {
    resolve_record_path_from_eval_home(record, instance, attempt, crate::layout::ploke_eval_home()?)
}

fn resolve_record_path_from_eval_home(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
    eval_home: PathBuf,
) -> Result<RecordResolution, PrepareError> {
    let instances_root = crate::layout::instances_dir()?;
    let selection = load_active_selection_at(&eval_home, OperatorProjectionRead::cli_operator())?;
    match (record, instance) {
        (Some(path), None) => Ok(RecordResolution {
            record_path: path,
            footer: None,
            warnings: Vec::new(),
        }),
        (None, explicit_instance) => {
            let resolved_instance = explicit_instance.or_else(|| selection.instance.clone());
            let (resolved_attempt, attempt_warning) =
                resolve_attempt_override(&selection, resolved_instance.as_deref(), attempt);
            if let Some(instance_id) = resolved_instance {
                let mut warnings = render_selection_warnings(&selection_with_resolution(
                    &selection,
                    Some(&instance_id),
                    resolved_attempt,
                ));
                if let Some(warning) = attempt_warning {
                    warnings.push(warning);
                }
                return resolve_instance_record_path(
                    &instances_root,
                    &instance_id,
                    resolved_attempt,
                    warnings,
                );
            }
            if resolved_attempt.is_some() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "an active attempt selection requires an active or explicit instance"
                        .to_string(),
                });
            }
            let last_run = crate::run_history::load_last_run_at(&eval_home)?;
            let record_path = last_run.run_dir.join("record.json.gz");
            let footer =
                match crate::run_registry::load_registration_for_run_dir(&last_run.run_dir)? {
                    Some(registration) => {
                        let attempt = attempt_number_for_registration(
                            &instances_root,
                            &registration.frozen_spec.task_id,
                            &registration.run_id,
                        )?;
                        Some(format!(
                            "resolved run: {} attempt {} (latest)",
                            registration.frozen_spec.task_id, attempt
                        ))
                    }
                    None => Some("resolved run: most recent completed run".to_string()),
                };
            Ok(RecordResolution {
                record_path,
                footer,
                warnings: render_selection_warnings(&selection),
            })
        }
        (Some(_), Some(_)) => Err(PrepareError::MissingRunManifest(
            instances_root.join("<instance>/runs/run-*/record.json.gz"),
        )),
    }
}

fn resolve_attempt_override(
    selection: &ActiveSelection,
    resolved_instance: Option<&str>,
    explicit_attempt: Option<u32>,
) -> (Option<u32>, Option<String>) {
    if explicit_attempt.is_some() {
        return (explicit_attempt, None);
    }
    let Some(selected_attempt) = selection.attempt else {
        return (None, None);
    };
    let Some(selected_instance) = selection.instance.as_deref() else {
        return (
            None,
            Some("selected attempt was ignored because no selected instance is active".to_string()),
        );
    };
    match resolved_instance {
        Some(instance) if instance == selected_instance => (Some(selected_attempt), None),
        Some(instance) => (
            None,
            Some(format!(
                "selected attempt {} for instance {} was ignored because instance {} was requested",
                selected_attempt, selected_instance, instance
            )),
        ),
        None => (None, None),
    }
}

pub(crate) fn selection_with_resolution(
    selection: &ActiveSelection,
    instance: Option<&str>,
    attempt: Option<u32>,
) -> ActiveSelection {
    let mut resolved = selection.clone();
    if let Some(instance) = instance {
        resolved.instance = Some(instance.to_string());
        resolved.attempt = attempt;
    }
    resolved
}

fn resolve_instance_record_path(
    instances_root: &Path,
    instance_id: &str,
    attempt: Option<u32>,
    mut warnings: Vec<String>,
) -> Result<RecordResolution, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    if !registrations.is_empty() {
        if has_legacy_instance_root_artifacts(instances_root, instance_id) {
            warnings.push(legacy_instance_root_warning(instance_id));
        }
        let selected_index = match attempt {
            Some(number) if number > 0 => {
                let index = (number - 1) as usize;
                if index >= registrations.len() {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "resolve_record_path",
                        detail: format!(
                            "instance {instance_id} has {} attempt(s); attempt {} is out of range",
                            registrations.len(),
                            number
                        ),
                    });
                }
                index
            }
            Some(_) => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "attempt numbers are 1-based".to_string(),
                });
            }
            None => registrations.len() - 1,
        };
        let selected = &registrations[selected_index];
        return Ok(RecordResolution {
            record_path: selected.artifacts.record_path.clone(),
            footer: attempt.is_none().then(|| {
                format!(
                    "resolved run: {} attempt {} (latest)",
                    instance_id,
                    selected_index + 1
                )
            }),
            warnings,
        });
    }

    if attempt.is_some() {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!(
                "instance {instance_id} has no registered attempts; cannot resolve a numbered attempt"
            ),
        });
    }

    Err(PrepareError::DatabaseSetup {
        phase: "resolve_record_path",
        detail: format!(
            "instance {instance_id} has no registered attempts; legacy instance-root artifacts are no longer used"
        ),
    })
}

pub(crate) fn list_attempt_registrations(
    instances_root: &Path,
    instance_id: &str,
) -> Result<Vec<RunRegistration>, PrepareError> {
    let mut registrations = list_registrations_for_instance(instances_root, instance_id)?;
    registrations.sort_by(|left, right| {
        run_registration_sort_key(left).cmp(&run_registration_sort_key(right))
    });
    Ok(registrations)
}

fn attempt_number_for_registration(
    instances_root: &Path,
    instance_id: &str,
    run_id: &str,
) -> Result<usize, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    registrations
        .iter()
        .position(|registration| registration.run_id == run_id)
        .map(|index| index + 1)
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!("run {run_id} is not registered under instance {instance_id}"),
        })
}

fn run_registration_sort_key(registration: &RunRegistration) -> (String, String) {
    (
        registration
            .lifecycle
            .finished_at
            .clone()
            .unwrap_or_else(|| registration.lifecycle.updated_at.clone()),
        registration.run_id.clone(),
    )
}

pub(crate) fn print_record_resolution_footer(resolution: &RecordResolution) {
    let _ = std::io::stdout().flush();
    for warning in &resolution.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(footer) = &resolution.footer {
        eprintln!("{footer}");
    }
}

pub(crate) fn print_selection_update(selection: &ActiveSelection) {
    println!(
        "campaign: {}",
        selection.campaign.as_deref().unwrap_or("(none)")
    );
    println!("batch: {}", selection.batch.as_deref().unwrap_or("(none)"));
    println!(
        "instance: {}",
        selection.instance.as_deref().unwrap_or("(none)")
    );
    println!(
        "attempt: {}",
        selection
            .attempt
            .map(|attempt| attempt.to_string())
            .unwrap_or_else(|| "(latest)".to_string())
    );
    for warning in render_selection_warnings(selection) {
        println!("warning: {warning}");
    }
}

pub(crate) fn sanitize_batch_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '-' };
        if normalized == '-' {
            if last_was_dash {
                continue;
            }
            last_was_dash = true;
            out.push('-');
        } else {
            last_was_dash = false;
            out.push(normalized.to_ascii_lowercase());
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "batch".to_string()
    } else {
        trimmed.to_string()
    }
}

fn run_doctor() -> Result<(), PrepareError> {
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut note = 0usize;

    println!("ploke-eval doctor");
    println!();

    let home = crate::layout::ploke_eval_home()?;
    println!("home: {}", home.display());

    let builtins = builtin_dataset_registry_entries();
    println!(
        "built-in datasets: {}{}",
        builtins.len(),
        if builtins.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                builtins
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    );

    check_dir(
        "datasets dir",
        datasets_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "models dir",
        models_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "repo cache dir",
        repos_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "run artifacts dir",
        instances_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "batch artifacts dir",
        batches_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "starting-db cache dir",
        starting_db_cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "cache dir",
        cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );

    match load_model_registry() {
        Ok(registry) => {
            ok += 1;
            println!(
                "[ok] model registry: {} models ({})",
                registry.data.len(),
                model_registry_file()?.display()
            );
        }
        Err(PrepareError::MissingModelRegistry(path)) => {
            warn += 1;
            println!("[warn] model registry: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model list",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_active_model() {
        Ok(active) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &active.model_id) {
                    ok += 1;
                    println!(
                        "[ok] active model: {} ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] active model: {} is not present in the current registry ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] active model: {} ({})",
                    active.model_id,
                    active_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingActiveModel(path)) => {
            warn += 1;
            println!("[warn] active model: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_parent_patcher_model() {
        Ok(selected) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &selected.model_id) {
                    ok += 1;
                    println!(
                        "[ok] parent patcher model: {} ({})",
                        selected.model_id,
                        parent_patcher_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] parent patcher model: {} is not present in the current registry ({})",
                        selected.model_id,
                        parent_patcher_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model parent-patcher set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] parent patcher model: {} ({})",
                    selected.model_id,
                    parent_patcher_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingParentPatcherModel(path)) => {
            warn += 1;
            println!(
                "[warn] parent patcher model: missing ({}); broad harness will fall back to the active model",
                path.display()
            );
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model parent-patcher set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match OpenRouter::resolve_api_key() {
        Ok(_) => {
            ok += 1;
            println!("[ok] OpenRouter API key: present");
        }
        Err(err) => {
            warn += 1;
            println!("[warn] OpenRouter API key: unavailable ({err})");
        }
    }

    match Google::route_config_available() {
        Ok(_) => {
            ok += 1;
            println!("[ok] Google route config: GOOGLE_PROJECT_ID and GOOGLE_REGION available");
        }
        Err(err) => {
            note += 1;
            println!("[note] Google route config: unavailable ({err})");
        }
    }

    match Google::auth_config_available() {
        Ok(_) => {
            ok += 1;
            println!("[ok] Google auth: ADC or explicit bearer token available");
        }
        Err(err) => {
            note += 1;
            println!("[note] Google auth: unavailable ({err})");
        }
    }

    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            ok += 1;
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("[ok] git: {version}");
        }
        Ok(output) => {
            warn += 1;
            println!(
                "[warn] git: command exited with status {}",
                output.status.code().unwrap_or(-1)
            );
        }
        Err(err) => {
            warn += 1;
            println!("[warn] git: unavailable ({err})");
        }
    }

    println!();
    println!(
        "summary: {ok} ok, {warn} warning{}, {note} note{}",
        if warn == 1 { "" } else { "s" },
        if note == 1 { "" } else { "s" }
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum MissingDirStatus {
    Warn,
    Note,
}

fn check_dir(
    label: &str,
    path: PathBuf,
    ok: &mut usize,
    warn: &mut usize,
    note: &mut usize,
    missing_status: MissingDirStatus,
) {
    if path.exists() {
        if path.is_dir() {
            *ok += 1;
            println!("[ok] {label}: {}", path.display());
        } else {
            *warn += 1;
            println!(
                "[warn] {label}: exists but is not a directory ({})",
                path.display()
            );
        }
    } else {
        match missing_status {
            MissingDirStatus::Warn => {
                *warn += 1;
                println!("[warn] {label}: missing ({})", path.display());
            }
            MissingDirStatus::Note => {
                *note += 1;
                println!("[note] {label}: missing ({})", path.display());
            }
        }
    }
}

fn print_advice(lines: &[&str]) {
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            println!("  next: {line}");
        } else {
            println!("  then: {line}");
        }
    }
}
