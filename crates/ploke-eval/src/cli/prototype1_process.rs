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
use chrono::Utc;
use ploke_core::EXECUTION_DEBUG_TARGET;
use std::process::Command as ProcessCommand;
use tracing::debug;

use super::*;
use crate::cli::prototype1_state::backend::{
    BackendError, GitBranch, GitWorktreeBackend, WorkspaceBackend,
};
use crate::cli::prototype1_state::channel::{Channel, Cursor, FileTransport, ToParent};
use crate::cli::prototype1_state::child::Child;
use crate::cli::prototype1_state::cli_facing::{
    Prototype1TreatmentEvidence, build_prototype1_treatment_evidence,
    ensure_treatment_branch_materialized, prepare_prototype1_treatment_campaign,
};
use crate::cli::prototype1_state::event::RecordedAt;
use crate::cli::prototype1_state::event::{Paths, Refs};
use crate::cli::prototype1_state::history::{
    ActorRef, ArtifactLocator, ArtifactRef, ArtifactSurface, BlockStore, DraftEntry, Entry,
    EntryKind, EvidenceRef, FsBlockStore, GenesisAuthority, LineageId, LineageState, Observation,
    OpenBlock, OpeningAuthority, OperationalEnvironment, ParentIdentityRef, PredecessorAuthority,
    ProcedureRef, Proposal, Regime, SealBlock, StoreHead, SubjectRef, SuccessorRef,
    SurfaceCommitment, TreeKeyHash,
};
use crate::cli::prototype1_state::identity::{
    ParentIdentity, parent_identity_commit_message, parent_identity_relpath, write_parent_identity,
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
use crate::inner::registry::RunRegistration;
use crate::intervention::{
    CommitPhase, Prototype1NodeStatus, Prototype1RunnerDisposition, Prototype1RunnerResult,
    RecordStore, ResolvedTreatmentBranch, project_node_status, write_node_projection,
    write_runner_result_at,
};
use crate::record::SubmissionArtifactState;
use ploke_records::evaluation::{BenchmarkPatchProjectionRecord, PatchProjectionCheckState};

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

#[cfg(test)]
mod tests {
    use ploke_records::ids::CampaignId;
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
    use crate::intervention::{
        PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeRecord, Prototype1NodeStatus,
    };
    use ploke_records::evaluation::{
        BENCHMARK_PATCH_PROJECTION_SCHEMA_V1, BenchmarkCheckoutRef, BenchmarkPatchProjectionRecord,
        MultiSweBenchTarget, PatchProjectionCheck, RunArtifactRef, SubmissionPatchRef,
    };

    fn test_node(root: &Path) -> Prototype1NodeRecord {
        let node_dir = root.join("nodes").join("node-1");
        Prototype1NodeRecord {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            node_id: "node-1".to_string(),
            parent_node_id: Some("parent-1".to_string()),
            generation: 1,
            instance_id: "BurntSushi__ripgrep-2209".to_string(),
            source_state_id: "source-1".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: Some("parent-branch".to_string()),
            branch_id: "branch-1".to_string(),
            candidate_id: "candidate-1".to_string(),
            target_relpath: PathBuf::from("src/lib.rs"),
            node_dir: node_dir.clone(),
            workspace_root: node_dir.join("worktree"),
            binary_path: node_dir.join("bin").join("ploke-eval"),
            runner_request_path: node_dir.join("runner-request.json"),
            runner_result_path: node_dir.join("runner-result.json"),
            status: Prototype1NodeStatus::BinaryBuilt,
            created_at: "2026-05-11T00:00:00Z".to_string(),
            updated_at: "2026-05-11T00:00:00Z".to_string(),
        }
    }

    fn resolved_branch_for(node: &Prototype1NodeRecord) -> ResolvedTreatmentBranch {
        ResolvedTreatmentBranch {
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            parent_branch_id: node.parent_branch_id.clone(),
            target_relpath: node.target_relpath.clone(),
            source_content: "pub fn before() {}\n".to_string(),
            source_content_hash: "source-hash".to_string(),
            selected_branch_id: Some(node.branch_id.clone()),
            branch: crate::intervention::TreatmentBranchNode {
                branch_id: node.branch_id.clone(),
                candidate_id: node.candidate_id.clone(),
                patch_id: None,
                branch_label: "candidate branch".to_string(),
                synthesized_spec_id: "spec-1".to_string(),
                proposed_content: "pub fn after() {}\n".to_string(),
                proposed_content_hash: "proposed-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: crate::intervention::TreatmentBranchStatus::Synthesized,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
            },
        }
    }

    fn live_node(root: &Path) -> Prototype1NodeRecord {
        let mut node = test_node(root);
        node.node_id = "node-live-child".to_string();
        node.instance_id = "ploke-live__child-target-1".to_string();
        node.branch_id = "branch-live-child".to_string();
        node.candidate_id = "candidate-live-child".to_string();
        node.target_relpath =
            PathBuf::from(ploke_tui::tools::ToolName::NsPatch.description_artifact_relpath());
        node.node_dir = root.join("nodes").join(&node.node_id);
        node.workspace_root = node.node_dir.join("worktree");
        node.binary_path = node.node_dir.join("bin").join("ploke-eval");
        node.runner_request_path = node.node_dir.join("runner-request.json");
        node.runner_result_path = node.node_dir.join("runner-result.json");
        node
    }

    fn live_branch_for(
        node: &Prototype1NodeRecord,
        source: &str,
        proposed: &str,
    ) -> ResolvedTreatmentBranch {
        let mut resolved = resolved_branch_for(node);
        resolved.target_relpath = node.target_relpath.clone();
        resolved.source_content = source.to_string();
        resolved.branch.proposed_content = proposed.to_string();
        resolved
    }

    fn write_text(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("path parent")).expect("create parent dir");
        fs::write(path, text).expect("write file");
    }

    fn git_output(repo_root: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(args)
            .output()
            .expect("git command");
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git stdout is utf8")
    }

    fn setup_successor_install_case(
        campaign_id: &CampaignId,
    ) -> (
        TempDir,
        crate::test_support::EnvGuard,
        PathBuf,
        Prototype1NodeRecord,
        ResolvedTreatmentBranch,
        GitBranch,
        ArtifactSurface,
        ArtifactSurface,
        ParentIdentity,
        ParentIdentity,
    ) {
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            eval_home.clone().into_os_string(),
        )]);
        let campaign_root = eval_home.join("campaigns").join(campaign_id.as_str());
        let prototype_root = campaign_root.join("prototype1");
        fs::create_dir_all(&prototype_root).expect("prototype root");
        fs::write(campaign_root.join("campaign.json"), "{}").expect("campaign manifest");

        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).expect("repo root");
        run_git_test(&repo_root, &["init", "-b", "main"]);
        run_git_test(
            &repo_root,
            &["config", "user.email", "prototype1-test@example.invalid"],
        );
        run_git_test(&repo_root, &["config", "user.name", "Prototype1 Test"]);
        write_text(
            &repo_root.join("crates/ploke-eval/src/lib.rs"),
            "pub fn authority_surface() {}\n",
        );
        for tool in ploke_tui::tools::ToolName::ALL {
            write_text(
                &repo_root.join(tool.description_artifact_relpath()),
                "tool description\n",
            );
        }
        write_text(
            &repo_root.join("src/lib.rs"),
            "pub fn value() -> u8 { 1 }\n",
        );
        run_git_test(&repo_root, &["add", "."]);
        run_git_test(&repo_root, &["commit", "--no-gpg-sign", "-m", "base"]);

        let parent_branch = "prototype1-parent-gen0";
        run_git_test(&repo_root, &["switch", "-c", parent_branch]);
        let current_parent = ParentIdentity::root_bootstrap(
            campaign_id.clone(),
            "node-parent",
            "BurntSushi__ripgrep-2209",
            "branch-parent",
            Some(parent_branch.to_string()),
        );
        write_parent_identity(&repo_root, &current_parent).expect("write gen0 identity");
        GitWorktreeBackend
            .persist_active_checkout_files(
                &repo_root,
                &[parent_identity_relpath()],
                &parent_identity_commit_message(&current_parent),
            )
            .expect("commit gen0 identity");
        GitWorktreeBackend
            .validate_parent_checkout(&repo_root, &current_parent)
            .expect("valid gen0 parent checkout");

        let mut node = test_node(&prototype_root);
        node.node_id = "node-child".to_string();
        node.parent_node_id = Some(current_parent.node_id().to_string());
        node.generation = 1;
        node.source_state_id = current_parent.branch_id().to_string();
        node.parent_branch_id = Some(current_parent.branch_id().to_string());
        node.branch_id = "branch-child".to_string();
        node.candidate_id = "candidate-child".to_string();
        node.node_dir = prototype_root.join("nodes").join(&node.node_id);
        node.workspace_root = prototype_root
            .join("workspaces")
            .join("edit-harness")
            .join(&node.node_id);
        node.binary_path = node.node_dir.join("bin").join("ploke-eval");
        node.runner_request_path = node.node_dir.join("runner-request.json");
        node.runner_result_path = node.node_dir.join("runner-result.json");
        let resolved = resolved_branch_for(&node);

        let branch = GitBranch("prototype1-successor-artifact".to_string());
        run_git_test(&repo_root, &["switch", "-c", &branch.0]);
        write_text(
            &repo_root.join(&resolved.target_relpath),
            &resolved.branch.proposed_content,
        );
        run_git_test(&repo_root, &["add", "."]);
        run_git_test(
            &repo_root,
            &["commit", "--no-gpg-sign", "-m", "candidate artifact"],
        );
        let artifact_surface = GitWorktreeBackend
            .artifact_surface(&repo_root)
            .expect("candidate artifact surface");
        run_git_test(&repo_root, &["switch", parent_branch]);
        let current_surface = GitWorktreeBackend
            .artifact_surface(&repo_root)
            .expect("current parent surface");

        let parent_identity = ParentIdentity::from_node(
            campaign_id.clone(),
            &node,
            Some(&current_parent),
            Some(branch.0.clone()),
        );

        (
            tmp,
            env,
            repo_root,
            node,
            resolved,
            branch,
            artifact_surface,
            current_surface,
            current_parent,
            parent_identity,
        )
    }

    fn selection_for_artifact(
        node: Prototype1NodeRecord,
        resolved: ResolvedTreatmentBranch,
        surface: ArtifactSurface,
    ) -> selection::Selection<selection::Artifact> {
        selection::Selection::artifact_for_test(
            node,
            SubjectRef::new("candidate:node-child:plan_index=0"),
            resolved,
            surface,
            selection::Source::CurrentGeneration,
            None,
        )
        .expect("typed artifact selection")
    }

    #[cfg(feature = "live_api_tests")]
    fn write_live_repo(eval_home: &Path) -> String {
        let repo_root = eval_home.join("repos/ploke-live/child-target");
        fs::create_dir_all(&repo_root).expect("create live source repo");
        run_git_test(&repo_root, &["init"]);
        write_text(
            &repo_root.join("Cargo.toml"),
            r#"[package]
name = "child-target"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
        );
        write_text(
            &repo_root.join("src/lib.rs"),
            r#"pub fn answer() -> &'static str {
    "wrong"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_returns_fixed() {
        assert_eq!(answer(), "fixed");
    }
}
"#,
        );
        run_git_test(&repo_root, &["add", "--all"]);
        run_git_test(
            &repo_root,
            &[
                "-c",
                "user.email=prototype1-test@example.invalid",
                "-c",
                "user.name=Prototype1 Test",
                "commit",
                "--no-gpg-sign",
                "-m",
                "base",
            ],
        );
        git_output(&repo_root, &["rev-parse", "HEAD"])
            .trim()
            .to_string()
    }

    #[cfg(feature = "live_api_tests")]
    fn write_live_dataset(eval_home: &Path, base_sha: &str) -> PathBuf {
        let dataset = eval_home.join("datasets/live-child-runner.jsonl");
        let body = "The repository has one source file to fix: `src/lib.rs`. \
The failing test is `answer_returns_fixed`. The function `answer()` currently returns the \
string literal `wrong`; replace that literal with `fixed`. Do not change `Cargo.toml`. \
After editing, use the cargo tool to run `cargo test`, then finish with the patch output.";
        let row = serde_json::json!({
            "instance_id": "ploke-live__child-target-1",
            "org": "ploke-live",
            "repo": "child-target",
            "number": 1,
            "title": "Fix the answer canary",
            "body": body,
            "base": { "sha": base_sha },
            "fix_patch": "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,5 +1,5 @@\n pub fn answer() -> &'static str {\n-    \"wrong\"\n+    \"fixed\"\n }\n"
        });
        write_text(&dataset, &format!("{row}\n"));
        dataset
    }

    #[cfg(feature = "live_api_tests")]
    fn write_live_campaign(
        eval_home: &Path,
        campaign_id: &CampaignId,
        dataset: PathBuf,
        model: &str,
    ) {
        let mut manifest = crate::CampaignManifest::new(campaign_id.clone());
        manifest.dataset_sources = vec![crate::target_registry::RegistryDatasetSource {
            key: None,
            path: dataset,
            label: "live-child-runner".to_string(),
            url: None,
        }];
        manifest.model_id = Some(model.to_string());
        manifest.route_source = Some(ploke_llm::request::models::ModelRouteSource::DirectGoogle);
        manifest.instances_root = Some(eval_home.join("instances").join(campaign_id.as_str()));
        manifest.batches_root = Some(eval_home.join("batches").join(campaign_id.as_str()));
        manifest.eval.limit = Some(1);
        manifest.eval.batch_prefix = Some("live-child-runner".to_string());
        manifest.eval.budget = crate::spec::EvalBudget {
            max_turns: 8,
            max_tool_calls: 24,
            wall_clock_secs: 900,
        };
        manifest.protocol.limit_runs = Some(1);
        manifest.protocol.max_concurrency = 1;
        manifest.protocol.tool_review_parallelism = 2;
        manifest.protocol.max_tokens = 3200;
        crate::save_campaign_manifest(&manifest).expect("save live baseline campaign");
    }

    fn treatment_with_instances(instance_ids: &[&str]) -> Prototype1TreatmentEvidence {
        Prototype1TreatmentEvidence {
            baseline_campaign_id: CampaignId::from("baseline"),
            branch_id: "branch-1".to_string(),
            treatment_campaign_id: CampaignId::from("treatment"),
            treatment_campaign_manifest: PathBuf::from("treatment/campaign.json"),
            treatment_closure_state_path: PathBuf::from("treatment/closure-state.json"),
            eval_policy: EvalCampaignPolicy::default(),
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_sources: Vec::new(),
            instances: instance_ids
                .iter()
                .map(|instance_id| {
                    crate::cli::prototype1_state::cli_facing::Prototype1TreatmentInstanceEvidence {
                        instance_id: (*instance_id).to_string(),
                        registration_path: None,
                        record_path: None,
                        metrics: None,
                        oracle_evaluation: None,
                        status: "complete".to_string(),
                    }
                })
                .collect(),
        }
    }

    fn complete_metrics() -> crate::OperationalRunMetrics {
        crate::OperationalRunMetrics {
            tool_calls_total: 1,
            tool_calls_failed: 0,
            patch_attempted: true,
            patch_apply_state: crate::PatchApplyState::Applied,
            submission_artifact_state: SubmissionArtifactState::Nonempty,
            patch_projection_check_state: PatchProjectionCheckState::Passed,
            partial_patch_failures: 0,
            same_file_patch_retry_count: 0,
            same_file_patch_max_streak: 0,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: true,
            convergence: true,
            oracle_eligible: true,
        }
    }

    fn oracle_evaluation(
        instance_id: &str,
        verdict: crate::mbe::Verdict,
    ) -> crate::mbe::OracleEvaluation {
        crate::mbe::OracleEvaluation {
            evidence: crate::mbe::OracleEvidence {
                report_path: PathBuf::from("mbe/final_report.json"),
                instance_id: instance_id.to_string(),
                report_id: format!("BurntSushi/ripgrep:pr-{instance_id}"),
                verdict,
            },
            instance_report_path: PathBuf::from("mbe/report.json"),
            instance_report: None,
            diagnostic: crate::mbe::OracleDiagnostic::MissingInstanceReport,
            missing_f2p_tests: Vec::new(),
            failed_fix_tests: Vec::new(),
            usable_for_selection: false,
        }
    }

    #[test]
    fn oracle_attachment_requires_all_configured_targets() {
        let mut treatment =
            treatment_with_instances(&["BurntSushi__ripgrep-2209", "BurntSushi__ripgrep-454"]);
        attach_treatment_oracle_evaluations(
            &mut treatment,
            &[
                "BurntSushi__ripgrep-2209".to_string(),
                "BurntSushi__ripgrep-454".to_string(),
            ],
            vec![oracle_evaluation(
                "BurntSushi__ripgrep-2209",
                crate::mbe::Verdict::Resolved,
            )],
        )
        .expect_err("missing configured target is rejected");
    }

    #[test]
    fn oracle_attachment_rejects_duplicate_and_unknown_targets() {
        let mut treatment = treatment_with_instances(&["BurntSushi__ripgrep-2209"]);
        attach_treatment_oracle_evaluations(
            &mut treatment,
            &["BurntSushi__ripgrep-2209".to_string()],
            vec![
                oracle_evaluation("BurntSushi__ripgrep-2209", crate::mbe::Verdict::Resolved),
                oracle_evaluation("BurntSushi__ripgrep-2209", crate::mbe::Verdict::Resolved),
            ],
        )
        .expect_err("duplicate oracle evaluation is rejected");

        let mut treatment = treatment_with_instances(&["BurntSushi__ripgrep-2209"]);
        attach_treatment_oracle_evaluations(
            &mut treatment,
            &["BurntSushi__ripgrep-2209".to_string()],
            vec![oracle_evaluation(
                "BurntSushi__ripgrep-454",
                crate::mbe::Verdict::Resolved,
            )],
        )
        .expect_err("unknown oracle evaluation is rejected");
    }

    #[test]
    fn oracle_attachment_attaches_each_configured_target() {
        let mut treatment = treatment_with_instances(&[
            "BurntSushi__ripgrep-2209",
            "BurntSushi__ripgrep-454",
            "BurntSushi__ripgrep-999",
        ]);
        attach_treatment_oracle_evaluations(
            &mut treatment,
            &[
                "BurntSushi__ripgrep-2209".to_string(),
                "BurntSushi__ripgrep-454".to_string(),
            ],
            vec![
                oracle_evaluation("BurntSushi__ripgrep-2209", crate::mbe::Verdict::Resolved),
                oracle_evaluation("BurntSushi__ripgrep-454", crate::mbe::Verdict::Unresolved),
            ],
        )
        .expect("oracle evaluations attach");

        assert!(treatment.instances[0].oracle_evaluation.is_some());
        assert!(treatment.instances[1].oracle_evaluation.is_some());
        assert!(treatment.instances[2].oracle_evaluation.is_none());
    }

    #[test]
    fn child_success_requires_complete_treatment_metrics() {
        let incomplete = treatment_with_instances(&["case-1"]);
        let error =
            require_complete_treatment(&incomplete).expect_err("missing metrics are not success");
        assert!(
            error
                .to_string()
                .contains("did not produce complete run metrics"),
            "{error}"
        );

        let mut complete = treatment_with_instances(&["case-1"]);
        complete.instances[0].metrics = Some(complete_metrics());
        require_complete_treatment(&complete).expect("complete metrics are success evidence");
    }

    #[tokio::test]
    async fn child_runner_failure_records_terminal_channel() {
        let tmp = tempdir().expect("tempdir");
        let campaign_id = CampaignId::from("live-child-runner-failure");
        let eval_home = tmp.path().join("eval-home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            eval_home.clone().into_os_string(),
        )]);
        let campaign_dir = eval_home.join("campaigns").join(campaign_id.as_str());
        let prototype1_root = campaign_dir.join("prototype1");
        let manifest_path = campaign_dir.join("campaign.json");
        fs::create_dir_all(&prototype1_root).expect("prototype1 root");

        let node = test_node(&prototype1_root);
        let request = crate::intervention::runner_request_from_node(&campaign_id, &node, true);
        let resolved = resolved_branch_for(&node);
        let runtime_id = RuntimeId::new();
        let journal_path = prototype1_root.join("transition-journal.jsonl");
        let channel_root =
            crate::cli::prototype1_state::invocation::channel_root(&node.node_dir, runtime_id);
        let invocation = crate::cli::prototype1_state::invocation::ChildInvocation::with_bootstrap(
            campaign_id.clone(),
            node.clone(),
            request,
            resolved,
            runtime_id,
            journal_path.clone(),
            channel_root,
        )
        .expect("valid child invocation bootstrap");
        let invocation_path =
            crate::cli::prototype1_state::invocation::invocation_path(&node.node_dir, runtime_id);
        crate::cli::prototype1_state::invocation::write_child_invocation(
            &invocation_path,
            &invocation,
        )
        .expect("write child invocation");

        // The workspace target file is intentionally absent. That forces the
        // real child runner through `run_prototype1_resolved_branch_treatment`
        // and into an early materialization failure without faking runner
        // success or treatment evidence.
        let result = execute_prototype1_runner_invocation(&invocation_path)
            .await
            .expect("runner converts treatment failure into terminal result");

        assert_eq!(result.status, Prototype1NodeStatus::Failed);
        assert_eq!(
            result.disposition,
            Prototype1RunnerDisposition::TreatmentFailed
        );

        let attempt_result_path =
            crate::cli::prototype1_state::invocation::result_path(&node.node_dir, runtime_id);
        let projection = crate::projection::OperatorProjectionRead::projection_module();
        let attempt_result =
            crate::intervention::load_runner_result_at(&attempt_result_path, projection.clone())
                .expect("attempt-scoped result");
        let latest_result = crate::intervention::load_runner_result_at(
            &node.runner_result_path,
            projection.clone(),
        )
        .expect("latest node result");
        let projected_node =
            crate::intervention::load_node_record(&manifest_path, &node.node_id, projection)
                .expect("projected failed node");

        assert_eq!(attempt_result, result);
        assert_eq!(latest_result, result);
        assert_eq!(projected_node.status, Prototype1NodeStatus::Failed);

        let journal = fs::read_to_string(&journal_path).expect("transition journal");
        assert!(journal.contains(r#""state":"ready""#), "{journal}");
        assert!(journal.contains(r#""state":"evaluating""#), "{journal}");
        assert!(
            journal.contains(r#""state":{"result_written""#)
                || journal.contains(r#""result_written""#),
            "{journal}"
        );

        let endpoints = invocation
            .channel_endpoints()
            .expect("child invocation carries channel endpoints");
        let (_, raw_messages) = crate::cli::prototype1_state::channel::Transport::read_since(
            &FileTransport,
            &endpoints.child_to_parent(),
            Cursor::start(),
        )
        .expect("read child-to-parent channel");
        let messages = raw_messages
            .iter()
            .map(|bytes| {
                serde_json::from_slice::<crate::cli::prototype1_state::channel::Envelope<ToParent>>(
                    bytes,
                )
                .expect("typed child-to-parent envelope")
            })
            .collect::<Vec<_>>();

        assert_eq!(messages.len(), 3);
        assert!(matches!(messages[0].body(), ToParent::Ready));
        assert!(matches!(messages[1].body(), ToParent::Evaluating));
        match messages[2].body() {
            ToParent::Result {
                runner_result,
                treatment,
            } => {
                assert_eq!(runner_result, &result);
                assert!(treatment.is_none());
            }
            other => panic!("expected terminal channel result, got {other:?}"),
        }
    }

    #[cfg(feature = "live_api_tests")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires Google ADC and runs a live child self-evaluation plus protocol closure"]
    /// WARNING: intentionally quarantined. The last live run exposed a path
    /// identity bug in this test setup: closure reconstruction can miss a
    /// completed treatment run when eval roots and run registrations use
    /// different canonical spellings. Keep this loud until the test is rebuilt.
    async fn live_google_child_runner_success() {
        panic!(
            "live_google_child_runner_success is intentionally quarantined: \
             its setup previously made a completed treatment run appear missing \
             through eval-root/run-registration path identity drift"
        );
        #[allow(unreachable_code)]
        {
            const TEST_NAME: &str = "live_google_child_runner_success";

            let started = std::time::Instant::now();
            let mut previous = started;
            if !crate::test_support::live_google_env_or_skip(TEST_NAME).await {
                return;
            }
            print_live_child_timing("google_auth_checked", started, &mut previous);

            let _llm_guard = crate::test_support::llm_lock().lock().await;
            let tmp = crate::test_support::live_tempdir("prototype1-child-runner-success-");
            let eval_home = tmp.path().join("eval-home");
            let _env = crate::test_support::env_guard_os(vec![(
                "PLOKE_EVAL_HOME",
                eval_home.clone().into_os_string(),
            )]);
            let model_id = crate::test_support::live_google_model_id();
            crate::test_support::write_direct_google_model_config(&eval_home, &model_id);
            print_live_child_timing("model_config_written", started, &mut previous);

            let campaign_id = CampaignId::from("live-child-runner-success");
            let base_sha = write_live_repo(&eval_home);
            let dataset = write_live_dataset(&eval_home, &base_sha);
            write_live_campaign(&eval_home, &campaign_id, dataset, &model_id.to_string());
            print_live_child_timing("campaign_written", started, &mut previous);

            let campaign_dir = eval_home.join("campaigns").join(campaign_id.as_str());
            let prototype1_root = campaign_dir.join("prototype1");
            fs::create_dir_all(&prototype1_root).expect("prototype1 root");
            let manifest_path = campaign_dir.join("campaign.json");
            let mut node = live_node(&prototype1_root);
            let source = "baseline ns_patch tool description\n";
            let proposed = "candidate ns_patch tool description\n";
            let target_path = node.workspace_root.join(&node.target_relpath);
            write_text(&target_path, source);
            crate::intervention::write_node_projection(&node).expect("write node projection");

            let request = crate::intervention::runner_request_from_node(&campaign_id, &node, true);
            let resolved = live_branch_for(&node, source, proposed);
            let runtime_id = RuntimeId::new();
            let journal_path = prototype1_root.join("transition-journal.jsonl");
            let channel_root =
                crate::cli::prototype1_state::invocation::channel_root(&node.node_dir, runtime_id);
            let invocation =
                crate::cli::prototype1_state::invocation::ChildInvocation::with_bootstrap(
                    campaign_id.clone(),
                    node.clone(),
                    request,
                    resolved,
                    runtime_id,
                    journal_path.clone(),
                    channel_root,
                )
                .expect("valid child invocation bootstrap");
            let invocation_path = crate::cli::prototype1_state::invocation::invocation_path(
                &node.node_dir,
                runtime_id,
            );
            crate::cli::prototype1_state::invocation::write_child_invocation(
                &invocation_path,
                &invocation,
            )
            .expect("write child invocation");
            print_live_child_timing("invocation_written", started, &mut previous);

            let result = execute_prototype1_runner_invocation(&invocation_path)
                .await
                .expect("live child runner invocation completes");
            print_live_child_timing("runner_returned", started, &mut previous);

            if result.status != Prototype1NodeStatus::Succeeded
                || result.disposition != Prototype1RunnerDisposition::Succeeded
            {
                eprintln!(
                    "[prototype1-child-live] preserving failed live tempdir={}",
                    tmp.path().display()
                );
                let detail = format!("{result:#?}");
                std::mem::forget(tmp);
                panic!("live child runner did not succeed:\n{detail}");
            }
            let treatment_campaign_id = result
                .treatment_campaign_id
                .as_ref()
                .expect("success result names treatment campaign");
            let treatment_state = crate::load_closure_state(treatment_campaign_id)
                .expect("load treatment closure state");
            assert_eq!(treatment_state.instances.len(), 1);
            assert_eq!(treatment_state.eval.complete_total, 1);

            let endpoints = invocation
                .channel_endpoints()
                .expect("child invocation carries channel endpoints");
            let (_, raw_messages) = crate::cli::prototype1_state::channel::Transport::read_since(
                &FileTransport,
                &endpoints.child_to_parent(),
                Cursor::start(),
            )
            .expect("read child-to-parent channel");
            let messages = raw_messages
                .iter()
                .map(|bytes| {
                    serde_json::from_slice::<
                        crate::cli::prototype1_state::channel::Envelope<ToParent>,
                    >(bytes)
                    .expect("typed child-to-parent envelope")
                })
                .collect::<Vec<_>>();

            assert_eq!(messages.len(), 3);
            assert!(matches!(messages[0].body(), ToParent::Ready));
            assert!(matches!(messages[1].body(), ToParent::Evaluating));
            match messages[2].body() {
                ToParent::Result {
                    runner_result,
                    treatment,
                } => {
                    assert_eq!(runner_result, &result);
                    let treatment = treatment
                        .as_ref()
                        .expect("successful child result carries treatment evidence");
                    assert_eq!(treatment.treatment_campaign_id, *treatment_campaign_id);
                    assert_eq!(treatment.instances.len(), 1);
                    assert_eq!(
                        treatment.instances[0].instance_id,
                        "ploke-live__child-target-1"
                    );
                    assert!(
                        treatment.instances[0]
                            .record_path
                            .as_ref()
                            .is_some_and(|p| p.exists()),
                        "terminal treatment evidence should point at the run record"
                    );
                    assert!(
                        treatment.instances[0]
                            .metrics
                            .as_ref()
                            .is_some_and(|m| m.nonempty_valid_patch),
                        "live child should produce non-empty valid patch metrics"
                    );
                }
                other => panic!("expected terminal channel result, got {other:?}"),
            }

            let target_repo = node
                .node_dir
                .join("instance-targets")
                .join(treatment_campaign_id)
                .join("ploke-live/child-target");
            let final_lib = fs::read_to_string(target_repo.join("src/lib.rs"))
                .expect("read child eval target after runner");
            assert!(
                final_lib.contains("\"fixed\""),
                "live child target should contain the intended fix:\n{final_lib}"
            );
            print_live_child_timing("assertions_complete", started, &mut previous);
        }
    }

    #[cfg(feature = "live_api_tests")]
    fn print_live_child_timing(
        phase: &str,
        started: std::time::Instant,
        previous: &mut std::time::Instant,
    ) {
        let now = std::time::Instant::now();
        eprintln!(
            "[prototype1-child-live] phase={phase} delta_ms={} total_ms={}",
            now.duration_since(*previous).as_millis(),
            now.duration_since(started).as_millis()
        );
        *previous = now;
    }

    #[test]
    fn child_instance_target_cache_is_outside_artifact_worktree() {
        let tmp = tempdir().expect("tempdir");
        let node = test_node(tmp.path());
        fs::create_dir_all(&node.workspace_root).expect("worktree");

        let cache = prepare_child_instance_target_cache(&node, &CampaignId::from("treatment-1"))
            .expect("cache");

        assert!(cache.starts_with(&node.node_dir));
        assert!(!cache.starts_with(&node.workspace_root));
        assert_eq!(
            cache,
            node.node_dir.join("instance-targets").join("treatment-1")
        );
        assert!(cache.is_dir());
    }

    #[test]
    fn child_cleanup_removes_instance_targets_without_removing_worktree() {
        let tmp = tempdir().expect("tempdir");
        let node = test_node(tmp.path());
        let manifest_path = tmp.path().join("campaign.json");
        fs::create_dir_all(&node.workspace_root).expect("worktree");
        fs::create_dir_all(node.binary_path.parent().expect("binary parent")).expect("bin dir");
        fs::write(&node.binary_path, b"binary").expect("binary");
        fs::create_dir_all(node.node_dir.join("target").join("debug")).expect("target");
        fs::create_dir_all(
            node.node_dir
                .join("instance-targets")
                .join("treatment-1")
                .join("BurntSushi")
                .join("ripgrep"),
        )
        .expect("instance target");

        cleanup_prototype1_child_build_products(
            &manifest_path,
            &CampaignId::from("campaign"),
            &node,
        )
        .expect("cleanup");

        assert!(!node.binary_path.exists());
        assert!(!node.node_dir.join("target").exists());
        assert!(!node.node_dir.join("instance-targets").exists());
        assert!(node.workspace_root.exists());
    }

    #[test]
    fn child_cleanup_removes_edit_harness_targets_without_removing_workspaces() {
        let tmp = tempdir().expect("tempdir");
        let node = test_node(tmp.path());
        let manifest_path = tmp.path().join("campaign.json");
        let edit_harness_root = tmp
            .path()
            .join("prototype1")
            .join("workspaces")
            .join("edit-harness");
        let first_workspace = edit_harness_root.join("node-1");
        let retry_workspace = edit_harness_root.join("node-1-r2");
        fs::create_dir_all(&node.workspace_root).expect("worktree");
        fs::create_dir_all(node.binary_path.parent().expect("binary parent")).expect("bin dir");
        fs::write(&node.binary_path, b"binary").expect("binary");
        fs::create_dir_all(first_workspace.join("target").join("debug")).expect("first target");
        fs::create_dir_all(retry_workspace.join("target").join("debug")).expect("retry target");
        fs::write(first_workspace.join("README.md"), b"first").expect("first marker");
        fs::write(retry_workspace.join("README.md"), b"retry").expect("retry marker");

        cleanup_prototype1_child_build_products(
            &manifest_path,
            &CampaignId::from("campaign"),
            &node,
        )
        .expect("cleanup");

        assert!(!first_workspace.join("target").exists());
        assert!(!retry_workspace.join("target").exists());
        assert_eq!(
            fs::read(first_workspace.join("README.md")).expect("first marker"),
            b"first"
        );
        assert_eq!(
            fs::read(retry_workspace.join("README.md")).expect("retry marker"),
            b"retry"
        );
    }

    #[test]
    fn child_cleanup_unlinks_edit_harness_target_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let tmp = tempdir().expect("tempdir");
        let node = test_node(tmp.path());
        let manifest_path = tmp.path().join("campaign.json");
        let workspace = tmp
            .path()
            .join("prototype1")
            .join("workspaces")
            .join("edit-harness")
            .join("node-1");
        let outside_target = tmp.path().join("outside-target");
        fs::create_dir_all(&node.workspace_root).expect("worktree");
        fs::create_dir_all(node.binary_path.parent().expect("binary parent")).expect("bin dir");
        fs::write(&node.binary_path, b"binary").expect("binary");
        fs::create_dir_all(&workspace).expect("workspace");
        fs::create_dir_all(&outside_target).expect("outside target");
        fs::write(outside_target.join("artifact"), b"keep").expect("outside marker");
        symlink(&outside_target, workspace.join("target")).expect("target symlink");

        cleanup_prototype1_child_build_products(
            &manifest_path,
            &CampaignId::from("campaign"),
            &node,
        )
        .expect("cleanup");

        assert!(!workspace.join("target").exists());
        assert_eq!(
            fs::read(outside_target.join("artifact")).expect("outside marker"),
            b"keep"
        );
    }

    #[test]
    fn child_artifact_workspace_accepts_broad_harness_artifact_root() {
        let tmp = tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let workspace_root = tmp
            .path()
            .join("prototype1")
            .join("workspaces")
            .join("edit-harness")
            .join("node-1");
        fs::create_dir_all(&workspace_root).expect("workspace root");
        run_git_test(&workspace_root, &["init"]);
        fs::write(workspace_root.join("README.md"), "artifact\n").expect("write artifact");
        run_git_test(&workspace_root, &["add", "."]);
        run_git_test(
            &workspace_root,
            &[
                "-c",
                "user.email=prototype1-test@example.invalid",
                "-c",
                "user.name=Prototype1 Test",
                "commit",
                "--no-gpg-sign",
                "-m",
                "artifact",
            ],
        );
        let mut node = test_node(tmp.path());
        node.workspace_root = workspace_root.clone();

        let workspace = child_artifact_workspace(&GitWorktreeBackend, &manifest_path, &node)
            .expect("broad harness artifact workspace");

        assert_eq!(workspace.root, workspace_root);
        assert!(!workspace.branch.0.is_empty());
    }

    #[test]
    fn successor_artifact_workspace_recovers_missing_broad_harness_checkout_from_branch() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(&repo_root).expect("repo root");
        run_git_test(&repo_root, &["init", "-b", "main"]);
        run_git_test(
            &repo_root,
            &["config", "user.email", "prototype1-test@example.invalid"],
        );
        run_git_test(&repo_root, &["config", "user.name", "Prototype1 Test"]);
        write_text(
            &repo_root.join("src/lib.rs"),
            "pub fn value() -> u8 { 1 }\n",
        );
        run_git_test(&repo_root, &["add", "."]);
        run_git_test(&repo_root, &["commit", "--no-gpg-sign", "-m", "base"]);

        let request_id = "broad-harness-request:node-1";
        let branch = GitWorktreeBackend.broad_harness_branch_name(request_id);
        run_git_test(&repo_root, &["switch", "-c", &branch.0]);
        write_text(
            &repo_root.join("src/lib.rs"),
            "pub fn value() -> u8 { 2 }\n",
        );
        run_git_test(&repo_root, &["add", "."]);
        run_git_test(
            &repo_root,
            &[
                "commit",
                "--no-gpg-sign",
                "-m",
                "prototype1 broad harness result broad-harness-request:node-1",
            ],
        );
        run_git_test(&repo_root, &["switch", "main"]);

        let campaign_root = tmp.path().join("campaign");
        let manifest_path = campaign_root.join("campaign.json");
        let mut node = test_node(&campaign_root);
        node.workspace_root = campaign_root
            .join("prototype1")
            .join("workspaces")
            .join("edit-harness")
            .join("node-1");

        let (workspace, artifact_branch) =
            successor_artifact_checkout(&GitWorktreeBackend, &manifest_path, &node)
                .expect("committed broad harness artifact branch");

        assert!(!node.workspace_root.exists());
        assert!(workspace.is_none());
        assert_eq!(artifact_branch, branch);
        GitWorktreeBackend
            .verify_artifact_target(
                &repo_root,
                &artifact_branch,
                &node.target_relpath,
                "pub fn value() -> u8 { 2 }\n",
            )
            .expect("branch carries proposed target");
    }

    #[test]
    fn successor_install_commits_selected_parent_identity() {
        let campaign_id = CampaignId::from("successor-identity-install");
        let (
            _tmp,
            _env,
            repo_root,
            node,
            resolved,
            artifact_branch,
            artifact_surface,
            current_surface,
            current_parent,
            selected_parent_identity,
        ) = setup_successor_install_case(&campaign_id);
        let selected = selection_for_artifact(node, resolved.clone(), artifact_surface);

        let installed = install_committed_successor_artifact(
            &campaign_id,
            &repo_root,
            &selected,
            artifact_branch.clone(),
            current_surface,
            selected_parent_identity.clone(),
            Some(current_parent),
        )
        .expect("install successor artifact");

        assert_eq!(installed.parent_identity, selected_parent_identity);
        let loaded = crate::cli::prototype1_state::identity::load_parent_identity(&repo_root)
            .expect("load active parent identity");
        assert_eq!(loaded, selected_parent_identity);
        GitWorktreeBackend
            .validate_parent_checkout(&repo_root, &selected_parent_identity)
            .expect("successor checkout validates with selected identity");

        let branch_identity = git_output(
            &repo_root,
            &[
                "show",
                &format!(
                    "{}:{}",
                    artifact_branch.0,
                    parent_identity_relpath().display()
                ),
            ],
        );
        let branch_identity: ParentIdentity =
            serde_json::from_str(&branch_identity).expect("branch identity json");
        assert_eq!(branch_identity, selected_parent_identity);

        assert_eq!(
            git_output(&repo_root, &["branch", "--show-current"]).trim(),
            artifact_branch.0
        );
        assert_eq!(
            git_output(&repo_root, &["log", "-1", "--format=%s"]).trim(),
            parent_identity_commit_message(&selected_parent_identity)
        );
        assert_eq!(
            fs::read_to_string(repo_root.join(&resolved.target_relpath)).expect("target content"),
            resolved.branch.proposed_content
        );
    }

    #[test]
    fn successor_install_rejects_branch_drift_after_selection() {
        let campaign_id = CampaignId::from("successor-drift-reject");
        let (
            _tmp,
            _env,
            repo_root,
            node,
            resolved,
            artifact_branch,
            artifact_surface,
            current_surface,
            current_parent,
            selected_parent_identity,
        ) = setup_successor_install_case(&campaign_id);
        let selected = selection_for_artifact(node, resolved, artifact_surface);

        run_git_test(&repo_root, &["switch", &artifact_branch.0]);
        write_text(&repo_root.join("Cargo.toml"), "[workspace]\nmembers = []\n");
        run_git_test(&repo_root, &["add", "Cargo.toml"]);
        run_git_test(
            &repo_root,
            &["commit", "--no-gpg-sign", "-m", "drift authority surface"],
        );
        run_git_test(
            &repo_root,
            &["switch", current_parent.artifact_branch().unwrap()],
        );
        let parent_branch = current_parent.artifact_branch().unwrap().to_string();

        let err = install_committed_successor_artifact(
            &campaign_id,
            &repo_root,
            &selected,
            artifact_branch,
            current_surface,
            selected_parent_identity,
            Some(current_parent),
        )
        .expect_err("branch drift must be rejected before successor install");

        match err {
            PrepareError::InvalidBatchSelection { detail } => {
                assert!(detail.contains("changed after selection"), "{detail}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(
            git_output(&repo_root, &["branch", "--show-current"]).trim(),
            parent_branch
        );
    }

    #[test]
    fn broad_harness_request_id_projection_preserves_retry_suffix() {
        assert_eq!(
            broad_harness_request_id_from_workspace_root(Path::new(
                "/tmp/prototype1/workspaces/edit-harness/node-abc"
            ))
            .as_deref(),
            Some("broad-harness-request:node-abc")
        );
        assert_eq!(
            broad_harness_request_id_from_workspace_root(Path::new(
                "/tmp/prototype1/workspaces/edit-harness/node-abc-r2"
            ))
            .as_deref(),
            Some("broad-harness-request:node-abc:r2")
        );
    }

    #[test]
    fn child_projection_gate_rejects_shared_checkout_cwd() {
        let tmp = tempdir().expect("tempdir");
        let node = test_node(tmp.path());
        fs::create_dir_all(&node.node_dir).expect("node dir");
        let run_root = tmp.path().join("runs").join("run-1");
        fs::create_dir_all(&run_root).expect("run root");
        let projection_path = run_root.join("benchmark-patch-projection.json");
        let registration_path = write_test_registration(&node, tmp.path(), &projection_path);
        write_test_projection(
            &node,
            &projection_path,
            PathBuf::from("/tmp/shared/BurntSushi/ripgrep"),
        );

        let treatment = Prototype1TreatmentEvidence {
            baseline_campaign_id: CampaignId::from("baseline"),
            branch_id: node.branch_id.clone(),
            treatment_campaign_id: CampaignId::from("treatment"),
            treatment_campaign_manifest: tmp.path().join("campaign.json"),
            treatment_closure_state_path: tmp.path().join("closure.json"),
            eval_policy: EvalCampaignPolicy::default(),
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_sources: Vec::new(),
            instances: vec![
                crate::cli::prototype1_state::cli_facing::Prototype1TreatmentInstanceEvidence {
                    instance_id: node.instance_id.clone(),
                    registration_path: Some(registration_path),
                    record_path: Some(run_root.join("record.json.gz")),
                    metrics: Some(crate::OperationalRunMetrics {
                        tool_calls_total: 1,
                        tool_calls_failed: 0,
                        patch_attempted: true,
                        patch_apply_state: crate::PatchApplyState::Applied,
                        submission_artifact_state: SubmissionArtifactState::Nonempty,
                        patch_projection_check_state: PatchProjectionCheckState::Passed,
                        partial_patch_failures: 0,
                        same_file_patch_retry_count: 0,
                        same_file_patch_max_streak: 0,
                        aborted: false,
                        aborted_repair_loop: false,
                        nonempty_valid_patch: true,
                        convergence: true,
                        oracle_eligible: true,
                    }),
                    oracle_evaluation: None,
                    status: "complete".to_string(),
                },
            ],
        };

        let error = validate_treatment_patch_projection(&node, &treatment)
            .expect_err("shared checkout must fail child gate");
        assert!(
            error
                .to_string()
                .contains("outside child instance target root")
        );
    }

    fn run_git_test(repo_root: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(args)
            .status()
            .expect("git command");
        assert!(status.success(), "git {args:?} failed");
    }

    fn write_test_registration(
        node: &Prototype1NodeRecord,
        root: &Path,
        projection_path: &Path,
    ) -> PathBuf {
        let storage_roots = RunStorageRoots::new(root.join("registries"), root.join("runs"));
        let intent = RunIntent {
            task_id: node.instance_id.clone(),
            repo_root: node
                .node_dir
                .join("instance-targets/treatment/BurntSushi/ripgrep"),
            storage_roots,
            base_sha: Some("abc123".to_string()),
            budget: EvalBudget::default(),
            model_id: Some("model".to_string()),
            provider_slug: Some("provider".to_string()),
            campaign_id: Some(CampaignId::from("treatment")),
            batch_id: Some("batch".to_string()),
            run_arm_id: "treatment".to_string(),
            run_role: RegisteredRunRole::Treatment,
        };
        let mut registration =
            RunRegistration::register_with_run_id(intent, "run-1").expect("registration");
        registration.artifacts.patch_projection = Some(projection_path.to_path_buf());
        registration.persist().expect("persist registration");
        registration.registry_path()
    }

    fn write_test_projection(node: &Prototype1NodeRecord, path: &Path, cwd: PathBuf) {
        let record = BenchmarkPatchProjectionRecord {
            schema_version: BENCHMARK_PATCH_PROJECTION_SCHEMA_V1.to_string(),
            benchmark: MultiSweBenchTarget {
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2209,
                instance_id: node.instance_id.clone(),
                base_sha: Some("abc123".to_string()),
            },
            run: RunArtifactRef {
                run_manifest: path.with_file_name("run.json"),
                run_root: path.parent().expect("run root").to_path_buf(),
                record_path: path.with_file_name("record.json.gz"),
            },
            candidate: None,
            checkout: BenchmarkCheckoutRef {
                cwd,
                head_sha: Some("def456".to_string()),
            },
            submission: SubmissionPatchRef {
                path: path.with_file_name("multi-swe-bench-submission.jsonl"),
                sha256: "00".repeat(32),
                byte_len: 1,
                line_count: 1,
                diff_base: "abc123".to_string(),
            },
            check: PatchProjectionCheck::Passed {
                checked_at: "2026-05-11T00:00:00Z".to_string(),
                detail: "test".to_string(),
            },
        };
        fs::write(path, serde_json::to_string_pretty(&record).expect("json"))
            .expect("write projection");
    }
}

pub(crate) fn record_prototype1_successor_ready(
    invocation: &SuccessorInvocation,
) -> Result<crate::cli::prototype1_state::invocation::SuccessorReadyRecord, PrepareError> {
    let record = crate::cli::prototype1_state::invocation::SuccessorReadyRecord {
        schema_version: crate::cli::prototype1_state::invocation::SUCCESSOR_READY_SCHEMA_VERSION
            .to_string(),
        campaign_id: invocation.campaign_id().clone(),
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
        campaign_id: invocation.campaign_id().clone(),
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

// ANCHOR: prototype1_successor_startup_validation
pub(crate) fn validate_prototype1_successor_continuation(
    invocation: &SuccessorInvocation,
    manifest_path: &Path,
) -> Result<ParentIdentity, PrepareError> {
    let store = FsBlockStore::for_campaign_manifest(manifest_path);
    let lineage_id = LineageId::new(invocation.campaign_id().clone());
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
    let identity = sealed.selected_parent_identity().clone();
    identity.validate_for_command(invocation.campaign_id(), Some(invocation.node_id()))?;
    Ok(identity)
}
// ANCHOR_END: prototype1_successor_startup_validation

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

// ANCHOR: prototype1_active_successor_binary_build
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
// ANCHOR_END: prototype1_active_successor_binary_build

fn prepare_prototype1_active_successor_runtime(
    campaign_id: &CampaignId,
    _manifest_path: &Path,
    selected: &selection::Selection<selection::Artifact>,
    active_parent_root: &Path,
    current_parent: &ParentIdentity,
) -> Result<(PathBuf, InstalledSuccessorArtifact), PrepareError> {
    let installed = install_prototype1_successor_artifact(
        campaign_id,
        active_parent_root,
        selected,
        current_parent,
    )?;
    let binary = build_prototype1_active_successor_binary(active_parent_root)?;
    Ok((binary, installed))
}

fn install_prototype1_successor_artifact(
    campaign_id: &CampaignId,
    active_parent_root: &Path,
    selected: &selection::Selection<selection::Artifact>,
    current_parent: &ParentIdentity,
) -> Result<InstalledSuccessorArtifact, PrepareError> {
    let backend = GitWorktreeBackend;
    let current_surface = backend
        .artifact_surface(active_parent_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_current_artifact_surface",
            detail: source.to_string(),
        })?;
    let artifact = selected.selected();
    let node = artifact.node();
    let resolved = artifact.resolved();
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let (workspace, artifact_branch) = successor_artifact_checkout(&backend, &manifest_path, node)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_prepare",
            detail: source.to_string(),
        })?;
    let selected_parent_identity = ParentIdentity::from_node(
        campaign_id.clone(),
        node,
        Some(current_parent),
        Some(artifact_branch.0.clone()),
    );

    if let Some(workspace) = workspace.filter(|_| node.workspace_root.exists()) {
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
        backend
            .remove(active_parent_root, &workspace)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_successor_worktree_cleanup",
                detail: source.to_string(),
            })?;
        cleanup_prototype1_child_build_products(&manifest_path, campaign_id, node)?;
        install_committed_successor_artifact(
            campaign_id,
            active_parent_root,
            selected,
            artifact_branch,
            current_surface,
            selected_parent_identity,
            Some(current_parent.clone()),
        )
    } else {
        install_committed_successor_artifact(
            campaign_id,
            active_parent_root,
            selected,
            artifact_branch,
            current_surface,
            selected_parent_identity,
            Some(current_parent.clone()),
        )
    }
}

fn successor_artifact_checkout(
    backend: &GitWorktreeBackend,
    campaign_manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<
    (
        Option<crate::cli::prototype1_state::backend::Workspace>,
        GitBranch,
    ),
    BackendError,
> {
    match child_artifact_workspace(backend, campaign_manifest_path, node) {
        Ok(workspace) => {
            let artifact_branch = workspace.branch.clone();
            Ok((Some(workspace), artifact_branch))
        }
        Err(BackendError::MissingPath { path })
            if path == node.workspace_root
                && is_broad_harness_workspace(campaign_manifest_path, &node.workspace_root) =>
        {
            committed_broad_harness_artifact_branch(backend, &node.workspace_root)
                .map(|branch| (None, branch))
        }
        Err(err) => Err(err),
    }
}

fn committed_broad_harness_artifact_branch(
    backend: &GitWorktreeBackend,
    workspace_root: &Path,
) -> Result<GitBranch, BackendError> {
    let Some(request_id) = broad_harness_request_id_from_workspace_root(workspace_root) else {
        return Err(BackendError::MissingPath {
            path: workspace_root.to_path_buf(),
        });
    };
    Ok(backend.broad_harness_branch_name(&request_id))
}

fn broad_harness_request_id_from_workspace_root(workspace_root: &Path) -> Option<String> {
    let workspace_name = workspace_root.file_name()?.to_str()?;
    if let Some((node_id, retry)) = workspace_name.rsplit_once("-r")
        && !node_id.is_empty()
        && retry.parse::<u32>().is_ok_and(|sequence| sequence > 1)
    {
        return Some(format!("broad-harness-request:{node_id}:r{retry}"));
    }
    Some(format!("broad-harness-request:{workspace_name}"))
}

fn child_artifact_workspace(
    backend: &GitWorktreeBackend,
    campaign_manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<crate::cli::prototype1_state::backend::Workspace, BackendError> {
    if is_broad_harness_workspace(campaign_manifest_path, &node.workspace_root) {
        return backend.workspace_for_artifact_root(&node.workspace_root);
    }

    backend.workspace_for_node(&node.node_id, &node.node_dir, &node.workspace_root)
}

fn is_broad_harness_workspace(campaign_manifest_path: &Path, workspace_root: &Path) -> bool {
    workspace_root.starts_with(broad_harness_workspace_root(campaign_manifest_path))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledSuccessorArtifact {
    artifact_ref: ArtifactRef,
    artifact_key: TreeKeyHash,
    surface: SurfaceCommitment,
    parent_identity: ParentIdentity,
}

fn install_committed_successor_artifact(
    campaign_id: &CampaignId,
    active_parent_root: &Path,
    selected: &selection::Selection<selection::Artifact>,
    artifact_branch: GitBranch,
    current_surface: ArtifactSurface,
    selected_parent_identity: ParentIdentity,
    previous_parent: Option<ParentIdentity>,
) -> Result<InstalledSuccessorArtifact, PrepareError> {
    let backend = GitWorktreeBackend;
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let artifact = selected.selected();
    let node = artifact.node();
    let resolved = artifact.resolved();
    let branch_key = backend
        .branch_tree_key_hash(active_parent_root, &artifact_branch)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_tree_key",
            detail: source.to_string(),
        })?;
    ensure_artifact_tree(&artifact_branch, artifact.artifact_surface(), &branch_key)?;
    backend
        .verify_artifact_target(
            active_parent_root,
            &artifact_branch,
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
            campaign_id.clone(),
            node.node_id.clone(),
            CommitPhase::Before,
            active_parent_root.to_path_buf(),
            artifact_branch.0.clone(),
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
        selected_branch = %artifact_branch.0,
        target_relpath = %resolved.target_relpath.display(),
    ));
    let _switched_commit =
        match backend.install_artifact_in_active_checkout(active_parent_root, &artifact_branch) {
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
    let installed_surface = backend
        .artifact_surface(active_parent_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_surface_before_identity",
            detail: source.to_string(),
        })?;
    let _selected_transition = ensure_installed_artifact(
        &artifact_branch,
        artifact.artifact_surface(),
        &installed_surface,
    )?;
    selected_parent_identity.validate_for_command(campaign_id, Some(&node.node_id))?;
    write_parent_identity(active_parent_root, &selected_parent_identity)?;
    let installed_commit = backend
        .persist_active_checkout_files(
            active_parent_root,
            &[parent_identity_relpath()],
            &parent_identity_commit_message(&selected_parent_identity),
        )
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_parent_identity_commit",
            detail: source.to_string(),
        })?;
    backend
        .validate_parent_checkout(active_parent_root, &selected_parent_identity)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_parent_checkout",
            detail: source.to_string(),
        })?;
    backend
        .verify_artifact_target(
            active_parent_root,
            &artifact_branch,
            &resolved.target_relpath,
            &resolved.branch.proposed_content,
        )
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_verify_after_identity",
            detail: source.to_string(),
        })?;
    append_prototype1_journal_entry(
        &manifest_path,
        JournalEntry::Successor(SuccessorRecord::checkout(
            campaign_id.clone(),
            node.node_id.clone(),
            CommitPhase::After,
            active_parent_root.to_path_buf(),
            artifact_branch.0.clone(),
            Some(installed_commit.0.clone()),
        )),
        "prototype1_successor_checkout_after_journal",
    )?;
    append_prototype1_journal_entry(
        &manifest_path,
        JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: campaign_id.clone(),
            previous_parent_identity: previous_parent,
            selected_parent_identity: selected_parent_identity.clone(),
            active_parent_root: active_parent_root.to_path_buf(),
            selected_branch: artifact_branch.0.clone(),
            installed_commit: installed_commit.0.clone(),
        }),
        "prototype1_successor_checkout_journal",
    )?;
    observe::Step::start(observe::span!(
        "prototype1.parent.checkout.advanced",
        campaign_id = %campaign_id,
        selected_parent_id = %selected_parent_identity.parent_id(),
        selected_node_id = %selected_parent_identity.node_id(),
        selected_generation = selected_parent_identity.generation(),
        selected_branch = %artifact_branch.0,
        installed_commit = %installed_commit.0,
        active_parent_root = %active_parent_root.display(),
    ))
    .success();
    let selected_surface = backend
        .artifact_surface(active_parent_root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_artifact_surface_after_install",
            detail: source.to_string(),
        })?;
    let _identity_transition =
        SurfaceCommitment::from_artifact_surfaces(&installed_surface, &selected_surface)
            .map_err(history_prepare_error)?;
    let surface = SurfaceCommitment::from_artifact_surfaces(&current_surface, &selected_surface)
        .map_err(history_prepare_error)?;
    Ok(InstalledSuccessorArtifact {
        artifact_ref: artifact.artifact_ref().clone(),
        artifact_key: selected_surface.tree_key().clone(),
        surface,
        parent_identity: selected_parent_identity,
    })
}

