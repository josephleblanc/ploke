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
//! 3. The parent crosses a move-only handoff transition from
//!    `Parent<Selectable>` to `Parent<Retired>`, locking lineage authority
//!    before the successor process is spawned.
//! 4. The parent hydrates and launches the next Parent from that active
//!    checkout, not from the temporary child worktree.
//! 5. The successor handoff token lets that next Parent validate continuation
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
//! - The predecessor must not remain in a ruling-capable parent state after the
//!   successor runtime is executable. After the selected Artifact is installed,
//!   successor spawn crosses the parent into `Parent<Retired>`, leaving only a
//!   retired observer in the predecessor process.
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
use crate::cli::prototype1_state::invocation::SuccessorInvocation;
use crate::loop_graph::RuntimeId;
use ploke_core::EXECUTION_DEBUG_TARGET;
use std::process::Command as ProcessCommand;
use tracing::debug;

use super::*;
use crate::BranchDisposition;
use crate::cli::prototype1_state::backend::{GitWorktreeBackend, WorkspaceBackend};
use crate::cli::prototype1_state::channel::{Channel, Cursor, FileTransport, ToParent};
use crate::cli::prototype1_state::cli_facing::{
    Prototype1BranchEvaluationReport, build_prototype1_branch_evaluation_report,
    ensure_treatment_branch_materialized, prepare_prototype1_treatment_campaign,
    prototype1_branch_evaluation_path,
};
use crate::cli::prototype1_state::event::RecordedAt;
use crate::cli::prototype1_state::history::{
    ActorRef, ArtifactLocator, ArtifactRef, BlockStore, DraftEntry, Entry, EntryKind, EvidenceRef,
    FsBlockStore, GenesisAuthority, LineageId, LineageState, Observation, OpenBlock,
    OpeningAuthority, OperationalEnvironment, ParentIdentityRef, PredecessorAuthority,
    ProcedureRef, Proposal, Regime, SealBlock, StoreHead, SubjectRef, SuccessorRef,
    SurfaceCommitment, TreeKeyCommitment, TreeKeyHash,
};
use crate::cli::prototype1_state::identity::{
    ParentIdentity, load_parent_identity_optional, parent_identity_commit_message,
    parent_identity_relpath, write_parent_identity,
};
use crate::cli::prototype1_state::inner::LockCrown;
use crate::cli::prototype1_state::journal::{
    ActiveCheckoutAdvancedEntry, ChildArtifactCommittedEntry, JournalEntry, PrototypeJournal,
    Streams, SuccessorHandoffEntry, prototype1_transition_journal_path,
};
use crate::cli::prototype1_state::observe;
use crate::cli::prototype1_state::parent::{Parent, Retired, Selectable};
use crate::cli::prototype1_state::selection;
use crate::cli::prototype1_state::successor::Record as SuccessorRecord;
use crate::intervention::{
    CommitPhase, Prototype1NodeStatus, Prototype1RunnerDisposition, Prototype1RunnerResult,
    RecordStore, ResolvedTreatmentBranch, TreatmentBranchEvaluationSummary, project_node_status,
    project_resolved_treatment_branch_evaluation, prototype1_branch_registry_path,
    write_node_projection, write_runner_result_at,
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

/// Parent-observed result of one successor bootstrap attempt.
pub(crate) struct Prototype1SuccessorHandoff {
    pub runtime_id: RuntimeId,
    pub pid: u32,
    pub ready_path: PathBuf,
}

/// How a selected successor runtime is launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuccessorHandoffMode {
    /// Spawn successor as a child process and wait for its ready record.
    Detached,
    /// Demo-only: replace the current parent process with the successor.
    #[cfg(feature = "demo")]
    Exec,
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

fn channel_error_phase(
    phase: &'static str,
    error: crate::cli::prototype1_state::channel::ChannelError<
        crate::cli::prototype1_state::channel::FileTransportError,
    >,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase,
        detail: format!("{error:?}"),
    }
}

