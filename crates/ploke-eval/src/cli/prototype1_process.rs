#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![forbid(unsafe_code)]

//! Prototype 1 treatment-evaluation process path.
//!
//! This module owns the parent/child execution seam for Prototype 1 branch
//! evaluation. It exists to keep the dangerous parts of the workflow
//! concentrated in one place:
//!
//! - mutating the live source tree to materialize a branch
//! - building a fresh child binary from that source state
//! - spawning exactly one child runner process
//! - waiting for that child to finish
//! - reading back the persisted runner/evaluation artifacts
//!
//! The key semantic boundary is that the parent process remains the already
//! compiled "current generation", while the child binary is compiled after the
//! branch materialization and therefore represents the candidate next
//! generation for this bounded surface.
//!
//! # End-to-end flow
//!
//! 1. The parent controller registers a node for a branch.
//! 2. The parent materializes that branch into the live source tree.
//! 3. The parent builds a fresh `ploke-eval` child binary from that source
//!    state.
//! 4. The parent spawns the child runner and waits for it to exit.
//! 5. The child executes exactly one treatment evaluation for that node.
//! 6. The child persists `runner-result.json` and exits.
//! 7. The parent reads the runner result and, on success, reads the persisted
//!    branch-evaluation artifact.
//!
//! # Safety invariants
//!
//! - This module has a single explicit child-process spawn site.
//! - The child runner executes one node and does not recurse or spawn further
//!   descendants.
//! - Compile failures and treatment failures are persisted as runner results
//!   instead of becoming implicit control-flow loss.
//! - Source-tree restoration is intentionally not handled here; the controller
//!   remains responsible for applying branch restore semantics around calls into
//!   this module.
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
//! This module is not the scheduler and is not yet a self-continuing
//! trampoline. It does not choose the next branch, recurse to a new generation,
//! or decide global continuation policy. Those remain controller concerns.
//!
//! Current process tree:
//!
//! ```text
//! parent: loop prototype1
//!   -> build child binary
//!   -> spawn child runner
//!   -> wait
//!
//! child: loop prototype1-runner --execute
//!   -> run one treatment evaluation
//!   -> write runner-result.json
//!   -> exit
//! ```
//!
//! Keeping this path local makes it easier to audit for runaway-process risks.
use ploke_core::EXECUTION_DEBUG_TARGET;
use tracing::{debug, instrument};

use super::*;

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
            repo_root: request.workspace_root.clone(),
            workspace_root: request.workspace_root.clone(),
            binary_path: node.binary_path.clone(),
            target_relpath: node.target_relpath.clone(),
            absolute_path: request.workspace_root.join(&node.target_relpath),
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

    crate::cli::prototype1_state::c3::record_child_ready(journal_path, entry).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "prototype1_child_ready",
            detail: err.to_string(),
        }
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

/// Materialize the selected branch into the live source tree for a node.
///
/// This function deliberately mutates the shared source tree used for building
/// the child binary. It relies on the controller's existing branch restore
/// semantics to recover the source tree after evaluation.
///
/// Side effects:
/// - writes the branch's proposed content into the target file
/// - updates node status to `workspace_staged`
fn stage_prototype1_runner_node(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    node_id: &str,
    repo_root: &Path,
) -> Result<crate::intervention::Prototype1NodeRecord, PrepareError> {
    let node = load_node_record(campaign_manifest_path, node_id)?;
    let resolved = resolve_treatment_branch(campaign_id, campaign_manifest_path, &node.branch_id)?;
    ensure_treatment_branch_materialized(
        campaign_id,
        campaign_manifest_path,
        &resolved,
        repo_root,
    )?;
    let (_, updated) = update_node_status(
        campaign_id,
        campaign_manifest_path,
        node_id,
        Prototype1NodeStatus::WorkspaceStaged,
    )?;
    Ok(updated)
}

/// Build the child binary for one node from the currently materialized source
/// tree.
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
    repo_root: &Path,
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
        .current_dir(repo_root)
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
    let _ = record_runner_result(campaign_id, &manifest_path, result.clone())?;
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
    let (_, node, _) = register_treatment_evaluation_node(
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

    let _ = stage_prototype1_runner_node(
        baseline_campaign_id,
        &baseline_manifest_path,
        &node.node_id,
        repo_root,
    )?;
    match build_prototype1_runner_binary(
        baseline_campaign_id,
        &baseline_manifest_path,
        &node.node_id,
        repo_root,
    )? {
        Prototype1NodeBuildOutcome::CompileFailed(result) => {
            return Ok(Prototype1NodeExecutionOutcome::Failed(result));
        }
        Prototype1NodeBuildOutcome::Built => {}
    }

    let request = load_runner_request(&baseline_manifest_path, &node.node_id)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %baseline_campaign_id,
        branch_id = %branch_id,
        node_id = %node.node_id,
        binary_path = %node.binary_path.display(),
        "spawning legacy prototype1 child runner"
    );
    let output = ProcessCommand::new(&node.binary_path)
        .args(&request.runner_args)
        .current_dir(repo_root)
        .output()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_runner_spawn",
            detail: source.to_string(),
        })?;

    let runner_result = if node.runner_result_path.exists() {
        Some(load_runner_result(&baseline_manifest_path, &node.node_id)?)
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
            let _ = record_runner_result(
                baseline_campaign_id,
                &baseline_manifest_path,
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