fn ensure_artifact_tree(
    branch: &GitBranch,
    selected: &ArtifactSurface,
    observed: &TreeKeyHash,
) -> Result<(), PrepareError> {
    if selected.tree_key() == observed {
        return Ok(());
    }

    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "successor artifact branch '{}' changed after selection: selected tree key {:?}, observed tree key {:?}",
            branch.0,
            selected.tree_key(),
            observed
        ),
    })
}

fn ensure_installed_artifact(
    branch: &GitBranch,
    selected: &ArtifactSurface,
    installed: &ArtifactSurface,
) -> Result<SurfaceCommitment, PrepareError> {
    ensure_artifact_tree(branch, selected, installed.tree_key())?;
    SurfaceCommitment::from_artifact_surfaces(selected, installed).map_err(history_prepare_error)
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

fn child_instance_targets_root(node_dir: &Path) -> PathBuf {
    node_dir.join("instance-targets")
}

fn prepare_child_instance_target_cache(
    node: &crate::intervention::Prototype1NodeRecord,
    treatment_campaign_id: &CampaignId,
) -> Result<PathBuf, PrepareError> {
    let root = child_instance_targets_root(&node.node_dir);
    let repo_cache = root.join(treatment_campaign_id);
    ensure_node_child_path(&node.node_dir, &repo_cache)?;
    if repo_cache.exists() {
        fs::remove_dir_all(&repo_cache).map_err(|source| PrepareError::WriteManifest {
            path: repo_cache.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&repo_cache).map_err(|source| PrepareError::CreateOutputDir {
        path: repo_cache.clone(),
        source,
    })?;
    Ok(repo_cache)
}

fn validate_treatment_patch_projection(
    node: &crate::intervention::Prototype1NodeRecord,
    treatment: &Prototype1TreatmentEvidence,
) -> Result<(), PrepareError> {
    for instance in &treatment.instances {
        let Some(metrics) = instance.metrics.as_ref() else {
            continue;
        };
        if metrics.submission_artifact_state == SubmissionArtifactState::Nonempty
            && metrics.patch_projection_check_state != PatchProjectionCheckState::Passed
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "treatment '{}' instance '{}' produced nonempty MBE submission without passed patch projection check ({:?})",
                    treatment.treatment_campaign_id,
                    instance.instance_id,
                    metrics.patch_projection_check_state
                ),
            });
        }
        if metrics.submission_artifact_state == SubmissionArtifactState::Nonempty {
            validate_child_patch_projection_checkout(node, instance)?;
        }
    }
    Ok(())
}

