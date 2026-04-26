#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![forbid(unsafe_code)]

//! Prototype 1 treatment-evaluation and successor-bootstrap process path.
//!
//! This module owns the parent/child/successor execution seam for Prototype 1.
//! It exists to keep the dangerous parts of the workflow concentrated in one
//! place:
//!
//! - realizing one node-owned workspace for a branch
//! - building a fresh child binary from that source state
//! - spawning exactly one child runner process
//! - waiting for that child to finish
//! - reading back the persisted runner/evaluation artifacts
//! - spawning the selected successor binary for bounded handoff acknowledgement
//!
//! The key semantic boundary is that the parent process remains the already
//! compiled "current generation", while the child binary is compiled after the
//! branch materialization and therefore represents the candidate next
//! generation for this bounded surface.
//!
//! # Child evaluation flow
//!
//! 1. The parent controller registers a node for a branch.
//! 2. The parent realizes that branch into a node-owned workspace.
//! 3. The parent builds a fresh `ploke-eval` child binary from that source
//!    state.
//! 4. The parent spawns the child runner and waits for it to exit.
//! 5. The child executes exactly one treatment evaluation for that node.
//! 6. The child persists `runner-result.json` and exits.
//! 7. The parent reads the runner result and, on success, reads the persisted
//!    branch-evaluation artifact.
//!
//! # Successor handoff flow
//!
//! 1. The parent selects one kept child/node as the successor candidate.
//! 2. The parent launches the binary built from that selected node's artifact
//!    state.
//! 3. The successor validates the continuation policy, writes a ready
//!    acknowledgement, and idles only within a bounded standby window.
//!
//! The current successor path is a bootstrap/handoff smoke path. It is not yet
//! the full rehydrating controller that starts the next generation itself.
//!
//! # Safety invariants
//!
//! - Process creation for child evaluation and successor bootstrap is localized
//!   in this module.
//! - The child runner executes one node and does not recurse or spawn further
//!   descendants.
//! - The successor runner is bounded by scheduler continuation policy and
//!   standby timeout.
//! - Compile failures and treatment failures are persisted as runner results
//!   instead of becoming implicit control-flow loss.
//! - The controller's parent workspace is not mutated during child evaluation;
//!   each node is realized in its own backend-managed workspace root.
//!
//! # Failure fallout
//!
//! --- DANGER ---
//!
//! If process recursion were accidentally introduced here later, host failure
//! would likely be a resource-exhaustion problem rather than data corruption:
//! process-count growth, CPU starvation, memory pressure, filesystem growth
//! from per-node build artifacts and child binaries, and eventual machine
//! unresponsiveness. In the worst case that can require a hard restart plus
//! cleanup of persisted node artifacts and any unreverted source-tree
//! materialization. That risk is the reason this module keeps process creation
//! localized and documented so aggressively.
//!
//! # Non-goals
//!
//! This module is not the scheduler and is not yet the full self-continuing
//! trampoline. It can bootstrap a selected successor, but it does not choose
//! among siblings or rehydrate the successor as a new generation controller.
//! Those remain controller/state-model concerns.
//!
//! Current process tree:
//!
//! ```text
//! parent: loop prototype1
//!   -> build child binary
//!   -> spawn child runner
//!   -> wait
//!   -> select successor elsewhere in controller/state path
//!   -> spawn selected successor binary
//!   -> wait for ready acknowledgement
//!
//! child: loop prototype1-runner --execute
//!   -> run one treatment evaluation
//!   -> write runner-result.json
//!   -> exit
//!
//! successor: loop prototype1-runner --execute
//!   -> validate continuation
//!   -> write successor-ready acknowledgement
//!   -> bounded standby
//! ```
//!
//! Keeping this path local makes it easier to audit for runaway-process risks.
use ploke_core::EXECUTION_DEBUG_TARGET;
use std::process::Command as ProcessCommand;
use tracing::{debug, instrument};

use super::*;
use crate::BranchDisposition;
use crate::cli::prototype1_state::backend::{GitWorktreeBackend, RealizeRequest, WorkspaceBackend};
use crate::cli::prototype1_state::cli_facing::{
    Prototype1BranchEvaluationReport, build_prototype1_branch_evaluation_report,
    ensure_treatment_branch_materialized, prepare_prototype1_treatment_campaign,
    prototype1_branch_evaluation_path, prototype1_source_generation,
};
use crate::cli::prototype1_state::journal::prototype1_transition_journal_path;
use crate::intervention::{
    Prototype1ContinuationDisposition, Prototype1NodeStatus, Prototype1RunnerDisposition, Prototype1RunnerResult, TreatmentBranchEvaluationSummary, clear_runner_result, decide_node_successor_continuation, load_node_record, load_or_default_branch_registry, load_or_register_treatment_evaluation_node, load_runner_request, load_runner_result_at, load_scheduler_state, prototype1_branch_registry_path, record_runner_result, record_treatment_branch_evaluation, resolve_treatment_branch, update_node_status, update_node_workspace_root, write_runner_result_at
};

