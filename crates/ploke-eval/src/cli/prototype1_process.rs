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
//! - realizing one temporary node-owned workspace for a candidate branch
//! - hydrating a child runtime from that source state for evaluation
//! - spawning exactly one child runner process
//! - waiting for that child to finish
//! - reading back the persisted runner/evaluation artifacts
//! - bootstrapping the selected successor runtime for bounded handoff
//!   acknowledgement
//!
//! The key semantic boundary is that the parent process remains the already
//! compiled "current generation", while the child runtime is hydrated after
//! branch materialization and therefore represents the candidate next
//! generation for this bounded surface. Temporary child worktrees are not
//! successor homes; after selection, the selected Artifact must be installed in
//! the stable active parent checkout before the successor becomes Parent.
//!
//! # Child evaluation flow
//!
//! 1. The parent controller registers a node for a branch.
//! 2. The parent realizes that branch into a node-owned workspace.
//! 3. The parent hydrates a fresh `ploke-eval` child runtime from that source
//!    state.
//! 4. The parent spawns the child runner and waits for it to exit.
//! 5. The child executes exactly one treatment evaluation for that node.
//! 6. The child persists `runner-result.json` and exits.
//! 7. The parent reads the runner result and, on success, reads the persisted
//!    branch-evaluation artifact.
//!
//! # Intended successor handoff flow
//!
//! 1. The parent selects one kept child/node as the successor candidate.
//! 2. The parent updates the stable active checkout to the selected Artifact.
//! 3. The parent hydrates and launches the next Parent from that active
//!    checkout, not from the temporary child worktree.
//! 4. The successor handoff token lets that next Parent validate continuation
//!    authority and write a ready acknowledgement before entering the same
//!    typed parent command path as the initial Parent.
//!
//! The current successor path is a bounded trampoline. Each new Parent runs one
//! typed generation, then hands off to the next Parent only when the scheduler
//! records `ContinueReady`.
//!
//! # Safety invariants and target constraints
//!
//! - Process creation for child evaluation and successor bootstrap is localized
//!   in this module.
//! - The child runner executes one node and does not recurse or spawn further
//!   descendants.
//! - The successor handoff is bounded by scheduler continuation policy and one
//!   typed parent generation.
//! - Temporary child worktrees and build products must become cleanup targets
//!   once evaluation, selection, and handoff no longer need them.
//! - Compile failures and treatment failures are persisted as runner results
//!   instead of becoming implicit control-flow loss.
//! - The controller's parent workspace is not mutated during child evaluation;
//!   each node is realized in its own backend-managed workspace root.
//! - The successor Parent should run from the same stable active checkout path
//!   the previous Parent used, after that checkout has been advanced to the
//!   selected Artifact. Any code path that instead makes the child worktree
//!   the successor Parent's long-lived home is transitional implementation debt.
//!
//! # Failure fallout
//!
//! --- DANGER ---
//!
//! If process recursion were accidentally introduced here later, host failure
//! would likely be a resource-exhaustion problem rather than data corruption:
//! process-count growth, CPU starvation, memory pressure, filesystem growth
//! from per-node worktrees/build artifacts and eventual machine
//! unresponsiveness. In the worst case that can require a hard restart plus
//! cleanup of persisted node artifacts and any unreverted source-tree
//! materialization. That risk is the reason this module keeps process creation
//! localized and documented so aggressively.
//!
//! # Non-goals
//!
//! This module is not the scheduler. It can bootstrap a selected successor and
//! delegate one rehydrated generation to the controller, but sibling selection
//! and durable parent authority remain controller/state-model concerns.
//!
//! Target process tree for the trampoline work:
//!
//! ```text
//! parent: loop prototype1
//!   -> hydrate child runtime in temporary node worktree
//!   -> spawn child runner
//!   -> wait
//!   -> select successor elsewhere in controller/state path
//!   -> update stable active checkout to selected Artifact
//!   -> hydrate and spawn successor Runtime from active checkout
//!   -> wait for ready acknowledgement
//!
//! child: loop prototype1-runner --execute
//!   -> run one treatment evaluation
//!   -> write runner-result.json
//!   -> exit
//!
//! successor: loop prototype1-state --handoff-invocation ...
//!   -> validate continuation
//!   -> write successor-ready acknowledgement
//!   -> enter the same typed parent path as the initial parent
//! ```
//!
//! Keeping this path local makes it easier to audit for runaway-process risks.
use ploke_core::EXECUTION_DEBUG_TARGET;
use std::process::Command as ProcessCommand;
use tracing::{debug, instrument};