fn maybe_attach_treatment_oracle(
    baseline_manifest_path: &Path,
    node: &crate::intervention::Prototype1NodeRecord,
    treatment_state: &crate::closure::ClosureState,
    treatment: &mut Prototype1TreatmentEvidence,
) -> Result<(), PrepareError> {
    if treatment.benchmark_family != crate::target_registry::BenchmarkFamily::MultiSweBenchRust {
        return Ok(());
    }
    let Some(admitted) =
        crate::cli::prototype1_state::profile::load_admitted_run_profile(baseline_manifest_path)?
    else {
        return Ok(());
    };
    let mbe = admitted.profile.execution.mbe;
    if !mbe.enabled {
        return Ok(());
    }
    let target_instances = admitted.profile.target.eval_instances();
    if target_instances.is_empty() {
        return Err(PrepareError::InvalidMbeRequest {
            detail: "execution.mbe.enabled = true requires target.instance or target.instances"
                .to_string(),
        });
    }

    let target_set = target_instances
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if target_set.len() != target_instances.len() {
        return Err(PrepareError::InvalidMbeRequest {
            detail: "MBE target instance set contains duplicate instance ids".to_string(),
        });
    }
    let mut target_state = treatment_state.clone();
    target_state
        .instances
        .retain(|row| target_set.contains(&row.instance_id));
    if target_state.instances.len() != target_set.len() {
        let present = target_state
            .instances
            .iter()
            .map(|row| row.instance_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let missing = target_set
            .iter()
            .find(|instance_id| !present.contains(instance_id.as_str()))
            .cloned()
            .unwrap_or_else(|| "<unknown>".to_string());
        return Err(PrepareError::InvalidMbeRequest {
            detail: format!(
                "treatment '{}' is missing configured MBE target instance '{}'",
                treatment.treatment_campaign_id, missing
            ),
        });
    }
    let request = crate::mbe::CohortRequest::from_treatment_state(
        node,
        &target_state,
        None,
        None,
        treatment_mbe_options(mbe.workers),
    )?;
    let run = request.run_harness(mbe.python)?;
    attach_treatment_oracle_evaluations(treatment, &target_instances, run.evaluations)
}

fn attach_treatment_oracle_evaluations(
    treatment: &mut Prototype1TreatmentEvidence,
    target_instances: &[String],
    evaluations: Vec<crate::mbe::OracleEvaluation>,
) -> Result<(), PrepareError> {
    let target_set = target_instances
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if target_set.is_empty() {
        return Err(PrepareError::InvalidMbeRequest {
            detail: "MBE oracle attachment requires at least one configured target instance"
                .to_string(),
        });
    }
    if target_set.len() != target_instances.len() {
        return Err(PrepareError::InvalidMbeRequest {
            detail: "MBE oracle attachment target set contains duplicate instance ids".to_string(),
        });
    }

    let mut evaluations_by_instance = std::collections::BTreeMap::new();
    for evaluation in evaluations {
        let instance_id = evaluation.evidence.instance_id.clone();
        if !target_set.contains(&instance_id) {
            return Err(PrepareError::InvalidMbeRequest {
                detail: format!(
                    "treatment '{}' produced oracle evaluation for unknown instance '{}'",
                    treatment.treatment_campaign_id, instance_id
                ),
            });
        }
        if evaluations_by_instance
            .insert(instance_id.clone(), evaluation)
            .is_some()
        {
            return Err(PrepareError::InvalidMbeRequest {
                detail: format!(
                    "treatment '{}' produced duplicate oracle evaluations for instance '{}'",
                    treatment.treatment_campaign_id, instance_id
                ),
            });
        }
    }

    let mut attached = std::collections::BTreeSet::new();
    for instance in &mut treatment.instances {
        if !target_set.contains(&instance.instance_id) {
            continue;
        }
        let Some(evaluation) = evaluations_by_instance.remove(&instance.instance_id) else {
            return Err(PrepareError::InvalidMbeRequest {
                detail: format!(
                    "treatment '{}' is missing oracle evaluation for configured instance '{}'",
                    treatment.treatment_campaign_id, instance.instance_id
                ),
            });
        };
        instance.oracle_evaluation = Some(evaluation);
        attached.insert(instance.instance_id.clone());
    }

    if let Some(missing) = target_set
        .iter()
        .find(|instance_id| !attached.contains(*instance_id))
        .cloned()
    {
        return Err(PrepareError::InvalidMbeRequest {
            detail: format!(
                "treatment '{}' is missing configured MBE target instance '{}'",
                treatment.treatment_campaign_id, missing
            ),
        });
    }

    Ok(())
}

fn treatment_mbe_options(workers: u32) -> crate::mbe::Options {
    let mut options = crate::mbe::Options::default();
    options.workers = crate::mbe::Workers {
        general: workers,
        build_image: workers,
        run_instance: workers,
    };
    options
}

fn validate_child_patch_projection_checkout(
    node: &crate::intervention::Prototype1NodeRecord,
    instance: &crate::cli::prototype1_state::cli_facing::Prototype1TreatmentInstanceEvidence,
) -> Result<(), PrepareError> {
    let registration_path =
        instance
            .registration_path
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "instance '{}' has nonempty MBE submission but no run registration path",
                    instance.instance_id
                ),
            })?;
    let registration =
        RunRegistration::load(registration_path).map_err(|source| PrepareError::ReadManifest {
            path: registration_path.clone(),
            source: std::io::Error::other(source.to_string()),
        })?;
    let projection_path = registration
        .artifacts
        .patch_projection
        .as_ref()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "instance '{}' has nonempty MBE submission but no patch projection artifact",
                instance.instance_id
            ),
        })?;
    let text =
        fs::read_to_string(projection_path).map_err(|source| PrepareError::ReadManifest {
            path: projection_path.clone(),
            source,
        })?;
    let projection: BenchmarkPatchProjectionRecord =
        serde_json::from_str(&text).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_patch_projection_decode",
            detail: format!(
                "failed to parse patch projection '{}': {source}",
                projection_path.display()
            ),
        })?;
    let targets_root = child_instance_targets_root(&node.node_dir);
    if !projection.checkout.cwd.starts_with(&targets_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "patch projection cwd '{}' is outside child instance target root '{}'",
                projection.checkout.cwd.display(),
                targets_root.display()
            ),
        });
    }
    if projection.checkout.cwd.starts_with(&node.workspace_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "patch projection cwd '{}' must not be inside child Artifact worktree '{}'",
                projection.checkout.cwd.display(),
                node.workspace_root.display()
            ),
        });
    }
    Ok(())
}