const SUCCESSOR_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const SUCCESSOR_READY_POLL: std::time::Duration = std::time::Duration::from_millis(50);
pub(crate) const SUCCESSOR_STANDBY_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(300);

/// Outcome of one parent-side node execution attempt.
///
/// A node either yields a fully materialized branch-evaluation report or a
/// persisted runner failure result that the controller can summarize as a
/// rejected branch outcome.
#[must_use = "node execution outcomes must be handled so failed child runs are not silently ignored"]
pub(super) enum Prototype1NodeExecutionOutcome {
    Evaluated(Prototype1BranchEvaluationReport),
    Failed(Prototype1RunnerResult),
}

/// Parent-observed result of one successor bootstrap attempt.
pub(crate) struct Prototype1SuccessorHandoff {
    pub runtime_id: crate::cli::prototype1_state::event::RuntimeId,
    pub pid: u32,
    pub ready_path: PathBuf,
}

#[must_use = "node build outcomes must be checked before attempting to spawn a child binary"]
enum Prototype1NodeBuildOutcome {
    Built,
    CompileFailed(Prototype1RunnerResult),
}

fn process_output_excerpt(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.is_empty() {
        return None;
    }
    let max_chars = 4000usize;
    let excerpt = if text.chars().count() > max_chars {
        let tail = text
            .chars()
            .rev()
            .take(max_chars)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        format!("...[truncated]\n{tail}")
    } else {
        text
    };
    Some(excerpt)
}

fn record_prototype1_child_ready(
    campaign_id: &str,
    manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    workspace_root: &Path,
    runtime_id: crate::cli::prototype1_state::event::RuntimeId,
    journal_path: &Path,
) -> Result<(), PrepareError> {
    let resolved = resolve_treatment_branch(campaign_id, manifest_path, &node.branch_id)?;
    let entry = crate::cli::prototype1_state::journal::ReadyEntry {
        runtime_id,
        recorded_at: crate::cli::prototype1_state::event::RecordedAt::now(),
        generation: node.generation,
        refs: crate::cli::prototype1_state::event::Refs {
            campaign_id: campaign_id.to_string(),
            node_id: node.node_id.clone(),
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            branch_id: node.branch_id.clone(),
            candidate_id: node.candidate_id.clone(),
            branch_label: resolved.branch.branch_label.clone(),
            spec_id: resolved.branch.synthesized_spec_id.clone(),
        },
        paths: crate::cli::prototype1_state::event::Paths {
            repo_root: workspace_root.to_path_buf(),
            workspace_root: workspace_root.to_path_buf(),
            binary_path: node.binary_path.clone(),
            target_relpath: node.target_relpath.clone(),
            absolute_path: workspace_root.join(&node.target_relpath),
        },
        pid: std::process::id(),
    };

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node.node_id,
        runtime_id = %runtime_id,
        journal_path = %journal_path.display(),
        "recording child ready handshake"
    );

    crate::cli::prototype1_state::c3::record_child_ready(journal_path.to_path_buf(), entry).map_err(
        |err| PrepareError::DatabaseSetup {
            phase: "prototype1_child_ready",
            detail: err.to_string(),
        },
    )
}

fn record_prototype1_child_ready_if_configured(
    campaign_id: &str,
    manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    request: &crate::intervention::Prototype1RunnerRequest,
) -> Result<(), PrepareError> {
    let Some(runtime_id) = std::env::var(crate::cli::prototype1_state::c3::RUNTIME_ID_ENV)
        .ok()
        .and_then(|value| uuid::Uuid::parse_str(&value).ok())
        .map(crate::cli::prototype1_state::event::RuntimeId)
    else {
        return Ok(());
    };
    let Some(journal_path) =
        std::env::var_os(crate::cli::prototype1_state::c3::JOURNAL_PATH_ENV).map(PathBuf::from)
    else {
        return Ok(());
    };
    record_prototype1_child_ready(
        campaign_id,
        manifest_path,
        node,
        &request.workspace_root,
        runtime_id,
        &journal_path,
    )
}