pub(crate) fn record_prototype1_successor_ready(
    invocation: &SuccessorInvocation,
) -> Result<crate::cli::prototype1_state::invocation::SuccessorReadyRecord, PrepareError> {
    let record = crate::cli::prototype1_state::invocation::SuccessorReadyRecord {
        schema_version: crate::cli::prototype1_state::invocation::SUCCESSOR_READY_SCHEMA_VERSION
            .to_string(),
        campaign_id: invocation.campaign_id().to_string(),
        node_id: invocation.node_id().to_string(),
        runtime_id: crate::cli::prototype1_state::invocation::record_runtime_id(
            invocation.runtime_id(),
        ),
        pid: std::process::id(),
        recorded_at: Utc::now().to_rfc3339(),
    };
    let ready_projection = invocation
        .channel_endpoints()
        .map(|endpoints| {
            let projection = endpoints.child_to_parent().path().to_path_buf();
            let channel = Channel::for_role(invocation, endpoints, FileTransport);
            channel
                .send_successor_ready(record.clone())
                .map_err(|err| channel_error_phase("prototype1_successor_channel_ready", err))?;
            Ok::<PathBuf, PrepareError>(projection)
        })
        .transpose()?
        .unwrap_or_else(|| invocation.journal_path().to_path_buf());
    append_successor_record(
        invocation.journal_path(),
        SuccessorRecord::ready(invocation, record.pid, ready_projection),
        "prototype1_successor_ready_journal",
    )?;
    Ok(record)
}

pub(crate) fn record_prototype1_successor_completion(
    invocation: &SuccessorInvocation,
    manifest_path: &Path,
    status: crate::cli::prototype1_state::invocation::SuccessorCompletionStatus,
    trace_path: Option<PathBuf>,
    detail: Option<String>,
) -> Result<crate::cli::prototype1_state::invocation::SuccessorCompletionRecord, PrepareError> {
    let _ = manifest_path;
    let record = crate::cli::prototype1_state::invocation::SuccessorCompletionRecord {
        schema_version:
            crate::cli::prototype1_state::invocation::SUCCESSOR_COMPLETION_SCHEMA_VERSION
                .to_string(),
        campaign_id: invocation.campaign_id().to_string(),
        node_id: invocation.node_id().to_string(),
        runtime_id: crate::cli::prototype1_state::invocation::record_runtime_id(
            invocation.runtime_id(),
        ),
        status,
        trace_path: trace_path.clone(),
        detail: detail.clone(),
        recorded_at: Utc::now().to_rfc3339(),
    };
    let completion_projection = invocation
        .channel_endpoints()
        .map(|endpoints| {
            let projection = endpoints.child_to_parent().path().to_path_buf();
            let channel = Channel::for_role(invocation, endpoints, FileTransport);
            channel
                .send_successor_completion(record.clone())
                .map_err(|err| {
                    channel_error_phase("prototype1_successor_channel_completion", err)
                })?;
            Ok::<PathBuf, PrepareError>(projection)
        })
        .transpose()?
        .unwrap_or_else(|| invocation.journal_path().to_path_buf());
    append_successor_record(
        invocation.journal_path(),
        SuccessorRecord::completed(
            invocation,
            status,
            completion_projection,
            trace_path,
            detail,
        ),
        "prototype1_successor_completion_journal",
    )?;
    Ok(record)
}

pub(crate) fn validate_prototype1_successor_continuation(
    invocation: &SuccessorInvocation,
    manifest_path: &Path,
) -> Result<(), PrepareError> {
    let store = FsBlockStore::for_campaign_manifest(manifest_path);
    let lineage_id = LineageId::new(invocation.campaign_id().to_string());
    let state = store
        .lineage_state(&lineage_id)
        .map_err(block_store_prepare_error)?;
    let head = match state.head() {
        StoreHead::Present(head) => head.clone(),
        StoreHead::Absent { .. } => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor continuation for node '{}' has no sealed History head",
                    invocation.node_id()
                ),
            });
        }
    };
    let sealed = store
        .sealed_head_block(&head)
        .map_err(block_store_prepare_error)?;
    let expected_runtime = ActorRef::Runtime(invocation.runtime_id());
    if sealed.selected_successor().runtime() != &expected_runtime {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor continuation runtime mismatch: invocation={} sealed={:?}",
                invocation.runtime_id(),
                sealed.selected_successor().runtime()
            ),
        });
    }
    if sealed.selected_successor().artifact() != sealed.active_artifact() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor continuation selected Artifact '{}' does not match sealed active Artifact '{}'",
                sealed.selected_successor().artifact().as_str(),
                sealed.active_artifact().as_str()
            ),
        });
    }
    Ok(())
}

