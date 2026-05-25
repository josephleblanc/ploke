use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::{
    ClosureClass, ResolvedCampaignConfig,
    campaign::resolve_campaign_config,
    campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1CandidateGenerator, Prototype1ControlCommand,
        Prototype1DoctorCommand, Prototype1PromptCommand,
    },
    closure::load_closure_state,
    intervention::{
        CompleteBaseline, Intervention, Prototype1ChildScheduleMode, Prototype1NodeRecord,
        Prototype1NodeStatus, Prototype1RunnerRequest, Prototype1RunnerResult, RecordStore,
        load_node_record, load_runner_request, load_runner_result,
    },
    projection::OperatorProjectionRead,
    run_registry::{RunExecutionStatus, list_registrations_for_instance},
    spec::PrepareError,
};

use crate::cli::prototype1_process::{
    SuccessorHandoffMode, persist_prototype1_buildable_child_artifact,
    spawn_and_handoff_prototype1_successor,
};
use crate::cli::prototype1_state::backend::GitWorktreeBackend;
use crate::cli::prototype1_state::{
    c1::{
        Acknowledged, Artifact, Binary, C2, Child, Parent as ParentLineage, Present, Prototype,
        Unacknowledged,
    },
    c2::{BuildChild, C3},
    c3::{C4, SpawnChild},
    c4::{ObserveChild, ObservedChild},
    cli_facing::{
        PlannedChildOutcome, Prototype1BranchEvaluationReport, compare_observed_child_treatment,
        ensure_prototype1_baseline_closure_state, establish_parent_baseline,
        live_successor_continuation_decision, prototype1_branch_evaluation_path,
        reserve_profile_child_budget, resolve_profile_child_plan, run_planned_child,
        select_artifact_for_handoff, select_successor_for_profile,
        selection_input_from_child_report,
    },
    edit_surface::harness_request::{
        BroadHarnessRequest, EvidenceRootKind, EvidenceRootLocation, HarnessChildBudget,
        ProtectedCoreAnchor, PublishedBroadHarnessRequest,
    },
    event::{ContentHash, RuntimeId, TransitionId},
    history::{ArtifactSurface, surface_attempt},
    identity::{ParentIdentity, load_parent_identity_optional, parent_identity_relpath},
    inner::At,
    journal::{
        CompletionEntry, JournalEntry, PrototypeJournal, SpawnEntry, SpawnObservation, SpawnPhase,
        prototype1_transition_journal_path,
    },
    parent::{
        Check, ChildFiles, ChildPlanFile, ChildPlanFiles, Genesis, Parent, Predecessor, Ready,
        Startup, Unchecked,
    },
    profile::{self, AdmittedRunProfile, Prototype1RunProfile, RunProfileCommitment},
    successor,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EffectiveRunControl {
    pub(crate) path: PathBuf,
    pub(crate) mode: profile::RunMode,
    pub(crate) parallel_cap: u32,
    pub(crate) defaulted_from_profile: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosedPhase {
    BaselineEval,
    BaselineProtocol,
    ChildPlan,
    Materialize,
    Build,
    Spawn,
    Observe,
    Select,
    Handoff,
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CurrentChildStatus {
    pub(crate) plan_index: usize,
    pub(crate) node_id: String,
    pub(crate) branch_id: String,
    pub(crate) status: Prototype1NodeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ActiveParentStatus {
    pub(crate) campaign_id: String,
    pub(crate) repo_root: PathBuf,
    pub(crate) parent_identity: ParentIdentity,
    pub(crate) run_profile: RunProfileCommitment,
    pub(crate) effective_control: EffectiveRunControl,
    pub(crate) prompt_preflight: PromptPreflight,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) protocol_preflight: Option<ProtocolLivePreflight>,
    pub(crate) phase: DiagnosedPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) current_child: Option<CurrentChildStatus>,
    pub(crate) blockers: Vec<String>,
    pub(crate) allowed_actions: Vec<String>,
    pub(crate) suggested_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct RuntimeContext {
    repo_root: PathBuf,
    campaign_id: String,
    manifest_path: PathBuf,
    resolved_campaign: ResolvedCampaignConfig,
    parent_identity: ParentIdentity,
    admitted_profile: AdmittedRunProfile,
    effective_control: EffectiveRunControl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PromptPreflight {
    pub(crate) outcome: PromptPreflightOutcome,
    pub(crate) checked: Vec<PromptReference>,
    pub(crate) prompt_files: Vec<PathBuf>,
    pub(crate) problems: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PromptPreflightOutcome {
    Skipped,
    Pending,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProtocolLivePreflight {
    pub(crate) outcome: ProtocolLivePreflightOutcome,
    pub(crate) model_id: String,
    pub(crate) provider: String,
    pub(crate) route_source: String,
    pub(crate) reasoning: String,
    pub(crate) max_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtocolLivePreflightOutcome {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PromptReference {
    pub(crate) label: String,
    pub(crate) path: PathBuf,
    pub(crate) kind: PromptReferenceKind,
    pub(crate) present: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PromptReferenceKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
struct ChildSnapshot {
    plan_index: usize,
    plan_child: ChildFiles,
    node: Prototype1NodeRecord,
    request: Prototype1RunnerRequest,
    evaluation_report: Option<Prototype1BranchEvaluationReport>,
    runtime_id: Option<RuntimeId>,
    artifact_surface: Option<ArtifactSurface>,
}

#[derive(Debug, Clone)]
struct SuccessorMarker {
    state: SuccessorMarkerState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuccessorMarkerState {
    Selected,
    Terminal,
}

#[derive(Debug, Clone)]
struct Diagnosis {
    context: RuntimeContext,
    phase: DiagnosedPhase,
    current_child: Option<CurrentChildStatus>,
    blockers: Vec<String>,
    notes: Vec<String>,
    prompt_preflight: PromptPreflight,
    child_plan: Option<ChildPlanFiles>,
    child_snapshots: Vec<ChildSnapshot>,
}

#[derive(Debug, Clone, Copy)]
enum ExecuteMode {
    Step,
    Continuous,
}

pub(crate) async fn doctor(command: Prototype1DoctorCommand) -> Result<(), PrepareError> {
    let mut status = diagnose_command(&command.control)?;
    if command.live_protocol_preflight {
        let context = resolve_context(command.control.repo_root.as_deref())?;
        attach_protocol_live_preflight(&context, &mut status).await;
    }
    render_status(command.control.format, &status)
}

pub(crate) async fn prompt(command: Prototype1PromptCommand) -> Result<(), PrepareError> {
    let context = resolve_context(command.repo_root.as_deref())?;
    let prompt = prompt_text(&context)?;
    print!("{prompt}");
    Ok(())
}

pub(crate) async fn resume(command: Prototype1ControlCommand) -> Result<(), PrepareError> {
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 256 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "prototype1-continue exceeded 256 phase advances without reaching a terminal state".to_string(),
            });
        }
        let diagnosis = diagnose(&resolve_context(command.repo_root.as_deref())?)?;
        if diagnosis.phase == DiagnosedPhase::Blocked {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "prototype1-continue refused: {}",
                    diagnosis.blockers.join("; ")
                ),
            });
        }
        if diagnosis.phase == DiagnosedPhase::Complete {
            let status = into_status(diagnosis);
            return render_status(command.format, &status);
        }
        advance(diagnosis, ExecuteMode::Continuous).await?;
    }
}

pub(crate) async fn step(command: Prototype1ControlCommand) -> Result<(), PrepareError> {
    let diagnosis = diagnose(&resolve_context(command.repo_root.as_deref())?)?;
    if diagnosis.phase == DiagnosedPhase::Blocked {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("prototype1-step refused: {}", diagnosis.blockers.join("; ")),
        });
    }
    if diagnosis.phase != DiagnosedPhase::Complete {
        advance(diagnosis, ExecuteMode::Step).await?;
    }
    let status = diagnose_command(&command)?;
    render_status(command.format, &status)
}

fn diagnose_command(
    command: &Prototype1ControlCommand,
) -> Result<ActiveParentStatus, PrepareError> {
    diagnose(&resolve_context(command.repo_root.as_deref())?).map(into_status)
}

fn render_status(
    format: InspectOutputFormat,
    status: &ActiveParentStatus,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(status).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!("prototype1 doctor");
            println!("{}", "-".repeat(40));
            println!("campaign_id: {}", status.campaign_id);
            println!("repo_root: {}", status.repo_root.display());
            println!("parent_node_id: {}", status.parent_identity.node_id());
            println!("generation: {}", status.parent_identity.generation());
            println!("phase: {}", phase_label(status.phase));
            println!(
                "parallel_cap: {}{}",
                status.effective_control.parallel_cap,
                if status.effective_control.defaulted_from_profile {
                    " (derived)"
                } else {
                    ""
                }
            );
            println!(
                "prompt_preflight: {} (checked={} prompt_files={})",
                prompt_preflight_label(status.prompt_preflight.outcome),
                status.prompt_preflight.checked.len(),
                status.prompt_preflight.prompt_files.len()
            );
            if let Some(preflight) = status.protocol_preflight.as_ref() {
                println!(
                    "protocol_live_preflight: {} model={} provider={} route={} reasoning={} max_tokens={}",
                    protocol_live_preflight_label(preflight.outcome),
                    preflight.model_id,
                    preflight.provider,
                    preflight.route_source,
                    preflight.reasoning,
                    preflight.max_tokens
                );
                if let Some(detail) = preflight.detail.as_deref() {
                    println!("  detail: {detail}");
                }
            }
            for reference in &status.prompt_preflight.checked {
                println!(
                    "  - {} {} {}",
                    if reference.present { "ok" } else { "missing" },
                    prompt_reference_kind_label(reference.kind),
                    reference.path.display()
                );
            }
            if let Some(child) = status.current_child.as_ref() {
                println!(
                    "current_child: plan_index={} node_id={} branch_id={} status={}",
                    child.plan_index,
                    child.node_id,
                    child.branch_id,
                    serde_json::to_string(&child.status)
                        .unwrap_or_else(|_| format!("{:?}", child.status))
                );
            }
            if !status.blockers.is_empty() {
                println!("blockers:");
                for blocker in &status.blockers {
                    println!("  - {}", blocker);
                }
            }
            if !status.notes.is_empty() {
                println!("notes:");
                for note in &status.notes {
                    println!("  - {}", note);
                }
            }
            println!("allowed_actions: {}", status.allowed_actions.join(", "));
            println!("commands:");
            for command in &status.suggested_commands {
                println!("  {}", command);
            }
        }
    }
    Ok(())
}

fn resolve_context(repo_root: Option<&Path>) -> Result<RuntimeContext, PrepareError> {
    let repo_root = repo_root
        .map(Path::to_path_buf)
        .unwrap_or(
            std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
                path: PathBuf::from("."),
                source,
            })?,
        );
    let Some(parent_identity) = load_parent_identity_optional(&repo_root)? else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1 control requires parent identity at '{}'; child worktree cwd diagnosis is not supported in v1",
                repo_root.join(parent_identity_relpath()).display()
            ),
        });
    };
    let campaign_id = parent_identity.campaign_id().to_string();
    let manifest_path = campaign_manifest_path(&campaign_id)?;
    let resolved_campaign = resolve_campaign_config(&campaign_id, &Default::default())?;
    let admitted_profile =
        profile::load_admitted_run_profile(&manifest_path)?.ok_or_else(|| {
            PrepareError::InvalidBatchSelection {
                detail: format!(
                    "prototype1 control requires admitted run profile at '{}'",
                    manifest_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join("prototype1/run-profile.toml")
                        .display()
                ),
            }
        })?;
    let effective_control = load_effective_control(&admitted_profile)?;
    Ok(RuntimeContext {
        repo_root,
        campaign_id,
        manifest_path,
        resolved_campaign,
        parent_identity,
        admitted_profile,
        effective_control,
    })
}