fn record_prototype1_successor_ready(
    invocation: &crate::cli::prototype1_state::invocation::SuccessorInvocation,
    manifest_path: &Path,
) -> Result<crate::cli::prototype1_state::invocation::SuccessorReadyRecord, PrepareError> {
    let node = load_node_record(manifest_path, invocation.node_id())?;
    let ready_path = crate::cli::prototype1_state::invocation::successor_ready_path(
        &node.node_dir,
        invocation.runtime_id(),
    );
    let record = crate::cli::prototype1_state::invocation::SuccessorReadyRecord {
        schema_version: crate::cli::prototype1_state::invocation::SUCCESSOR_READY_SCHEMA_VERSION
            .to_string(),
        campaign_id: invocation.campaign_id().to_string(),
        node_id: invocation.node_id().to_string(),
        runtime_id: invocation.runtime_id(),
        pid: std::process::id(),
        recorded_at: Utc::now().to_rfc3339(),
    };
    crate::cli::prototype1_state::invocation::write_successor_ready_record(&ready_path, &record)?;
    Ok(record)
}

fn validate_prototype1_successor_continuation(
    invocation: &crate::cli::prototype1_state::invocation::SuccessorInvocation,
    manifest_path: &Path,
) -> Result<(), PrepareError> {
    let scheduler = load_scheduler_state(manifest_path)?;
    let node = load_node_record(manifest_path, invocation.node_id())?;
    let decision = decide_node_successor_continuation(&scheduler, &node, Some("keep"));

    if decision.disposition == Prototype1ContinuationDisposition::ContinueReady {
        return Ok(());
    }

    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "successor continuation rejected for node '{}' with disposition {:?} (next_generation={}, total_nodes_after_continue={})",
            invocation.node_id(),
            decision.disposition,
            decision.next_generation,
            decision.total_nodes_after_continue
        ),
    })
}

fn load_prototype1_branch_evaluation_report(
    path: &Path,
) -> Result<Prototype1BranchEvaluationReport, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn spawn_prototype1_child_runner(
    binary_path: &Path,
    repo_root: &Path,
    invocation_path: &Path,
    invocation: &crate::cli::prototype1_state::invocation::ChildInvocation,
) -> Result<std::process::Output, PrepareError> {
    crate::cli::prototype1_state::invocation::write_child_invocation(invocation_path, invocation)?;
    let child_argv = invocation.launch_args(invocation_path);
    ProcessCommand::new(binary_path)
        .args(&child_argv)
        .current_dir(repo_root)
        .output()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_runner_spawn",
            detail: source.to_string(),
        })
}

fn spawn_prototype1_successor(
    binary_path: &Path,
    repo_root: &Path,
    invocation_path: &Path,
    invocation: &crate::cli::prototype1_state::invocation::SuccessorInvocation,
) -> Result<std::process::Child, PrepareError> {
    crate::cli::prototype1_state::invocation::write_successor_invocation(
        invocation_path,
        invocation,
    )?;
    let child_argv = invocation.launch_args(invocation_path);
    let mut command = ProcessCommand::new(binary_path);
    command
        .args(&child_argv)
        .current_dir(repo_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .spawn()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_spawn",
            detail: source.to_string(),
        })
}

fn wait_for_prototype1_successor_ready(
    child: &mut std::process::Child,
    ready_path: &Path,
) -> Result<Option<crate::cli::prototype1_state::invocation::SuccessorReadyRecord>, PrepareError> {
    let start = std::time::Instant::now();
    loop {
        if ready_path.exists() {
            let ready =
                crate::cli::prototype1_state::invocation::load_successor_ready_record(ready_path)?;
            return Ok(Some(ready));
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_poll",
                detail: source.to_string(),
            })?
        {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_successor_ready",
                detail: format!(
                    "successor exited before acknowledging handoff (exit_code={:?})",
                    status.code()
                ),
            });
        }
        if start.elapsed() >= SUCCESSOR_READY_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(SUCCESSOR_READY_POLL);
    }
}

pub(crate) fn spawn_and_handoff_prototype1_successor(
    campaign_id: &str,
    node_id: &str,
) -> Result<Option<Prototype1SuccessorHandoff>, PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let node = load_node_record(&manifest_path, node_id)?;
    // Successor bootstrap must execute the binary built from the selected
    // node artifact, not the original parent controller binary.
    let successor_binary_path = node.binary_path.clone();
    if !successor_binary_path.is_file() {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_successor_binary",
            detail: format!(
                "selected successor binary '{}' was not found",
                successor_binary_path.display()
            ),
        });
    }
    let runtime_id = crate::cli::prototype1_state::event::RuntimeId::new();
    let invocation_path =
        crate::cli::prototype1_state::invocation::invocation_path(&node.node_dir, runtime_id);
    let invocation = crate::cli::prototype1_state::invocation::SuccessorInvocation::new(
        campaign_id.to_string(),
        node.node_id.clone(),
        runtime_id,
        prototype1_transition_journal_path(&manifest_path),
    );
    let ready_path =
        crate::cli::prototype1_state::invocation::successor_ready_path(&node.node_dir, runtime_id);
    if ready_path.exists() {
        fs::remove_file(&ready_path).map_err(|source| PrepareError::WriteManifest {
            path: ready_path.clone(),
            source,
        })?;
    }

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node_id,
        runtime_id = %runtime_id,
        successor_binary_path = %successor_binary_path.display(),
        invocation_path = %invocation_path.display(),
        ready_path = %ready_path.display(),
        "spawning detached prototype1 successor"
    );

    let mut child = spawn_prototype1_successor(
        &successor_binary_path,
        &node.workspace_root,
        &invocation_path,
        &invocation,
    )?;
    let pid = child.id();
    match wait_for_prototype1_successor_ready(&mut child, &ready_path)? {
        Some(_) => Ok(Some(Prototype1SuccessorHandoff {
            runtime_id,
            pid,
            ready_path,
        })),
        None => Ok(None),
    }
}