pub(crate) fn validate_child_surface(
    active_parent_root: &Path,
    child_root: &Path,
    phase: &'static str,
) -> Result<SurfaceCommitment, PrepareError> {
    GitWorktreeBackend
        .surface_commitment(active_parent_root, child_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase,
            detail: source.to_string(),
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
    _manifest_path: &Path,
    selected: &selection::Selection<selection::Artifact>,
    active_parent_root: &Path,
) -> Result<(PathBuf, SurfaceCommitment), PrepareError> {
    let surface = install_prototype1_successor_artifact(campaign_id, active_parent_root, selected)?;
    let binary = build_prototype1_active_successor_binary(active_parent_root)?;
    Ok((binary, surface))
}

fn install_prototype1_successor_artifact(
    campaign_id: &str,
    active_parent_root: &Path,
    selected: &selection::Selection<selection::Artifact>,
) -> Result<SurfaceCommitment, PrepareError> {
    let backend = GitWorktreeBackend;
    let artifact = selected.selected();
    let node = artifact.node();
    let resolved = artifact.resolved();
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
        let surface = backend
            .surface_commitment(active_parent_root, &workspace.root)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_surface_commitment",
                detail: source.to_string(),
            })?;
        backend
            .remove(active_parent_root, &workspace)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_worktree_cleanup",
                detail: source.to_string(),
            })?;
        let manifest_path = campaign_manifest_path(campaign_id)?;
        cleanup_prototype1_child_build_products(&manifest_path, campaign_id, node)?;
        install_committed_successor_artifact(
            campaign_id,
            active_parent_root,
            selected,
            workspace,
            surface,
            previous_parent,
        )
    } else {
        let surface = backend
            .surface_commitment(active_parent_root, &active_parent_root)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_surface_commitment_reuse",
                detail: source.to_string(),
            })?;
        install_committed_successor_artifact(
            campaign_id,
            active_parent_root,
            selected,
            workspace,
            surface,
            previous_parent,
        )
    }
}

fn install_committed_successor_artifact(
    campaign_id: &str,
    active_parent_root: &Path,
    selected: &selection::Selection<selection::Artifact>,
    workspace: crate::cli::prototype1_state::backend::Workspace,
    surface: SurfaceCommitment,
    previous_parent: Option<ParentIdentity>,
) -> Result<SurfaceCommitment, PrepareError> {
    let backend = GitWorktreeBackend;
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let artifact = selected.selected();
    let node = artifact.node();
    let resolved = artifact.resolved();
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
        &manifest_path,
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
    let checkout_step = observe::Step::start(observe::span!(
        "prototype1.parent.checkout.active",
        campaign_id = %campaign_id,
        node_id = %node.node_id,
        generation = node.generation,
        active_parent_root = %active_parent_root.display(),
        selected_branch = %workspace.branch.0,
        target_relpath = %resolved.target_relpath.display(),
    ));
    let installed_commit =
        match backend.install_artifact_in_active_checkout(active_parent_root, &workspace.branch) {
            Ok(installed_commit) => {
                checkout_step.success();
                installed_commit
            }
            Err(source) => {
                let error = PrepareError::DatabaseSetup {
                    phase: "prototype1_successor_checkout_switch",
                    detail: source.to_string(),
                };
                checkout_step.fail("prototype1_successor_checkout_switch", &error);
                return Err(error);
            }
        };
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
        &manifest_path,
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
        &manifest_path,
        JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: campaign_id.to_string(),
            previous_parent_identity: previous_parent,
            selected_parent_identity: identity.clone(),
            active_parent_root: active_parent_root.to_path_buf(),
            selected_branch: workspace.branch.0.clone(),
            installed_commit: installed_commit.0.clone(),
        }),
        "prototype1_successor_checkout_journal",
    )?;
    observe::Step::start(observe::span!(
        "prototype1.parent.checkout.advanced",
        campaign_id = %campaign_id,
        selected_parent_id = %identity.parent_id(),
        selected_node_id = %identity.node_id(),
        selected_generation = identity.generation(),
        selected_branch = %workspace.branch.0,
        installed_commit = %installed_commit.0,
        active_parent_root = %active_parent_root.display(),
    ))
    .success();
    Ok(surface)
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

pub(crate) fn cleanup_prototype1_child_build_products(
    manifest_path: &Path,
    campaign_id: &str,
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<(), PrepareError> {
    ensure_node_child_path(&node.node_dir, &node.binary_path)?;
    match fs::remove_file(&node.binary_path) {
        Ok(()) => observe::Step::start(observe::span!(
            "prototype1.cleanup.binary",
            campaign_id = %campaign_id,
            node_id = %node.node_id,
            generation = node.generation,
            manifest_path = %manifest_path.display(),
            path = %node.binary_path.display(),
        ))
        .removed(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.binary",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %node.binary_path.display(),
            ))
            .missing()
        }
        Err(source) => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.binary",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %node.binary_path.display(),
            ))
            .fail("child_binary_remove", &source);
            return Err(PrepareError::WriteManifest {
                path: node.binary_path.clone(),
                source,
            });
        }
    }

    let target_dir = node.node_dir.join("target");
    remove_node_target(manifest_path, campaign_id, node, &target_dir)
}