fn load_effective_control(
    admitted: &AdmittedRunProfile,
) -> Result<EffectiveRunControl, PrepareError> {
    let path = admitted.commitment.profile_path.clone();
    let derived_parallel_cap = admitted.profile.default_parallel_cap();
    let parallel_cap = admitted
        .profile
        .control
        .parallel_cap
        .unwrap_or(derived_parallel_cap);
    if parallel_cap == 0 || parallel_cap > derived_parallel_cap {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "profile control.parallel_cap {} widens admitted fanout {} at '{}'",
                parallel_cap,
                derived_parallel_cap,
                path.display()
            ),
        });
    }
    Ok(EffectiveRunControl {
        path,
        mode: admitted.profile.control.mode,
        parallel_cap,
        defaulted_from_profile: admitted.profile.control.parallel_cap.is_none(),
    })
}

fn prompt_text(context: &RuntimeContext) -> Result<String, PrepareError> {
    if context
        .admitted_profile
        .profile
        .generation
        .candidate_generator()
        != Prototype1CandidateGenerator::BroadHarnessRequest
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "prototype1-prompt requires a broad-harness request run profile".to_string(),
        });
    }

    let parent_node_id = context.parent_identity.node_id();
    if let Some(published) = load_published_broad_requests(&context.manifest_path)?
        .into_iter()
        .filter(|published| published.request().parent_node_id.as_str() == parent_node_id)
        .last()
    {
        return fs::read_to_string(published.prompt_path()).map_err(|source| {
            PrepareError::ReadManifest {
                path: published.prompt_path().to_path_buf(),
                source,
            }
        });
    }

    Ok(broad_request_for_current_parent(context).render_prompt())
}

fn broad_request_for_current_parent(context: &RuntimeContext) -> BroadHarnessRequest {
    let prototype_root = prototype1_root(&context.manifest_path);
    let parent_node_id = context.parent_identity.node_id().to_string();
    let candidate_workspace = prototype_root
        .join("workspaces/edit-harness")
        .join(&parent_node_id);
    let submitted_result_path = prototype_root
        .join("messages/edit-harness-result")
        .join(format!("{parent_node_id}.json"));
    BroadHarnessRequest::prototype1_workspace(
        parent_node_id,
        context.repo_root.clone(),
        HarnessChildBudget {
            min_children: context.admitted_profile.profile.search.children.min,
            max_children: context.admitted_profile.profile.search.children.max,
        },
        candidate_workspace,
        &prototype_root,
        &submitted_result_path,
    )
}

fn into_status(diagnosis: Diagnosis) -> ActiveParentStatus {
    let mut notes = diagnosis.notes;
    if diagnosis.context.effective_control.defaulted_from_profile {
        notes.push(
            "profile [control].parallel_cap missing; using derived cap from [search]".to_string(),
        );
    }
    ActiveParentStatus {
        campaign_id: diagnosis.context.campaign_id,
        repo_root: diagnosis.context.repo_root.clone(),
        parent_identity: diagnosis.context.parent_identity,
        run_profile: diagnosis.context.admitted_profile.commitment,
        effective_control: diagnosis.context.effective_control,
        prompt_preflight: diagnosis.prompt_preflight,
        protocol_preflight: None,
        phase: diagnosis.phase,
        current_child: diagnosis.current_child,
        blockers: diagnosis.blockers,
        allowed_actions: allowed_actions_for_phase(diagnosis.phase),
        suggested_commands: suggested_commands(diagnosis.phase, &diagnosis.context.repo_root),
        notes,
    }
}

async fn attach_protocol_live_preflight(context: &RuntimeContext, status: &mut ActiveParentStatus) {
    let preflight = run_protocol_live_preflight(context).await;
    if preflight.outcome == ProtocolLivePreflightOutcome::Failed {
        let detail = preflight
            .detail
            .clone()
            .unwrap_or_else(|| "live protocol preflight failed".to_string());
        status
            .blockers
            .push(format!("protocol live preflight failed: {detail}"));
        status.phase = DiagnosedPhase::Blocked;
        status.allowed_actions = allowed_actions_for_phase(status.phase);
        status.suggested_commands = suggested_commands(status.phase, &status.repo_root);
    }
    status.protocol_preflight = Some(preflight);
}

#[derive(Debug, Deserialize)]
struct ProtocolLivePreflightOk {
    ok: bool,
}

const PROTOCOL_LIVE_PREFLIGHT_TEXT_CANARY_MAX_TOKENS: u32 = 64;
const PROTOCOL_LIVE_PREFLIGHT_REASONING_CANARY_MIN_TOKENS: u32 = 256;
const PROTOCOL_LIVE_PREFLIGHT_REASONING_CANARY_MAX_TOKENS: u32 = 512;

fn protocol_live_preflight_max_tokens(
    admitted_max_tokens: u32,
    reasoning: ploke_protocol::ProtocolReasoningPolicy,
) -> u32 {
    let admitted_max_tokens = admitted_max_tokens.max(1);
    if reasoning.mode == ploke_protocol::ProtocolReasoningMode::Disabled {
        return admitted_max_tokens.min(PROTOCOL_LIVE_PREFLIGHT_TEXT_CANARY_MAX_TOKENS);
    }

    if admitted_max_tokens <= PROTOCOL_LIVE_PREFLIGHT_REASONING_CANARY_MIN_TOKENS {
        admitted_max_tokens
    } else {
        admitted_max_tokens.min(PROTOCOL_LIVE_PREFLIGHT_REASONING_CANARY_MAX_TOKENS)
    }
}

async fn run_protocol_live_preflight(context: &RuntimeContext) -> ProtocolLivePreflight {
    let policy = context.admitted_profile.profile.protocol_policy();
    let max_tokens = protocol_live_preflight_max_tokens(policy.max_tokens, policy.reasoning);
    let cfg = match crate::cli::protocol_llm_config(
        Some(context.resolved_campaign.model_id.clone()),
        Some(context.resolved_campaign.route_source),
        context.resolved_campaign.provider_slug.clone(),
        30,
        1,
        max_tokens,
        policy.reasoning,
    ) {
        Ok(cfg) => cfg,
        Err(err) => {
            return ProtocolLivePreflight {
                outcome: ProtocolLivePreflightOutcome::Failed,
                model_id: context.resolved_campaign.model_id.clone(),
                provider: context
                    .resolved_campaign
                    .provider_slug
                    .clone()
                    .unwrap_or_else(|| "auto/openrouter".to_string()),
                route_source: "unresolved".to_string(),
                reasoning: policy.reasoning.display_label(),
                max_tokens,
                detail: Some(sanitize_protocol_preflight_detail(&err.to_string())),
            };
        }
    };
    let prompt = ploke_protocol::JsonChatPrompt {
        system: "Return JSON only. Do not use markdown.".to_string(),
        user: "Return exactly this JSON object: {\"ok\":true}".to_string(),
    };
    let client = reqwest::Client::new();
    let result =
        ploke_protocol::adjudicate_json::<ProtocolLivePreflightOk>(&client, &cfg, &prompt).await;

    match result {
        Ok(result) if result.parsed.ok => ProtocolLivePreflight {
            outcome: ProtocolLivePreflightOutcome::Passed,
            model_id: cfg.model_id.clone(),
            provider: cfg.provider_display().to_string(),
            route_source: protocol_route_source_label(cfg.route_source).to_string(),
            reasoning: cfg.reasoning.display_label(),
            max_tokens: cfg.max_tokens,
            detail: None,
        },
        Ok(_) => ProtocolLivePreflight {
            outcome: ProtocolLivePreflightOutcome::Failed,
            model_id: cfg.model_id.clone(),
            provider: cfg.provider_display().to_string(),
            route_source: protocol_route_source_label(cfg.route_source).to_string(),
            reasoning: cfg.reasoning.display_label(),
            max_tokens: cfg.max_tokens,
            detail: Some("live request returned parseable JSON but not the sentinel".to_string()),
        },
        Err(error) => ProtocolLivePreflight {
            outcome: ProtocolLivePreflightOutcome::Failed,
            model_id: cfg.model_id.clone(),
            provider: cfg.provider_display().to_string(),
            route_source: protocol_route_source_label(cfg.route_source).to_string(),
            reasoning: cfg.reasoning.display_label(),
            max_tokens: cfg.max_tokens,
            detail: Some(classify_protocol_preflight_error(&error)),
        },
    }
}

fn protocol_route_source_label(
    route_source: ploke_llm::request::models::ModelRouteSource,
) -> &'static str {
    if route_source.is_direct_google() {
        "direct_google"
    } else {
        "openrouter"
    }
}

fn protocol_live_preflight_label(outcome: ProtocolLivePreflightOutcome) -> &'static str {
    match outcome {
        ProtocolLivePreflightOutcome::Passed => "passed",
        ProtocolLivePreflightOutcome::Failed => "failed",
    }
}

fn classify_protocol_preflight_error(error: &ploke_protocol::ProtocolLlmError) -> String {
    match error {
        ploke_protocol::ProtocolLlmError::Request(message) => {
            let lower = message.to_ascii_lowercase();
            let class = if lower.contains("reasoning") {
                "provider_request_shape"
            } else if lower.contains("401")
                || lower.contains("403")
                || lower.contains("auth")
                || lower.contains("permission")
                || lower.contains("429")
                || lower.contains("quota")
                || lower.contains("resource_exhausted")
            {
                "provider_env"
            } else {
                "provider_request"
            };
            format!("{class}: {}", sanitize_protocol_preflight_detail(message))
        }
        ploke_protocol::ProtocolLlmError::MissingContent => {
            "provider_response: response had no visible content".to_string()
        }
        ploke_protocol::ProtocolLlmError::UnexpectedToolCalls => {
            "provider_response: response returned tool calls instead of JSON content".to_string()
        }
        ploke_protocol::ProtocolLlmError::ParseJson { detail, .. } => {
            format!(
                "provider_response: JSON parse failed: {}",
                sanitize_protocol_preflight_detail(detail)
            )
        }
        ploke_protocol::ProtocolLlmError::InvalidModelId { detail, .. }
        | ploke_protocol::ProtocolLlmError::InvalidConfig { detail } => {
            format!(
                "protocol_config: {}",
                sanitize_protocol_preflight_detail(detail)
            )
        }
    }
}

fn sanitize_protocol_preflight_detail(detail: &str) -> String {
    let mut safe = detail.to_string();
    for marker in [
        "authorization",
        "api_key",
        "apikey",
        "bearer",
        "credential",
        "credentials",
        "\"user_id\"",
    ] {
        if let Some(index) = safe.to_ascii_lowercase().find(marker) {
            safe.truncate(index);
            safe.push_str("<redacted>");
            break;
        }
    }
    const MAX: usize = 240;
    if safe.chars().count() > MAX {
        safe = safe.chars().take(MAX).collect::<String>();
        safe.push_str("...");
    }
    safe
}

fn allowed_actions_for_phase(phase: DiagnosedPhase) -> Vec<String> {
    match phase {
        DiagnosedPhase::Blocked => vec!["doctor".to_string()],
        DiagnosedPhase::Complete => vec!["doctor".to_string()],
        _ => vec![
            "doctor".to_string(),
            "continue".to_string(),
            "step".to_string(),
        ],
    }
}

fn suggested_commands(phase: DiagnosedPhase, repo_root: &Path) -> Vec<String> {
    match phase {
        DiagnosedPhase::Blocked | DiagnosedPhase::Complete => Vec::new(),
        _ => vec![
            format!(
                "cd {} && ./target/debug/ploke-eval loop prototype1-continue --repo-root .",
                repo_root.display()
            ),
            format!(
                "cd {} && ./target/debug/ploke-eval loop prototype1-step --repo-root .",
                repo_root.display()
            ),
        ],
    }
}

fn prompt_preflight_label(outcome: PromptPreflightOutcome) -> &'static str {
    match outcome {
        PromptPreflightOutcome::Skipped => "skipped",
        PromptPreflightOutcome::Pending => "pending",
        PromptPreflightOutcome::Passed => "passed",
        PromptPreflightOutcome::Failed => "failed",
    }
}

fn prompt_reference_kind_label(kind: PromptReferenceKind) -> &'static str {
    match kind {
        PromptReferenceKind::File => "file",
        PromptReferenceKind::Directory => "dir",
    }
}