use super::*;
use crate::BranchDisposition;
use crate::cli::prototype1_state::backend::{GitWorktreeBackend, RealizeRequest, WorkspaceBackend};
use crate::cli::prototype1_state::child::{Child, Ready};
use crate::cli::prototype1_state::cli_facing::{
    Prototype1BranchEvaluationReport, build_prototype1_branch_evaluation_report,
    ensure_treatment_branch_materialized, prepare_prototype1_treatment_campaign,
    prototype1_branch_evaluation_path, prototype1_source_generation,
};
use crate::cli::prototype1_state::event::{Paths, RecordedAt, Refs};
use crate::cli::prototype1_state::identity::{
    ParentIdentity, load_parent_identity_optional, parent_identity_commit_message,
    parent_identity_relpath, write_parent_identity,
};
use crate::cli::prototype1_state::journal::{
    ActiveCheckoutAdvancedEntry, ChildArtifactCommittedEntry, JournalEntry, PrototypeJournal,
    Streams, SuccessorHandoffEntry, prototype1_transition_journal_path,
};
use crate::cli::prototype1_state::successor::Record as SuccessorRecord;
use crate::intervention::{
    CommitPhase, Prototype1ContinuationDisposition, Prototype1NodeStatus,
    Prototype1RunnerDisposition, Prototype1RunnerResult, RecordStore,
    TreatmentBranchEvaluationSummary, clear_runner_result, load_node_record,
    load_or_default_branch_registry, load_or_register_treatment_evaluation_node,
    load_runner_request, load_runner_result_at, load_scheduler_state,
    prototype1_branch_registry_path, record_runner_result, record_treatment_branch_evaluation,
    resolve_treatment_branch, select_treatment_branch, update_node_status,
    update_node_workspace_root, write_runner_result_at,
};

const SUCCESSOR_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const SUCCESSOR_READY_POLL: std::time::Duration = std::time::Duration::from_millis(50);

fn append_prototype1_journal_entry(
    manifest_path: &Path,
    entry: JournalEntry,
    phase: &'static str,
) -> Result<(), PrepareError> {
    let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(manifest_path));
    journal
        .append(entry)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase,
            detail: source.to_string(),
        })
}

fn append_successor_record(
    journal_path: &Path,
    record: SuccessorRecord,
    phase: &'static str,
) -> Result<(), PrepareError> {
    let mut journal = PrototypeJournal::new(journal_path);
    journal
        .append(JournalEntry::Successor(record))
        .map_err(|source| PrepareError::DatabaseSetup {
            phase,
            detail: source.to_string(),
        })
}

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
) -> Result<Child<Ready>, PrepareError> {
    let resolved = resolve_treatment_branch(campaign_id, manifest_path, &node.branch_id)?;
    let refs = Refs {
        campaign_id: campaign_id.to_string(),
        node_id: node.node_id.clone(),
        instance_id: node.instance_id.clone(),
        source_state_id: node.source_state_id.clone(),
        branch_id: node.branch_id.clone(),
        candidate_id: node.candidate_id.clone(),
        branch_label: resolved.branch.branch_label.clone(),
        spec_id: resolved.branch.synthesized_spec_id.clone(),
    };
    let paths = Paths {
        repo_root: workspace_root.to_path_buf(),
        workspace_root: workspace_root.to_path_buf(),
        binary_path: node.binary_path.clone(),
        target_relpath: node.target_relpath.clone(),
        absolute_path: workspace_root.join(&node.target_relpath),
    };

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node.node_id,
        runtime_id = %runtime_id,
        journal_path = %journal_path.display(),
        "recording child ready handshake"
    );

    Child::new(
        journal_path.to_path_buf(),
        runtime_id,
        node.generation,
        refs,
        paths,
        std::process::id(),
    )
    .ready()
    .map_err(|err| PrepareError::DatabaseSetup {
        phase: "prototype1_child_ready",
        detail: err.to_string(),
    })
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
    .map(|_| ())
}