/// Construct a persisted runner result for child-binary build failure.
///
/// This keeps compile failure in the explicit node/result state machine instead
/// of leaking out as an unstructured process error.
fn build_compile_failed_runner_result(
    campaign_id: &str,
    node: &crate::intervention::Prototype1NodeRecord,
    output: &std::process::Output,
) -> Prototype1RunnerResult {
    Prototype1RunnerResult {
        schema_version: crate::intervention::PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Failed,
        disposition: Prototype1RunnerDisposition::CompileFailed,
        treatment_campaign_id: None,
        evaluation_artifact_path: None,
        detail: Some("child binary build failed".to_string()),
        exit_code: output.status.code(),
        stdout_excerpt: process_output_excerpt(&output.stdout),
        stderr_excerpt: process_output_excerpt(&output.stderr),
        recorded_at: Utc::now().to_rfc3339(),
    }
}

/// Construct a persisted runner result for failure after the child binary
/// exists but before a successful evaluation report is produced.
fn build_treatment_failed_runner_result(
    campaign_id: &str,
    node: &crate::intervention::Prototype1NodeRecord,
    detail: impl Into<String>,
    exit_code: Option<i32>,
    stdout_excerpt: Option<String>,
    stderr_excerpt: Option<String>,
) -> Prototype1RunnerResult {
    Prototype1RunnerResult {
        schema_version: crate::intervention::PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Failed,
        disposition: Prototype1RunnerDisposition::TreatmentFailed,
        treatment_campaign_id: None,
        evaluation_artifact_path: None,
        detail: Some(detail.into()),
        exit_code,
        stdout_excerpt,
        stderr_excerpt,
        recorded_at: Utc::now().to_rfc3339(),
    }
}

/// Construct the success result written by a child runner after it completes
/// one treatment evaluation.
fn build_succeeded_runner_result(
    campaign_id: &str,
    node: &crate::intervention::Prototype1NodeRecord,
    report: &Prototype1BranchEvaluationReport,
) -> Prototype1RunnerResult {
    Prototype1RunnerResult {
        schema_version: crate::intervention::PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Succeeded,
        disposition: Prototype1RunnerDisposition::Succeeded,
        treatment_campaign_id: Some(report.treatment_campaign_id.clone()),
        evaluation_artifact_path: Some(report.evaluation_artifact_path.clone()),
        detail: None,
        exit_code: Some(0),
        stdout_excerpt: None,
        stderr_excerpt: None,
        recorded_at: Utc::now().to_rfc3339(),
    }
}

fn record_attempt_runner_result(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    runtime_id: crate::cli::prototype1_state::event::RuntimeId,
    result: Prototype1RunnerResult,
) -> Result<Prototype1RunnerResult, PrepareError> {
    let attempt_path =
        crate::cli::prototype1_state::invocation::result_path(&node.node_dir, runtime_id);
    let _ = write_runner_result_at(&attempt_path, &result)?;
    let _ = record_runner_result(campaign_id, campaign_manifest_path, result.clone())?;
    Ok(result)
}

