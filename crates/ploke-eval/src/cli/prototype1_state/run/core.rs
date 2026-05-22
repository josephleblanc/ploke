use std::{
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::{
    ClosureClass, ResolvedCampaignConfig,
    campaign::resolve_campaign_config,
    campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1CandidateGenerator, Prototype1ControlCommand,
        Prototype1PromptCommand,
    },
    closure::load_closure_state,
    intervention::{
        CompleteBaseline, Intervention, Prototype1ChildScheduleMode, Prototype1NodeRecord,
        Prototype1NodeStatus, Prototype1RunnerRequest, Prototype1RunnerResult, RecordStore,
        load_node_record, load_runner_request, load_runner_result,
    },
    projection::OperatorProjectionRead,
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
    event::{ContentHash, RuntimeId},
    history::{ArtifactSurface, surface_attempt},
    identity::{ParentIdentity, load_parent_identity_optional, parent_identity_relpath},
    inner::At,
    journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
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

pub(crate) async fn doctor(command: Prototype1ControlCommand) -> Result<(), PrepareError> {
    let status = diagnose_command(&command)?;
    render_status(command.format, &status)
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
        phase: diagnosis.phase,
        current_child: diagnosis.current_child,
        blockers: diagnosis.blockers,
        allowed_actions: allowed_actions_for_phase(diagnosis.phase),
        suggested_commands: suggested_commands(&diagnosis.context.repo_root),
        notes,
    }
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

fn suggested_commands(repo_root: &Path) -> Vec<String> {
    vec![
        format!(
            "cd {} && ./target/debug/ploke-eval loop prototype1-continue --repo-root .",
            repo_root.display()
        ),
        format!(
            "cd {} && ./target/debug/ploke-eval loop prototype1-step --repo-root .",
            repo_root.display()
        ),
    ]
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

    for published in load_published_broad_requests(&context.manifest_path)? {
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
        checked.extend(prompt_refs_from_request(
            published.request(),
            published.workspace_path(),
            true,
            true,
        ));
    }

    Ok(PromptPreflight::from_parts(
        checked,
        prompt_files,
        problems,
        notes,
    ))
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
                    return Ok(Diagnosis {
                        context: context.clone(),
                        phase: DiagnosedPhase::BaselineEval,
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
                        phase: DiagnosedPhase::BaselineProtocol,
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
                    phase: DiagnosedPhase::BaselineEval,
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
    crate::cli::advance_protocol_or_block(
        &context.resolved_campaign,
        &context.resolved_campaign.protocol,
    )
    .await
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
            match ObserveChild::new()
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
        Control, Execution, Generation, Protocol, Prototype1RunProfile, RunMode, Search, Selection,
        Storage, Target,
    };

    fn profile(schedule: Prototype1ChildScheduleMode, min: u32, max: u32) -> Prototype1RunProfile {
        Prototype1RunProfile {
            schema_version: crate::cli::prototype1_state::profile::RUN_PROFILE_SCHEMA_VERSION
                .to_string(),
            name: "test-profile".to_string(),
            storage: Storage::default(),
            target: Target::default(),
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
}