pub(crate) fn record_prototype1_successor_ready(
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
    append_successor_record(
        invocation.journal_path(),
        SuccessorRecord::ready(invocation, record.pid, ready_path),
        "prototype1_successor_ready_journal",
    )?;
    Ok(record)
}

pub(crate) fn record_prototype1_successor_completion(
    invocation: &crate::cli::prototype1_state::invocation::SuccessorInvocation,
    manifest_path: &Path,
    status: crate::cli::prototype1_state::invocation::SuccessorCompletionStatus,
    trace_path: Option<PathBuf>,
    detail: Option<String>,
) -> Result<crate::cli::prototype1_state::invocation::SuccessorCompletionRecord, PrepareError> {
    let node = load_node_record(manifest_path, invocation.node_id())?;
    let completion_path = crate::cli::prototype1_state::invocation::successor_completion_path(
        &node.node_dir,
        invocation.runtime_id(),
    );
    let record = crate::cli::prototype1_state::invocation::SuccessorCompletionRecord {
        schema_version:
            crate::cli::prototype1_state::invocation::SUCCESSOR_COMPLETION_SCHEMA_VERSION
                .to_string(),
        campaign_id: invocation.campaign_id().to_string(),
        node_id: invocation.node_id().to_string(),
        runtime_id: invocation.runtime_id(),
        status,
        trace_path: trace_path.clone(),
        detail: detail.clone(),
        recorded_at: Utc::now().to_rfc3339(),
    };
    crate::cli::prototype1_state::invocation::write_successor_completion_record(
        &completion_path,
        &record,
    )?;
    append_successor_record(
        invocation.journal_path(),
        SuccessorRecord::completed(invocation, status, completion_path, trace_path, detail),
        "prototype1_successor_completion_journal",
    )?;
    Ok(record)
}

pub(crate) fn validate_prototype1_successor_continuation(
    invocation: &crate::cli::prototype1_state::invocation::SuccessorInvocation,
    manifest_path: &Path,
) -> Result<(), PrepareError> {
    validate_prototype1_successor_node_continuation(manifest_path, invocation.node_id())
}

fn validate_prototype1_successor_node_continuation(
    manifest_path: &Path,
    node_id: &str,
) -> Result<(), PrepareError> {
    let scheduler = load_scheduler_state(manifest_path)?;
    let node = load_node_record(manifest_path, node_id)?;
    let decision = scheduler
        .last_continuation_decision
        .as_ref()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor continuation for node '{}' has no recorded continuation decision",
                node_id
            ),
        })?;

    if decision.disposition == Prototype1ContinuationDisposition::ContinueReady
        && decision.selected_next_branch_id.as_deref() == Some(node.branch_id.as_str())
    {
        return Ok(());
    }

    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "successor continuation rejected for node '{}' with disposition {:?} selected_next_branch_id={:?} node_branch_id={} (next_generation={}, total_nodes_after_continue={})",
            node_id,
            decision.disposition,
            decision.selected_next_branch_id,
            node.branch_id,
            decision.next_generation,
            decision.total_nodes_after_continue
        ),
    })
}

fn build_prototype1_active_successor_binary(repo_root: &Path) -> Result<PathBuf, PrepareError> {
    let output = ProcessCommand::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("ploke-eval")
        .arg("--bin")
        .arg("ploke-eval")
        .current_dir(repo_root)
        .output()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_build",
            detail: source.to_string(),
        })?;

    if !output.status.success() {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_successor_build",
            detail: format!(
                "successor build failed (exit_code={:?}, stdout={:?}, stderr={:?})",
                output.status.code(),
                process_output_excerpt(&output.stdout),
                process_output_excerpt(&output.stderr)
            ),
        });
    }

    let binary_path = repo_root
        .join("target")
        .join("debug")
        .join(format!("ploke-eval{}", std::env::consts::EXE_SUFFIX));
    if !binary_path.is_file() {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_successor_build",
            detail: format!(
                "successor build completed but '{}' was not found",
                binary_path.display()
            ),
        });
    }
    Ok(binary_path)
}