fn remove_node_target(
    manifest_path: &Path,
    campaign_id: &str,
    node: &crate::intervention::Prototype1NodeRecord,
    target_dir: &Path,
) -> Result<(), PrepareError> {
    ensure_node_child_path(&node.node_dir, &target_dir)?;
    match fs::remove_dir_all(&target_dir) {
        Ok(()) => observe::Step::start(observe::span!(
            "prototype1.cleanup.target",
            campaign_id = %campaign_id,
            node_id = %node.node_id,
            generation = node.generation,
            manifest_path = %manifest_path.display(),
            path = %target_dir.display(),
        ))
        .removed(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.target",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %target_dir.display(),
            ))
            .missing()
        }
        Err(source) => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.target",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %target_dir.display(),
            ))
            .fail("node_target_remove", &source);
            return Err(PrepareError::WriteManifest {
                path: target_dir.to_path_buf(),
                source,
            });
        }
    }
    Ok(())
}

pub(crate) fn persist_prototype1_buildable_child_artifact(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    active_parent_root: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    resolved: &ResolvedTreatmentBranch,
) -> Result<SurfaceCommitment, PrepareError> {
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
    let surface = validate_child_surface(
        active_parent_root,
        &workspace.root,
        "prototype1_child_surface_commitment_after_persist",
    )?;
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
    Ok(surface)
}