pub(crate) fn cleanup_prototype1_child_build_products(
    manifest_path: &Path,
    campaign_id: &CampaignId,
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
    remove_node_target(manifest_path, campaign_id, node, &target_dir)?;
    remove_child_instance_targets(
        manifest_path,
        campaign_id,
        node,
        &child_instance_targets_root(&node.node_dir),
    )?;
    remove_broad_harness_targets(manifest_path, campaign_id, node)
}

fn remove_node_target(
    manifest_path: &Path,
    campaign_id: &CampaignId,
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

fn remove_child_instance_targets(
    manifest_path: &Path,
    campaign_id: &CampaignId,
    node: &crate::intervention::Prototype1NodeRecord,
    targets_root: &Path,
) -> Result<(), PrepareError> {
    ensure_node_child_path(&node.node_dir, targets_root)?;
    match fs::remove_dir_all(targets_root) {
        Ok(()) => observe::Step::start(observe::span!(
            "prototype1.cleanup.instance_targets",
            campaign_id = %campaign_id,
            node_id = %node.node_id,
            generation = node.generation,
            manifest_path = %manifest_path.display(),
            path = %targets_root.display(),
        ))
        .removed(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.instance_targets",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %targets_root.display(),
            ))
            .missing()
        }
        Err(source) => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.instance_targets",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %targets_root.display(),
            ))
            .fail("child_instance_targets_remove", &source);
            return Err(PrepareError::WriteManifest {
                path: targets_root.to_path_buf(),
                source,
            });
        }
    }
    Ok(())
}