fn phase_label(phase: DiagnosedPhase) -> &'static str {
    match phase {
        DiagnosedPhase::BaselineEval => "baseline_eval",
        DiagnosedPhase::BaselineProtocol => "baseline_protocol",
        DiagnosedPhase::ChildPlan => "child_plan",
        DiagnosedPhase::Materialize => "materialize",
        DiagnosedPhase::Build => "build",
        DiagnosedPhase::Spawn => "spawn",
        DiagnosedPhase::Observe => "observe",
        DiagnosedPhase::Select => "select",
        DiagnosedPhase::Handoff => "handoff",
        DiagnosedPhase::Complete => "complete",
        DiagnosedPhase::Blocked => "blocked",
    }
}

fn prompt_preflight(
    context: &RuntimeContext,
    require_future_prompt_refs: bool,
) -> Result<PromptPreflight, PrepareError> {
    if context
        .admitted_profile
        .profile
        .generation
        .candidate_generator()
        != crate::cli::Prototype1CandidateGenerator::BroadHarnessRequest
    {
        return Ok(PromptPreflight::skipped(
            "run profile does not use broad-harness prompt generation",
        ));
    }

    let prototype_root = prototype1_root(&context.manifest_path);
    let mut checked = Vec::new();
    let mut prompt_files = Vec::new();
    let mut problems = Vec::new();
    let mut notes = Vec::new();

    let template = BroadHarnessRequest::prototype1_workspace(
        context.parent_identity.node_id().to_string(),
        context.repo_root.clone(),
        HarnessChildBudget {
            min_children: context.admitted_profile.profile.search.children.min,
            max_children: context.admitted_profile.profile.search.children.max,
        },
        context.repo_root.clone(),
        &prototype_root,
        &prototype_root.join("messages/edit-harness-result/preflight.json"),
    );
    checked.extend(prompt_refs_from_request(
        &template,
        &context.repo_root,
        false,
        require_future_prompt_refs,
    ));
    if !require_future_prompt_refs {
        notes.push(
            "future evidence-root prompt references are deferred until baseline closure is complete"
                .to_string(),
        );
    }

    let live_request_ids = live_broad_request_ids_from_child_plan(context, &mut problems)?;
    for published in load_published_broad_requests(&context.manifest_path)? {
        extend_published_prompt_checks(
            &published,
            live_request_ids.as_ref(),
            &mut checked,
            &mut prompt_files,
            &mut problems,
            &mut notes,
        )?;
    }

    Ok(PromptPreflight::from_parts(
        checked,
        prompt_files,
        problems,
        notes,
    ))
}

fn live_broad_request_ids_from_child_plan(
    context: &RuntimeContext,
    problems: &mut Vec<String>,
) -> Result<Option<BTreeSet<String>>, PrepareError> {
    let mut child_plan_blockers = Vec::new();
    let Some(plan) = load_child_plan(context, &mut child_plan_blockers)? else {
        return Ok(None);
    };
    problems.extend(
        child_plan_blockers
            .into_iter()
            .map(|problem| format!("child-plan prompt preflight: {problem}")),
    );

    let mut request_ids = BTreeSet::new();
    for child in plan.children() {
        let Some(evidence) = child.harness_evidence() else {
            problems.push(format!(
                "broad-harness child plan node '{}' is missing request-bound harness evidence",
                child.node_id()
            ));
            continue;
        };
        request_ids.insert(evidence.request().request_id().to_string());
    }
    Ok(Some(request_ids))
}

fn extend_published_prompt_checks(
    published: &PublishedBroadHarnessRequest,
    live_request_ids: Option<&BTreeSet<String>>,
    checked: &mut Vec<PromptReference>,
    prompt_files: &mut Vec<PathBuf>,
    problems: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Result<(), PrepareError> {
    prompt_files.push(published.prompt_path().to_path_buf());
    checked.push(PromptReference::check(
        "published prompt file",
        published.prompt_path().to_path_buf(),
        PromptReferenceKind::File,
    ));
    match fs::read_to_string(published.prompt_path()) {
        Ok(prompt) => {
            let rendered = published.request().render_prompt();
            if prompt != rendered {
                problems.push(format!(
                    "published prompt '{}' does not match its typed request render",
                    published.prompt_path().display()
                ));
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: published.prompt_path().to_path_buf(),
                source,
            });
        }
    }

    let check_workspace_refs = match live_request_ids {
        Some(request_ids) => request_ids.contains(published.request_id()),
        None => published.workspace_path().is_dir(),
    };
    if check_workspace_refs {
        checked.extend(prompt_refs_from_request(
            published.request(),
            published.workspace_path(),
            true,
            true,
        ));
    } else if live_request_ids.is_some() {
        notes.push(format!(
            "published broad-harness prompt '{}' is not referenced by the child plan; candidate workspace checks skipped",
            published.prompt_path().display()
        ));
    } else {
        notes.push(format!(
            "published broad-harness prompt '{}' has no child plan and candidate workspace '{}' is not materialized; candidate workspace checks skipped",
            published.prompt_path().display(),
            published.workspace_path().display()
        ));
    }

    Ok(())
}

fn prototype1_root(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn load_published_broad_requests(
    campaign_manifest_path: &Path,
) -> Result<Vec<PublishedBroadHarnessRequest>, PrepareError> {
    let request_dir = prototype1_root(campaign_manifest_path).join("messages/edit-harness-request");
    let entries = match fs::read_dir(&request_dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: request_dir,
                source,
            });
        }
    };
    let mut requests = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: request_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
            path: path.clone(),
            source,
        })?;
        let published =
            serde_json::from_str::<PublishedBroadHarnessRequest>(&text).map_err(|source| {
                PrepareError::ParseManifest {
                    path: path.clone(),
                    source,
                }
            })?;
        requests.push(published);
    }
    requests.sort_by(|left, right| left.request_path().cmp(right.request_path()));
    Ok(requests)
}

fn prompt_refs_from_request(
    request: &BroadHarnessRequest,
    protected_core_root: &Path,
    include_candidate_workspace: bool,
    include_evidence_roots: bool,
) -> Vec<PromptReference> {
    let mut refs = Vec::new();
    if include_candidate_workspace {
        refs.push(PromptReference::check(
            "candidate workspace referenced by prompt",
            request.workspace.candidate_workspace_path().to_path_buf(),
            PromptReferenceKind::Directory,
        ));
    }
    if include_evidence_roots {
        refs.extend(prompt_evidence_ref(
            request,
            EvidenceRootKind::Evaluations,
            "past benchmark results referenced by prompt",
        ));
        refs.extend(prompt_evidence_ref(
            request,
            EvidenceRootKind::Nodes,
            "prior attempts and conversation history referenced by prompt",
        ));
    }
    match &request.protected_core.anchor {
        ProtectedCoreAnchor::AuthorityConstant { code_path, .. } => {
            refs.push(PromptReference::check(
                "protected core file referenced by prompt",
                protected_core_root.join(code_path),
                PromptReferenceKind::File,
            ));
        }
    }
    refs
}

fn prompt_evidence_ref(
    request: &BroadHarnessRequest,
    kind: EvidenceRootKind,
    label: &'static str,
) -> Option<PromptReference> {
    request
        .evidence_roots
        .iter()
        .find(|root| root.kind == kind)
        .and_then(|root| match &root.location {
            EvidenceRootLocation::Directory { path } => Some(PromptReference::check(
                label,
                path.clone(),
                PromptReferenceKind::Directory,
            )),
            EvidenceRootLocation::File { path } => Some(PromptReference::check(
                label,
                path.clone(),
                PromptReferenceKind::File,
            )),
            EvidenceRootLocation::NodeScopedDirectory { nodes_root, .. } => Some(
                PromptReference::check(label, nodes_root.clone(), PromptReferenceKind::Directory),
            ),
            EvidenceRootLocation::AttachedReport { .. } => None,
        })
}

fn extend_prompt_preflight_blockers(preflight: &PromptPreflight, blockers: &mut Vec<String>) {
    if preflight.outcome != PromptPreflightOutcome::Failed {
        return;
    }
    for reference in preflight
        .checked
        .iter()
        .filter(|reference| !reference.present)
    {
        blockers.push(format!(
            "prompt preflight missing {} for {}: '{}'",
            prompt_reference_kind_label(reference.kind),
            reference.label,
            reference.path.display()
        ));
    }
    for problem in &preflight.problems {
        blockers.push(format!("prompt preflight: {problem}"));
    }
}

fn extend_baseline_eval_registration_blockers(
    closure: &crate::closure::ClosureState,
    blockers: &mut Vec<String>,
) -> Result<(), PrepareError> {
    for row in &closure.instances {
        let registrations =
            list_registrations_for_instance(&closure.config.instances_root, &row.instance_id)?;

        if let Some(registration) = registrations.iter().find(|registration| {
            matches!(
                registration.lifecycle.execution_status,
                RunExecutionStatus::Registered | RunExecutionStatus::Running
            )
        }) {
            let record_state = if registration.artifacts.record_path.exists() {
                "record exists"
            } else {
                "record missing"
            };
            let turn_summary_state = registration
                .artifacts
                .turn_summary
                .as_ref()
                .map(|path| {
                    if path.exists() {
                        "turn summary exists"
                    } else {
                        "turn summary missing"
                    }
                })
                .unwrap_or("turn summary unregistered");
            blockers.push(format!(
                "baseline eval has nonterminal registered attempt '{}' for instance '{}' \
                 with execution_status={:?}, updated_at={}, {}, {}; refusing to start \
                 another baseline eval until this attempt is classified or abandoned",
                registration.run_id,
                row.instance_id,
                registration.lifecycle.execution_status,
                registration.lifecycle.updated_at,
                record_state,
                turn_summary_state
            ));
            continue;
        }

        if row.eval_status == ClosureClass::Partial {
            blockers.push(format!(
                "baseline eval for instance '{}' is partial in closure state; refusing to \
                 rerun over partial evidence without explicit classification",
                row.instance_id
            ));
        }
    }

    Ok(())
}

impl PromptPreflight {
    fn skipped(note: impl Into<String>) -> Self {
        Self {
            outcome: PromptPreflightOutcome::Skipped,
            checked: Vec::new(),
            prompt_files: Vec::new(),
            problems: Vec::new(),
            notes: vec![note.into()],
        }
    }

    fn from_parts(
        mut checked: Vec<PromptReference>,
        mut prompt_files: Vec<PathBuf>,
        problems: Vec<String>,
        notes: Vec<String>,
    ) -> Self {
        checked.sort_by(|left, right| {
            (&left.path, left.kind, &left.label).cmp(&(&right.path, right.kind, &right.label))
        });
        checked.dedup_by(|left, right| {
            left.path == right.path && left.kind == right.kind && left.label == right.label
        });
        prompt_files.sort();
        prompt_files.dedup();
        let missing = checked.iter().any(|reference| !reference.present);
        let outcome = if missing || !problems.is_empty() {
            PromptPreflightOutcome::Failed
        } else if checked.is_empty() && prompt_files.is_empty() {
            PromptPreflightOutcome::Pending
        } else {
            PromptPreflightOutcome::Passed
        };
        Self {
            outcome,
            checked,
            prompt_files,
            problems,
            notes,
        }
    }
}

impl PromptReference {
    fn check(label: impl Into<String>, path: PathBuf, kind: PromptReferenceKind) -> Self {
        let present = match kind {
            PromptReferenceKind::File => path.is_file(),
            PromptReferenceKind::Directory => path.is_dir(),
        };
        Self {
            label: label.into(),
            path,
            kind,
            present,
        }
    }
}

