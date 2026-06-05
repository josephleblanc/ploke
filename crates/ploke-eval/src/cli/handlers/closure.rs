use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;

use ploke_llm::request::models::ModelRouteSource;
use ploke_llm::{ModelId, ProviderKey};
use ploke_protocol::Procedure;
use ploke_protocol::tool_calls::{review, segment, trace};
use ploke_protocol::{JsonAdjudicator, JsonLlmConfig, ProtocolReasoningPolicy};
use ploke_records::protocol::InterventionIssueDetectionArtifact;
use serde::Serialize;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::campaign::{
    EvalCampaignPolicy, ProtocolCampaignPolicy, ResolvedCampaignConfig, dataset_files_from_sources,
    dataset_keys_from_sources, resolve_campaign_config,
};
use crate::cli::provider::parse_provider_key;
use crate::cli::{
    ClosureAdvanceAllCommand, ClosureAdvanceCommand, ClosureAdvanceEvalCommand,
    ClosureAdvanceProtocolCommand, ClosureAdvanceSubcommand, ClosureCommand,
    ClosureRecomputeCommand, ClosureStatusCommand, ClosureSubcommand, InspectOutputFormat,
    PROTOCOL_HTTP_MAX_ATTEMPTS, PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS, TOOL_CALL_REVIEW_TIMEOUT_SECS,
    build_segment_review_subject, build_tool_call_review_subject, build_tool_call_sequence_subject,
    is_retryable_intent_segmentation_error, join_indices, resolve_protocol_model_id,
    resolve_protocol_route, sanitize_batch_component,
    tool_call_intent_segmentation_error_to_prepare, tool_call_review_error_to_prepare,
    tool_call_segment_review_error_to_prepare,
};
use crate::closure::{
    ClosureClass, ClosureRecomputeRequest, closure_state_path, load_closure_state,
    recompute_closure_state, render_closure_status,
};
use crate::inner::registry::RunRegistration;
use crate::layout::{batches_dir, repos_dir};
use crate::msb::PrepareMsbBatchRequest;
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolAggregateError, load_protocol_aggregate,
};
use crate::protocol_artifacts::{
    StoredProtocolArtifactFile, list_protocol_artifact_load_results, list_protocol_artifacts,
    load_protocol_artifact, write_protocol_artifact,
};
use crate::record::read_compressed_record;
use crate::run_history::RunDirPreference;
use crate::runner::{BatchRunArtifactPaths, BatchRunSummary, RunMsbAgentBatchRequest};
use crate::spec::{OutputMode, PrepareError, PrepareWrite, PreparedCampaignContext};
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry,
};

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
pub(crate) struct ProtocolBatchExecution {
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
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EvalBatchPlan {
    batch_id: String,
    dataset_label: String,
    dataset_path: PathBuf,
    instances: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClosureAdvanceEvalReport {
    pub(crate) campaign_id: String,
    pub(crate) dry_run: bool,
    pub(crate) before: crate::closure::EvalClosureSummary,
    pub(crate) after: crate::closure::EvalClosureSummary,
    pub(crate) selected_instances: Vec<String>,
    pub(crate) selected_batches: Vec<EvalBatchPlan>,
    pub(crate) executed_batches: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProtocolRunPlan {
    pub(crate) instance_id: String,
    pub(crate) segmentation_needed: bool,
    pub(crate) missing_call_indices: Vec<usize>,
    pub(crate) missing_segment_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProtocolRunState {
    pub(crate) instance_id: String,
    pub(crate) tool_calls_total: usize,
    pub(crate) protocol_eligible: bool,
    pub(crate) artifact_count: usize,
    pub(crate) segmentation_present: bool,
    pub(crate) call_review_count: usize,
    pub(crate) segment_review_count: usize,
    pub(crate) aggregate_available: bool,
    pub(crate) missing_call_indices: Vec<usize>,
    pub(crate) missing_segment_indices: Vec<usize>,
    pub(crate) next_step: ProtocolNextStep,
    pub(crate) next_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) aggregate_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProtocolNextStep {
    Ineligible,
    IntentSegmentation,
    ToolCallReview { index: usize },
    ToolCallSegmentReview { segment_index: usize },
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProtocolRunTask {
    instance_id: String,
    record_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProtocolRunExecution {
    instance_id: String,
    plan: ProtocolRunPlan,
    segmentations_created: usize,
    call_reviews_created: usize,
    segment_reviews_created: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClosureAdvanceProtocolReport {
    pub(crate) campaign_id: String,
    pub(crate) dry_run: bool,
    pub(crate) before: crate::closure::ProtocolClosureSummary,
    pub(crate) after: crate::closure::ProtocolClosureSummary,
    pub(crate) selected_runs: Vec<ProtocolRunPlan>,
    pub(crate) executed_runs: usize,
    pub(crate) segmentations_created: usize,
    pub(crate) call_reviews_created: usize,
    pub(crate) segment_reviews_created: usize,
    pub(crate) failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClosureAdvanceAllReport {
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

pub(crate) fn ensure_repo_cache_clone_preflight(
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

pub(crate) fn ensure_prepared_runs_under_repo_cache(
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

pub(crate) fn protocol_report_allows_continue(report: &ClosureAdvanceProtocolReport) -> bool {
    protocol_report_made_progress(report) || report.after.status == ClosureClass::Complete
}

pub(crate) fn protocol_report_made_progress(report: &ClosureAdvanceProtocolReport) -> bool {
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

pub(crate) fn format_protocol_selected_runs(runs: &[ProtocolRunPlan]) -> String {
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

pub(crate) fn format_remaining_protocol_work(
    summary: &crate::closure::ProtocolClosureSummary,
) -> String {
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

pub(crate) fn protocol_run_plan(
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

pub(crate) fn protocol_state_for_run(
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

pub(crate) fn protocol_next_command(
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

pub(crate) fn print_protocol_state_table(state: &ProtocolRunState) {
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

pub(crate) async fn execute_protocol_intent_segments_quiet(
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

pub(crate) async fn execute_protocol_tool_call_review_quiet(
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

pub(crate) async fn run_tool_call_review_with_json_retries(
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

pub(crate) async fn run_tool_call_segment_review_with_json_retries(
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

pub(crate) fn is_retryable_local_analysis_review_error(err: &review::ToolCallReviewError) -> bool {
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

pub(crate) async fn execute_protocol_tool_call_segment_review_quiet(
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