/// Realize the selected branch into one node-owned workspace for a node.
///
/// Side effects:
/// - realizes or reuses a backend-managed child workspace under the node dir
/// - writes the branch's proposed content into the node workspace target file
/// - persists the realized workspace root onto the node and runner request
/// - updates node status to `workspace_staged`
fn stage_prototype1_runner_node(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    node_id: &str,
    repo_root: &Path,
) -> Result<crate::intervention::Prototype1NodeRecord, PrepareError> {
    let node = load_node_record(campaign_manifest_path, node_id)?;
    let resolved = resolve_treatment_branch(campaign_id, campaign_manifest_path, &node.branch_id)?;
    let realized = GitWorktreeBackend
        .realize(&RealizeRequest {
            repo_root: repo_root.to_path_buf(),
            node_id: node.node_id.clone(),
            node_dir: node.node_dir.clone(),
            target_relpath: resolved.target_relpath.clone(),
            source_content: resolved.source_content.clone(),
            proposed_content: resolved.branch.proposed_content.clone(),
        })
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_runner_realize",
            detail: source.to_string(),
        })?;
    let _ = update_node_status(
        campaign_id,
        campaign_manifest_path,
        node_id,
        Prototype1NodeStatus::WorkspaceStaged,
    )?;
    let (_, updated, _) =
        update_node_workspace_root(campaign_id, campaign_manifest_path, node_id, realized.root)?;
    Ok(updated)
}

/// Build the child binary for one node from its realized workspace root.
///
/// Cargo scratch artifacts are isolated under `node/target/` so the build does
/// not pollute the repo-level `target/`. The promoted executable is copied to
/// `node/bin/` so it can remain available even if `node/target/` is later
/// cleaned.
///
/// Compile failure is normalized into a persisted runner result rather than
/// leaving the node in an ambiguous partially-built state.
fn build_prototype1_runner_binary(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    node_id: &str,
) -> Result<Prototype1NodeBuildOutcome, PrepareError> {
    let node = load_node_record(campaign_manifest_path, node_id)?;
    // Keep Cargo's full build scratch (deps, fingerprints, incremental state, and the raw
    // binary) inside a node-local target dir rather than polluting the repo's shared target/.
    let target_dir = node.node_dir.join("target");
    fs::create_dir_all(&target_dir).map_err(|source| PrepareError::CreateOutputDir {
        path: target_dir.clone(),
        source,
    })?;
    if let Some(parent) = node.binary_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let output = ProcessCommand::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("ploke-eval")
        .arg("--bin")
        .arg("ploke-eval")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&node.workspace_root)
        .output()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_runner_build",
            detail: source.to_string(),
        })?;

    if !output.status.success() {
        let result = build_compile_failed_runner_result(campaign_id, &node, &output);
        let _ = record_runner_result(campaign_id, campaign_manifest_path, result.clone())?;
        return Ok(Prototype1NodeBuildOutcome::CompileFailed(result));
    }

    let built_binary = target_dir
        .join("debug")
        .join(format!("ploke-eval{}", std::env::consts::EXE_SUFFIX));
    if !built_binary.exists() {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_runner_build",
            detail: format!(
                "build succeeded but child binary '{}' was not found",
                built_binary.display()
            ),
        });
    }

    // Copy the promoted child executable out of Cargo's scratch tree so later cleanup can drop
    // node/target without deleting the runnable artifact we want to keep for lineage/debugging.
    fs::copy(&built_binary, &node.binary_path).map_err(|source| PrepareError::WriteManifest {
        path: node.binary_path.clone(),
        source,
    })?;
    let _ = update_node_status(
        campaign_id,
        campaign_manifest_path,
        node_id,
        Prototype1NodeStatus::BinaryBuilt,
    )?;
    Ok(Prototype1NodeBuildOutcome::Built)
}