fn prepare_prototype1_active_successor_runtime(
    campaign_id: &str,
    manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    active_parent_root: &Path,
) -> Result<PathBuf, PrepareError> {
    validate_prototype1_successor_node_continuation(manifest_path, &node.node_id)?;
    let _ = select_treatment_branch(campaign_id, manifest_path, &node.branch_id)?;
    let resolved = resolve_treatment_branch(campaign_id, manifest_path, &node.branch_id)?;
    install_prototype1_successor_artifact(campaign_id, active_parent_root, node, &resolved)?;
    build_prototype1_active_successor_binary(active_parent_root)
}

fn install_prototype1_successor_artifact(
    campaign_id: &str,
    active_parent_root: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    resolved: &crate::intervention::ResolvedTreatmentBranch,
) -> Result<(), PrepareError> {
    let backend = GitWorktreeBackend;
    let workspace = backend
        .workspace_for_node(&node.node_id, &node.node_dir, &node.workspace_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_prepare",
            detail: source.to_string(),
        })?;
    let previous_parent = load_parent_identity_optional(active_parent_root)?;

    if node.workspace_root.exists() {
        let message = format!(
            "prototype1: persist successor artifact for node {}",
            node.node_id
        );
        let _ = backend
            .persist_workspace_target(&workspace, &resolved.target_relpath, &message)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_artifact_commit",
                detail: source.to_string(),
            })?;
        let identity = ParentIdentity::from_node(
            campaign_id.to_string(),
            node,
            previous_parent.as_ref(),
            Some(workspace.branch.0.clone()),
        );
        let _ = write_parent_identity(&workspace.root, &identity)?;
        let identity_message = parent_identity_commit_message(&identity);
        let _ = backend
            .persist_workspace_files(&workspace, &[parent_identity_relpath()], &identity_message)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_parent_identity_commit",
                detail: source.to_string(),
            })?;
        backend
            .remove(active_parent_root, &workspace)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_worktree_cleanup",
                detail: source.to_string(),
            })?;
        cleanup_prototype1_child_build_products(node)?;
    }

    backend
        .verify_artifact_target(
            active_parent_root,
            &workspace.branch,
            &resolved.target_relpath,
            &resolved.branch.proposed_content,
        )
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_verify",
            detail: source.to_string(),
        })?;
    append_prototype1_journal_entry(
        &campaign_manifest_path(campaign_id)?,
        JournalEntry::Successor(SuccessorRecord::checkout(
            campaign_id.to_string(),
            node.node_id.clone(),
            CommitPhase::Before,
            active_parent_root.to_path_buf(),
            workspace.branch.0.clone(),
            None,
        )),
        "prototype1_successor_checkout_before_journal",
    )?;
    let installed_commit = backend
        .install_artifact_in_active_checkout(active_parent_root, &workspace.branch)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_checkout_switch",
            detail: source.to_string(),
        })?;
    let identity =
        crate::cli::prototype1_state::identity::load_parent_identity(active_parent_root)?;
    identity.validate_for_command(campaign_id, Some(&node.node_id))?;
    backend
        .validate_parent_checkout(active_parent_root, &identity)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_parent_checkout",
            detail: source.to_string(),
        })?;
    append_prototype1_journal_entry(
        &campaign_manifest_path(campaign_id)?,
        JournalEntry::Successor(SuccessorRecord::checkout(
            campaign_id.to_string(),
            node.node_id.clone(),
            CommitPhase::After,
            active_parent_root.to_path_buf(),
            workspace.branch.0.clone(),
            Some(installed_commit.0.clone()),
        )),
        "prototype1_successor_checkout_after_journal",
    )?;
    append_prototype1_journal_entry(
        &campaign_manifest_path(campaign_id)?,
        JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: campaign_id.to_string(),
            previous_parent_identity: previous_parent,
            selected_parent_identity: identity,
            active_parent_root: active_parent_root.to_path_buf(),
            selected_branch: workspace.branch.0.clone(),
            installed_commit: installed_commit.0,
        }),
        "prototype1_successor_checkout_journal",
    )?;
    Ok(())
}