fn diagnose(context: &RuntimeContext) -> Result<Diagnosis, PrepareError> {
    let mut blockers = Vec::new();
    let mut notes = Vec::new();

    if context.parent_identity.generation() == 0 {
        match load_closure_state(&context.campaign_id) {
            Ok(closure) => {
                if closure.eval.status != ClosureClass::Complete {
                    let prompt_preflight = prompt_preflight(context, false)?;
                    extend_prompt_preflight_blockers(&prompt_preflight, &mut blockers);
                    extend_baseline_eval_registration_blockers(&closure, &mut blockers)?;
                    return Ok(Diagnosis {
                        context: context.clone(),
                        phase: phase_or_blocked(DiagnosedPhase::BaselineEval, &blockers),
                        current_child: None,
                        blockers,
                        notes,
                        prompt_preflight,
                        child_plan: None,
                        child_snapshots: Vec::new(),
                    });
                }
                if closure.protocol.status != ClosureClass::Complete {
                    let prompt_preflight = prompt_preflight(context, false)?;
                    extend_prompt_preflight_blockers(&prompt_preflight, &mut blockers);
                    return Ok(Diagnosis {
                        context: context.clone(),
                        phase: phase_or_blocked(DiagnosedPhase::BaselineProtocol, &blockers),
                        current_child: None,
                        blockers,
                        notes,
                        prompt_preflight,
                        child_plan: None,
                        child_snapshots: Vec::new(),
                    });
                }
            }
            Err(_) => {
                let prompt_preflight = prompt_preflight(context, false)?;
                extend_prompt_preflight_blockers(&prompt_preflight, &mut blockers);
                return Ok(Diagnosis {
                    context: context.clone(),
                    phase: phase_or_blocked(DiagnosedPhase::BaselineEval, &blockers),
                    current_child: None,
                    blockers,
                    notes,
                    prompt_preflight,
                    child_plan: None,
                    child_snapshots: Vec::new(),
                });
            }
        }
    } else {
        let report_path = prototype1_branch_evaluation_path(
            &context.manifest_path,
            context.parent_identity.branch_id(),
        );
        if !report_path.exists() {
            blockers.push(format!(
                "selected-child baseline report is missing at '{}'",
                report_path.display()
            ));
        }
    }

    let prompt_preflight = prompt_preflight(context, true)?;
    extend_prompt_preflight_blockers(&prompt_preflight, &mut blockers);
    let child_plan = load_child_plan(context, &mut blockers)?;
    let child_snapshots = if let Some(plan) = child_plan.as_ref() {
        load_child_snapshots(context, plan, &mut blockers)?
    } else {
        Vec::new()
    };
    let successor_marker = latest_successor_marker(context, &mut blockers)?;

    if !blockers.is_empty() {
        return Ok(Diagnosis {
            context: context.clone(),
            phase: DiagnosedPhase::Blocked,
            current_child: None,
            blockers,
            notes,
            prompt_preflight,
            child_plan,
            child_snapshots,
        });
    }

    if child_plan.is_none() {
        return Ok(Diagnosis {
            context: context.clone(),
            phase: DiagnosedPhase::ChildPlan,
            current_child: None,
            blockers,
            notes,
            prompt_preflight,
            child_plan,
            child_snapshots,
        });
    }

    if let Some(snapshot) = child_snapshots
        .iter()
        .find(|snapshot| !is_terminal_status(snapshot.node.status))
    {
        let phase = match snapshot.node.status {
            Prototype1NodeStatus::Planned => DiagnosedPhase::Materialize,
            Prototype1NodeStatus::WorkspaceStaged => DiagnosedPhase::Build,
            Prototype1NodeStatus::BinaryBuilt => DiagnosedPhase::Spawn,
            Prototype1NodeStatus::Running => DiagnosedPhase::Observe,
            Prototype1NodeStatus::Succeeded | Prototype1NodeStatus::Failed => unreachable!(),
        };
        return Ok(Diagnosis {
            context: context.clone(),
            phase,
            current_child: Some(CurrentChildStatus {
                plan_index: snapshot.plan_index,
                node_id: snapshot.node.node_id.clone(),
                branch_id: snapshot.node.branch_id.clone(),
                status: snapshot.node.status,
            }),
            blockers,
            notes,
            prompt_preflight,
            child_plan,
            child_snapshots,
        });
    }

    let phase = if let Some(marker) = successor_marker.as_ref() {
        terminal_phase_from_marker(marker.state)
    } else if selection_available(
        context,
        &child_snapshots,
        child_plan
            .as_ref()
            .map(|plan| plan.rejected_surface_attempts())
            .unwrap_or(&[]),
    )? {
        DiagnosedPhase::Select
    } else {
        notes.push("admitted successor selection resolved to none".to_string());
        DiagnosedPhase::Complete
    };
    Ok(Diagnosis {
        context: context.clone(),
        phase,
        current_child: None,
        blockers,
        notes,
        prompt_preflight,
        child_plan,
        child_snapshots,
    })
}

fn phase_or_blocked(phase: DiagnosedPhase, blockers: &[String]) -> DiagnosedPhase {
    if blockers.is_empty() {
        phase
    } else {
        DiagnosedPhase::Blocked
    }
}

fn terminal_phase_from_marker(marker: SuccessorMarkerState) -> DiagnosedPhase {
    match marker {
        SuccessorMarkerState::Selected => DiagnosedPhase::Handoff,
        SuccessorMarkerState::Terminal => DiagnosedPhase::Complete,
    }
}

fn selection_available(
    context: &RuntimeContext,
    child_snapshots: &[ChildSnapshot],
    rejected_surface_attempts: &[surface_attempt::Evidence],
) -> Result<bool, PrepareError> {
    let child_outcomes = reconstruct_terminal_outcomes(child_snapshots)?;
    Ok(select_successor_for_profile(
        &context.manifest_path,
        &context.parent_identity,
        &child_outcomes,
        rejected_surface_attempts,
        &context.admitted_profile.profile,
    )?
    .is_some())
}

fn load_child_plan(
    context: &RuntimeContext,
    blockers: &mut Vec<String>,
) -> Result<Option<ChildPlanFiles>, PrepareError> {
    let at = At::<ChildPlanFile>::resolve((
        context.manifest_path.clone(),
        context.parent_identity.node_id().to_string(),
    ));
    if !at.path().exists() {
        return Ok(None);
    }
    let bytes = fs::read(at.path()).map_err(|source| PrepareError::ReadManifest {
        path: at.path().to_path_buf(),
        source,
    })?;
    let plan = serde_json::from_slice::<ChildPlanFiles>(&bytes).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not decode child plan '{}': {source}",
                at.path().display()
            ),
        }
    })?;
    if plan.parent_node_id() != context.parent_identity.node_id() {
        blockers.push(format!(
            "child plan recipient '{}' does not match active parent '{}'",
            plan.parent_node_id(),
            context.parent_identity.node_id()
        ));
    }
    let expected_generation = context.parent_identity.generation() + 1;
    if plan.child_generation() != expected_generation {
        blockers.push(format!(
            "child plan generation {} does not match expected generation {}",
            plan.child_generation(),
            expected_generation
        ));
    }
    Ok(Some(plan))
}

fn load_child_snapshots(
    context: &RuntimeContext,
    plan: &ChildPlanFiles,
    blockers: &mut Vec<String>,
) -> Result<Vec<ChildSnapshot>, PrepareError> {
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&context.manifest_path));
    let entries = journal
        .load_entries()
        .map_err(|err| PrepareError::InvalidBatchSelection {
            detail: format!("failed to read transition journal: {err}"),
        })?;
    let mut snapshots = Vec::with_capacity(plan.children().len());
    for (plan_index, plan_child) in plan.children().iter().cloned().enumerate() {
        let node_id = plan_child.node_id().to_string();
        let node = load_node_record(
            &context.manifest_path,
            &node_id,
            OperatorProjectionRead::cli_operator(),
        )
        .unwrap_or_else(|_| plan_child.node_record().clone());
        let request = load_runner_request(
            &context.manifest_path,
            &node_id,
            OperatorProjectionRead::cli_operator(),
        )
        .unwrap_or_else(|_| plan_child.runner_request().clone());
        let runner_result = match load_runner_result(
            &context.manifest_path,
            &node_id,
            OperatorProjectionRead::cli_operator(),
        ) {
            Ok(result) => Some(result),
            Err(PrepareError::ReadManifest { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                None
            }
            Err(err) => return Err(err),
        };
        if is_terminal_status(node.status) && runner_result.is_none() {
            blockers.push(format!(
                "node '{}' is {:?} but runner-result.json is missing",
                node.node_id, node.status
            ));
        }
        let runtime_id = latest_runtime_for_node(&entries, &node.node_id);
        if node.status == Prototype1NodeStatus::Running && runtime_id.is_none() {
            blockers.push(format!(
                "node '{}' is running but no spawned child runtime was found in the transition journal",
                node.node_id
            ));
        }
        if let Some(runtime_id) = runtime_id {
            extend_dead_running_child_blockers(
                &entries,
                &node.node_id,
                node.status,
                &node.node_dir,
                runtime_id,
                blockers,
            );
            extend_stale_observe_blockers(
                &entries,
                &node.node_id,
                runtime_id,
                context
                    .admitted_profile
                    .profile
                    .execution
                    .observe_child_stale_after(),
                blockers,
            );
        }
        let evaluation_report = load_evaluation_report(&context.manifest_path, &node.branch_id)?;
        let artifact_surface = if matches!(
            node.status,
            Prototype1NodeStatus::BinaryBuilt
                | Prototype1NodeStatus::Running
                | Prototype1NodeStatus::Succeeded
                | Prototype1NodeStatus::Failed
        ) && node.workspace_root.exists()
        {
            GitWorktreeBackend
                .artifact_surface(&node.workspace_root)
                .ok()
        } else {
            None
        };
        snapshots.push(ChildSnapshot {
            plan_index,
            plan_child,
            node,
            request,
            evaluation_report,
            runtime_id,
            artifact_surface,
        });
    }
    Ok(snapshots)
}

fn load_evaluation_report(
    manifest_path: &Path,
    branch_id: &str,
) -> Result<Option<Prototype1BranchEvaluationReport>, PrepareError> {
    let path = prototype1_branch_evaluation_path(manifest_path, branch_id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(PrepareError::ReadManifest { path, source }),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|source| PrepareError::ParseManifest { path, source })
}

fn latest_runtime_for_node(entries: &[JournalEntry], node_id: &str) -> Option<RuntimeId> {
    entries.iter().rev().find_map(|entry| match entry {
        JournalEntry::SpawnChild(entry) if entry.refs.node_id == node_id => Some(entry.runtime_id),
        JournalEntry::ChildReady(entry) if entry.refs.node_id == node_id => Some(entry.runtime_id),
        JournalEntry::ObserveChild(entry) if entry.refs.node_id == node_id => {
            Some(entry.runtime_id)
        }
        _ => None,
    })
}

fn extend_dead_running_child_blockers(
    entries: &[JournalEntry],
    node_id: &str,
    node_status: Prototype1NodeStatus,
    node_dir: &Path,
    runtime_id: RuntimeId,
    blockers: &mut Vec<String>,
) {
    if node_status != Prototype1NodeStatus::Running {
        return;
    }
    if pending_observe_for_runtime(entries, node_id, runtime_id).is_some() {
        return;
    }

    let runner_result_path =
        crate::cli::prototype1_state::invocation::result_path(node_dir, runtime_id);
    if runner_result_path.exists() {
        return;
    }

    let Some(spawn) = latest_spawn_for_runtime(entries, runtime_id) else {
        return;
    };
    if spawn.refs.node_id != node_id
        || spawn.phase != SpawnPhase::Observed
        || !matches!(spawn.result, Some(SpawnObservation::Acknowledged))
    {
        return;
    }
    let Some(child_pid) = spawn.child_pid else {
        return;
    };
    if pid_alive(child_pid) {
        return;
    }

    let stream_state = spawn
        .streams
        .as_ref()
        .map(stream_freshness_detail)
        .unwrap_or_else(|| "stream files are unknown".to_string());
    blockers.push(format!(
        "node '{node_id}' is running for runtime '{runtime_id}', but child pid {child_pid} \
         is not visible and no runner result exists at '{}'; {stream_state}",
        runner_result_path.display()
    ));
}