/// Execute one branch evaluation in-process inside the child runner binary.
///
/// This is the leaf treatment-evaluation operation. It:
/// - assumes the parent already materialized the branch and built this binary
/// - runs exactly one treatment `eval -> protocol -> compare` path
/// - persists a `runner-result.json`
/// - does not spawn any additional processes
///
/// This function is intentionally terminal with respect to process creation.
pub(super) async fn run_prototype1_branch_evaluation(
    baseline_campaign_id: &str,
    branch_id: &str,
    repo_root: &Path,
    stop_on_error: bool,
) -> Result<Prototype1BranchEvaluationReport, PrepareError> {
    let _run_scope = TimingTrace::scope(format!("loop.prototype1_branch.evaluate.{branch_id}"));
    let baseline_manifest_path = campaign_manifest_path(baseline_campaign_id)?;
    let branch_registry_path = prototype1_branch_registry_path(&baseline_manifest_path);
    let resolved_branch =
        resolve_treatment_branch(baseline_campaign_id, &baseline_manifest_path, branch_id)?;
    {
        let _scope = TimingTrace::scope(format!(
            "loop.prototype1_branch.evaluate.materialize.{branch_id}"
        ));
        ensure_treatment_branch_materialized(
            baseline_campaign_id,
            &baseline_manifest_path,
            &resolved_branch,
            repo_root,
        )?;
    }

    let baseline_resolved =
        resolve_campaign_config(baseline_campaign_id, &CampaignOverrides::default())?;
    let treatment_campaign = {
        let _scope = TimingTrace::scope(format!(
            "loop.prototype1_branch.evaluate.prepare_campaign.{branch_id}"
        ));
        prepare_prototype1_treatment_campaign(&baseline_resolved, branch_id)?
    };
    let mut eval_policy = treatment_campaign.resolved.eval.clone();
    if stop_on_error {
        eval_policy.stop_on_error = true;
    }
    {
        let _scope =
            TimingTrace::scope(format!("loop.prototype1_branch.evaluate.eval.{branch_id}"));
        let _ = advance_eval_closure(&treatment_campaign.resolved, &eval_policy, false).await?;
    }

    let mut protocol_policy = treatment_campaign.resolved.protocol.clone();
    if stop_on_error {
        protocol_policy.stop_on_error = true;
    }
    {
        let _scope = TimingTrace::scope(format!(
            "loop.prototype1_branch.evaluate.protocol.{branch_id}"
        ));
        let _ =
            advance_protocol_closure(&treatment_campaign.resolved, &protocol_policy, false).await?;
    }

    let baseline_state = load_closure_state(baseline_campaign_id)?;
    let treatment_state = load_closure_state(&treatment_campaign.campaign_id)?;
    let evaluation_artifact_path =
        prototype1_branch_evaluation_path(&baseline_manifest_path, branch_id);
    let report = {
        let _scope = TimingTrace::scope(format!(
            "loop.prototype1_branch.evaluate.compare.{branch_id}"
        ));
        build_prototype1_branch_evaluation_report(
            baseline_campaign_id,
            branch_id,
            &branch_registry_path,
            &evaluation_artifact_path,
            &treatment_campaign,
            &baseline_state,
            &treatment_state,
        )?
    };
    {
        let _scope = TimingTrace::scope(format!(
            "loop.prototype1_branch.evaluate.persist_report.{branch_id}"
        ));
        write_json_file_pretty(&evaluation_artifact_path, &report)?;
    }

    let rejected_instances = report
        .compared_instances
        .iter()
        .filter(|row| {
            row.evaluation
                .as_ref()
                .is_some_and(|evaluation| evaluation.disposition == BranchDisposition::Reject)
                || row.status != "compared"
        })
        .count();
    let summary = TreatmentBranchEvaluationSummary {
        baseline_campaign_id: baseline_campaign_id.to_string(),
        treatment_campaign_id: report.treatment_campaign_id.clone(),
        compared_instances: report.compared_instances.len(),
        rejected_instances,
        overall_disposition: report.overall_disposition.clone(),
        evaluated_at: Utc::now().to_rfc3339(),
    };
    {
        let _scope = TimingTrace::scope(format!(
            "loop.prototype1_branch.evaluate.persist_summary.{branch_id}"
        ));
        let _ = record_treatment_branch_evaluation(
            baseline_campaign_id,
            &baseline_manifest_path,
            branch_id,
            summary,
        )?;
    }

    Ok(report)
}

/// Child-runner entrypoint for `loop prototype1-runner --execute`.
///
/// The child process resolves its node/request state, marks the node running,
/// performs one treatment evaluation, records a terminal runner result, and
/// returns that result to its caller.
///
/// This function does not recurse and does not choose any follow-on work.
#[instrument(
    target = "ploke_exec",
    level = "debug",
    skip(stop_on_error),
    fields(campaign = %campaign_id, node_id = %node_id, stop_on_error)
)]
pub(super) async fn execute_prototype1_runner_node(
    campaign_id: &str,
    node_id: &str,
    stop_on_error: bool,
) -> Result<Prototype1RunnerResult, PrepareError> {
    let runtime_id = std::env::var(crate::cli::prototype1_state::c3::RUNTIME_ID_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(crate::cli::prototype1_state::event::RuntimeId::new);
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let node = load_node_record(&manifest_path, node_id)?;
    let request = load_runner_request(&manifest_path, node_id)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node_id,
        workspace_root = %request.workspace_root.display(),
        binary_path = %node.binary_path.display(),
        "loaded prototype1 runner node"
    );
    let _ = update_node_status(
        campaign_id,
        &manifest_path,
        node_id,
        Prototype1NodeStatus::Running,
    )?;
    record_prototype1_child_ready_if_configured(campaign_id, &manifest_path, &node, &request)?;

    let outcome = run_prototype1_branch_evaluation(
        campaign_id,
        &node.branch_id,
        &request.workspace_root,
        stop_on_error,
    )
    .await;

    let result = match outcome {
        Ok(report) => build_succeeded_runner_result(campaign_id, &node, &report),
        Err(err) => build_treatment_failed_runner_result(
            campaign_id,
            &node,
            err.to_string(),
            None,
            None,
            None,
        ),
    };
    let result =
        record_attempt_runner_result(campaign_id, &manifest_path, &node, runtime_id, result)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node_id,
        disposition = ?result.disposition,
        status = ?result.status,
        "prototype1 runner node completed"
    );
    Ok(result)
}