fn ensure_node_child_path(node_dir: &Path, path: &Path) -> Result<(), PrepareError> {
    if path.starts_with(node_dir) {
        return Ok(());
    }
    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "refusing to cleanup path '{}' outside node dir '{}'",
            path.display(),
            node_dir.display()
        ),
    })
}

fn cleanup_prototype1_child_build_products(
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<(), PrepareError> {
    ensure_node_child_path(&node.node_dir, &node.binary_path)?;
    match fs::remove_file(&node.binary_path) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(PrepareError::WriteManifest {
                path: node.binary_path.clone(),
                source,
            });
        }
    }

    let target_dir = node.node_dir.join("target");
    ensure_node_child_path(&node.node_dir, &target_dir)?;
    match fs::remove_dir_all(&target_dir) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(PrepareError::WriteManifest {
                path: target_dir,
                source,
            });
        }
    }
    Ok(())
}

fn cleanup_prototype1_child_workspace(
    active_parent_root: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<(), PrepareError> {
    let backend = GitWorktreeBackend;
    let workspace = backend
        .workspace_for_node(&node.node_id, &node.node_dir, &node.workspace_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_cleanup_prepare",
            detail: source.to_string(),
        })?;
    backend
        .remove(active_parent_root, &workspace)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_worktree_cleanup",
            detail: source.to_string(),
        })?;
    cleanup_prototype1_child_build_products(node)
}

pub(crate) fn persist_prototype1_buildable_child_artifact(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    active_parent_root: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<(), PrepareError> {
    let backend = GitWorktreeBackend;
    let workspace = backend
        .workspace_for_node(&node.node_id, &node.node_dir, &node.workspace_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_artifact_prepare",
            detail: source.to_string(),
        })?;
    let message = format!(
        "prototype1: persist buildable artifact for node {}",
        node.node_id
    );
    let target_commit = backend
        .persist_workspace_target(&workspace, &node.target_relpath, &message)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_artifact_commit",
            detail: source.to_string(),
        })?;
    let previous_parent = load_parent_identity_optional(active_parent_root)?;
    let identity = ParentIdentity::from_node(
        campaign_id.to_string(),
        node,
        previous_parent.as_ref(),
        Some(workspace.branch.0.clone()),
    );
    let _ = write_parent_identity(&workspace.root, &identity)?;
    let identity_message = parent_identity_commit_message(&identity);
    let identity_commit = backend
        .persist_workspace_files(&workspace, &[parent_identity_relpath()], &identity_message)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_identity_commit",
            detail: source.to_string(),
        })?;
    let resolved = resolve_treatment_branch(campaign_id, campaign_manifest_path, &node.branch_id)?;
    backend
        .verify_artifact_target(
            active_parent_root,
            &workspace.branch,
            &node.target_relpath,
            &resolved.branch.proposed_content,
        )
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_artifact_verify",
            detail: source.to_string(),
        })?;
    append_prototype1_journal_entry(
        campaign_manifest_path,
        JournalEntry::ChildArtifactCommitted(ChildArtifactCommittedEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: campaign_id.to_string(),
            parent_identity: previous_parent,
            child_identity: identity,
            node_id: node.node_id.clone(),
            generation: node.generation,
            target_relpath: node.target_relpath.clone(),
            child_branch: workspace.branch.0.clone(),
            target_commit: target_commit.0,
            identity_commit: identity_commit.0,
        }),
        "prototype1_child_artifact_journal",
    )?;
    Ok(())
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
    streams: &Streams,
) -> Result<std::process::Child, PrepareError> {
    crate::cli::prototype1_state::invocation::write_successor_invocation(
        invocation_path,
        invocation,
    )?;
    let child_argv = invocation.launch_args(invocation_path)?;
    let (stdout, stderr) = open_runtime_streams(streams)?;
    let mut command = ProcessCommand::new(binary_path);
    command
        .args(&child_argv)
        .current_dir(repo_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout))
        .stderr(std::process::Stdio::from(stderr));
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