fn extend_stale_observe_blockers(
    entries: &[JournalEntry],
    node_id: &str,
    runtime_id: RuntimeId,
    stale_after: Duration,
    blockers: &mut Vec<String>,
) {
    let Some(before) = pending_observe_for_runtime(entries, node_id, runtime_id) else {
        return;
    };
    let now = crate::cli::prototype1_state::event::RecordedAt::now();
    let age_ms = now.0.saturating_sub(before.recorded_at.0) as u64;
    if before.runner_result_path.exists() {
        return;
    }
    if age_ms < stale_after.as_millis() as u64 {
        return;
    }

    let spawn = latest_spawn_for_runtime(entries, runtime_id);
    let child_pid = spawn.and_then(|entry| entry.child_pid);
    let pid_state = child_pid
        .map(|pid| {
            if pid_alive(pid) {
                format!("child pid {pid} is still visible")
            } else {
                format!("child pid {pid} is not visible")
            }
        })
        .unwrap_or_else(|| "child pid is unknown".to_string());
    let stream_state = spawn
        .and_then(|entry| entry.streams.as_ref())
        .map(stream_freshness_detail)
        .unwrap_or_else(|| "stream files are unknown".to_string());

    blockers.push(format!(
        "node '{node_id}' observe_child is stale/hung for runtime '{runtime_id}': \
         observe_child:before is {age_ms}ms old, runner result is missing at '{}', \
         {pid_state}, {stream_state}",
        before.runner_result_path.display()
    ));
}

fn pending_observe_for_runtime<'a>(
    entries: &'a [JournalEntry],
    node_id: &str,
    runtime_id: RuntimeId,
) -> Option<&'a CompletionEntry> {
    let mut pending = BTreeMap::<TransitionId, &CompletionEntry>::new();
    for entry in entries {
        let JournalEntry::ObserveChild(entry) = entry else {
            continue;
        };
        if entry.refs.node_id != node_id || entry.runtime_id != runtime_id {
            continue;
        }
        match entry.phase {
            crate::intervention::CommitPhase::Before => {
                pending.insert(entry.transition_id, entry);
            }
            crate::intervention::CommitPhase::After => {
                pending.remove(&entry.transition_id);
            }
        }
    }
    pending.into_values().max_by_key(|entry| entry.recorded_at)
}

fn latest_spawn_for_runtime<'a>(
    entries: &'a [JournalEntry],
    runtime_id: RuntimeId,
) -> Option<&'a SpawnEntry> {
    entries.iter().rev().find_map(|entry| match entry {
        JournalEntry::SpawnChild(entry) if entry.runtime_id == runtime_id => Some(entry),
        _ => None,
    })
}

fn stream_freshness_detail(streams: &crate::cli::prototype1_state::journal::Streams) -> String {
    let latest = [&streams.stdout, &streams.stderr]
        .into_iter()
        .filter_map(|path| fs::metadata(path).ok()?.modified().ok())
        .max();
    let Some(latest) = latest else {
        return "stdout/stderr files have no readable modification time".to_string();
    };
    match SystemTime::now().duration_since(latest) {
        Ok(age) => format!("stdout/stderr last changed {}ms ago", age.as_millis()),
        Err(_) => "stdout/stderr modification time is in the future".to_string(),
    }
}

fn pid_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

fn latest_successor_marker(
    context: &RuntimeContext,
    blockers: &mut Vec<String>,
) -> Result<Option<SuccessorMarker>, PrepareError> {
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&context.manifest_path));
    let entries = journal
        .load_entries()
        .map_err(|err| PrepareError::InvalidBatchSelection {
            detail: format!("failed to read transition journal: {err}"),
        })?;
    for entry in entries.iter().rev() {
        match entry {
            JournalEntry::Successor(record) => {
                let node = match load_node_record(
                    &context.manifest_path,
                    &record.node_id,
                    OperatorProjectionRead::cli_operator(),
                ) {
                    Ok(node) => node,
                    Err(_) => {
                        blockers.push(format!(
                            "successor record for node '{}' cannot be matched to a node record",
                            record.node_id
                        ));
                        continue;
                    }
                };
                if node.parent_node_id.as_deref() != Some(context.parent_identity.node_id())
                    || node.generation != context.parent_identity.generation() + 1
                {
                    continue;
                }
                let state = match &record.state {
                    successor::State::Selected { .. } => SuccessorMarkerState::Selected,
                    successor::State::Stopped { .. }
                    | successor::State::Spawned { .. }
                    | successor::State::Checkout { .. }
                    | successor::State::Ready { .. }
                    | successor::State::TimedOut { .. }
                    | successor::State::ExitedBeforeReady { .. }
                    | successor::State::Completed { .. } => SuccessorMarkerState::Terminal,
                };
                return Ok(Some(SuccessorMarker { state }));
            }
            JournalEntry::SuccessorHandoff(entry) => {
                let node = match load_node_record(
                    &context.manifest_path,
                    &entry.node_id,
                    OperatorProjectionRead::cli_operator(),
                ) {
                    Ok(node) => node,
                    Err(_) => continue,
                };
                if node.parent_node_id.as_deref() == Some(context.parent_identity.node_id())
                    && node.generation == context.parent_identity.generation() + 1
                {
                    return Ok(Some(SuccessorMarker {
                        state: SuccessorMarkerState::Terminal,
                    }));
                }
            }
            _ => {}
        }
    }
    Ok(None)
}

fn is_terminal_status(status: Prototype1NodeStatus) -> bool {
    matches!(
        status,
        Prototype1NodeStatus::Succeeded | Prototype1NodeStatus::Failed
    )
}

async fn advance(diagnosis: Diagnosis, mode: ExecuteMode) -> Result<(), PrepareError> {
    match diagnosis.phase {
        DiagnosedPhase::BaselineEval => advance_baseline_eval(&diagnosis.context).await,
        DiagnosedPhase::BaselineProtocol => advance_baseline_protocol(&diagnosis.context).await,
        DiagnosedPhase::ChildPlan => advance_child_plan(&diagnosis.context).await,
        DiagnosedPhase::Materialize
        | DiagnosedPhase::Build
        | DiagnosedPhase::Spawn
        | DiagnosedPhase::Observe => advance_child_phase(diagnosis, mode).await,
        DiagnosedPhase::Select => advance_select(diagnosis),
        DiagnosedPhase::Handoff => advance_handoff(diagnosis).await,
        DiagnosedPhase::Complete | DiagnosedPhase::Blocked => Ok(()),
    }
}

async fn advance_baseline_eval(context: &RuntimeContext) -> Result<(), PrepareError> {
    crate::cli::advance_eval_closure(
        &context.resolved_campaign,
        &context.resolved_campaign.eval,
        false,
        None,
    )
    .await?;
    Ok(())
}

async fn advance_baseline_protocol(context: &RuntimeContext) -> Result<(), PrepareError> {
    let protocol_policy = context.admitted_profile.profile.protocol_policy();
    crate::cli::advance_protocol_or_block(&context.resolved_campaign, &protocol_policy).await
}

fn active_parent_ready(context: &RuntimeContext) -> Result<Parent<Ready>, PrepareError> {
    let backend = GitWorktreeBackend;
    let parent =
        Parent::<Unchecked>::load(&context.manifest_path, context.parent_identity.clone())?.check(
            &backend,
            &context.manifest_path,
            Check {
                campaign_id: &context.campaign_id,
                active_root: &context.repo_root,
            },
        )?;
    if context.parent_identity.generation() == 0 {
        parent.ready(Startup::<Genesis>::from_history(
            &context.parent_identity,
            &context.manifest_path,
        )?)
    } else {
        parent.ready(Startup::<Predecessor>::from_history(
            &context.parent_identity,
            &context.manifest_path,
            &context.repo_root,
        )?)
    }
}

async fn advance_child_plan(context: &RuntimeContext) -> Result<(), PrepareError> {
    ensure_prototype1_baseline_closure_state(&context.resolved_campaign)?;
    let parent = active_parent_ready(context)?;
    let search_policy = context.admitted_profile.profile.search_policy();
    let child_budget = reserve_profile_child_budget(
        &search_policy,
        persisted_node_count(&context.manifest_path)?,
    )?;
    let _ = resolve_profile_child_plan(
        &context.campaign_id,
        &context.manifest_path,
        &context.repo_root,
        parent,
        &context.admitted_profile.profile,
        child_budget,
    )
    .await?;
    Ok(())
}

async fn advance_child_phase(diagnosis: Diagnosis, mode: ExecuteMode) -> Result<(), PrepareError> {
    let plan =
        diagnosis
            .child_plan
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "child phase execution requires an existing child plan".to_string(),
            })?;
    let baseline = establish_parent_baseline(
        &diagnosis.context.campaign_id,
        &diagnosis.context.resolved_campaign,
        &diagnosis.context.manifest_path,
        &diagnosis.context.parent_identity,
    )
    .await?;
    let branch_log_gate = Arc::new(Mutex::new(()));
    let cap = match mode {
        ExecuteMode::Step => 1usize,
        ExecuteMode::Continuous => diagnosis.context.effective_control.parallel_cap as usize,
    }
    .max(1);
    let phase = diagnosis.phase;
    let targets = diagnosis
        .child_snapshots
        .iter()
        .filter(|snapshot| matches_phase(snapshot.node.status, phase))
        .take(cap)
        .cloned()
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Ok(());
    }
    let mut join_set = JoinSet::new();
    for target in targets {
        let context = diagnosis.context.clone();
        let baseline = baseline.clone();
        let branch_log_gate = branch_log_gate.clone();
        let plan_child_count = plan.children().len();
        let _ = plan_child_count;
        join_set.spawn_blocking(move || {
            advance_one_child(&context, baseline, branch_log_gate, target, phase)
        });
    }
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(inner) => inner?,
            Err(err) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!("child phase task failed to join: {err}"),
                });
            }
        }
    }
    Ok(())
}

fn matches_phase(status: Prototype1NodeStatus, phase: DiagnosedPhase) -> bool {
    matches!(
        (status, phase),
        (Prototype1NodeStatus::Planned, DiagnosedPhase::Materialize)
            | (Prototype1NodeStatus::WorkspaceStaged, DiagnosedPhase::Build)
            | (Prototype1NodeStatus::BinaryBuilt, DiagnosedPhase::Spawn)
            | (Prototype1NodeStatus::Running, DiagnosedPhase::Observe)
    )
}