fn spawn_prototype1_successor(
    binary_path: &Path,
    repo_root: &Path,
    invocation_path: &Path,
    invocation: &SuccessorInvocation,
    retired_parent: &Parent<Retired>,
    streams: &Streams,
) -> Result<std::process::Child, PrepareError> {
    crate::cli::prototype1_state::invocation::write_successor_invocation_for_retired_parent(
        retired_parent,
        invocation_path,
        invocation,
    )?;
    let child_argv = invocation.launch_args_for_retired_parent(retired_parent, invocation_path)?;
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

#[cfg(feature = "demo")]
fn exec_prototype1_successor(
    binary_path: &Path,
    repo_root: &Path,
    invocation_path: &Path,
    invocation: &SuccessorInvocation,
    retired_parent: &Parent<Retired>,
) -> Result<(), PrepareError> {
    let child_argv = invocation.launch_args_for_retired_parent(retired_parent, invocation_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        let mut command = ProcessCommand::new(binary_path);
        command
            .args(&child_argv)
            .current_dir(repo_root)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        let source = command.exec();
        Err(PrepareError::DatabaseSetup {
            phase: "prototype1_successor_exec",
            detail: source.to_string(),
        })
    }

    #[cfg(not(unix))]
    {
        let _ = (binary_path, repo_root, child_argv);
        Err(PrepareError::DatabaseSetup {
            phase: "prototype1_successor_exec",
            detail: "demo exec handoff is only supported on Unix".to_string(),
        })
    }
}

fn runtime_streams(node_dir: &Path, runtime_id: RuntimeId) -> Streams {
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
    channel: Option<&Channel<Parent<Retired>, FileTransport>>,
) -> Result<SuccessorWait, PrepareError> {
    let start = std::time::Instant::now();
    let mut cursor = Cursor::start();
    loop {
        if let Some(channel) = channel {
            let (next_cursor, messages) =
                channel
                    .recv_from_child(cursor)
                    .map_err(|err| PrepareError::DatabaseSetup {
                        phase: "prototype1_successor_channel_ready",
                        detail: format!("{err:?}"),
                    })?;
            cursor = next_cursor;
            if messages
                .iter()
                .any(|message| matches!(message.body(), ToParent::SuccessorReady { .. }))
            {
                return Ok(SuccessorWait::Ready);
            }
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
    selected: selection::Selection<selection::Artifact>,
    active_parent_root: &Path,
    parent: Parent<Selectable>,
    selection_entry: crate::cli::prototype1_state::history::SelectionDecisionEntry,
    mode: SuccessorHandoffMode,
) -> Result<(Parent<Retired>, Option<Prototype1SuccessorHandoff>), PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let artifact = selected.selected();
    let node = artifact.node();
    let (active_successor_binary_path, surface) = prepare_prototype1_active_successor_runtime(
        campaign_id,
        &manifest_path,
        &selected,
        active_parent_root,
    )?;
    let runtime_id = RuntimeId::new();
    let successor_artifact = artifact.artifact_ref().clone();
    let parent_actor = parent_actor_ref(parent.identity());
    let handoff_block = handoff_block_fields(
        campaign_id,
        active_parent_root,
        parent.identity(),
        &manifest_path,
        successor_artifact.clone(),
        surface,
    )?;
    let seal = SealBlock::from_handoff(
        EvidenceRef::new(format!("prototype1:successor-handoff:{runtime_id}")),
        SuccessorRef::new(ActorRef::Runtime(runtime_id), successor_artifact.clone()),
        successor_artifact.clone(),
        RecordedAt::now(),
    );
    let HandoffBlock {
        open,
        expected_state,
        artifact_key,
    } = handoff_block;
    let predecessor_block_hash = expected_state.head().block_hash().copied();
    let seal_step = observe::Step::start(observe::span!(
        "prototype1.history.seal",
        campaign_id = %campaign_id,
        node_id = %node.node_id,
        generation = node.generation,
        runtime_id = %runtime_id,
        predecessor_block_hash = ?predecessor_block_hash,
    ));
    let (retired_parent, sealed_block) =
        match parent.seal_block_with_artifact(open, seal, |crown, block| {
            let environment = OperationalEnvironment::new()
                .artifact(successor_artifact.clone())
                .binary(EvidenceRef::new(format!(
                    "path:{}",
                    active_successor_binary_path.display()
                )))
                .procedure_version(ProcedureRef::new("prototype1:successor-handoff:v1"))
                .recorder(EvidenceRef::new("prototype1:history-block"));
            let (_, artifact_claim) = crown
                .admit_claim(
                    &ArtifactLocator,
                    artifact_key,
                    parent_actor.clone(),
                    environment,
                    ProcedureRef::new("prototype1:single-ruler-local:v1"),
                    RecordedAt::now(),
                )
                .map_err(|source| source.into_history_error())?;

            let selection_payload_hash = selection_entry.decision_hash()?;
            let selection_environment = OperationalEnvironment::new()
                .artifact(successor_artifact.clone())
                .procedure_version(ProcedureRef::new("prototype1:successor-selection:v1"))
                .recorder(EvidenceRef::new("prototype1:history-entry"));
            let selection_observation = Observation {
                observer: parent_actor.clone(),
                recorder: parent_actor.clone(),
                operational_environment: selection_environment,
                payload_ref: EvidenceRef::new("inline:selection-decision"),
                payload_hash: selection_payload_hash,
                observed_at: RecordedAt::now(),
                recorded_at: RecordedAt::now(),
            };
            let selection_entry = Entry::draft_selection_decision(
                DraftEntry {
                    entry_kind: EntryKind::Decision,
                    subject: SubjectRef::new(format!(
                        "successor-selection:generation:{}",
                        node.generation
                    )),
                    executor: parent_actor.clone(),
                    input_refs: Vec::new(),
                    output_refs: vec![EvidenceRef::new(format!(
                        "successor:selected:{}",
                        node.node_id
                    ))],
                    occurred_at: RecordedAt::now(),
                },
                selection_entry,
            )
            .observe(selection_observation)
            .propose(Proposal {
                proposer: parent_actor.clone(),
                procedure_or_policy: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            });
            let _ = crown.admit_entry(block, selection_entry, parent_actor.clone())?;
            Ok(artifact_claim)
        }) {
            Ok(result) => {
                seal_step.success();
                result
            }
            Err(source) => {
                let error = history_prepare_error(source);
                seal_step.fail("history_seal", &error);
                return Err(error);
            }
        };
    let history_store = FsBlockStore::for_campaign_manifest(&manifest_path);
    let append_step = observe::Step::start(observe::span!(
        "prototype1.history.append",
        campaign_id = %campaign_id,
        node_id = %node.node_id,
        generation = node.generation,
        runtime_id = %runtime_id,
        predecessor_block_hash = ?predecessor_block_hash,
    ));
    let stored_block = match history_store.append(&expected_state, &sealed_block) {
        Ok(stored_block) => {
            append_step.success();
            stored_block
        }
        Err(source) => {
            let error = block_store_prepare_error(source);
            append_step.fail("history_append", &error);
            return Err(error);
        }
    };
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node.node_id,
        block_height = stored_block.block_height(),
        block_hash = %stored_block.block_hash(),
        "prototype1 History block sealed before successor runtime spawn"
    );
    let invocation_path =
        crate::cli::prototype1_state::invocation::invocation_path(&node.node_dir, runtime_id);
    let invocation = SuccessorInvocation::from_retired_parent(
        &retired_parent,
        campaign_id.to_string(),
        node.node_id.clone(),
        runtime_id,
        prototype1_transition_journal_path(&manifest_path),
        active_parent_root.to_path_buf(),
    );
    let successor_channel_endpoints = invocation.channel_endpoints();
    let ready_path = successor_channel_endpoints
        .as_ref()
        .map(|endpoints| endpoints.child_to_parent().path().to_path_buf())
        .unwrap_or_else(|| invocation.journal_path().to_path_buf());
    let successor_channel = successor_channel_endpoints
        .clone()
        .map(|endpoints| Channel::for_parent(&retired_parent, endpoints, FileTransport));
    let streams = runtime_streams(&node.node_dir, runtime_id);

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        node_id = %node.node_id,
        runtime_id = %runtime_id,
        selected_candidate = %artifact.candidate().as_str(),
        selection_source = ?artifact.source(),
        selected_primary_runtime_id = ?artifact.primary_runtime_id(),
        active_successor_binary_path = %active_successor_binary_path.display(),
        active_parent_root = %active_parent_root.display(),
        invocation_path = %invocation_path.display(),
        ready_path = %ready_path.display(),
        stdout = %streams.stdout.display(),
        stderr = %streams.stderr.display(),
        "spawning detached prototype1 successor from active checkout"
    );

    let spawn_step = observe::Step::start(observe::span!(
        "prototype1.successor.spawn",
        campaign_id = %campaign_id,
        node_id = %node.node_id,
        generation = node.generation,
        runtime_id = %runtime_id,
        active_parent_root = %active_parent_root.display(),
        binary_path = %active_successor_binary_path.display(),
        invocation_path = %invocation_path.display(),
        ready_path = %ready_path.display(),
    ));

    #[cfg(feature = "demo")]
    if mode == SuccessorHandoffMode::Exec {
        crate::cli::prototype1_state::invocation::write_successor_invocation_for_retired_parent(
            &retired_parent,
            &invocation_path,
            &invocation,
        )?;
        let pid = std::process::id();
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
            "prototype1_successor_exec_journal",
        )?;
        append_prototype1_journal_entry(
            &manifest_path,
            JournalEntry::SuccessorHandoff(SuccessorHandoffEntry {
                recorded_at: RecordedAt::now(),
                campaign_id: campaign_id.to_string(),
                node_id: node.node_id.clone(),
                runtime_id,
                active_parent_root: active_parent_root.to_path_buf(),
                binary_path: active_successor_binary_path.clone(),
                invocation_path: invocation_path.clone(),
                ready_path: ready_path.clone(),
                streams: Some(streams.clone()),
                pid,
            }),
            "prototype1_successor_exec_handoff_journal",
        )?;
        spawn_step.success();
        debug!(
            target: EXECUTION_DEBUG_TARGET,
            campaign = %campaign_id,
            node_id = %node.node_id,
            runtime_id = %runtime_id,
            pid,
            "demo exec handoff replacing parent process with successor"
        );
        exec_prototype1_successor(
            &active_successor_binary_path,
            active_parent_root,
            &invocation_path,
            &invocation,
            &retired_parent,
        )?;
        unreachable!("successful exec replaces the current process");
    }

    let _ = mode;
    let mut child = match spawn_prototype1_successor(
        &active_successor_binary_path,
        active_parent_root,
        &invocation_path,
        &invocation,
        &retired_parent,
        &streams,
    ) {
        Ok(child) => {
            spawn_step.success();
            child
        }
        Err(error) => {
            spawn_step.fail("successor_spawn", &error);
            return Err(error);
        }
    };
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
    let ready_step = observe::Step::start(observe::span!(
        "prototype1.successor.ready_wait",
        campaign_id = %campaign_id,
        node_id = %node.node_id,
        generation = node.generation,
        runtime_id = %runtime_id,
        pid = pid,
        active_parent_root = %active_parent_root.display(),
        binary_path = %active_successor_binary_path.display(),
        invocation_path = %invocation_path.display(),
        ready_path = %ready_path.display(),
    ));
    match wait_for_prototype1_successor_ready(&mut child, successor_channel.as_ref())? {
        SuccessorWait::Ready => {
            ready_step.success();
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
            Ok((
                retired_parent,
                Some(Prototype1SuccessorHandoff {
                    runtime_id,
                    pid,
                    ready_path,
                }),
            ))
        }
        SuccessorWait::TimedOut { waited_ms } => {
            ready_step.timed_out();
            append_successor_record(
                invocation.journal_path(),
                SuccessorRecord::timed_out(&invocation, waited_ms, ready_path),
                "prototype1_successor_timeout_journal",
            )?;
            Ok((retired_parent, None))
        }
        SuccessorWait::ExitedBeforeReady { exit_code } => {
            ready_step.exited_before_ready();
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

struct HandoffBlock {
    open: OpenBlock,
    expected_state: LineageState,
    artifact_key: TreeKeyHash,
}

fn handoff_block_fields(
    campaign_id: &str,
    active_parent_root: &Path,
    parent_identity: &ParentIdentity,
    manifest_path: &Path,
    active_artifact: ArtifactRef,
    surface: SurfaceCommitment,
) -> Result<HandoffBlock, PrepareError> {
    let store = FsBlockStore::for_campaign_manifest(manifest_path);
    let lineage_id = LineageId::new(campaign_id.to_string());
    let state = store
        .lineage_state(&lineage_id)
        .map_err(block_store_prepare_error)?;
    let parent_actor = parent_actor_ref(parent_identity);
    let backend = GitWorktreeBackend;
    let artifact_key = backend
        .clean_tree_key(active_parent_root)
        .map_err(backend_prepare_error)?
        .tree_key_hash()
        .map_err(history_prepare_error)?;
    let (block_height, parent_block_hashes, opening_authority) = match state.head() {
        StoreHead::Present(head) => {
            let predecessor = *head.block_hash();
            (
                head.block_height() + 1,
                vec![predecessor],
                OpeningAuthority::Predecessor(PredecessorAuthority::new(predecessor)),
            )
        }
        StoreHead::Absent { .. } => (
            0,
            Vec::new(),
            OpeningAuthority::Genesis(GenesisAuthority::new(
                ProcedureRef::new("prototype1:bootstrap:single-ruler:v1"),
                artifact_key.clone(),
                ParentIdentityRef::new(EvidenceRef::new(format!(
                    "artifact-path:{}",
                    parent_identity_relpath().display()
                ))),
            )),
        ),
    };

    Ok(HandoffBlock {
        open: OpenBlock {
            lineage_id,
            block_height,
            parent_block_hashes,
            opened_from_state: state.root().clone(),
            regime: Regime::prototype1_baseline(block_height),
            opening_authority,
            opened_by: parent_actor.clone(),
            opened_from_artifact: active_artifact,
            ruling_authority: parent_actor,
            policy_ref: ProcedureRef::new("prototype1:single-ruler-local:v1"),
            surface,
            opened_at: RecordedAt::now(),
        },
        expected_state: state,
        artifact_key,
    })
}

fn parent_actor_ref(parent_identity: &ParentIdentity) -> ActorRef {
    ActorRef::Process(format!("parent:{}", parent_identity.parent_id()))
}

fn history_prepare_error(
    error: crate::cli::prototype1_state::history::HistoryError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_history",
        detail: error.to_string(),
    }
}

fn block_store_prepare_error(
    error: crate::cli::prototype1_state::history::BlockStoreError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_history_store",
        detail: error.to_string(),
    }
}