fn runtime_streams(
    node_dir: &Path,
    runtime_id: crate::cli::prototype1_state::event::RuntimeId,
) -> Streams {
    let dir = node_dir.join("streams").join(runtime_id.to_string());
    Streams {
        stdout: dir.join("stdout.log"),
        stderr: dir.join("stderr.log"),
    }
}

fn open_runtime_streams(streams: &Streams) -> Result<(std::fs::File, std::fs::File), PrepareError> {
    let dir = streams
        .stdout
        .parent()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!("invalid stdout stream path '{}'", streams.stdout.display()),
        })?;
    std::fs::create_dir_all(dir).map_err(|source| PrepareError::WriteManifest {
        path: dir.to_path_buf(),
        source,
    })?;
    let stdout =
        std::fs::File::create(&streams.stdout).map_err(|source| PrepareError::WriteManifest {
            path: streams.stdout.clone(),
            source,
        })?;
    let stderr =
        std::fs::File::create(&streams.stderr).map_err(|source| PrepareError::WriteManifest {
            path: streams.stderr.clone(),
            source,
        })?;
    Ok((stdout, stderr))
}

enum SuccessorWait {
    Ready,
    TimedOut { waited_ms: u64 },
    ExitedBeforeReady { exit_code: Option<i32> },
}

fn wait_for_prototype1_successor_ready(
    child: &mut std::process::Child,
    ready_path: &Path,
) -> Result<SuccessorWait, PrepareError> {
    let start = std::time::Instant::now();
    loop {
        if ready_path.exists() {
            let ready =
                crate::cli::prototype1_state::invocation::load_successor_ready_record(ready_path)?;
            let _ = ready;
            return Ok(SuccessorWait::Ready);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_poll",
                detail: source.to_string(),
            })?
        {
            return Ok(SuccessorWait::ExitedBeforeReady {
                exit_code: status.code(),
            });
        }
        if start.elapsed() >= SUCCESSOR_READY_TIMEOUT {
            let waited_ms = start.elapsed().as_millis() as u64;
            let _ = child.kill();
            let _ = child.wait();
            return Ok(SuccessorWait::TimedOut { waited_ms });
        }
        std::thread::sleep(SUCCESSOR_READY_POLL);
    }
}