fn advance_one_child(
    context: &RuntimeContext,
    baseline: CompleteBaseline,
    branch_log_gate: Arc<Mutex<()>>,
    snapshot: ChildSnapshot,
    phase: DiagnosedPhase,
) -> Result<(), PrepareError> {
    match phase {
        DiagnosedPhase::Materialize => {
            let _ = run_planned_child(
                context.campaign_id.clone(),
                context.manifest_path.clone(),
                context.repo_root.clone(),
                prototype1_transition_journal_path(&context.manifest_path),
                context.parent_identity.clone(),
                baseline,
                branch_log_gate,
                crate::cli::Prototype1StateStopAfter::Materialize,
                context
                    .admitted_profile
                    .profile
                    .execution
                    .observe_child_stale_after(),
                snapshot.plan_index,
                snapshot.plan_child,
            )?;
            Ok(())
        }
        DiagnosedPhase::Build => {
            let c2 = resume_c2(context, &snapshot)?;
            let mut journal =
                PrototypeJournal::new(prototype1_transition_journal_path(&context.manifest_path));
            match BuildChild::new()
                .transition(c2, &mut journal)
                .map_err(|err| PrepareError::InvalidBatchSelection {
                    detail: format!("prototype1-step build failed: {err:?}"),
                })? {
                crate::intervention::Outcome::Advanced(c3) => {
                    let _ = persist_prototype1_buildable_child_artifact(
                        &context.campaign_id,
                        &context.manifest_path,
                        &context.repo_root,
                        &context.parent_identity,
                        c3.node(),
                        c3.resolved(),
                    )?;
                    Ok(())
                }
                crate::intervention::Outcome::Rejected(_) => Ok(()),
            }
        }
        DiagnosedPhase::Spawn => {
            let c3 = resume_c3(context, &snapshot)?;
            let mut journal =
                PrototypeJournal::new(prototype1_transition_journal_path(&context.manifest_path));
            let _ = SpawnChild::new()
                .transition(c3, &mut journal)
                .map_err(|err| PrepareError::InvalidBatchSelection {
                    detail: format!("prototype1-step spawn failed: {err:?}"),
                })?;
            Ok(())
        }
        DiagnosedPhase::Observe => {
            let c4 = resume_c4(context, &snapshot)?;
            let mut journal =
                PrototypeJournal::new(prototype1_transition_journal_path(&context.manifest_path));
            match ObserveChild::new(
                context
                    .admitted_profile
                    .profile
                    .execution
                    .observe_child_stale_after(),
            )
            .transition(c4, &mut journal)
            .map_err(|err| PrepareError::InvalidBatchSelection {
                detail: format!("prototype1-step observe failed: {err:?}"),
            })? {
                crate::intervention::Outcome::Advanced(c5) => {
                    if let ObservedChild::Succeeded(successful) = &c5.observed {
                        let report = compare_observed_child_treatment(
                            &context.campaign_id,
                            &context.manifest_path,
                            &baseline,
                            c5.base.resolved(),
                            &successful.treatment,
                            &branch_log_gate,
                        )?;
                        let _ = selection_input_from_child_report(c5.base.node(), &report);
                    }
                    Ok(())
                }
                crate::intervention::Outcome::Rejected(_) => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

fn resume_c2(context: &RuntimeContext, snapshot: &ChildSnapshot) -> Result<C2, PrepareError> {
    if snapshot.node.status != Prototype1NodeStatus::WorkspaceStaged {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "expected workspace_staged for node '{}', found {:?}",
                snapshot.node.node_id, snapshot.node.status
            ),
        });
    }
    Ok(Prototype {
        campaign_id: context.campaign_id.clone(),
        campaign_manifest_path: context.manifest_path.clone(),
        node: snapshot.node.clone(),
        request: snapshot.request.clone(),
        resolved: snapshot.plan_child.resolved().clone(),
        artifact: Artifact {
            repo_root: snapshot.node.workspace_root.clone(),
            target_relpath: snapshot.plan_child.resolved().target_relpath.clone(),
            source_content_hash: ContentHash(
                snapshot.plan_child.resolved().source_content_hash.clone(),
            ),
            current_content_hash: ContentHash(
                snapshot
                    .plan_child
                    .resolved()
                    .branch
                    .proposed_content_hash
                    .clone(),
            ),
            proposed_content_hash: ContentHash(
                snapshot
                    .plan_child
                    .resolved()
                    .branch
                    .proposed_content_hash
                    .clone(),
            ),
            _lineage: PhantomData::<Child>,
        },
        binary: Binary {
            parent_running: true,
            child_path: snapshot.node.binary_path.clone(),
            child_runtime: None,
            _lineage: PhantomData::<ParentLineage>,
            _child: PhantomData,
            _ack: PhantomData,
        },
    })
}

fn resume_c3(context: &RuntimeContext, snapshot: &ChildSnapshot) -> Result<C3, PrepareError> {
    if snapshot.node.status != Prototype1NodeStatus::BinaryBuilt {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "expected binary_built for node '{}', found {:?}",
                snapshot.node.node_id, snapshot.node.status
            ),
        });
    }
    if !snapshot.node.binary_path.exists() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "node '{}' is binary_built but binary '{}' is missing",
                snapshot.node.node_id,
                snapshot.node.binary_path.display()
            ),
        });
    }
    Ok(Prototype {
        campaign_id: context.campaign_id.clone(),
        campaign_manifest_path: context.manifest_path.clone(),
        node: snapshot.node.clone(),
        request: snapshot.request.clone(),
        resolved: snapshot.plan_child.resolved().clone(),
        artifact: Artifact {
            repo_root: snapshot.node.workspace_root.clone(),
            target_relpath: snapshot.plan_child.resolved().target_relpath.clone(),
            source_content_hash: ContentHash(
                snapshot.plan_child.resolved().source_content_hash.clone(),
            ),
            current_content_hash: ContentHash(
                snapshot
                    .plan_child
                    .resolved()
                    .branch
                    .proposed_content_hash
                    .clone(),
            ),
            proposed_content_hash: ContentHash(
                snapshot
                    .plan_child
                    .resolved()
                    .branch
                    .proposed_content_hash
                    .clone(),
            ),
            _lineage: PhantomData::<Child>,
        },
        binary: Binary {
            parent_running: true,
            child_path: snapshot.node.binary_path.clone(),
            child_runtime: None,
            _lineage: PhantomData::<ParentLineage>,
            _child: PhantomData::<Present>,
            _ack: PhantomData::<Unacknowledged>,
        },
    })
}

fn resume_c4(context: &RuntimeContext, snapshot: &ChildSnapshot) -> Result<C4, PrepareError> {
    if snapshot.node.status != Prototype1NodeStatus::Running {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "expected running for node '{}', found {:?}",
                snapshot.node.node_id, snapshot.node.status
            ),
        });
    }
    let runtime_id = snapshot
        .runtime_id
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "node '{}' is running but no child runtime id was recovered",
                snapshot.node.node_id
            ),
        })?;
    Ok(Prototype {
        campaign_id: context.campaign_id.clone(),
        campaign_manifest_path: context.manifest_path.clone(),
        node: snapshot.node.clone(),
        request: snapshot.request.clone(),
        resolved: snapshot.plan_child.resolved().clone(),
        artifact: Artifact {
            repo_root: snapshot.node.workspace_root.clone(),
            target_relpath: snapshot.plan_child.resolved().target_relpath.clone(),
            source_content_hash: ContentHash(
                snapshot.plan_child.resolved().source_content_hash.clone(),
            ),
            current_content_hash: ContentHash(
                snapshot
                    .plan_child
                    .resolved()
                    .branch
                    .proposed_content_hash
                    .clone(),
            ),
            proposed_content_hash: ContentHash(
                snapshot
                    .plan_child
                    .resolved()
                    .branch
                    .proposed_content_hash
                    .clone(),
            ),
            _lineage: PhantomData::<Child>,
        },
        binary: Binary {
            parent_running: true,
            child_path: snapshot.node.binary_path.clone(),
            child_runtime: Some(runtime_id),
            _lineage: PhantomData::<ParentLineage>,
            _child: PhantomData::<Present>,
            _ack: PhantomData::<Acknowledged>,
        },
    })
}

fn reconstruct_terminal_outcomes(
    snapshots: &[ChildSnapshot],
) -> Result<Vec<PlannedChildOutcome>, PrepareError> {
    snapshots
        .iter()
        .map(|snapshot| {
            let outcome = match snapshot.node.status {
                Prototype1NodeStatus::Succeeded => {
                    let disposition = snapshot
                        .evaluation_report
                        .as_ref()
                        .map(|report| format!("{:?}", report.overall_disposition))
                        .unwrap_or_else(|| "Reject".to_string());
                    format!("completed:{disposition}")
                }
                Prototype1NodeStatus::Failed => "completed:Reject".to_string(),
                other => {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "cannot reconstruct selection input from nonterminal node '{}' in state {:?}",
                            snapshot.node.node_id, other
                        ),
                    });
                }
            };
            Ok(PlannedChildOutcome {
                plan_index: snapshot.plan_index,
                node: snapshot.node.clone(),
                node_id: snapshot.node.node_id.clone(),
                outcome,
                node_status: snapshot.node.status,
                workspace_root: snapshot.node.workspace_root.clone(),
                binary_path: snapshot.node.binary_path.clone(),
                resolved: snapshot.plan_child.resolved().clone(),
                child_runtime: snapshot.runtime_id.map(|id| id.to_string()),
                evaluation_report: snapshot.evaluation_report.clone(),
                selection_input: snapshot
                    .evaluation_report
                    .as_ref()
                    .map(|report| selection_input_from_child_report(&snapshot.node, report)),
                surface: snapshot.plan_child.surface().cloned(),
                artifact_surface: snapshot.artifact_surface.clone(),
            })
        })
        .collect::<Result<Vec<_>, PrepareError>>()
}

fn advance_select(diagnosis: Diagnosis) -> Result<(), PrepareError> {
    let child_outcomes = reconstruct_terminal_outcomes(&diagnosis.child_snapshots)?;
    let selection = select_successor_for_profile(
        &diagnosis.context.manifest_path,
        &diagnosis.context.parent_identity,
        &child_outcomes,
        diagnosis
            .child_plan
            .as_ref()
            .map(|plan| plan.rejected_surface_attempts())
            .unwrap_or(&[]),
        &diagnosis.context.admitted_profile.profile,
    )?;
    if let Some((decision, material)) = selection {
        let selected = material.selected_artifact()?;
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(
            &diagnosis.context.manifest_path,
        ));
        journal
            .append(JournalEntry::Successor(
                successor::Record::selected_with_decision(
                    diagnosis.context.campaign_id.clone(),
                    selected.node().node_id.clone(),
                    live_successor_continuation_decision(
                        &diagnosis.context.manifest_path,
                        &diagnosis.context.parent_identity,
                        &diagnosis.context.admitted_profile.profile.search_policy(),
                        &decision,
                        &material,
                        selected.node(),
                    )?,
                    decision,
                ),
            ))
            .map_err(|err| PrepareError::InvalidBatchSelection {
                detail: format!("failed to append successor selection record: {err}"),
            })?;
    }
    Ok(())
}

async fn advance_handoff(diagnosis: Diagnosis) -> Result<(), PrepareError> {
    let child_outcomes = reconstruct_terminal_outcomes(&diagnosis.child_snapshots)?;
    let Some((selection_decision, material)) = select_successor_for_profile(
        &diagnosis.context.manifest_path,
        &diagnosis.context.parent_identity,
        &child_outcomes,
        diagnosis
            .child_plan
            .as_ref()
            .map(|plan| plan.rejected_surface_attempts())
            .unwrap_or(&[]),
        &diagnosis.context.admitted_profile.profile,
    )?
    else {
        return Ok(());
    };
    let selected = material.selected_artifact()?;
    let node = selected.node().clone();
    let decision = live_successor_continuation_decision(
        &diagnosis.context.manifest_path,
        &diagnosis.context.parent_identity,
        &diagnosis.context.admitted_profile.profile.search_policy(),
        &selection_decision,
        &material,
        &node,
    )?;
    let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(
        &diagnosis.context.manifest_path,
    ));
    if decision.disposition.allows_successor() {
        let search_policy = diagnosis.context.admitted_profile.profile.search_policy();
        let child_budget = reserve_profile_child_budget(
            &search_policy,
            persisted_node_count(&diagnosis.context.manifest_path)?,
        )?;
        let parent = resolve_profile_child_plan(
            &diagnosis.context.campaign_id,
            &diagnosis.context.manifest_path,
            &diagnosis.context.repo_root,
            active_parent_ready(&diagnosis.context)?,
            &diagnosis.context.admitted_profile.profile,
            child_budget,
        )
        .await?
        .parent;
        let selected_artifact = select_artifact_for_handoff(&selection_decision, &material)?;
        let selection_entry = material.into_entry(selection_decision)?;
        let _ = spawn_and_handoff_prototype1_successor(
            &diagnosis.context.campaign_id,
            selected_artifact,
            &diagnosis.context.repo_root,
            parent,
            selection_entry,
            SuccessorHandoffMode::Detached,
        )?;
    } else {
        journal
            .append(JournalEntry::Successor(successor::Record::stopped(
                diagnosis.context.campaign_id.clone(),
                node.node_id.clone(),
                decision,
                selection_decision,
            )))
            .map_err(|err| PrepareError::InvalidBatchSelection {
                detail: format!("failed to append stopped successor record: {err}"),
            })?;
    }
    Ok(())
}