fn broad_harness_workspace_root(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1/workspaces/edit-harness")
}

fn remove_broad_harness_targets(
    manifest_path: &Path,
    campaign_id: &CampaignId,
    node: &crate::intervention::Prototype1NodeRecord,
) -> Result<(), PrepareError> {
    let workspace_root = broad_harness_workspace_root(manifest_path);
    match fs::read_dir(&workspace_root) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|source| PrepareError::WriteManifest {
                    path: workspace_root.clone(),
                    source,
                })?;
                let entry_path = entry.path();
                let file_type =
                    entry
                        .file_type()
                        .map_err(|source| PrepareError::WriteManifest {
                            path: entry_path.clone(),
                            source,
                        })?;
                if !file_type.is_dir() {
                    continue;
                }
                let target_dir = entry_path.join("target");
                remove_broad_harness_target(manifest_path, campaign_id, node, &target_dir)?;
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.edit_harness_targets",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %workspace_root.display(),
            ))
            .missing();
        }
        Err(source) => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.edit_harness_targets",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %workspace_root.display(),
            ))
            .fail("edit_harness_targets_read", &source);
            return Err(PrepareError::WriteManifest {
                path: workspace_root,
                source,
            });
        }
    }
    Ok(())
}

fn remove_broad_harness_target(
    manifest_path: &Path,
    campaign_id: &CampaignId,
    node: &crate::intervention::Prototype1NodeRecord,
    target_dir: &Path,
) -> Result<(), PrepareError> {
    let workspace_root = broad_harness_workspace_root(manifest_path);
    if !target_dir.starts_with(&workspace_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "refusing to cleanup edit-harness target '{}' outside '{}'",
                target_dir.display(),
                workspace_root.display()
            ),
        });
    }
    let file_type = match fs::symlink_metadata(target_dir) {
        Ok(metadata) => metadata.file_type(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.edit_harness_target",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %target_dir.display(),
            ))
            .missing();
            return Ok(());
        }
        Err(source) => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.edit_harness_target",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %target_dir.display(),
            ))
            .fail("edit_harness_target_stat", &source);
            return Err(PrepareError::WriteManifest {
                path: target_dir.to_path_buf(),
                source,
            });
        }
    };
    if file_type.is_symlink() {
        match fs::remove_file(target_dir) {
            Ok(()) => observe::Step::start(observe::span!(
                "prototype1.cleanup.edit_harness_target",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %target_dir.display(),
            ))
            .removed(),
            Err(source) => {
                observe::Step::start(observe::span!(
                    "prototype1.cleanup.edit_harness_target",
                    campaign_id = %campaign_id,
                    node_id = %node.node_id,
                    generation = node.generation,
                    manifest_path = %manifest_path.display(),
                    path = %target_dir.display(),
                ))
                .fail("edit_harness_target_symlink_remove", &source);
                return Err(PrepareError::WriteManifest {
                    path: target_dir.to_path_buf(),
                    source,
                });
            }
        }
        return Ok(());
    }
    if !file_type.is_dir() {
        return Ok(());
    }
    match fs::remove_dir_all(target_dir) {
        Ok(()) => observe::Step::start(observe::span!(
            "prototype1.cleanup.edit_harness_target",
            campaign_id = %campaign_id,
            node_id = %node.node_id,
            generation = node.generation,
            manifest_path = %manifest_path.display(),
            path = %target_dir.display(),
        ))
        .removed(),
        Err(source) => {
            observe::Step::start(observe::span!(
                "prototype1.cleanup.edit_harness_target",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                manifest_path = %manifest_path.display(),
                path = %target_dir.display(),
            ))
            .fail("edit_harness_target_remove", &source);
            return Err(PrepareError::WriteManifest {
                path: target_dir.to_path_buf(),
                source,
            });
        }
    }
    Ok(())
}