#[instrument(
    target = "ploke_exec",
    level = "debug",
    skip(invocation_path),
    fields(invocation_path = %invocation_path.display())
)]
pub(super) async fn execute_prototype1_runner_invocation(
    invocation_path: &Path,
) -> Result<Prototype1RunnerResult, PrepareError> {
    let invocation = match crate::cli::prototype1_state::invocation::load_executable(
        invocation_path,
    )? {
        crate::cli::prototype1_state::invocation::InvocationAuthority::Child(invocation) => {
            invocation
        }
        crate::cli::prototype1_state::invocation::InvocationAuthority::Successor(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor invocation '{}' must be executed via execute_prototype1_successor_invocation",
                    invocation_path.display()
                ),
            });
        }
    };
    let manifest_path = campaign_manifest_path(invocation.campaign_id())?;
    let node = load_node_record(&manifest_path, invocation.node_id())?;
    let request = load_runner_request(&manifest_path, invocation.node_id())?;

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %invocation.campaign_id(),
        node_id = %invocation.node_id(),
        workspace_root = %request.workspace_root.display(),
        invocation_path = %invocation_path.display(),
        "loaded executable prototype1 child invocation"
    );

    let _ = update_node_status(
        invocation.campaign_id(),
        &manifest_path,
        invocation.node_id(),
        Prototype1NodeStatus::Running,
    )?;
    record_prototype1_child_ready(
        invocation.campaign_id(),
        &manifest_path,
        &node,
        &request.workspace_root,
        invocation.runtime_id(),
        invocation.journal_path(),
    )?;

    let outcome = run_prototype1_branch_evaluation(
        invocation.campaign_id(),
        &request.branch_id,
        &request.workspace_root,
        request.stop_on_error,
    )
    .await;

    let result = match outcome {
        Ok(report) => build_succeeded_runner_result(invocation.campaign_id(), &node, &report),
        Err(err) => build_treatment_failed_runner_result(
            invocation.campaign_id(),
            &node,
            err.to_string(),
            None,
            None,
            None,
        ),
    };
    let result = record_attempt_runner_result(
        invocation.campaign_id(),
        &manifest_path,
        &node,
        invocation.runtime_id(),
        result,
    )?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %invocation.campaign_id(),
        node_id = %invocation.node_id(),
        disposition = ?result.disposition,
        status = ?result.status,
        "prototype1 child invocation completed"
    );
    Ok(result)
}

#[instrument(
    target = "ploke_exec",
    level = "debug",
    skip(invocation_path),
    fields(invocation_path = %invocation_path.display())
)]
pub(super) async fn execute_prototype1_successor_invocation(
    invocation_path: &Path,
) -> Result<crate::cli::prototype1_state::invocation::SuccessorReadyRecord, PrepareError> {
    let invocation = match crate::cli::prototype1_state::invocation::load_executable(
        invocation_path,
    )? {
        crate::cli::prototype1_state::invocation::InvocationAuthority::Successor(invocation) => {
            invocation
        }
        crate::cli::prototype1_state::invocation::InvocationAuthority::Child(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "child invocation '{}' must be executed via execute_prototype1_runner_invocation",
                    invocation_path.display()
                ),
            });
        }
    };
    let manifest_path = campaign_manifest_path(invocation.campaign_id())?;
    validate_prototype1_successor_continuation(&invocation, &manifest_path)?;
    let ready = record_prototype1_successor_ready(&invocation, &manifest_path)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %invocation.campaign_id(),
        node_id = %invocation.node_id(),
        runtime_id = %invocation.runtime_id(),
        pid = ready.pid,
        standby_secs = SUCCESSOR_STANDBY_TIMEOUT.as_secs(),
        "prototype1 successor acknowledged handoff"
    );
    std::thread::sleep(SUCCESSOR_STANDBY_TIMEOUT);
    Ok(ready)
}