fn persisted_node_count(campaign_manifest_path: &Path) -> Result<u32, PrepareError> {
    let nodes_dir = campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join("nodes");
    let entries = match fs::read_dir(&nodes_dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: nodes_dir,
                source,
            });
        }
    };
    let mut count = 0u32;
    for entry in entries {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: nodes_dir.clone(),
            source,
        })?;
        if entry
            .file_type()
            .map_err(|source| PrepareError::ReadManifest {
                path: entry.path(),
                source,
            })?
            .is_dir()
        {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cli::prototype1_state::profile::{
        Control, Execution, Generation, ModelDefaults, Protocol, Prototype1RunProfile, RunMode,
        Search, Selection, Storage, Target,
    };

    fn profile(schedule: Prototype1ChildScheduleMode, min: u32, max: u32) -> Prototype1RunProfile {
        Prototype1RunProfile {
            schema_version: crate::cli::prototype1_state::profile::RUN_PROFILE_SCHEMA_VERSION
                .to_string(),
            name: "test-profile".to_string(),
            storage: Storage::default(),
            target: Target::default(),
            model: ModelDefaults::default(),
            search: Search {
                max_generations: 4,
                max_total_nodes: 32,
                children: crate::intervention::Prototype1ChildBudget { min, max },
                schedule,
                stop_on_first_keep: false,
                require_keep_for_continuation: false,
                explore_from_rejected: true,
            },
            generation: Generation::default(),
            selection: Selection::default(),
            protocol: Protocol::default(),
            execution: Execution::default(),
            control: Control::default(),
        }
    }

    fn admitted(profile: Prototype1RunProfile) -> AdmittedRunProfile {
        AdmittedRunProfile {
            commitment: RunProfileCommitment {
                schema_version:
                    crate::cli::prototype1_state::profile::RUN_PROFILE_COMMITMENT_SCHEMA_VERSION
                        .to_string(),
                profile_path: PathBuf::from("run-profile.toml"),
                sha256: "test-sha".to_string(),
                source_path: None,
                admitted_at: String::new(),
            },
            profile,
        }
    }

    fn write_protected_core(repo: &Path) {
        let path = repo.join("crates/ploke-eval/src/cli/prototype1_state/backend.rs");
        fs::create_dir_all(path.parent().expect("backend parent")).expect("create backend parent");
        fs::write(path, "pub const EVAL_CORE_SURFACE_ROOT: &[&str] = &[];\n")
            .expect("write backend");
    }

    fn request_admission_binding_for_test()
    -> crate::cli::prototype1_state::edit_surface::harness_request::RequestAdmissionBinding {
        let artifact_id = crate::loop_graph::ArtifactId::new("artifact:prompt-preflight-base");
        crate::cli::prototype1_state::edit_surface::harness_request::RequestAdmissionBinding::from_admission(
            &crate::cli::prototype1_state::backend::EditSurfaceAdmission::new(
                crate::loop_graph::Coordinate {
                    runtime_id: RuntimeId::new(),
                    target: crate::loop_graph::OperationTarget::Artifact {
                        artifact_id,
                    },
                },
                crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(
                    "workspace except ploke-eval",
                ),
            ),
        )
        .expect("construct request admission binding")
    }

    fn write_published_prompt_for_test(published: &PublishedBroadHarnessRequest) {
        if let Some(parent) = published.request_path().parent() {
            fs::create_dir_all(parent).expect("create request dir");
        }
        fs::write(
            published.request_path(),
            serde_json::to_string_pretty(published).expect("serialize published request"),
        )
        .expect("write request json");
        fs::write(published.prompt_path(), published.request().render_prompt())
            .expect("write prompt");
    }

    #[test]
    fn prompt_preflight_accepts_existing_prompt_references() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        write_protected_core(&repo);
        let prototype = temp.path().join("campaign/prototype1");
        fs::create_dir_all(prototype.join("evaluations")).expect("create evals");
        fs::write(prototype.join("evaluations/branch-sample.json"), "{}\n")
            .expect("write eval sample");
        fs::create_dir_all(prototype.join("nodes")).expect("create nodes");
        let request = BroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo.clone(),
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            repo.clone(),
            &prototype,
            &prototype.join("messages/edit-harness-result/node-parent.json"),
        );

        let preflight = PromptPreflight::from_parts(
            prompt_refs_from_request(&request, &repo, true, true),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(preflight.outcome, PromptPreflightOutcome::Passed);
        assert!(preflight.checked.iter().all(|reference| reference.present));
    }

    #[test]
    fn prompt_preflight_fails_missing_prompt_reference() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        write_protected_core(&repo);
        let prototype = temp.path().join("campaign/prototype1");
        fs::create_dir_all(prototype.join("evaluations")).expect("create evals");
        let request = BroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo.clone(),
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            repo.clone(),
            &prototype,
            &prototype.join("messages/edit-harness-result/node-parent.json"),
        );

        let preflight = PromptPreflight::from_parts(
            prompt_refs_from_request(&request, &repo, true, true),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(preflight.outcome, PromptPreflightOutcome::Failed);
        assert!(preflight.checked.iter().any(|reference| {
            !reference.present && reference.path.ends_with("prototype1/nodes")
        }));
    }

    #[test]
    fn prompt_preflight_skips_unused_published_slot_workspaces_after_child_plan() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        let prototype = temp.path().join("campaign/prototype1");
        fs::create_dir_all(prototype.join("nodes")).expect("create nodes");
        fs::create_dir_all(prototype.join("evaluations")).expect("create evals");
        fs::write(prototype.join("evaluations/branch-sample.json"), "{}\n")
            .expect("write eval sample");
        let request_path = prototype.join("messages/edit-harness-request/node-parent.json");
        let prompt_path = prototype.join("messages/edit-harness-request/node-parent.md");
        let result_path = prototype.join("messages/edit-harness-result/node-parent.json");
        let binding = request_admission_binding_for_test();

        let first = PublishedBroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo.clone(),
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            &prototype,
            request_path.clone(),
            prompt_path.clone(),
            result_path.clone(),
            binding.clone(),
        );
        write_published_prompt_for_test(&first);
        write_protected_core(first.workspace_path());

        let second = PublishedBroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo,
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            &prototype,
            request_path,
            prompt_path,
            result_path,
            binding,
        );
        write_published_prompt_for_test(&second);

        let mut live_request_ids = BTreeSet::new();
        live_request_ids.insert(first.request_id().to_string());
        let mut checked = Vec::new();
        let mut prompt_files = Vec::new();
        let mut problems = Vec::new();
        let mut notes = Vec::new();
        for published in [&first, &second] {
            extend_published_prompt_checks(
                published,
                Some(&live_request_ids),
                &mut checked,
                &mut prompt_files,
                &mut problems,
                &mut notes,
            )
            .expect("extend prompt checks");
        }

        let preflight = PromptPreflight::from_parts(checked, prompt_files, problems, notes);

        assert_eq!(preflight.outcome, PromptPreflightOutcome::Passed);
        assert_eq!(preflight.prompt_files.len(), 2);
        assert!(preflight.notes.iter().any(|note| {
            note.contains("not referenced by the child plan") && note.contains("node-parent-r2.md")
        }));
        assert!(
            !preflight
                .checked
                .iter()
                .any(|reference| reference.path.starts_with(second.workspace_path()))
        );
    }

    #[test]
    fn prompt_preflight_skips_unmaterialized_published_slots_before_child_plan() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        let prototype = temp.path().join("campaign/prototype1");
        fs::create_dir_all(prototype.join("nodes")).expect("create nodes");
        fs::create_dir_all(prototype.join("evaluations")).expect("create evals");
        fs::write(prototype.join("evaluations/branch-sample.json"), "{}\n")
            .expect("write eval sample");
        let request_path = prototype.join("messages/edit-harness-request/node-parent.json");
        let prompt_path = prototype.join("messages/edit-harness-request/node-parent.md");
        let result_path = prototype.join("messages/edit-harness-result/node-parent.json");
        let binding = request_admission_binding_for_test();

        let first = PublishedBroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo.clone(),
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            &prototype,
            request_path.clone(),
            prompt_path.clone(),
            result_path.clone(),
            binding.clone(),
        );
        write_published_prompt_for_test(&first);
        write_protected_core(first.workspace_path());

        let second = PublishedBroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo.clone(),
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            &prototype,
            request_path.clone(),
            prompt_path.clone(),
            result_path.clone(),
            binding.clone(),
        );
        write_published_prompt_for_test(&second);
        write_protected_core(second.workspace_path());

        let future = PublishedBroadHarnessRequest::prototype1_workspace(
            "node-parent".to_string(),
            repo,
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            &prototype,
            request_path,
            prompt_path,
            result_path,
            binding,
        );
        write_published_prompt_for_test(&future);

        let mut checked = Vec::new();
        let mut prompt_files = Vec::new();
        let mut problems = Vec::new();
        let mut notes = Vec::new();
        for published in [&first, &second, &future] {
            extend_published_prompt_checks(
                published,
                None,
                &mut checked,
                &mut prompt_files,
                &mut problems,
                &mut notes,
            )
            .expect("extend prompt checks");
        }

        let preflight = PromptPreflight::from_parts(checked, prompt_files, problems, notes);

        assert_eq!(preflight.outcome, PromptPreflightOutcome::Passed);
        assert_eq!(preflight.prompt_files.len(), 3);
        assert!(
            preflight
                .checked
                .iter()
                .any(|reference| reference.path.starts_with(second.workspace_path()))
        );
        assert!(
            !preflight
                .checked
                .iter()
                .any(|reference| reference.path.starts_with(future.workspace_path()))
        );
        assert!(
            preflight.notes.iter().any(|note| {
                note.contains("no child plan") && note.contains("node-parent-r3.md")
            })
        );
    }

    #[test]
    fn profile_default_parallel_cap_uses_full_batch_max() {
        let profile = profile(Prototype1ChildScheduleMode::FullBatch, 2, 6);
        assert_eq!(profile.default_parallel_cap(), 6);
    }

    #[test]
    fn profile_default_parallel_cap_uses_adaptive_min() {
        let profile = profile(Prototype1ChildScheduleMode::AdaptiveBatch, 2, 6);
        assert_eq!(profile.default_parallel_cap(), 2);
    }

    #[test]
    fn missing_profile_control_parallel_cap_defaults_from_search() {
        let profile = profile(Prototype1ChildScheduleMode::FullBatch, 2, 6);
        let admitted = admitted(profile);

        let effective = load_effective_control(&admitted).expect("load default control");

        assert!(effective.defaulted_from_profile);
        assert_eq!(effective.mode, RunMode::Continuous);
        assert_eq!(effective.parallel_cap, 6);
    }

    #[test]
    fn profile_control_accepts_narrowing_parallel_cap() {
        let mut profile = profile(Prototype1ChildScheduleMode::FullBatch, 2, 6);
        profile.control = Control {
            mode: RunMode::Step,
            parallel_cap: Some(1),
        };
        let admitted = admitted(profile);

        let effective = load_effective_control(&admitted).expect("load narrowed control");

        assert!(!effective.defaulted_from_profile);
        assert_eq!(effective.mode, RunMode::Step);
        assert_eq!(effective.parallel_cap, 1);
    }

    #[test]
    fn profile_control_rejects_widening_parallel_cap() {
        let mut profile = profile(Prototype1ChildScheduleMode::FullBatch, 2, 6);
        profile.control.parallel_cap = Some(9);

        let err = profile.validate().expect_err("widening rejected");
        assert!(err.to_string().contains("widens admitted fanout"));
    }

    #[test]
    fn protocol_live_preflight_budget_bounds_reasoning_canary() {
        for reasoning in [
            ploke_protocol::ProtocolReasoningPolicy::auto(),
            ploke_protocol::ProtocolReasoningPolicy::omit(),
            ploke_protocol::ProtocolReasoningPolicy::effort(ploke_llm::ReasoningEffort::Low),
        ] {
            assert_eq!(protocol_live_preflight_max_tokens(0, reasoning), 1);
            assert_eq!(protocol_live_preflight_max_tokens(128, reasoning), 128);
            assert_eq!(protocol_live_preflight_max_tokens(300, reasoning), 300);
            assert_eq!(protocol_live_preflight_max_tokens(4000, reasoning), 512);
        }
    }

    #[test]
    fn protocol_live_preflight_budget_keeps_disabled_reasoning_canary_small() {
        let reasoning = ploke_protocol::ProtocolReasoningPolicy::disabled();

        assert_eq!(protocol_live_preflight_max_tokens(0, reasoning), 1);
        assert_eq!(protocol_live_preflight_max_tokens(16, reasoning), 16);
        assert_eq!(protocol_live_preflight_max_tokens(4000, reasoning), 64);
    }

    #[test]
    fn terminal_phase_from_marker_preserves_handoff_and_complete() {
        assert_eq!(
            terminal_phase_from_marker(SuccessorMarkerState::Selected),
            DiagnosedPhase::Handoff
        );
        assert_eq!(
            terminal_phase_from_marker(SuccessorMarkerState::Terminal),
            DiagnosedPhase::Complete
        );
    }

    #[test]
    fn blocked_and_complete_phases_do_not_suggest_advance_commands() {
        let repo_root = Path::new("/tmp/prototype1");

        for phase in [DiagnosedPhase::Blocked, DiagnosedPhase::Complete] {
            assert_eq!(allowed_actions_for_phase(phase), vec!["doctor"]);
            assert!(suggested_commands(phase, repo_root).is_empty());
        }
    }

    #[test]
    fn runnable_phases_suggest_advance_commands() {
        let repo_root = Path::new("/tmp/prototype1");
        let commands = suggested_commands(DiagnosedPhase::Observe, repo_root);

        assert_eq!(
            allowed_actions_for_phase(DiagnosedPhase::Observe),
            vec!["doctor", "continue", "step"]
        );
        assert_eq!(commands.len(), 2);
        assert!(commands[0].contains("prototype1-continue"));
        assert!(commands[1].contains("prototype1-step"));
    }

    #[test]
    fn nonterminal_baseline_eval_registration_adds_doctor_blocker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let eval_home = temp.path();
        let instances_root = eval_home.join("instances/prototype1/campaign");
        let instance_id = "BurntSushi__ripgrep-2209";
        let registration =
            write_running_registration_for_test(eval_home, &instances_root, instance_id);
        let closure = closure_state_for_test(instances_root, instance_id, ClosureClass::Missing);
        let mut blockers = Vec::new();

        extend_baseline_eval_registration_blockers(&closure, &mut blockers)
            .expect("extend blockers");

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains(&registration.run_id));
        assert!(blockers[0].contains("nonterminal registered attempt"));
        assert!(blockers[0].contains("record missing"));
        assert!(blockers[0].contains("turn summary missing"));
    }

    #[test]
    fn partial_baseline_eval_closure_adds_doctor_blocker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let instances_root = temp.path().join("instances/prototype1/campaign");
        let instance_id = "BurntSushi__ripgrep-2209";
        let closure = closure_state_for_test(instances_root, instance_id, ClosureClass::Partial);
        let mut blockers = Vec::new();

        extend_baseline_eval_registration_blockers(&closure, &mut blockers)
            .expect("extend blockers");

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("partial in closure state"));
    }

    #[test]
    fn stale_observe_before_adds_doctor_blocker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let node_id = "node-stale-observe";
        let runner_result_path = temp.path().join("missing-result.json");
        let entries = vec![observe_before_entry(
            node_id,
            runtime_id,
            &runner_result_path,
            crate::cli::prototype1_state::event::RecordedAt(0),
        )];
        let mut blockers = Vec::new();

        extend_stale_observe_blockers(
            &entries,
            node_id,
            runtime_id,
            Duration::from_secs(10),
            &mut blockers,
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("observe_child is stale/hung"));
        assert!(blockers[0].contains("missing-result.json"));
        assert!(blockers[0].contains("child pid is unknown"));
    }

    #[test]
    fn dead_acknowledged_running_child_adds_doctor_blocker_before_observe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let node_id = "node-dead-child";
        let entries = vec![spawn_observed_entry(
            node_id,
            runtime_id,
            temp.path(),
            u32::MAX,
        )];
        let mut blockers = Vec::new();

        extend_dead_running_child_blockers(
            &entries,
            node_id,
            Prototype1NodeStatus::Running,
            temp.path(),
            runtime_id,
            &mut blockers,
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("node 'node-dead-child' is running"));
        assert!(blockers[0].contains("child pid 4294967295 is not visible"));
        assert!(blockers[0].contains("results/"));
    }

    fn write_running_registration_for_test(
        eval_home: &Path,
        instances_root: &Path,
        instance_id: &str,
    ) -> crate::inner::registry::RunRegistration {
        use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
        use crate::inner::registry::{RunLifecyclePhase, RunPhaseStatus, RunRegistration};
        use crate::spec::EvalBudget;

        let run_id = "run-running-baseline";
        let runs_dir = instances_root.join(instance_id).join("runs");
        let registries_dir = instances_root
            .parent()
            .expect("instances root parent")
            .join("registries");
        let intent = RunIntent {
            task_id: instance_id.to_string(),
            repo_root: eval_home.join("repos/BurntSushi/ripgrep"),
            storage_roots: RunStorageRoots::new(registries_dir, runs_dir),
            base_sha: Some("base".to_string()),
            budget: EvalBudget {
                max_turns: 40,
                max_tool_calls: 200,
                wall_clock_secs: 1800,
            },
            model_id: Some("google/gemini-3.5-flash".to_string()),
            provider_slug: Some("google".to_string()),
            campaign_id: Some("campaign".to_string()),
            batch_id: Some("batch".to_string()),
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
        };
        let mut registration =
            RunRegistration::register_with_run_id(intent, run_id).expect("registration");
        registration.artifacts.turn_summary =
            Some(registration.run_root().join("agent-turn-summary.json"));
        fs::create_dir_all(registration.run_root()).expect("create run root");
        fs::write(registration.artifacts.indexing_status.clone(), "{}")
            .expect("write indexing status");
        registration.mark_execution_started(Some("run setup started".to_string()));
        registration.update_phase(
            RunLifecyclePhase::Patching,
            RunPhaseStatus::InProgress,
            Some("agent inquiry running".to_string()),
        );
        registration.persist().expect("persist registration");
        registration
    }

    fn closure_state_for_test(
        instances_root: PathBuf,
        instance_id: &str,
        eval_status: ClosureClass,
    ) -> crate::closure::ClosureState {
        use crate::closure::{
            ClosureArtifactRefs, ClosureConfig, ClosureInstanceRow, EvalClosureSummary,
            ProtocolClosureSummary, RegistryClosureSummary, RegistryInstanceStatus,
        };
        use crate::target_registry::{BenchmarkFamily, RegistryDatasetSource};

        crate::closure::ClosureState {
            schema_version: crate::closure::CLOSURE_STATE_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign".to_string(),
            updated_at: "2026-05-25T00:00:00Z".to_string(),
            config: ClosureConfig {
                benchmark_family: BenchmarkFamily::MultiSweBenchRust,
                model_id: Some("google/gemini-3.5-flash".to_string()),
                provider_slug: Some("google".to_string()),
                route_source: None,
                registry_path: None,
                dataset_sources: vec![RegistryDatasetSource {
                    key: None,
                    path: PathBuf::from("slice.jsonl"),
                    label: "slice".to_string(),
                    url: None,
                }],
                required_procedures: Vec::new(),
                instances_root,
                batches_root: PathBuf::from("batches"),
                framework: Default::default(),
            },
            registry: RegistryClosureSummary {
                expected_total: 1,
                mapped_total: 1,
                missing_total: 0,
                ambiguous_total: 0,
                status: ClosureClass::Complete,
            },
            eval: EvalClosureSummary {
                expected_total: 1,
                complete_total: usize::from(eval_status == ClosureClass::Complete),
                failed_total: usize::from(eval_status == ClosureClass::Failed),
                missing_total: usize::from(eval_status == ClosureClass::Missing),
                partial_total: usize::from(eval_status == ClosureClass::Partial),
                in_progress_total: 0,
                status: eval_status,
                last_transition_at: None,
            },
            protocol: ProtocolClosureSummary {
                expected_total: 0,
                full_total: 0,
                partial_total: 0,
                failed_total: 0,
                missing_total: 0,
                incompatible_total: 0,
                ineligible_total: 0,
                in_progress_total: 0,
                status: ClosureClass::Missing,
                required_procedures: Vec::new(),
                status_by_procedure: Default::default(),
                last_transition_at: None,
            },
            instances: vec![ClosureInstanceRow {
                instance_id: instance_id.to_string(),
                dataset_label: "slice".to_string(),
                repo_family: "BurntSushi__ripgrep".to_string(),
                registry_status: RegistryInstanceStatus::Mapped,
                eval_status,
                protocol_status: ClosureClass::Ineligible,
                eval_failure: None,
                protocol_failure: None,
                artifacts: ClosureArtifactRefs::default(),
                protocol_procedures: Default::default(),
                protocol_counts: None,
                last_event_at: None,
            }],
        }
    }

    fn observe_before_entry(
        node_id: &str,
        runtime_id: RuntimeId,
        runner_result_path: &Path,
        recorded_at: crate::cli::prototype1_state::event::RecordedAt,
    ) -> JournalEntry {
        use crate::cli::prototype1_state::event::{
            ChildRuntimeLifecycle, LineageMark, Paths, Refs, World,
        };

        JournalEntry::ObserveChild(CompletionEntry {
            transition_id: TransitionId::new(),
            runtime_id,
            phase: crate::intervention::CommitPhase::Before,
            recorded_at,
            generation: 1,
            refs: Refs {
                campaign_id: "campaign".to_string(),
                node_id: node_id.to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state".to_string(),
                branch_id: "branch".to_string(),
                candidate_id: "candidate".to_string(),
                branch_label: "branch".to_string(),
                spec_id: "spec".to_string(),
            },
            paths: Paths {
                repo_root: runner_result_path.parent().unwrap().to_path_buf(),
                workspace_root: runner_result_path.parent().unwrap().to_path_buf(),
                binary_path: runner_result_path.parent().unwrap().join("ploke-eval"),
                target_relpath: PathBuf::from("src/lib.rs"),
                absolute_path: runner_result_path.parent().unwrap().join("src/lib.rs"),
            },
            world: World {
                node_status: Prototype1NodeStatus::Running,
                running_binary: true,
                running_lineage: LineageMark::Parent,
                artifact_lineage: LineageMark::Child,
                child_lifecycle: Some(ChildRuntimeLifecycle::Acknowledged),
            },
            child_lifecycle: ChildRuntimeLifecycle::Acknowledged,
            runner_result_path: runner_result_path.to_path_buf(),
            result: None,
        })
    }

    fn spawn_observed_entry(
        node_id: &str,
        runtime_id: RuntimeId,
        node_dir: &Path,
        child_pid: u32,
    ) -> JournalEntry {
        use crate::cli::prototype1_state::event::{
            ChildRuntimeLifecycle, LineageMark, Paths, Refs, World,
        };

        JournalEntry::SpawnChild(SpawnEntry {
            runtime_id,
            phase: SpawnPhase::Observed,
            recorded_at: crate::cli::prototype1_state::event::RecordedAt(0),
            generation: 1,
            refs: Refs {
                campaign_id: "campaign".to_string(),
                node_id: node_id.to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state".to_string(),
                branch_id: "branch".to_string(),
                candidate_id: "candidate".to_string(),
                branch_label: "branch".to_string(),
                spec_id: "spec".to_string(),
            },
            paths: Paths {
                repo_root: node_dir.to_path_buf(),
                workspace_root: node_dir.to_path_buf(),
                binary_path: node_dir.join("ploke-eval"),
                target_relpath: PathBuf::from("src/lib.rs"),
                absolute_path: node_dir.join("src/lib.rs"),
            },
            world: World {
                node_status: Prototype1NodeStatus::Running,
                running_binary: true,
                running_lineage: LineageMark::Parent,
                artifact_lineage: LineageMark::Child,
                child_lifecycle: Some(ChildRuntimeLifecycle::Acknowledged),
            },
            child_lifecycle: ChildRuntimeLifecycle::Acknowledged,
            parent_pid: std::process::id(),
            child_pid: Some(child_pid),
            argv: vec!["loop".to_string(), "prototype1-runner".to_string()],
            streams: None,
            result: Some(SpawnObservation::Acknowledged),
        })
    }
}