pub(crate) fn persist_prototype1_buildable_child_artifact(
    campaign_id: &CampaignId,
    campaign_manifest_path: &Path,
    active_parent_root: &Path,
    current_parent: &ParentIdentity,
    node: &crate::intervention::Prototype1NodeRecord,
    resolved: &ResolvedTreatmentBranch,
) -> Result<ArtifactSurface, PrepareError> {
    let backend = GitWorktreeBackend;
    let workspace =
        child_artifact_workspace(&backend, campaign_manifest_path, node).map_err(|source| {
            PrepareError::DatabaseSetup {
                phase: "prototype1_child_artifact_prepare",
                detail: source.to_string(),
            }
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
    let identity = ParentIdentity::from_node(
        campaign_id.clone(),
        node,
        Some(current_parent),
        Some(workspace.branch.0.clone()),
    );
    let surface = validate_child_surface(
        active_parent_root,
        &workspace.root,
        "prototype1_child_surface_commitment_after_persist",
    )?;
    let artifact_surface = backend
        .artifact_surface(&workspace.root)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_child_artifact_surface_after_persist",
            detail: source.to_string(),
        })?;
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
            campaign_id: campaign_id.clone(),
            parent_identity: Some(current_parent.clone()),
            child_identity: identity,
            node_id: node.node_id.clone(),
            generation: node.generation,
            target_relpath: node.target_relpath.clone(),
            child_branch: workspace.branch.0.clone(),
            target_commit: target_commit.0,
            identity_commit: None,
        }),
        "prototype1_child_artifact_journal",
    )?;
    let expected_surface = SurfaceCommitment::from_artifact_surfaces(
        &GitWorktreeBackend
            .artifact_surface(active_parent_root)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_parent_artifact_surface_for_child",
                detail: source.to_string(),
            })?,
        &artifact_surface,
    )
    .map_err(history_prepare_error)?;
    if surface != expected_surface {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "persisted child Artifact surface transition mismatch for node {}",
                node.node_id
            ),
        });
    }
    Ok(artifact_surface)
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