pub(crate) fn spawn_and_handoff_prototype1_successor(
    campaign_id: &str,
    node_id: &str,
    active_parent_root: &Path,
) -> Result<Option<Prototype1SuccessorHandoff>, PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let node = load_node_record(&manifest_path, node_id)?;
    let active_successor_binary_path = prepare_prototype1_active_successor_runtime(
        campaign_id,
        &manifest_path,
        &node,
        active_parent_root,
    )?;
    let runtime_id = crate::cli::prototype1_state::event::RuntimeId::new();
    let invocation_path =
        crate::cli::prototype1_state::invocation::invocation_path(&node.node_dir, runtime_id);
    let invocation = crate::cli::prototype1_state::invocation::SuccessorInvocation::new(
        campaign_id.to_string(),
        node.node_id.clone(),
        runtime_id,
        prototype1_transition_journal_path(&manifest_path),
        active_parent_root.to_path_buf(),
    );
    let ready_path =
        crate::cli::prototype1_state::invocation::successor_ready_path(&node.node_dir, runtime_id);
    let streams = runtime_streams(&node.node_dir, runtime_id);
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
        active_successor_binary_path = %active_successor_binary_path.display(),
        active_parent_root = %active_parent_root.display(),
        invocation_path = %invocation_path.display(),
        ready_path = %ready_path.display(),
        stdout = %streams.stdout.display(),
        stderr = %streams.stderr.display(),
        "spawning detached prototype1 successor from active checkout"
    );

    let mut child = spawn_prototype1_successor(
        &active_successor_binary_path,
        active_parent_root,
        &invocation_path,
        &invocation,
        &streams,
    )?;
    let pid = child.id();
    append_successor_record(
        invocation.journal_path(),
        SuccessorRecord::spawned(
            &invocation,
            pid,
            active_parent_root.to_path_buf(),
            active_successor_binary_path.clone(),
            invocation_path.clone(),
            ready_path.clone(),
            streams.clone(),
        ),
        "prototype1_successor_start_journal",
    )?;
    match wait_for_prototype1_successor_ready(&mut child, &ready_path)? {
        SuccessorWait::Ready => {
            append_prototype1_journal_entry(
                &manifest_path,
                JournalEntry::SuccessorHandoff(SuccessorHandoffEntry {
                    recorded_at: RecordedAt::now(),
                    campaign_id: campaign_id.to_string(),
                    node_id: node.node_id.clone(),
                    runtime_id,
                    active_parent_root: active_parent_root.to_path_buf(),
                    binary_path: active_successor_binary_path,
                    invocation_path,
                    ready_path: ready_path.clone(),
                    streams: Some(streams),
                    pid,
                }),
                "prototype1_successor_handoff_journal",
            )?;
            Ok(Some(Prototype1SuccessorHandoff {
                runtime_id,
                pid,
                ready_path,
            }))
        }
        SuccessorWait::TimedOut { waited_ms } => {
            append_successor_record(
                invocation.journal_path(),
                SuccessorRecord::timed_out(&invocation, waited_ms, ready_path),
                "prototype1_successor_timeout_journal",
            )?;
            Ok(None)
        }
        SuccessorWait::ExitedBeforeReady { exit_code } => {
            append_successor_record(
                invocation.journal_path(),
                SuccessorRecord::exited_before_ready(&invocation, exit_code),
                "prototype1_successor_exit_journal",
            )?;
            Err(PrepareError::DatabaseSetup {
                phase: "prototype1_successor_ready",
                detail: format!(
                    "successor exited before acknowledging handoff (exit_code={exit_code:?})"
                ),
            })
        }
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
/// not pollute the repo-level `target/`. The executable copied to `node/bin/`
/// is a temporary child-evaluation launch artifact, not durable runtime
/// identity; cleanup removes it once the evaluation result has been recorded.
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
    let child = record_prototype1_child_ready(
        invocation.campaign_id(),
        &manifest_path,
        &node,
        &request.workspace_root,
        invocation.runtime_id(),
        invocation.journal_path(),
    )?
    .evaluating()
    .map_err(|err| PrepareError::DatabaseSetup {
        phase: "prototype1_child_evaluating",
        detail: err.to_string(),
    })?;

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
    let runner_result_path = crate::cli::prototype1_state::invocation::result_path(
        &node.node_dir,
        invocation.runtime_id(),
    );
    let result = record_attempt_runner_result(
        invocation.campaign_id(),
        &manifest_path,
        &node,
        invocation.runtime_id(),
        result,
    )?;
    let _child =
        child
            .result_written(runner_result_path)
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "prototype1_child_result_written",
                detail: err.to_string(),
            })?;
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

/// Parent-side staged execution path for one treatment branch.
///
/// This is the main controller-facing process helper. It:
/// 1. registers or reloads the node
/// 2. materializes the branch into a temporary node workspace
/// 3. builds a fresh child runtime from that workspace
/// 4. spawns exactly one child runner process and waits for it
/// 5. reads back `runner-result.json`
/// 6. loads the branch-evaluation artifact on success
/// 7. removes the temporary child worktree and node-local build products
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
        None,
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
            cleanup_prototype1_child_workspace(repo_root, &node)?;
            return Ok(Prototype1NodeExecutionOutcome::Failed(result));
        }
        Prototype1NodeBuildOutcome::Built => {}
    }
    persist_prototype1_buildable_child_artifact(
        baseline_campaign_id,
        &baseline_manifest_path,
        repo_root,
        &node,
    )?;

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
        cleanup_prototype1_child_workspace(repo_root, &node)?;
        return Ok(Prototype1NodeExecutionOutcome::Failed(runner_result));
    }

    let outcome = (|| {
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
        Ok(Prototype1NodeExecutionOutcome::Evaluated(report))
    })();
    cleanup_prototype1_child_workspace(repo_root, &node)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %baseline_campaign_id,
        branch_id = %branch_id,
        node_id = %node.node_id,
        "legacy prototype1 child runner succeeded"
    );
    outcome
}