/// Parent-side staged execution path for one treatment branch.
///
/// This is the main controller-facing process helper. It:
/// 1. registers or reloads the node
/// 2. materializes the branch into the live source tree
/// 3. builds a fresh child binary
/// 4. spawns exactly one child runner process and waits for it
/// 5. reads back `runner-result.json`
/// 6. loads the branch-evaluation artifact on success
///
/// Failure behavior:
/// - compile failures become `Prototype1NodeExecutionOutcome::Failed`
/// - child treatment failures become `Prototype1NodeExecutionOutcome::Failed`
/// - missing runner-result artifacts are converted into an explicit failure
///   result instead of being silently ignored
///
/// This function is the single explicit child-process spawn site for the
/// Prototype 1 treatment path.
#[instrument(
    target = "ploke_exec",
    level = "debug",
    skip(repo_root),
    fields(campaign = %baseline_campaign_id, branch_id = %branch_id, repo_root = %repo_root.display(), stop_on_error)
)]
pub(super) async fn run_prototype1_branch_evaluation_via_child(
    baseline_campaign_id: &str,
    branch_id: &str,
    repo_root: &Path,
    stop_on_error: bool,
) -> Result<Prototype1NodeExecutionOutcome, PrepareError> {
    let baseline_manifest_path = campaign_manifest_path(baseline_campaign_id)?;
    let registry = load_or_default_branch_registry(baseline_campaign_id, &baseline_manifest_path)?;
    let source_node = registry
        .source_nodes
        .iter()
        .find(|source| {
            source
                .branches
                .iter()
                .any(|branch| branch.branch_id == branch_id)
        })
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!("branch '{branch_id}' is not present in the branch registry"),
        })?;
    let generation = prototype1_source_generation(&registry, source_node) + 1;
    let resolved =
        resolve_treatment_branch(baseline_campaign_id, &baseline_manifest_path, branch_id)?;
    let (_, node, _) = load_or_register_treatment_evaluation_node(
        baseline_campaign_id,
        &baseline_manifest_path,
        &resolved,
        generation,
        repo_root,
        stop_on_error,
    )?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %baseline_campaign_id,
        branch_id = %branch_id,
        node_id = %node.node_id,
        "registered prototype1 child-eval node"
    );

    let node = stage_prototype1_runner_node(
        baseline_campaign_id,
        &baseline_manifest_path,
        &node.node_id,
        repo_root,
    )?;
    match build_prototype1_runner_binary(
        baseline_campaign_id,
        &baseline_manifest_path,
        &node.node_id,
    )? {
        Prototype1NodeBuildOutcome::CompileFailed(result) => {
            return Ok(Prototype1NodeExecutionOutcome::Failed(result));
        }
        Prototype1NodeBuildOutcome::Built => {}
    }

    let _ = clear_runner_result(&baseline_manifest_path, &node.node_id)?;
    let runtime_id = crate::cli::prototype1_state::event::RuntimeId::new();
    let journal_path = prototype1_transition_journal_path(&baseline_manifest_path);
    let invocation_path =
        crate::cli::prototype1_state::invocation::invocation_path(&node.node_dir, runtime_id);
    let invocation = crate::cli::prototype1_state::invocation::ChildInvocation::new(
        baseline_campaign_id.to_string(),
        node.node_id.clone(),
        runtime_id,
        journal_path.clone(),
    );
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %baseline_campaign_id,
        branch_id = %branch_id,
        node_id = %node.node_id,
        runtime_id = %runtime_id,
        binary_path = %node.binary_path.display(),
        workspace_root = %node.workspace_root.display(),
        invocation_path = %invocation_path.display(),
        "spawning prototype1 leaf child runner"
    );
    let output = spawn_prototype1_child_runner(
        &node.binary_path,
        &node.workspace_root,
        &invocation_path,
        &invocation,
    )?;

    let attempt_result_path =
        crate::cli::prototype1_state::invocation::result_path(&node.node_dir, runtime_id);
    let runner_result = if attempt_result_path.exists() {
        Some(load_runner_result_at(&attempt_result_path)?)
    } else {
        None
    };

    let runner_result = match runner_result {
        Some(result) => result,
        None => {
            let failure = build_treatment_failed_runner_result(
                baseline_campaign_id,
                &node,
                "child runner exited without writing a runner result artifact",
                output.status.code(),
                process_output_excerpt(&output.stdout),
                process_output_excerpt(&output.stderr),
            );
            let failure = record_attempt_runner_result(
                baseline_campaign_id,
                &baseline_manifest_path,
                &node,
                runtime_id,
                failure.clone(),
            )?;
            failure
        }
    };

    if runner_result.disposition != Prototype1RunnerDisposition::Succeeded {
        return Ok(Prototype1NodeExecutionOutcome::Failed(runner_result));
    }

    let evaluation_artifact_path =
        runner_result
            .evaluation_artifact_path
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "runner result for node '{}' did not include an evaluation artifact path",
                    node.node_id
                ),
            })?;
    let report = load_prototype1_branch_evaluation_report(evaluation_artifact_path)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %baseline_campaign_id,
        branch_id = %branch_id,
        node_id = %node.node_id,
        "legacy prototype1 child runner succeeded"
    );
    Ok(Prototype1NodeExecutionOutcome::Evaluated(report))
}