// ANCHOR: prototype1_spawn_and_handoff_successor
pub(crate) fn spawn_and_handoff_prototype1_successor(
    campaign_id: &CampaignId,
    selected: selection::Selection<selection::Artifact>,
    active_parent_root: &Path,
    parent: Parent<Selectable>,
    selection_entry: crate::cli::prototype1_state::history::SelectionDecisionEntry,
    mode: SuccessorHandoffMode,
) -> Result<(Parent<Retired>, Option<Prototype1SuccessorHandoff>), PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let artifact = selected.selected();
    let node = artifact.node();
    let (active_successor_binary_path, installed) = prepare_prototype1_active_successor_runtime(
        campaign_id,
        &manifest_path,
        &selected,
        active_parent_root,
        parent.identity(),
    )?;
    let runtime_id = RuntimeId::new();
    let successor_artifact = installed.artifact_ref.clone();
    let parent_actor = parent_actor_ref(parent.identity());
    let handoff_block =
        handoff_block_fields(campaign_id, parent.identity(), &manifest_path, &installed)?;
    let seal = SealBlock::from_handoff(
        EvidenceRef::new(format!("prototype1:successor-handoff:{runtime_id}")),
        SuccessorRef::new(ActorRef::Runtime(runtime_id), successor_artifact.clone()),
        installed.parent_identity.clone(),
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
        campaign_id.clone(),
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
                campaign_id: campaign_id.clone(),
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
                    campaign_id: campaign_id.clone(),
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
// ANCHOR_END: prototype1_spawn_and_handoff_successor

struct HandoffBlock {
    open: OpenBlock,
    expected_state: LineageState,
    artifact_key: TreeKeyHash,
}

// ANCHOR: prototype1_handoff_block_fields
fn handoff_block_fields(
    campaign_id: &CampaignId,
    parent_identity: &ParentIdentity,
    manifest_path: &Path,
    installed: &InstalledSuccessorArtifact,
) -> Result<HandoffBlock, PrepareError> {
    let store = FsBlockStore::for_campaign_manifest(manifest_path);
    let lineage_id = LineageId::new(campaign_id.clone());
    let state = store
        .lineage_state(&lineage_id)
        .map_err(block_store_prepare_error)?;
    let parent_actor = parent_actor_ref(parent_identity);
    let artifact_key = installed.artifact_key.clone();
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
            opened_from_artifact: installed.artifact_ref.clone(),
            ruling_authority: parent_actor,
            policy_ref: ProcedureRef::new("prototype1:single-ruler-local:v1"),
            surface: installed.surface.clone(),
            opened_at: RecordedAt::now(),
        },
        expected_state: state,
        artifact_key,
    })
}
// ANCHOR_END: prototype1_handoff_block_fields

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