fn backend_prepare_error(
    error: crate::cli::prototype1_state::backend::BackendError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_history_tree_key",
        detail: error.to_string(),
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
    _campaign_id: &str,
    _campaign_manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    runtime_id: RuntimeId,
    result: Prototype1RunnerResult,
) -> Result<Prototype1RunnerResult, PrepareError> {
    let attempt_path =
        crate::cli::prototype1_state::invocation::result_path(&node.node_dir, runtime_id);
    let _ = write_runner_result_at(&attempt_path, &result)?;
    let _ = write_runner_result_at(&node.runner_result_path, &result)?;
    let node = project_node_status(node, result.status);
    write_node_projection(&node)?;
    Ok(result)
}

pub(super) async fn run_prototype1_resolved_branch_evaluation(
    baseline_campaign_id: &str,
    baseline_manifest_path: &Path,
    resolved_branch: &ResolvedTreatmentBranch,
    repo_root: &Path,
    stop_on_error: bool,
) -> Result<Prototype1BranchEvaluationReport, PrepareError> {
    let branch_id = resolved_branch.branch.branch_id.as_str();
    macro_rules! eval_span {
        ($name:literal, $phase:literal) => {
            observe::span!(
                $name,
                operation = "SelfEvaluation",
                phase = $phase,
                campaign_id = %baseline_campaign_id,
                branch_id = %branch_id
            )
        };
        ($name:literal, $phase:literal, $($fields:tt)+) => {
            observe::span!(
                $name,
                operation = "SelfEvaluation",
                phase = $phase,
                campaign_id = %baseline_campaign_id,
                branch_id = %branch_id,
                $($fields)+
            )
        };
    }
    macro_rules! step {
        ($name:literal, $phase:literal, $run:expr $(,)?) => {
            observe::result(eval_span!($name, $phase), $run)
        };
        ($name:literal, $phase:literal, $run:expr, $($fields:tt)+) => {
            observe::result(eval_span!($name, $phase, $($fields)+), $run)
        };
    }
    macro_rules! async_step {
        ($name:literal, $phase:literal, $future:expr $(,)?) => {
            observe::future(eval_span!($name, $phase), $future)
        };
        ($name:literal, $phase:literal, $future:expr, $($fields:tt)+) => {
            observe::future(eval_span!($name, $phase, $($fields)+), $future)
        };
    }

    let step = observe::Step::start(observe::span!(
        "prototype1.child.evaluate.run",
        operation = "SelfEvaluation",
        phase = "Run",
        campaign_id = %baseline_campaign_id,
        branch_id = %branch_id,
        repo_root = %repo_root.display(),
        stop_on_error,
    ));
    let outcome = async {
        let _run_scope = TimingTrace::scope(format!("loop.prototype1_branch.evaluate.{branch_id}"));
        let branch_registry_path = prototype1_branch_registry_path(&baseline_manifest_path);
        step!(
            "prototype1.child.evaluate.materialize",
            "Materialize",
            || {
                ensure_treatment_branch_materialized(
                    resolved_branch,
                    repo_root,
                )
            },
            repo_root = %repo_root.display(),
        )?;

        let baseline_resolved = step!(
            "prototype1.child.evaluate.resolve_baseline_campaign",
            "ResolveBaselineCampaign",
            || resolve_campaign_config(baseline_campaign_id, &CampaignOverrides::default()),
        )?;
        let baseline_state = step!(
            "prototype1.child.evaluate.load_baseline_state",
            "LoadBaselineState",
            || load_closure_state(baseline_campaign_id),
        )?;
        let treatment_campaign = step!(
            "prototype1.child.evaluate.prepare_treatment_campaign",
            "PrepareTreatmentCampaign",
            || prepare_prototype1_treatment_campaign(&baseline_resolved, branch_id),
        )?;
        let mut eval_policy = treatment_campaign.resolved.eval.clone();
        if stop_on_error {
            eval_policy.stop_on_error = true;
        }
        async_step!(
            "prototype1.child.evaluate.eval_closure",
            "EvalClosure",
            advance_eval_closure(&treatment_campaign.resolved, &eval_policy, false),
            treatment_campaign_id = %treatment_campaign.campaign_id,
            stop_on_error = eval_policy.stop_on_error,
        )
        .await?;

        let mut protocol_policy = treatment_campaign.resolved.protocol.clone();
        if stop_on_error {
            protocol_policy.stop_on_error = true;
        }
        async_step!(
            "prototype1.child.evaluate.protocol_closure",
            "ProtocolClosure",
            advance_protocol_closure(&treatment_campaign.resolved, &protocol_policy, false),
            treatment_campaign_id = %treatment_campaign.campaign_id,
            stop_on_error = protocol_policy.stop_on_error,
        )
        .await?;

        let treatment_state = step!(
            "prototype1.child.evaluate.load_treatment_state",
            "LoadTreatmentState",
            || load_closure_state(&treatment_campaign.campaign_id),
            treatment_campaign_id = %treatment_campaign.campaign_id,
        )?;
        let evaluation_artifact_path =
            prototype1_branch_evaluation_path(baseline_manifest_path, branch_id);
        let report = step!(
            "prototype1.child.evaluate.compare",
            "Compare",
            || {
                build_prototype1_branch_evaluation_report(
                    baseline_campaign_id,
                    branch_id,
                    &branch_registry_path,
                    &evaluation_artifact_path,
                    &treatment_campaign,
                    &baseline_state,
                    &treatment_state,
                )
            },
            treatment_campaign_id = %treatment_campaign.campaign_id,
            evaluation_artifact_path = %evaluation_artifact_path.display(),
        )?;
        step!(
            "prototype1.child.evaluate.persist_report",
            "PersistEvaluationReport",
            || write_json_file_pretty(&evaluation_artifact_path, &report),
            treatment_campaign_id = %treatment_campaign.campaign_id,
            evaluation_artifact_path = %evaluation_artifact_path.display(),
        )?;

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
        step!(
            "prototype1.child.evaluate.persist_summary",
            "PersistEvaluationSummary",
            || {
                project_resolved_treatment_branch_evaluation(
                    baseline_campaign_id,
                    baseline_manifest_path,
                    resolved_branch,
                    summary,
                )
            },
            treatment_campaign_id = %treatment_campaign.campaign_id,
        )?;

        Ok(report)
    }
    .await;

    match outcome {
        Ok(report) => {
            step.success();
            Ok(report)
        }
        Err(error) => {
            step.fail("self_evaluation", &error);
            Err(error)
        }
    }
}