/// Construct a persisted runner result for failure after the child binary
/// exists but before a successful evaluation report is produced.
fn build_treatment_failed_runner_result(
    campaign_id: &CampaignId,
    node: &crate::intervention::Prototype1NodeRecord,
    detail: impl Into<String>,
    exit_code: Option<i32>,
    stdout_excerpt: Option<String>,
    stderr_excerpt: Option<String>,
) -> Prototype1RunnerResult {
    Prototype1RunnerResult {
        schema_version: crate::intervention::PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.clone(),
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
    campaign_id: &CampaignId,
    node: &crate::intervention::Prototype1NodeRecord,
    treatment: &Prototype1TreatmentEvidence,
) -> Prototype1RunnerResult {
    Prototype1RunnerResult {
        schema_version: crate::intervention::PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.clone(),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Succeeded,
        disposition: Prototype1RunnerDisposition::Succeeded,
        treatment_campaign_id: Some(treatment.treatment_campaign_id.clone()),
        evaluation_artifact_path: None,
        detail: None,
        exit_code: Some(0),
        stdout_excerpt: None,
        stderr_excerpt: None,
        recorded_at: Utc::now().to_rfc3339(),
    }
}

fn record_attempt_runner_result(
    _campaign_id: &CampaignId,
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
    let node = invocation.node_record()?.clone();
    let request = invocation.runner_request()?.clone();
    let resolved = invocation.resolved()?.clone();

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %invocation.campaign_id(),
        node_id = %invocation.node_id(),
        workspace_root = %request.workspace_root.display(),
        invocation_path = %invocation_path.display(),
        "loaded executable prototype1 child invocation"
    );

    let child = Child::new(
        invocation.journal_path().to_path_buf(),
        invocation.runtime_id(),
        node.generation,
        Refs {
            campaign_id: invocation.campaign_id().clone(),
            node_id: invocation.node_id().to_string(),
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            branch_id: node.branch_id.clone(),
            candidate_id: node.candidate_id.clone(),
            branch_label: resolved.branch.branch_label.clone(),
            spec_id: resolved.branch.synthesized_spec_id.clone(),
        },
        Paths {
            repo_root: request.workspace_root.clone(),
            workspace_root: request.workspace_root.clone(),
            binary_path: request.binary_path.clone(),
            target_relpath: request.target_relpath.clone(),
            absolute_path: request.workspace_root.join(&request.target_relpath),
        },
        std::process::id(),
    );
    let channel = invocation
        .channel_endpoints()
        .map(|endpoints| Channel::for_child(&child, endpoints, FileTransport));
    let child = child.ready().map_err(|err| PrepareError::DatabaseSetup {
        phase: "prototype1_child_ready",
        detail: err.to_string(),
    })?;
    let channel = match channel {
        Some(channel) => Some(
            channel
                .send_ready()
                .map_err(|err| PrepareError::DatabaseSetup {
                    phase: "prototype1_child_channel_ready",
                    detail: format!("{err:?}"),
                })?
                .0,
        ),
        None => None,
    };
    let child = child
        .evaluating()
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "prototype1_child_evaluating",
            detail: err.to_string(),
        })?;
    let channel = match channel {
        Some(channel) => Some(
            channel
                .send_evaluating()
                .map_err(|err| PrepareError::DatabaseSetup {
                    phase: "prototype1_child_channel_evaluating",
                    detail: format!("{err:?}"),
                })?
                .0,
        ),
        None => None,
    };

    let outcome = run_prototype1_resolved_branch_treatment(
        invocation.campaign_id(),
        &manifest_path,
        &resolved,
        &node,
        &request.workspace_root,
        request.stop_on_error,
    )
    .await;

    // Treatment evidence leaves the child only through the terminal channel
    // result. Attempt result files and Child<ResultWritten> are reconstruction
    // projections, so failures carry no treatment payload.
    let (result, treatment) = match outcome {
        Ok(evidence) => (
            build_succeeded_runner_result(invocation.campaign_id(), &node, &evidence),
            Some(evidence),
        ),
        Err(err) => (
            build_treatment_failed_runner_result(
                invocation.campaign_id(),
                &node,
                err.to_string(),
                None,
                None,
                None,
            ),
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
    let _child = child
        .result_written(runner_result_path.clone())
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "prototype1_child_result_written",
            detail: err.to_string(),
        })?;
    if let Some(channel) = channel {
        let _ = channel.send_terminal_result(result.clone(), treatment);
    }
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

pub(super) async fn run_prototype1_resolved_branch_treatment(
    baseline_campaign_id: &CampaignId,
    baseline_manifest_path: &Path,
    resolved_branch: &ResolvedTreatmentBranch,
    node: &crate::intervention::Prototype1NodeRecord,
    repo_root: &Path,
    stop_on_error: bool,
) -> Result<Prototype1TreatmentEvidence, PrepareError> {
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
        let treatment_campaign = step!(
            "prototype1.child.evaluate.prepare_treatment_campaign",
            "PrepareTreatmentCampaign",
            || prepare_prototype1_treatment_campaign(&baseline_resolved, branch_id),
        )?;
        let instance_repo_cache = step!(
            "prototype1.child.evaluate.prepare_instance_target",
            "PrepareInstanceTarget",
            || prepare_child_instance_target_cache(node, &treatment_campaign.campaign_id),
            path = %node.node_dir.display(),
        )?;
        let mut eval_policy = treatment_campaign.resolved.eval.clone();
        if stop_on_error {
            eval_policy.stop_on_error = true;
        }
        async_step!(
            "prototype1.child.evaluate.eval_closure",
            "EvalClosure",
            advance_eval_closure(
                &treatment_campaign.resolved,
                &eval_policy,
                false,
                Some(instance_repo_cache.as_path()),
            ),
            treatment_campaign_id = %treatment_campaign.campaign_id,
            instance_repo_cache = %instance_repo_cache.display(),
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
        let mut treatment = step!(
            "prototype1.child.evaluate.treatment_evidence",
            "BuildTreatmentEvidence",
            || {
                build_prototype1_treatment_evidence(
                    baseline_campaign_id,
                    branch_id,
                    &treatment_campaign,
                    &treatment_state,
                )
            },
            treatment_campaign_id = %treatment_campaign.campaign_id,
        )?;
        step!(
            "prototype1.child.evaluate.complete_treatment_gate",
            "CompleteTreatmentGate",
            || require_complete_treatment(&treatment),
            treatment_campaign_id = %treatment_campaign.campaign_id,
        )?;
        step!(
            "prototype1.child.evaluate.patch_projection_gate",
            "PatchProjectionGate",
            || validate_treatment_patch_projection(node, &treatment),
            treatment_campaign_id = %treatment_campaign.campaign_id,
        )?;
        step!(
            "prototype1.child.evaluate.mbe_oracle",
            "MbeOracle",
            || maybe_attach_treatment_oracle(
                baseline_manifest_path,
                node,
                &treatment_state,
                &mut treatment,
            ),
            treatment_campaign_id = %treatment_campaign.campaign_id,
        )?;

        Ok(treatment)
    }
    .await;

    match outcome {
        Ok(treatment) => {
            step.success();
            Ok(treatment)
        }
        Err(error) => {
            step.fail("self_evaluation", &error);
            Err(error)
        }
    }
}

fn require_complete_treatment(treatment: &Prototype1TreatmentEvidence) -> Result<(), PrepareError> {
    if treatment.instances.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "treatment '{}' produced no instance evidence",
                treatment.treatment_campaign_id
            ),
        });
    }

    // Missing metrics means the closure state could not resolve a complete
    // treatment run record. Do not emit a success-shaped channel result without
    // the treatment evidence the parent needs for comparison.
    for instance in &treatment.instances {
        if instance.metrics.is_none() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "treatment '{}' instance '{}' did not produce complete run metrics (status={})",
                    treatment.treatment_campaign_id, instance.instance_id, instance.status
                ),
            });
        }
    }

    Ok(())
}
