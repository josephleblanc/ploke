use super::*;

use crate::cli::prototype1_state::cli_facing::CandidateGenerationConfig;
use crate::cli::prototype1_state::edit_surface::harness_request::{
    PublishedBroadHarnessRequest, RequestAdmissionBinding,
};
use crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId;
use crate::cli::{
    InspectOutputFormat, Prototype1CandidateGenerator, Prototype1StateCommand,
    Prototype1StateStopAfter, Prototype1SuccessorSelection, Prototype1TraversalMetrics,
};
use crate::intervention::Prototype1NodeRecord;
use ploke_records::identity::ParentIdentityRecord;
use std::sync::{Arc, Mutex};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::field::{Field, Visit};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

use crate::intervention::{
    PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1SearchPolicy, TreatmentBranchNode,
    TreatmentBranchStatus,
};
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};

#[derive(Clone, Default)]
struct TraceLines(Arc<Mutex<Vec<String>>>);

impl TraceLines {
    fn push(&self, line: String) {
        self.0.lock().expect("trace lock").push(line);
    }

    fn snapshot(&self) -> Vec<String> {
        self.0.lock().expect("trace lock").clone()
    }
}

#[derive(Default)]
struct TraceFields {
    values: Vec<String>,
}

impl TraceFields {
    fn push(&mut self, field: &Field, value: impl Into<String>) {
        self.values
            .push(format!("{}={}", field.name(), value.into()));
    }

    fn finish(self) -> String {
        self.values.join(" ")
    }
}

impl Visit for TraceFields {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.push(field, format!("{value:?}"));
    }
}

struct TraceLayer {
    lines: TraceLines,
}

impl<S> Layer<S> for TraceLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
        let mut fields = TraceFields::default();
        attrs.record(&mut fields);
        self.lines.push(format!(
            "span:{} {}",
            attrs.metadata().name(),
            fields.finish()
        ));
    }

    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = TraceFields::default();
        event.record(&mut fields);
        self.lines.push(format!(
            "event:{} {}",
            event.metadata().target(),
            fields.finish()
        ));
    }
}

fn collect_traces<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
    let lines = TraceLines::default();
    let subscriber = Registry::default().with(TraceLayer {
        lines: lines.clone(),
    });
    let guard = tracing::subscriber::set_default(subscriber);
    let result = f();
    drop(guard);
    (result, lines.snapshot())
}

async fn collect_traces_async<T>(f: impl std::future::Future<Output = T>) -> (T, Vec<String>) {
    let lines = TraceLines::default();
    let subscriber = Registry::default().with(TraceLayer {
        lines: lines.clone(),
    });
    let guard = tracing::subscriber::set_default(subscriber);
    let result = f.await;
    drop(guard);
    (result, lines.snapshot())
}

fn trace_contains(lines: &[String], needles: &[&str]) -> bool {
    lines
        .iter()
        .any(|line| needles.iter().all(|needle| line.contains(needle)))
}

fn dump_trace_if_requested(trace: &[String]) {
    if std::env::var_os("PLOKE_TEST_LOG").is_some() {
        for line in trace {
            eprintln!("trace: {line}");
        }
    }
}

fn json_fixture<T>(text: &str) -> T
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(text).expect("fixture deserializes")
}

fn state_command_without_ids() -> Prototype1StateCommand {
    Prototype1StateCommand {
        campaign: None,
        node_id: None,
        repo_root: None,
        init_parent_identity: false,
        identity_branch: None,
        identity_instance: None,
        handoff_invocation: None,
        stop_after: Prototype1StateStopAfter::Complete,
        successor_selection: Prototype1SuccessorSelection::HistoryScoreChildProp,
        successor_selection_seed: 0,
        successor_selection_metrics: Prototype1TraversalMetrics::Operational,
        candidate_generator: Prototype1CandidateGenerator::BroadHarnessRequest,
        format: InspectOutputFormat::Table,
    }
}

#[test]
fn candidate_generation_config_dispatches_broad_harness_surface_by_default() {
    let command = state_command_without_ids();
    let config = CandidateGenerationConfig::from_command(&command);

    assert_eq!(config, CandidateGenerationConfig::BroadHarnessRequest);
}

#[test]
fn complete_child_budget_reserves_remaining_node_slots() {
    let policy = Prototype1SearchPolicy {
        max_total_nodes: 20,
        child_budget: Prototype1ChildBudget::new(2, 6),
        ..Prototype1SearchPolicy::default()
    };

    let reserved =
        reserve_complete_child_budget(&policy, 17).expect("remaining slots should reserve");

    assert_eq!(reserved, Prototype1ChildBudget::new(2, 3));
}

#[test]
fn complete_child_budget_rejects_below_min_remaining_slots() {
    let policy = Prototype1SearchPolicy {
        max_total_nodes: 20,
        child_budget: Prototype1ChildBudget::new(4, 4),
        ..Prototype1SearchPolicy::default()
    };

    let error = reserve_complete_child_budget(&policy, 17).expect_err("remaining slots below min");

    assert!(
        error
            .to_string()
            .contains("remaining node slots 3 are below required min children 4"),
        "{error}"
    );
}

#[test]
fn complete_child_budget_rejects_reached_total_node_limit() {
    let policy = Prototype1SearchPolicy {
        max_total_nodes: 20,
        child_budget: Prototype1ChildBudget::new(2, 6),
        ..Prototype1SearchPolicy::default()
    };

    let error = reserve_complete_child_budget(&policy, 20).expect_err("limit reached");

    assert!(
        error
            .to_string()
            .contains("persisted node count 20 has reached max_total_nodes 20"),
        "{error}"
    );
}

fn test_manifest_path(root: &Path) -> PathBuf {
    root.join("campaign.toml")
}

fn write_test_node(manifest_path: &Path, node: &Prototype1NodeRecord) {
    let node_dir = prototype1_nodes_dir(manifest_path).join(&node.node_id);
    fs::create_dir_all(&node_dir).expect("node dir");
    fs::write(
        node_dir.join("node.json"),
        serde_json::to_vec_pretty(node).expect("node json"),
    )
    .expect("write node");
}

fn parent_identity_for(node_id: &str, generation: u32) -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
            .to_string(),
        campaign_id: "campaign".to_string(),
        parent_id: node_id.to_string(),
        node_id: node_id.to_string(),
        generation,
        instance_id: Some("clap-rs__clap-3670".to_string()),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: format!("branch-{node_id}"),
        artifact_branch: Some(format!("prototype1-{node_id}")),
        created_at: "2026-05-06T00:00:00Z".to_string(),
    })
}

fn append_parent_started(manifest_path: &Path, identity: ParentIdentity) {
    let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(manifest_path));
    journal
        .append(JournalEntry::ParentStarted(ParentStartedEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: identity.campaign_id().to_string(),
            parent_identity: identity,
            repo_root: manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            handoff_runtime_id: None,
            pid: 1,
        }))
        .expect("append parent started");
}

fn selection_material_from_history() -> SelectionSealMaterial {
    SelectionSealMaterial {
        procedure: ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
        scope: SelectionScope::all_admitted_candidates(),
        selected_candidate: SubjectRef::new("candidate:node-history:plan_index=0"),
        selected_occurrence_id: None,
        selected_membership_id: None,
        considered: Vec::new(),
        considered_sources: Vec::new(),
        projection_failures: Vec::new(),
        traversal: Some(TraversalEvidence {
            seed: 0,
            strategy: StrategyKind::default(),
            selected_source: Some(TraversalCandidateSource::History),
        }),
        metrics: selection_metrics_for(&[], &[]),
        selected_from_generation_outcomes: false,
    }
}

fn selection_metrics_for(
    considered: &[EvaluationPayload],
    sources: &[TraversalCandidateSource],
) -> crate::successor_selection::metrics::Set {
    crate::successor_selection::metrics::Set::from_considered(
        crate::successor_selection::metrics::Policy::default(),
        considered,
        sources,
    )
    .expect("selection metrics")
}

fn successor_decision_for(node: &Prototype1NodeRecord) -> SuccessorDecision {
    SuccessorDecision {
        procedure_id: crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID.to_string(),
        candidate_node_id: node.node_id.clone(),
        selected_branch_id: Some(node.branch_id.clone()),
        branch_disposition: "keep".to_string(),
        outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
        findings: Vec::new(),
        rationale: Vec::new(),
    }
}

#[test]
fn historical_selection_can_continue_when_unspent_and_bounded() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = test_manifest_path(tmp.path());
    let parent = parent_identity_for("node-current", 1);
    append_parent_started(&manifest_path, parent.clone());
    let mut node = test_node(tmp.path(), "node-history", "branch-history", "candidate-1");
    node.generation = 1;
    node.parent_node_id = Some("node-root".to_string());
    write_test_node(&manifest_path, &node);

    let policy = Prototype1SearchPolicy {
        max_generations: 15,
        max_total_nodes: 96,
        ..Prototype1SearchPolicy::default()
    };
    let decision = live_successor_continuation_decision(
        &manifest_path,
        &parent,
        &policy,
        &successor_decision_for(&node),
        &selection_material_from_history(),
        &node,
    )
    .expect("continuation decision");

    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::ContinueHistoricalTraversal
    );
    assert!(decision.disposition.allows_successor());
}

#[test]
fn historical_selection_allows_archive_parent_revisit() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = test_manifest_path(tmp.path());
    let parent = parent_identity_for("node-current", 1);
    append_parent_started(&manifest_path, parent.clone());
    append_parent_started(&manifest_path, parent_identity_for("node-history", 1));
    let mut node = test_node(tmp.path(), "node-history", "branch-history", "candidate-1");
    node.generation = 1;
    node.parent_node_id = Some("node-root".to_string());
    write_test_node(&manifest_path, &node);

    let policy = Prototype1SearchPolicy {
        max_generations: 15,
        max_total_nodes: 96,
        ..Prototype1SearchPolicy::default()
    };

    let decision = live_successor_continuation_decision(
        &manifest_path,
        &parent,
        &policy,
        &successor_decision_for(&node),
        &selection_material_from_history(),
        &node,
    )
    .expect("continuation decision");

    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::ContinueHistoricalTraversal
    );
    assert!(decision.disposition.allows_successor());
}

#[test]
fn historical_selection_rejects_exhausted_parent_turn_budget() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = test_manifest_path(tmp.path());
    let parent = parent_identity_for("node-current", 1);
    append_parent_started(&manifest_path, parent_identity_for("node-root", 0));
    append_parent_started(&manifest_path, parent.clone());
    let mut node = test_node(tmp.path(), "node-history", "branch-history", "candidate-1");
    node.generation = 1;
    node.parent_node_id = Some("node-root".to_string());
    write_test_node(&manifest_path, &node);
    let policy = Prototype1SearchPolicy {
        max_generations: 1,
        ..Prototype1SearchPolicy::default()
    };

    let decision = live_successor_continuation_decision(
        &manifest_path,
        &parent,
        &policy,
        &successor_decision_for(&node),
        &selection_material_from_history(),
        &node,
    )
    .expect("continuation decision");

    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::StopHistoricalTraversalBudget
    );
    assert!(!decision.disposition.allows_successor());
}

#[test]
fn candidate_generation_config_dispatches_broad_harness_surface() {
    let mut command = state_command_without_ids();
    command.candidate_generator = Prototype1CandidateGenerator::BroadHarnessRequest;
    let config = CandidateGenerationConfig::from_command(&command);

    assert_eq!(config, CandidateGenerationConfig::BroadHarnessRequest);
}

#[test]
fn candidate_generation_config_dispatches_deterministic_tui_tools_fixture() {
    let mut command = state_command_without_ids();
    command.candidate_generator = Prototype1CandidateGenerator::DeterministicTuiTools;
    let config = CandidateGenerationConfig::from_command(&command);

    assert_eq!(config, CandidateGenerationConfig::DeterministicTuiTools);
}

#[test]
fn state_run_shape_prefers_admitted_campaign_profile() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let profile_text = r#"
schema_version = "prototype1-run-profile.v1"
name = "overnight-edit-surface"

[search]
max_generations = 15
max_total_nodes = 96
children = { min = 6, max = 6 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
explore_from_rejected = true

[generation]
source = "broad-harness-request"

[selection]
strategy = "history-score-child-prop"
evidence = "operational-and-protocol"
seed = 7

[execution]
stop_after = "complete"
observe_child_stale_after_secs = 17
"#;
    let parsed =
        toml::from_str::<profile::Prototype1RunProfile>(profile_text).expect("profile parses");
    parsed.validate().expect("profile validates");
    let operator = profile::OperatorRunProfile {
        source_path: tmp.path().join("source.toml"),
        profile: parsed,
    };
    profile::admit_run_profile(&manifest_path, &operator).expect("profile admitted");
    let command = state_command_without_ids();

    let shape =
        Prototype1StateRunShape::resolve(&command, &manifest_path).expect("run shape resolves");

    assert_eq!(
        shape.candidate_generation,
        CandidateGenerationConfig::BroadHarnessRequest
    );
    assert_eq!(
        shape.successor_selection_metrics,
        Prototype1TraversalMetrics::OperationalAndProtocol
    );
    assert_eq!(shape.successor_selection_seed, 7);
    assert_eq!(shape.observe_child_stale_after, Duration::from_secs(17));
    assert_eq!(
        shape.successor_oracle_mode,
        crate::successor_selection::OracleMode::RecordOnly
    );
    assert!(shape.successor_oracle_require_evidence);
}

#[test]
fn checked_edit_surface_candidate_is_accepted_by_tui_child_plan_consumer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    init_indexed_repo(tmp.path());
    let relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let target = tmp.path().join(&relpath);
    fs::create_dir_all(target.parent().expect("target has parent")).expect("create target dir");
    fs::write(&target, "let old = 1;\n").expect("write target");
    index_repo(tmp.path());
    let source_hash = format!("{:x}", Sha256::digest("let old = 1;\n".as_bytes()));
    let base_artifact_id = crate::intervention::text_file_artifact_id(&relpath, "let old = 1;\n");
    let checked = GitWorktreeBackend
            .validate_edit_surface_candidate(
                tmp.path(),
                test_edit_surface_admission(
                    Prototype1EditSurface::PlokeTuiTools,
                    base_artifact_id,
                ),
                crate::cli::prototype1_state::backend::EditProposal {
                    surface: Prototype1EditSurface::PlokeTuiTools,
                    proposal_id: "proposal-1".to_string(),
                    run_id: "run-1".to_string(),
                    proposal_producer:
                        crate::cli::prototype1_state::edit_surface::request_policy::ProposalProducer::NonRouter,
                    generator_surface: GitWorktreeBackend
                        .generator_surface_for_proposed_touches(
                            &relpath,
                            "let old = 1;\n",
                            &[crate::cli::prototype1_state::backend::ProposedTouch {
                                target: "code_edit".to_string(),
                                relpath: relpath.clone(),
                                start: 4,
                                end: 7,
                                expected_file_hash: source_hash.clone(),
                                replacement: "new".to_string(),
                            }],
                        )
                        .expect("generator surface"),
                    reported_after_file_hash: None,
                    touches: vec![crate::cli::prototype1_state::backend::ProposedTouch {
                        target: "code_edit".to_string(),
                        relpath: relpath.clone(),
                        start: 4,
                        end: 7,
                        expected_file_hash: source_hash,
                        replacement: "new".to_string(),
                    }],
                },
            )
            .expect("checked edit");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-child");
    node.target_relpath = relpath.clone();
    node.operation_target = None;
    node.base_artifact_id = None;
    node.patch_id = None;
    node.derived_artifact_id = None;

    let child = child_files_from_checked_edit(
        "campaign",
        Prototype1EditSurface::PlokeTuiTools,
        node,
        &checked,
        false,
    )
    .expect("child files");
    let resolved = child.resolved();
    let node = child.node_record();

    assert_eq!(resolved.target_relpath, relpath);
    assert_eq!(resolved.source_content, "let old = 1;\n");
    assert_eq!(resolved.branch.proposed_content, "let new = 1;\n");
    assert_eq!(resolved.branch.patch_id.as_ref(), Some(checked.patch_id()));
    assert_eq!(
        resolved.branch.derived_artifact_id.as_ref(),
        Some(checked.derived_artifact_id())
    );
    assert_eq!(
        node.base_artifact_id.as_ref(),
        Some(checked.base_artifact_id())
    );
    assert_eq!(node.patch_id.as_ref(), Some(checked.patch_id()));
    assert_eq!(
        node.derived_artifact_id.as_ref(),
        Some(checked.derived_artifact_id())
    );
    let surface = child.surface().expect("checked edit evidence");
    assert_eq!(surface.producer_id, TUI_EDIT_SURFACE_PRODUCER_ID);
    assert_eq!(surface.proposal_id, "proposal-1");
    assert_eq!(surface.run_id, "run-1");
    assert_eq!(surface.policy, TUI_EDIT_SURFACE_POLICY_ID);
    assert_eq!(surface.target_relpath, relpath);
    assert_eq!(&surface.base.artifact_id, checked.base_artifact_id());
    assert_eq!(&surface.after.artifact_id, checked.derived_artifact_id());
    assert_eq!(&surface.patch_id, checked.patch_id());
    assert_eq!(surface.source_content_hash, checked.source_content_hash());
    assert_eq!(
        surface.proposed_content_hash,
        checked.proposed_content_hash()
    );
    assert_eq!(surface.touches.len(), 1);
    assert_eq!(surface.touches[0].replacement, "new");
    assert!(surface.delta_id.starts_with("surface-delta:"));

    validate_requested_tui_surface_child(&child).expect("tui child-plan consumer accepts child");
}

#[test]
fn legacy_child_files_serialize_without_surface_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let node = test_node(tmp.path(), "node-child", "branch-child", "candidate-child");
    let child = ChildFiles::from_resolved("campaign", node.clone(), test_resolved(&node), false);

    let serialized = serde_json::to_value(&child).expect("child files json");

    assert!(serialized.get("surface").is_none());
    let decoded: ChildFiles = serde_json::from_value(serialized).expect("legacy child files");
    assert!(decoded.surface().is_none());
}

#[derive(Debug, Clone, Copy)]
struct NoopBackend;

impl WorkspaceBackend for NoopBackend {
    type Branch = String;
    type Head = String;
    type Root = PathBuf;
    type TreeKey = String;

    fn realize(
        &self,
        _request: &crate::cli::prototype1_state::backend::RealizeRequest,
    ) -> Result<
        crate::cli::prototype1_state::backend::Workspace<Self::Branch, Self::Head, Self::Root>,
        crate::cli::prototype1_state::backend::BackendError,
    > {
        unimplemented!("not needed by parent readiness tests")
    }

    fn remove(
        &self,
        _repo_root: &Path,
        _workspace: &crate::cli::prototype1_state::backend::Workspace<
            Self::Branch,
            Self::Head,
            Self::Root,
        >,
    ) -> Result<(), crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn workspace_for_node(
        &self,
        _node_id: &str,
        _node_dir: &Path,
        _workspace_root: &Path,
    ) -> Result<
        crate::cli::prototype1_state::backend::Workspace<Self::Branch, Self::Head, Self::Root>,
        crate::cli::prototype1_state::backend::BackendError,
    > {
        unimplemented!("not needed by parent readiness tests")
    }

    fn persist_workspace_target(
        &self,
        _workspace: &crate::cli::prototype1_state::backend::Workspace<
            Self::Branch,
            Self::Head,
            Self::Root,
        >,
        _target_relpath: &Path,
        _message: &str,
    ) -> Result<Self::Head, crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn persist_workspace_files(
        &self,
        _workspace: &crate::cli::prototype1_state::backend::Workspace<
            Self::Branch,
            Self::Head,
            Self::Root,
        >,
        _relpaths: &[PathBuf],
        _message: &str,
    ) -> Result<Self::Head, crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn verify_artifact_target(
        &self,
        _repo_root: &Path,
        _artifact: &Self::Branch,
        _target_relpath: &Path,
        _expected_content: &str,
    ) -> Result<(), crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn install_artifact_in_active_checkout(
        &self,
        _active_parent_root: &Path,
        _artifact: &Self::Branch,
    ) -> Result<Self::Head, crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn checkout_fresh_parent_branch(
        &self,
        _active_parent_root: &Path,
        _branch: &str,
    ) -> Result<Self::Head, crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn persist_active_checkout_files(
        &self,
        _active_parent_root: &Path,
        _relpaths: &[PathBuf],
        _message: &str,
    ) -> Result<Self::Head, crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn validate_parent_checkout(
        &self,
        _active_parent_root: &Path,
        _identity: &ParentIdentity,
    ) -> Result<(), crate::cli::prototype1_state::backend::BackendError> {
        Ok(())
    }

    fn clean_tree_key(
        &self,
        _active_parent_root: &Path,
    ) -> Result<Self::TreeKey, crate::cli::prototype1_state::backend::BackendError> {
        unimplemented!("not needed by parent readiness tests")
    }

    fn surface_commitment(
        &self,
        _before_root: &Path,
        _after_root: &Path,
    ) -> Result<
        crate::cli::prototype1_state::history::SurfaceCommitment,
        crate::cli::prototype1_state::backend::BackendError,
    > {
        unimplemented!("not needed by parent readiness tests")
    }
}

fn init_indexed_repo(repo_root: &Path) {
    fs::create_dir_all(repo_root).expect("create repo root");
    let status = std::process::Command::new("git")
        .current_dir(repo_root)
        .arg("init")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git init");
    assert!(status.success(), "git init failed");
}

fn index_repo(repo_root: &Path) {
    let status = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["add", "--all"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git add");
    assert!(status.success(), "git add failed");
}

fn commit_indexed_repo(repo_root: &Path, message: &str) {
    let status = std::process::Command::new("git")
        .current_dir(repo_root)
        .args([
            "-c",
            "user.email=prototype1-test@example.invalid",
            "-c",
            "user.name=Prototype1 Test",
            "commit",
            "--no-gpg-sign",
            "-m",
            message,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit failed");
}

fn write_surface_target(repo_root: &Path, relpath: &Path, content: &str) {
    let target = repo_root.join(relpath);
    fs::create_dir_all(target.parent().expect("target parent")).expect("create target dir");
    fs::write(target, content).expect("write target");
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();

    let mut relpath = PathBuf::new();
    for _ in common..from_components.len() {
        relpath.push("..");
    }
    for component in &to_components[common..] {
        relpath.push(component.as_os_str());
    }
    relpath
}

fn write_broad_surface_targets(repo_root: &Path) -> Vec<PathBuf> {
    init_indexed_repo(repo_root);
    let mut allowed = vec![
        PathBuf::from("crates/ploke-core/src/lib.rs"),
        PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
        PathBuf::from("crates/ploke-tui/src/rag/tools.rs"),
        PathBuf::from("crates/ploke-tui/src/rag/editing.rs"),
        PathBuf::from("docs/operator-note.md"),
    ];
    allowed.extend(
        ToolName::ALL
            .iter()
            .map(|tool| PathBuf::from(tool.description_artifact_relpath())),
    );
    allowed.sort();
    allowed.dedup();
    for relpath in &allowed {
        let content = if relpath.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            "pub fn sentinel() {}\n"
        } else {
            "surface fixture\n"
        };
        write_surface_target(repo_root, relpath, content);
    }
    write_surface_target(
        repo_root,
        Path::new("crates/ploke-eval/src/lib.rs"),
        "pub fn protected() {}\n",
    );
    index_repo(repo_root);
    allowed
}

fn ready_parent_for_test(manifest_path: &Path, repo_root: &Path) -> Parent<Ready> {
    let identity = test_parent_identity();
    let unchecked =
        Parent::<Unchecked>::load(manifest_path, identity.clone()).expect("load unchecked parent");
    let checked = unchecked
        .check(
            &NoopBackend,
            manifest_path,
            Check {
                campaign_id: &identity.campaign_id(),
                active_root: repo_root,
            },
        )
        .expect("checked parent");
    let startup =
        Startup::<Genesis>::from_history(checked.identity(), manifest_path).expect("startup");
    checked.ready(startup).expect("ready parent")
}

fn count_broad_requests(manifest_path: &Path) -> usize {
    let request_dir = prototype1_campaign_root(manifest_path).join("messages/edit-harness-request");
    match fs::read_dir(request_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .count(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => 0,
        Err(source) => panic!("read request dir: {source}"),
    }
}

fn test_broad_request_admission_binding() -> RequestAdmissionBinding {
    RequestAdmissionBinding::from_admission(&EditSurfaceAdmission::new(
        Coordinate {
            runtime_id: RuntimeId::new(),
            target: OperationTarget::Artifact {
                artifact_id: ArtifactId::new("artifact:node-parent-base"),
            },
        },
        SurfacePolicyId::new("workspace except ploke-eval"),
    ))
    .expect("construct request admission binding")
}

fn submitted_broad_harness_result_for_paths(
    published: &PublishedBroadHarnessRequest,
    changed_paths: &[PathBuf],
) -> SubmittedBroadHarnessResult {
    SubmittedBroadHarnessResult::bind(
        published,
        SubmittedHarnessReturnEvidence {
            authority_boundary: published
                .request()
                .return_evidence
                .authority_boundary
                .clone(),
            change_summary: SubmittedChangeSummary {
                changed_files: changed_paths
                    .iter()
                    .cloned()
                    .map(|workspace_relpath| SubmittedFileChange {
                        workspace_relpath,
                        summary: "candidate broad harness change".to_string(),
                    })
                    .collect(),
            },
            guiding_evidence: Vec::new(),
            rationale: SubmittedImprovementRationale {
                hypothesis: "multi-file candidate should become one Artifact".to_string(),
                expected_descendant_effect: "child materialization uses admitted artifact evidence"
                    .to_string(),
            },
            checks: Vec::new(),
        },
    )
    .expect("submitted broad harness result")
}

fn admit_broad_slot_for_test(
    repo_root: &Path,
    slot: &HarnessRequestSlot,
    changed_paths: &[PathBuf],
    label: &str,
) -> AdmittedBroadHarnessResult {
    GitWorktreeBackend
        .prepare_broad_harness_workspace(repo_root, &slot.published)
        .expect("prepare broad harness workspace");
    for relpath in changed_paths {
        write_surface_target(
            slot.published.workspace_path(),
            relpath,
            &format!("candidate edit {label} for {}\n", relpath.display()),
        );
    }
    let submitted = submitted_broad_harness_result_for_paths(&slot.published, changed_paths);
    GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            repo_root,
            EditSurfaceAdmission::new(
                slot.published.admission_binding().coordinate().clone(),
                SurfacePolicyId::new(slot.published.admission_binding().policy_id().as_str()),
            ),
            &slot.published,
            &submitted,
        )
        .expect("admit broad harness slot")
}

fn submit_broad_slot_for_test(
    repo_root: &Path,
    slot: &HarnessRequestSlot,
    changed_paths: &[PathBuf],
    label: &str,
) {
    GitWorktreeBackend
        .prepare_broad_harness_workspace(repo_root, &slot.published)
        .expect("prepare broad harness workspace");
    for relpath in changed_paths {
        write_surface_target(
            slot.published.workspace_path(),
            relpath,
            &format!("candidate edit {label} for {}\n", relpath.display()),
        );
    }
    let changed_paths = changed_paths
        .iter()
        .map(|relpath| slot.published.workspace_path().join(relpath))
        .collect::<Vec<_>>();
    let target_dir = slot.published.workspace_path().join("target");
    fs::create_dir_all(&target_dir).expect("create broad slot target dir");
    fs::write(target_dir.join("canary"), "scratch build output\n")
        .expect("write broad slot target canary");
    write_headless_tui_submission(slot, &changed_paths).expect("write broad harness submission");
}

#[test]
fn provider_unavailable_headless_tui_terminal_is_typed_prepare_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(&repo_root, Path::new("src/lib.rs"), "pub fn canary() {}\n");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "provider unavailable fixture");

    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
    )
    .expect("published broad harness request");
    let slot = HarnessRequestSlot {
        request_path: publication.request_path,
        published: publication.published,
    };
    let reason = "Error: API error (status 401): UNAUTHENTICATED".to_string();
    let run = tui_adapter::HeadlessRun::from_parts_for_test(
        Vec::new(),
        Some(tui_adapter::HeadlessTerminal::ProviderUnavailable {
            reason: reason.clone(),
        }),
    );
    let terminal = run.terminal().expect("terminal");

    let err = finish_broad_headless_tui_attempt(
        &GitWorktreeBackend,
        &slot,
        &repo_root,
        false,
        &run,
        terminal,
    )
    .expect_err("provider errors must stop child planning");

    let PrepareError::ProviderUnavailable { phase, detail } = err else {
        panic!("expected typed provider unavailable error, got {err:?}");
    };
    assert_eq!(phase, "broad_headless_tui_attempt");
    assert!(detail.contains(&reason), "unexpected detail: {detail}");
}

#[tokio::test]
async fn broad_tui_prep_failure_is_setup_blocker() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(&repo_root, Path::new("src/lib.rs"), "pub fn canary() {}\n");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "workspace prep fixture");

    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
    )
    .expect("published broad harness request");
    let slot = HarnessRequestSlot {
        request_path: publication.request_path,
        published: publication.published,
    };
    fs::create_dir_all(slot.published.workspace_path()).expect("create unmanaged workspace path");
    let options = BroadTuiAttemptOptions {
        model: None,
        max_attempts: Some(1),
        timeout_secs: Some(60),
    };

    let err = run_broad_headless_tui_attempt_with_options(&slot, &options)
        .await
        .expect_err("workspace preparation failures must stop child planning");

    let PrepareError::DatabaseSetup { phase, detail } = err else {
        panic!("expected setup blocker, got {err:?}");
    };
    assert_eq!(phase, "broad_headless_tui_workspace");
    assert!(
        detail.contains("failed to prepare broad headless-tui workspace"),
        "unexpected detail: {detail}"
    );
    assert!(
        detail.contains("not managed by git worktree metadata"),
        "unexpected detail: {detail}"
    );
}

#[tokio::test]
async fn zero_admission_batch_is_persisted() {
    let historical_request: PublishedBroadHarnessRequest = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-zero-admission-child-plan/node-18f71c7f3b1718b8.request.json"
    ));
    let historical_summary: tui_adapter::evidence::Summary = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-zero-admission-child-plan/node-18f71c7f3b1718b8.headless-tui.json"
    ));
    assert_eq!(
        historical_request.request_id(),
        "broad-harness-request:node-18f71c7f3b1718b8"
    );
    assert!(matches!(
        historical_summary.terminal,
        Some(tui_adapter::evidence::Terminal::TimedOut { secs: 240 })
    ));

    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "zero admission fixture");
    // Parent<Ready> is the last parent-only state before child-plan authority is
    // resolved. No child runtime channel or ChildFiles exist yet.
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let budget: Prototype1ChildBudget = Prototype1ChildBudget::new(2, 3);

    // Publishing the broad-harness request consumes Parent<Ready> and returns a
    // batch carrying Parent<AwaitingHarnessPlan>; from here the controller must
    // either lock a ChildPlan message or fail without pretending the phase is
    // still fresh.
    let batch: HarnessRequestBatch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    let first_diagnostics =
        broad_headless_tui_diagnostics_path(batch.slots[0].published.submitted_result_path());
    write_json_file_pretty(&first_diagnostics, &historical_summary)
        .expect("write historical diagnostic into temp slot");
    let request_count_before = count_broad_requests(&manifest_path);

    // Zero admitted children is an error, but it still represents an attempted
    // child-plan phase. The below-min branch must accept the harness-plan state
    // back to Parent<Ready>, lock a rejected-attempt-only ChildPlan, and then
    // return InvalidBatchSelection.
    let (result, failed_batch_trace) = collect_traces(|| {
        publish_broad_harness_child_plan_from_admitted_batch(
            ChildPlanEnv {
                campaign_id: "campaign",
                manifest_path: &manifest_path,
                repo_root: &repo_root,
            },
            batch,
            Vec::new(),
        )
    });
    dump_trace_if_requested(&failed_batch_trace);
    let err = match result {
        Ok(_) => panic!("zero admitted broad harness batch must not seal children"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("broad harness admitted 0 child transaction(s)"),
        "{err}"
    );
    assert!(trace_contains(
        &failed_batch_trace,
        &[
            "event=typestate_transition",
            "transition=Parent<AwaitingHarnessPlan>->Parent<Ready>",
            "phase=typestate_transition",
            "outcome=committed",
        ],
    ));
    assert!(trace_contains(
        &failed_batch_trace,
        &[
            "event=typestate_transition",
            "transition=Parent<Ready>->Parent<Planned>",
            "phase=failed_batch_persistence",
            "record_access=write",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));

    // Retry starts again from Parent<Ready>. A persisted ChildPlanFile should
    // move through Locked<ChildPlan> -> Parent<Planned> -> Parent<Selectable>
    // through the same profile-dispatched child-plan resolver used by
    // advance_child_plan, instead of minting fresh request slots.
    let resumed_parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let run_profile = toml::from_str::<profile::Prototype1RunProfile>(
        r#"
schema_version = "prototype1-run-profile.v1"
name = "zero-admission-replay"
"#,
    )
    .expect("profile parses");
    run_profile.validate().expect("profile validates");
    let (planned_result, replay_trace) = collect_traces_async(resolve_profile_child_plan(
        "campaign",
        &manifest_path,
        &repo_root,
        resumed_parent,
        &run_profile,
        budget,
    ))
    .await;
    dump_trace_if_requested(&replay_trace);
    let planned: PlannedChildren = planned_result
        .expect("failed broad harness batch should be recoverable as rejected evidence");
    assert!(trace_contains(
        &replay_trace,
        &[
            "event=typestate_transition",
            "transition=Parent<Ready>->Parent<Planned>",
            "phase=retry_replay",
            "record_access=read",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));
    assert!(trace_contains(
        &replay_trace,
        &[
            "event=typestate_transition",
            "transition=ChildPlan->Parent<Selectable>",
            "phase=message_receive",
            "record_access=read",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));

    assert!(planned.children.is_empty());
    assert!(
        !planned.rejected_surface_attempts.is_empty(),
        "failed broad harness batch must persist rejected attempt evidence"
    );
    assert_eq!(
        count_broad_requests(&manifest_path),
        request_count_before,
        "replaying child_plan must reuse the persisted failed batch instead of minting fresh request slots"
    );
    assert!(
        planned.rejected_surface_attempts.iter().any(|attempt| {
            matches!(
                &attempt.outcome,
                surface_attempt::Outcome::Rejected { reason }
                    if reason.contains("timed out after 240 seconds")
            )
        }),
        "historical timeout should be visible in parent-readable rejected attempt evidence: {:?}",
        planned.rejected_surface_attempts
    );
}

// regr:timeoutapplied:22-05-26_01-27
#[test]
fn timed_out_headless_tui_applied_attempt_blocks_submitted_result_for_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(
        &repo_root,
        Path::new("src/lib.rs"),
        r#"pub fn timeout_applied_canary() -> &'static str {
    "before"
}
"#,
    );
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "timeout applied handoff fixture");

    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
    )
    .expect("published broad harness request");
    let slot = HarnessRequestSlot {
        request_path: publication.request_path,
        published: publication.published,
    };
    GitWorktreeBackend
        .prepare_broad_harness_workspace(&repo_root, &slot.published)
        .expect("prepare broad harness workspace");

    let relpath = PathBuf::from("src/lib.rs");
    let candidate_path = slot.published.workspace_path().join(&relpath);
    fs::write(
        &candidate_path,
        r#"pub fn timeout_applied_canary() -> &'static str {
    "after"
}
"#,
    )
    .expect("write candidate edit");

    let proposal_id = uuid::Uuid::from_u128(0x45a2_e262_0000_0000_0000_000000000001);
    let run = tui_adapter::HeadlessRun::from_parts_for_test(
        vec![tui_adapter::HeadlessAttempt::applied_for_test(
            1,
            proposal_id,
            vec![candidate_path.clone()],
        )],
        Some(tui_adapter::HeadlessTerminal::TimedOut { secs: 900 }),
    );
    let terminal = run.terminal().expect("terminal");

    let err = finish_broad_headless_tui_attempt(
        &GitWorktreeBackend,
        &slot,
        &repo_root,
        false,
        &run,
        terminal,
    )
    .expect_err("timed-out applied run must not publish submission");
    let detail = err.to_string();
    assert!(
        detail.contains("timed out after 900 seconds"),
        "unexpected error: {detail}"
    );
    assert!(
        detail.contains("refusing to publish submitted broad-harness result"),
        "unexpected error: {detail}"
    );
    assert!(
        !slot.published.submitted_result_path().exists(),
        "timed-out applied run must not write submitted result at {}",
        slot.published.submitted_result_path().display()
    );

    let outcome = GitWorktreeBackend
        .validate_tui_attempt(&repo_root, &slot.published)
        .expect("validate candidate workspace");
    let diff = match outcome {
        TuiAttemptOutcome::Accepted(diff) => diff,
        TuiAttemptOutcome::Rejected(rejection) => {
            panic!("expected timed-out applied workspace to be accepted, got {rejection:?}")
        }
    };
    assert_eq!(diff.changed_paths(), &[relpath]);
}

#[test]
fn tui_edit_surface_producer_creates_default_checked_candidates() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let allowed = write_broad_surface_targets(tmp.path());
    let manifest = test_manifest_path(tmp.path());
    let parent = ready_parent_for_test(&manifest, tmp.path());

    let generated = produce_deterministic_tui_tools_candidates(
        tmp.path(),
        &parent,
        Prototype1SearchPolicy::default().child_budget,
    )
    .expect("checked candidates");
    let checked = &generated.checked;

    assert!(checked.len() >= Prototype1SearchPolicy::default().child_budget.min as usize);
    assert!(checked.len() <= Prototype1SearchPolicy::default().child_budget.max as usize);
    assert!(generated.rejected_attempts.is_empty());
    let hashes = checked
        .iter()
        .map(|candidate| candidate.proposed_content_hash().to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(hashes.len(), checked.len());
    assert!(checked.iter().all(|candidate| {
        candidate.surface() == Prototype1EditSurface::PlokeTuiTools
            && allowed
                .iter()
                .any(|allowed| allowed == candidate.target_relpath())
            && !candidate
                .target_relpath()
                .starts_with(EVAL_CORE_SURFACE_ROOT)
    }));
}

#[test]
fn deterministic_tui_surface_producer_labels_scaffold_noop_candidates() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_broad_surface_targets(tmp.path());
    let manifest = test_manifest_path(tmp.path());
    let parent = ready_parent_for_test(&manifest, tmp.path());

    let generated = produce_deterministic_tui_tools_candidates(
        tmp.path(),
        &parent,
        Prototype1ChildBudget::new(1, 1),
    )
    .expect("checked deterministic scaffold candidate");
    let candidate = generated.checked.first().expect("checked candidate");
    let proposed = candidate.proposed_content();

    assert!(
        proposed.contains("prototype1 deterministic scaffold/no-op candidate"),
        "proposal content did not label deterministic scaffold/no-op candidate: {proposed}"
    );
    assert!(
        proposed.contains("not semantic improvement evidence"),
        "proposal content did not reject semantic improvement evidence: {proposed}"
    );
}

#[test]
fn deterministic_surface_producer_dedupes_duplicate_proposed_contents() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_broad_surface_targets(tmp.path());
    let replacements = vec![
        "\n// duplicate candidate\n".to_string(),
        "\n// duplicate candidate\n".to_string(),
        "\n// distinct candidate\n".to_string(),
    ];

    let proposals = deterministic_surface_proposals(tmp.path(), "seed", replacements.clone(), 1, 3)
        .expect("deduped proposals");
    assert_eq!(proposals.len(), 2);

    let error = deterministic_surface_proposals(tmp.path(), "seed", replacements, 3, 3)
        .expect_err("duplicates cannot satisfy min");
    let PrepareError::InvalidBatchSelection { detail } = error else {
        panic!("unexpected error variant");
    };
    assert!(detail.contains("fewer than required minimum 3"));
}

#[test]
fn tui_edit_surface_parent_selection_publishes_child_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 1);

    let receipt = publish_deterministic_tui_tools_child_plan(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        parent,
        budget,
    )
    .expect("published child plan");

    let body = receipt.plan.body();
    assert_eq!(receipt.parent.node().node_id, "node-parent");
    assert_eq!(body.children().len(), 1);
    assert!(body.rejected_surface_attempts().is_empty());
    assert!(receipt.rejected_surface_attempts.is_empty());
    assert_eq!(body.parent_node_id(), "node-parent");
    assert!(body.message().exists());
    for child in body.children() {
        let node = child.node_record();
        assert_eq!(node.parent_node_id.as_deref(), Some("node-parent"));
        assert_eq!(node.generation, 1);
        assert!(!node.target_relpath.starts_with(EVAL_CORE_SURFACE_ROOT));
        assert_eq!(
            child.resolved().branch.synthesized_spec_id,
            TUI_EDIT_SURFACE_PRODUCER_ID
        );
        assert!(node.node_dir.join("node.json").exists());
        assert!(node.runner_request_path.exists());
    }
}

#[test]
fn broad_workspace_edit_surface_republication_uses_request_scoped_family_paths() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent_identity = test_parent_identity();
    let admission_binding = test_broad_request_admission_binding();
    let budget = Prototype1ChildBudget::new(2, 3);

    let first = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &parent_identity,
        budget,
        admission_binding.clone(),
    )
    .expect("first publication");
    let second = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &parent_identity,
        budget,
        admission_binding,
    )
    .expect("second publication");

    let prototype_root = prototype1_campaign_root(&manifest_path);
    assert_eq!(
        first.request_path,
        prototype_root.join("messages/edit-harness-request/node-parent.json")
    );
    assert_eq!(
        second.request_path,
        prototype_root.join("messages/edit-harness-request/node-parent-r2.json")
    );
    assert_eq!(
        second.published.prompt_path(),
        prototype_root
            .join("messages/edit-harness-request/node-parent-r2.md")
            .as_path()
    );
    assert_eq!(
        second.published.submitted_result_path(),
        prototype_root
            .join("messages/edit-harness-result/node-parent-r2.json")
            .as_path()
    );
    assert_eq!(
        second.published.workspace_path(),
        prototype_root
            .join("workspaces/edit-harness/node-parent-r2")
            .as_path()
    );
    assert_ne!(first.request_path, second.request_path);
    assert_ne!(
        first.published.prompt_path(),
        second.published.prompt_path()
    );
    assert_ne!(
        first.published.submitted_result_path(),
        second.published.submitted_result_path()
    );
    assert_ne!(
        first.published.workspace_path(),
        second.published.workspace_path()
    );
    assert!(second.request_path.exists());
    assert!(second.published.prompt_path().exists());
    assert!(!second.published.submitted_result_path().exists());
}

#[test]
fn broad_batch_publication_allocates_request_slots() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(2, 3);

    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("broad harness should allocate request slots from active artifact head");

    assert_eq!(batch.child_budget, budget);
    assert_eq!(batch.patch_generation_parallel_cap, 3);
    assert_eq!(batch.slots.len(), 9);
    assert_eq!(
        batch.parent.harness_request().request_id(),
        batch.slots[0].published.request_id()
    );
    assert!(
        batch.slots[0]
            .published
            .admission_binding()
            .base_artifact_id()
            .as_str()
            .starts_with("artifact:git-commit:")
    );
    assert_eq!(
        batch.slots[0].published.request_id(),
        "broad-harness-request:node-parent"
    );
    assert_eq!(
        batch.slots[1].published.request_id(),
        "broad-harness-request:node-parent:r2"
    );
    assert_eq!(
        batch.slots[2].published.request_id(),
        "broad-harness-request:node-parent:r3"
    );
    assert_eq!(
        batch.slots[8].published.request_id(),
        "broad-harness-request:node-parent:r9"
    );
    for slot in &batch.slots {
        assert_eq!(slot.published.request().child_budget.min_children, 1);
        assert_eq!(slot.published.request().child_budget.max_children, 1);
        assert!(slot.request_path.exists());
        assert!(slot.published.prompt_path().exists());
        assert!(!slot.published.submitted_result_path().exists());
    }
}

#[test]
fn broad_batch_default_cap_respects_small_max() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 2);

    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("broad harness should allocate request slots");

    assert_eq!(batch.child_budget, budget);
    assert_eq!(batch.patch_generation_parallel_cap, 2);
    assert_eq!(batch.slots.len(), 6);
}

#[test]
fn broad_batch_uses_explicit_parallel_targets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(2, 3).with_parallel_targets(2);

    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("broad harness should allocate request slots");

    assert_eq!(batch.child_budget, budget);
    assert_eq!(batch.patch_generation_parallel_cap, 2);
    assert_eq!(batch.slots.len(), 9);
}

#[test]
fn broad_headless_tui_diagnostics_path_sits_beside_submitted_result() {
    let submitted =
        PathBuf::from("/tmp/prototype1/messages/edit-harness-result/node-parent-r2.json");

    let diagnostics = broad_headless_tui_diagnostics_path(&submitted);

    assert_eq!(
        diagnostics,
        PathBuf::from(
            "/tmp/prototype1/messages/edit-harness-result/node-parent-r2.headless-tui.json",
        )
    );
}

#[test]
fn broad_tui_attempt_options_can_lower_published_live_budget() {
    let contract =
        crate::cli::prototype1_state::edit_surface::harness_request::contract::Bundle::prototype1(
            Path::new("/tmp/prototype1"),
        );
    let options = BroadTuiAttemptOptions::from_cli(
        Some("moonshotai/kimi-k2".to_string()),
        None,
        Some(1),
        Some(180),
    )
    .expect("options");

    assert_eq!(effective_broad_tui_max_attempts(&contract, &options), 1);
    assert_eq!(effective_broad_tui_timeout_secs(&contract, &options), 180);
}

#[test]
fn broad_tui_attempt_provider_requires_model_override() {
    let err = BroadTuiAttemptOptions::from_cli(None, Some("anthropic".to_string()), Some(1), None)
        .expect_err("provider without model should fail");

    match err {
        PrepareError::InvalidBatchSelection { detail } => {
            assert!(detail.contains("requires --model-id"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn broad_tui_attempt_google_provider_selects_google_router() {
    let options = BroadTuiAttemptOptions::from_cli(
        Some("google/gemini-2.5-flash".to_string()),
        Some("google".to_string()),
        Some(1),
        None,
    )
    .expect("google headless model selection");

    let model = options.model().expect("model selection");
    assert!(matches!(
        model.router(),
        ploke_llm::router_only::RouterVariants::Google(_)
    ));
    assert!(model.provider().is_none());
}

#[cfg(feature = "live_api_tests")]
fn live_google_headless_tui_model_id() -> String {
    let raw = std::env::var("PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID")
        .or_else(|_| std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL"))
        .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string());
    if raw.contains('/') {
        raw
    } else {
        format!("google/{raw}")
    }
}

#[cfg(feature = "live_api_tests")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "expected-failing counterexample for the unsupported Google Vertex broad headless-TUI path"]
async fn xfail_google_vertex_broad_headless_tui_attempt_applies_edit_from_published_request() {
    // regr:googlevertex:23-05-26_19-10
    //
    // This is intentionally tracked as a counterexample, not as proof of the
    // supported Google route. It preserves the earlier endpoint experiment:
    // ploke-eval published request -> cli_facing runner -> tui_adapter ->
    // vanilla ploke-tui llm_manager -> Google router configured for the Vertex
    // OpenAI-compatible/ADC path.
    //
    // Keep the supported Google path covered by the direct-Google registry,
    // model-picker, session-loop, eval-router, and protocol tests. If we later
    // decide to support this Vertex broad-headless endpoint path, remove the
    // tracker row in docs/active/agents/expected-failing-regression-tests.md,
    // rename/unignore this test, and make the assertions below the positive
    // acceptance contract for that newly supported path.
    crate::test_support::install_default_google_route_env();
    let model_id = live_google_headless_tui_model_id();
    let options = BroadTuiAttemptOptions::from_cli(
        Some(model_id.clone()),
        Some("google".to_string()),
        Some(1),
        Some(240),
    )
    .expect("google model selection");
    let model = options.model().expect("model selection");
    assert!(matches!(
        model.router(),
        ploke_llm::router_only::RouterVariants::Google(_)
    ));
    assert!(model.provider().is_none());

    let base = std::env::var_os("PLOKE_EVAL_LIVE_TUI_CANARY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ploke-eval-live-google-broad-headless"));
    let artifact_root = base.join(format!("run-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir_all(&artifact_root).expect("create live artifact root");
    println!(
        "live Google broad headless-TUI artifacts: {}",
        artifact_root.display()
    );

    let manifest_path = artifact_root.join("campaign.json");
    let repo_root = artifact_root.join("repo");
    init_indexed_repo(&repo_root);
    fs::write(
        repo_root.join("Cargo.toml"),
        r#"[package]
name = "ploke-eval-live-google-broad-headless"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
    )
    .expect("write Cargo.toml");
    write_surface_target(
        &repo_root,
        Path::new("src/lib.rs"),
        r#"pub fn broad_surface_canary() -> &'static str {
    "before"
}
"#,
    );
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "google broad headless fixture");

    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
    )
    .expect("published broad harness request");
    fs::write(
        publication.published.prompt_path(),
        r#"Call the apply_code_edit tool exactly once. Do not call any other tool. Do not answer in prose before the tool call.
Use exactly this JSON payload:
{"edits":[{"file":"src/lib.rs","canon":"crate::broad_surface_canary","node_type":"function","code":"pub fn broad_surface_canary() -> &'static str {\n    \"after\"\n}"}],"confidence":0.99}
"#,
    )
    .expect("write live Google canary prompt");

    let slot = HarnessRequestSlot {
        request_path: publication.request_path.clone(),
        published: publication.published,
    };
    let executor = run_broad_headless_tui_attempt_with_options(&slot, &options)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "live Google broad headless-TUI attempt failed for '{}': {err}; artifacts at {}",
                slot.request_path.display(),
                artifact_root.display()
            )
        });
    assert!(
        executor.is_some(),
        "expected broad headless-TUI executor for '{}'",
        slot.request_path.display()
    );

    let diagnostics_path =
        broad_headless_tui_diagnostics_path(slot.published.submitted_result_path());
    let diagnostics = fs::read(&diagnostics_path).unwrap_or_else(|err| {
        panic!(
            "missing headless diagnostics '{}': {err}; artifacts at {}",
            diagnostics_path.display(),
            artifact_root.display()
        )
    });
    let diagnostics: tui_adapter::evidence::Summary = serde_json::from_slice(&diagnostics)
        .unwrap_or_else(|err| {
            panic!(
                "invalid headless diagnostics '{}': {err}; artifacts at {}",
                diagnostics_path.display(),
                artifact_root.display()
            )
        });
    assert!(
        matches!(
            diagnostics.terminal,
            Some(tui_adapter::evidence::Terminal::Applied { .. })
        ),
        "expected applied terminal in diagnostics; got {:?}; artifacts at {}",
        diagnostics.terminal,
        artifact_root.display()
    );
    let requested_tools = diagnostics
        .events
        .iter()
        .filter_map(|event| match event {
            tui_adapter::evidence::Event::ToolRequest { tool, .. } => Some(tool.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        requested_tools.contains(&"apply_code_edit"),
        "expected apply_code_edit tool request, got {requested_tools:?}; artifacts at {}",
        artifact_root.display()
    );
    assert!(
        diagnostics.events.iter().any(|event| matches!(
            event,
            tui_adapter::evidence::Event::Turn { outcome, .. } if outcome == "completed"
        )),
        "expected completed chat turn in diagnostics; artifacts at {}",
        artifact_root.display()
    );

    let submitted = fs::read(slot.published.submitted_result_path()).unwrap_or_else(|err| {
        panic!(
            "missing submitted broad harness result '{}': {err}; artifacts at {}",
            slot.published.submitted_result_path().display(),
            artifact_root.display()
        )
    });
    let submitted: SubmittedBroadHarnessResult =
        serde_json::from_slice(&submitted).expect("submitted broad harness result should decode");
    submitted
        .verify_request(&slot.published)
        .expect("submitted result remains bound to the published request");

    let outcome = GitWorktreeBackend
        .validate_tui_attempt(&repo_root, &slot.published)
        .expect("validate broad headless-TUI workspace");
    let diff = match outcome {
        TuiAttemptOutcome::Accepted(diff) => diff,
        TuiAttemptOutcome::Rejected(rejection) => {
            panic!(
                "expected accepted broad headless-TUI workspace, got {rejection:?}; artifacts at {}",
                artifact_root.display()
            )
        }
    };
    assert_eq!(diff.changed_paths(), &[PathBuf::from("src/lib.rs")]);
    let final_lib = fs::read_to_string(slot.published.workspace_path().join("src/lib.rs"))
        .expect("read candidate src/lib.rs");
    assert!(
        final_lib.contains("\"after\"") && !final_lib.contains("\"before\""),
        "expected Google-applied sentinel edit, got:\n{final_lib}\nartifacts at {}",
        artifact_root.display()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "operator splice test for one published broad headless-TUI request"]
async fn live_broad_headless_tui_attempt_from_published_request_env() {
    let Some(request_path) =
        std::env::var_os("PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH").map(PathBuf::from)
    else {
        println!(
            "skipping: set PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH to a published broad harness request JSON"
        );
        return;
    };

    let request_bytes = fs::read(&request_path).unwrap_or_else(|err| {
        panic!(
            "could not read published broad harness request '{}': {err}",
            request_path.display()
        )
    });
    let published = serde_json::from_slice::<PublishedBroadHarnessRequest>(&request_bytes)
        .unwrap_or_else(|err| {
            panic!(
                "could not decode published broad harness request '{}': {err}",
                request_path.display()
            )
        });
    let slot = HarnessRequestSlot {
        request_path: request_path.clone(),
        published,
    };

    println!("broad splice request: {}", request_path.display());
    println!("prompt: {}", slot.published.prompt_path().display());
    println!("workspace: {}", slot.published.workspace_path().display());
    println!(
        "diagnostics: {}",
        broad_headless_tui_diagnostics_path(slot.published.submitted_result_path()).display()
    );

    let model_id = std::env::var("PLOKE_EVAL_HEADLESS_TUI_MODEL_ID").ok();
    let provider = std::env::var("PLOKE_EVAL_HEADLESS_TUI_PROVIDER").ok();
    let options = match (model_id, provider) {
        (None, None) => BroadTuiAttemptOptions::for_parent_patcher_defaults(None, None),
        (model_id, provider) => BroadTuiAttemptOptions::from_cli(model_id, provider, None, None),
    }
    .unwrap_or_else(|err| {
        panic!(
            "could not resolve broad headless-TUI model override for '{}': {err}",
            request_path.display()
        )
    });

    let executor = run_broad_headless_tui_attempt_with_options(&slot, &options)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "published broad headless-TUI attempt failed for '{}': {err}",
                request_path.display()
            )
        });

    let outcome = GitWorktreeBackend
        .validate_tui_attempt(
            slot.published.request().workspace.source_repository_path(),
            &slot.published,
        )
        .unwrap_or_else(|err| {
            panic!(
                "broad headless-TUI workspace validation errored for '{}': {err}",
                slot.published.workspace_path().display()
            )
        });

    let diff = match outcome {
        TuiAttemptOutcome::Accepted(diff) => diff,
        TuiAttemptOutcome::Rejected(rejection) => {
            panic!(
                "broad headless-TUI workspace rejected after executor {:?}: {:?}",
                executor, rejection
            )
        }
    };

    println!("executor: {:?}", executor);
    println!("base_head: {}", diff.base_head());
    println!("changed_paths:");
    for path in diff.changed_paths() {
        println!("- {}", path.display());
    }
}

#[test]
fn broad_harness_complete_run_reaches_request_continuation_hook() {
    CandidateGenerationConfig::BroadHarnessRequest
        .ensure_live_complete_admitted()
        .expect("broad harness should reach the request-bound continuation hook");
}

#[test]
fn broad_harness_rejects_unbound_existing_child_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 1);
    let receipt = publish_deterministic_tui_tools_child_plan(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        parent,
        budget,
    )
    .expect("published deterministic child plan");

    let err = CandidateGenerationConfig::BroadHarnessRequest
        .validate_received_child_plan(receipt.plan.body().children())
        .expect_err("broad harness must not consume unbound child plans");

    let PrepareError::InvalidBatchSelection { detail } = err else {
        panic!("unexpected error variant");
    };
    assert!(detail.contains("was not produced by the headless TUI adapter"));
}

#[test]
fn broad_harness_child_requires_request_bound_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut node = test_node(tmp.path(), "node-broad", "branch-broad", "candidate-broad");
    node.base_artifact_id = Some(crate::loop_graph::ArtifactId::new("artifact:base"));
    node.derived_artifact_id = Some(crate::loop_graph::ArtifactId::new("artifact:derived"));
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = "prototype1:broad-headless-tui-adapter-v1".to_string();
    resolved.branch.apply_id = Some("broad-harness-request:node-parent".to_string());
    resolved.branch.generation_coordinate = Some(crate::loop_graph::Coordinate {
        runtime_id: crate::loop_graph::RuntimeId::new(),
        target: crate::loop_graph::OperationTarget::Artifact {
            artifact_id: node.base_artifact_id.clone().expect("base artifact"),
        },
    });
    resolved.branch.generation_target = Some(crate::loop_graph::OperationTarget::Artifact {
        artifact_id: node.base_artifact_id.clone().expect("base artifact"),
    });
    let child = ChildFiles::from_resolved("campaign", node, resolved, false);

    let err = validate_requested_broad_harness_child(&child)
        .expect_err("request-bound broad child evidence is mandatory");

    let PrepareError::InvalidBatchSelection { detail } = err else {
        panic!("unexpected error variant");
    };
    assert!(detail.contains("missing request-bound admission evidence"));
}

#[test]
fn broad_harness_multi_file_admission_mints_one_artifact_child() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let changed_paths = {
        let allowed = write_broad_surface_targets(&repo_root);
        commit_indexed_repo(&repo_root, "broad surface fixture");
        vec![allowed[0].clone(), allowed[1].clone()]
    };
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        parent.identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
    )
    .expect("published request");
    let awaiting_parent = parent.awaiting_harness_plan_for_request((&publication.published).into());
    let receipt = HarnessRequestReceipt {
        parent: awaiting_parent,
        request_path: publication.request_path,
        published: publication.published,
    };
    let candidate_root = receipt.published.workspace_path().to_path_buf();
    GitWorktreeBackend
        .prepare_broad_harness_workspace(&repo_root, &receipt.published)
        .expect("prepare broad harness workspace");
    for relpath in &changed_paths {
        write_surface_target(
            &candidate_root,
            relpath,
            &format!("candidate edit for {}\n", relpath.display()),
        );
    }
    let submitted = submitted_broad_harness_result_for_paths(&receipt.published, &changed_paths);
    let admitted = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            &repo_root,
            EditSurfaceAdmission::new(
                receipt.published.admission_binding().coordinate().clone(),
                SurfacePolicyId::new(receipt.published.admission_binding().policy_id().as_str()),
            ),
            &receipt.published,
            &submitted,
        )
        .expect("admit broad harness result");
    let admitted_derived = admitted.derived_artifact_id().clone();

    let child_plan = publish_broad_harness_child_plan_from_admitted(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        receipt,
        admitted,
    )
    .expect("multi-file admitted transaction should mint one child artifact");

    let children = child_plan.plan.body().children();
    assert_eq!(children.len(), 1);
    let child = &children[0];
    let evidence = child.harness_evidence().expect("harness evidence");
    assert_eq!(evidence.changed_paths(), changed_paths.as_slice());
    assert_eq!(
        evidence
            .workspace()
            .expect("workspace evidence")
            .candidate_root,
        candidate_root
    );
    assert_eq!(
        evidence
            .artifact()
            .expect("artifact evidence")
            .derived_artifact_id,
        admitted_derived
    );

    let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let c1 = C1::from_child_plan(
        "campaign",
        manifest_path.clone(),
        child.node_record().clone(),
        child.runner_request().clone(),
        child.resolved().clone(),
        repo_root,
    )
    .expect("load c1");
    let c2 = match MaterializeBranch::new()
        .transition_with_harness(c1, evidence, &mut journal)
        .expect("broad materialization")
    {
        Outcome::Advanced(next) => next,
        Outcome::Rejected(never) => match never {},
    };

    assert_eq!(c2.artifact().repo_root(), candidate_root.as_path());
    assert_eq!(c2.node().workspace_root, candidate_root);
    assert_eq!(c2.request().workspace_root, c2.node().workspace_root);
}

#[test]
fn broad_harness_materialization_accepts_relative_parent_repo_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let allowed = write_broad_surface_targets(&repo_root);
    let changed_paths = vec![allowed[0].clone(), allowed[1].clone()];
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let admission_binding = test_broad_request_admission_binding();
    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        &parent_identity,
        Prototype1ChildBudget::new(1, 1),
        admission_binding.clone(),
    )
    .expect("publish broad request");
    let awaiting_parent = parent.awaiting_harness_plan_for_request((&publication.published).into());
    let receipt = HarnessRequestReceipt {
        parent: awaiting_parent,
        request_path: publication.request_path,
        published: publication.published,
    };
    GitWorktreeBackend
        .prepare_broad_harness_workspace(&repo_root, &receipt.published)
        .expect("prepare broad harness workspace");
    let candidate_root = receipt.published.workspace_path().to_path_buf();
    for relpath in &changed_paths {
        write_surface_target(
            &candidate_root,
            relpath,
            &format!("candidate edit for {}\n", relpath.display()),
        );
    }
    let submitted = submitted_broad_harness_result_for_paths(&receipt.published, &changed_paths);
    let admitted = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            &repo_root,
            EditSurfaceAdmission::new(
                receipt.published.admission_binding().coordinate().clone(),
                SurfacePolicyId::new(receipt.published.admission_binding().policy_id().as_str()),
            ),
            &receipt.published,
            &submitted,
        )
        .expect("admit broad harness result");

    let child_plan = publish_broad_harness_child_plan_from_admitted(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        receipt,
        admitted,
    )
    .expect("multi-file admitted transaction should mint one child artifact");

    let child = &child_plan.plan.body().children()[0];
    let evidence = child.harness_evidence().expect("harness evidence");
    let cwd = std::env::current_dir().expect("current dir");
    let relative_repo_root = relative_path(&cwd, &repo_root);

    let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let c1 = C1::from_child_plan(
        "campaign",
        manifest_path.clone(),
        child.node_record().clone(),
        child.runner_request().clone(),
        child.resolved().clone(),
        relative_repo_root,
    )
    .expect("load c1 from relative repo root");
    let c2 = match MaterializeBranch::new()
        .transition_with_harness(c1, evidence, &mut journal)
        .expect("relative parent repo root should still materialize broad harness child")
    {
        Outcome::Advanced(next) => next,
        Outcome::Rejected(never) => match never {},
    };

    let workspace = evidence.workspace().expect("workspace evidence");
    assert_eq!(
        c2.artifact().repo_root(),
        workspace.candidate_root.as_path()
    );
    assert_eq!(c2.node().workspace_root, workspace.candidate_root);
}

#[tokio::test]
async fn broad_harness_batch_admits_three_transactions_into_three_children() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(3, 3).with_parallel_targets(2);
    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    assert_eq!(batch.patch_generation_parallel_cap, 2);

    let submitted_indexes = [0_usize, 1, 2];
    let target_dirs = submitted_indexes
        .iter()
        .enumerate()
        .map(|(index, slot_index)| {
            let slot = &batch.slots[*slot_index];
            let changed_paths = vec![allowed[index].clone()];
            submit_broad_slot_for_test(&repo_root, slot, &changed_paths, &format!("slot-{index}"));
            slot.published.workspace_path().join("target")
        })
        .collect::<Vec<_>>();
    let untouched_target = batch.slots[3].published.workspace_path().join("target");
    fs::create_dir_all(&untouched_target).expect("create untouched target dir");

    let (receipt, trace) = collect_traces_async(admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
    ))
    .await;
    dump_trace_if_requested(&trace);
    let receipt =
        receipt.expect("three admitted broad transactions should seal one three-child plan");
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=Parent<AwaitingHarnessPlan>->Parent<Ready>",
            "phase=typestate_transition",
            "outcome=committed",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=Parent<Ready>->Parent<Planned>",
            "phase=batch_admission",
            "record_access=write",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=ChildPlan->Parent<Selectable>",
            "phase=message_receive",
            "record_access=read",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));

    let children = receipt.plan.body().children();
    assert_eq!(children.len(), 3);
    for target_dir in target_dirs {
        assert!(
            !target_dir.exists(),
            "submitted slot target dir should be cleaned: {}",
            target_dir.display()
        );
    }
    assert!(
        untouched_target.exists(),
        "slot beyond max admission should not be spawned or cleaned"
    );
    let request_ids = children
        .iter()
        .map(|child| {
            child
                .harness_evidence()
                .expect("harness evidence")
                .request()
                .request_id()
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(request_ids.len(), 3);
    let candidate_ids = children
        .iter()
        .map(|child| child.resolved().branch.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        candidate_ids,
        BTreeSet::from([
            "broad-harness-g1-01".to_string(),
            "broad-harness-g1-02".to_string(),
            "broad-harness-g1-03".to_string(),
        ])
    );
    for child in children {
        validate_requested_broad_harness_child(child)
            .expect("batch child carries admitted transaction evidence");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn broad_slots_run_in_parallel() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let probe_dir = tmp.path().join("slot-probe");
    let summary_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "src/tests/fixtures/prototype1-zero-admission-child-plan/node-18f71c7f3b1718b8.headless-tui.json",
    );
    let _env = crate::test_support::env_guard_os(vec![
        (
            "PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE",
            summary_fixture.into_os_string(),
        ),
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "2".into()),
        (
            "PLOKE_EVAL_BROAD_TUI_SLOT_PROBE_DIR",
            probe_dir.clone().into_os_string(),
        ),
        ("PLOKE_EVAL_BROAD_TUI_SLOT_PROBE_WAIT_FOR", "2".into()),
    ]);

    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget: Prototype1ChildBudget = Prototype1ChildBudget::new(2, 2).with_parallel_targets(2);
    let batch: HarnessRequestBatch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    assert_eq!(batch.patch_generation_parallel_cap, 2);
    assert_eq!(
        batch.slots.len(),
        2,
        "test-scoped slot limit keeps this fanout proof focused"
    );
    assert_eq!(count_broad_requests(&manifest_path), 2);

    let (result, trace) = collect_traces_async(admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
    ))
    .await;
    dump_trace_if_requested(&trace);
    let err = match result {
        Ok(_) => panic!("fixture-backed parallel slots should not admit children"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("broad harness admitted 0 child transaction(s)"),
        "{err}"
    );

    for slot_index in [0, 1] {
        assert!(
            probe_dir.join(format!("start-{slot_index}")).exists(),
            "slot {slot_index} should have reached the test barrier"
        );
        assert!(
            probe_dir.join(format!("release-{slot_index}")).exists(),
            "slot {slot_index} should have observed the other active slot before finishing"
        );
    }
    assert!(
        !probe_dir.join("start-2").exists(),
        "the two-child cap should not start a third concurrent slot"
    );
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=Parent<Ready>->Parent<Planned>",
            "phase=failed_batch_persistence",
            "record_access=write",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));

    let at = crate::cli::prototype1_state::inner::At::<ChildPlanFile>::resolve((
        manifest_path.clone(),
        parent_identity.node_id().to_string(),
    ));
    let bytes = fs::read(at.path()).expect("child plan file should persist rejected attempts");
    let plan: ChildPlanFiles = serde_json::from_slice(&bytes).expect("child plan decodes");
    assert!(plan.children().is_empty());
    assert_eq!(
        plan.rejected_surface_attempts().len(),
        2,
        "both concurrently-started slots should be visible as rejected parent-readable evidence"
    );
}

#[tokio::test]
async fn child_fanout_is_parallel() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let probe_dir = tmp.path().join("child-probe");
    let _env = crate::test_support::env_guard_os(vec![
        (
            "PLOKE_EVAL_CHILD_FANOUT_PROBE_DIR",
            probe_dir.clone().into_os_string(),
        ),
        ("PLOKE_EVAL_CHILD_FANOUT_PROBE_WAIT_FOR", "2".into()),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(3, 3).with_parallel_targets(2);
    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    assert_eq!(batch.patch_generation_parallel_cap, 2);

    for (index, slot) in batch.slots.iter().take(3).enumerate() {
        submit_broad_slot_for_test(
            &repo_root,
            slot,
            &[allowed[index].clone()],
            &format!("slot-{index}"),
        );
    }
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
    )
    .await
    .expect("three admitted children");
    let children = receipt.plan.body().children().to_vec();
    assert_eq!(children.len(), 3);

    let baseline = CompleteBaseline::complete(
        "campaign".to_string(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: parent_identity
                .instance_id()
                .expect("test parent instance")
                .to_string(),
            registration_path: None,
            record_path: tmp.path().join("baseline-record.json.gz"),
            metrics: test_metrics(false, true, 0),
        }],
    )
    .expect("complete baseline");
    let outcomes = run_child_fanout(
        "campaign",
        &manifest_path,
        &repo_root,
        &prototype1_transition_journal_path(&manifest_path),
        &parent_identity,
        &baseline,
        Prototype1StateStopAfter::Materialize,
        Duration::from_secs(30),
        Prototype1ChildScheduleMode::FullBatch,
        budget,
        0,
        children,
    )
    .await
    .expect("materialize first child fanout batch");

    assert_eq!(
        outcomes.len(),
        2,
        "materialize step should run one parallel batch capped by parallel_targets"
    );
    for plan_index in [0, 1] {
        assert!(
            probe_dir.join(format!("start-{plan_index}")).exists(),
            "child {plan_index} should reach the fanout barrier"
        );
        assert!(
            probe_dir.join(format!("release-{plan_index}")).exists(),
            "child {plan_index} should see the other concurrent child before materializing"
        );
    }
    assert!(
        !probe_dir.join("start-2").exists(),
        "materialize step should not start the third child in the first capped batch"
    );
    for outcome in &outcomes {
        assert_eq!(outcome.outcome, "materialized");
        assert_eq!(outcome.node_status, Prototype1NodeStatus::WorkspaceStaged);
        assert!(
            outcome.workspace_root.exists(),
            "materialized child workspace should exist"
        );
    }
}

#[cfg(unix)]
fn install_fake_cargo(fake_bin: &Path, child_script: &str) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(fake_bin).expect("fake bin dir");
    let fake_cargo = fake_bin.join("cargo");
    fs::write(
        &fake_cargo,
        format!(
            r#"#!/bin/sh
set -eu
if [ "${{1:-}}" = "build" ]; then
  mkdir -p "$CARGO_TARGET_DIR/debug"
  child="$CARGO_TARGET_DIR/debug/ploke-eval"
  cat > "$child" <<'PLOKE_FAKE_CHILD'
{child_script}
PLOKE_FAKE_CHILD
  chmod +x "$child"
fi
exit 0
"#
        ),
    )
    .expect("write fake cargo");
    let mut permissions = fs::metadata(&fake_cargo)
        .expect("fake cargo metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_cargo, permissions).expect("chmod fake cargo");

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path = fake_bin.as_os_str().to_os_string();
    path.push(":");
    path.push(old_path);
    path
}

#[cfg(unix)]
#[tokio::test]
async fn child_build_promotes_binary_and_cleans_scratch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let fake_bin = tmp.path().join("fake-bin");
    let path = install_fake_cargo(&fake_bin, "#!/bin/sh\nexit 0\n");
    let _env = crate::test_support::env_guard_os(vec![("PATH", path)]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(1, 1);
    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let baseline = CompleteBaseline::complete(
        "campaign".to_string(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: parent_identity
                .instance_id()
                .expect("test parent instance")
                .to_string(),
            registration_path: None,
            record_path: tmp.path().join("baseline-record.json.gz"),
            metrics: test_metrics(false, true, 0),
        }],
    )
    .expect("complete baseline");

    let outcome = run_planned_child(
        "campaign".to_string(),
        manifest_path.clone(),
        repo_root.clone(),
        prototype1_transition_journal_path(&manifest_path),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Build,
        Duration::from_secs(30),
        0,
        child,
    )
    .expect("build child with fake cargo");

    assert_eq!(outcome.outcome, "built");
    assert_eq!(outcome.node_status, Prototype1NodeStatus::BinaryBuilt);
    assert!(outcome.binary_path.exists(), "promoted child binary exists");
    assert!(
        !node.node_dir.join("target").exists(),
        "temporary child build target should be removed after a successful build"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn child_spawn_observes_ready() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let fake_bin = tmp.path().join("fake-bin");
    let path = install_fake_cargo(
        &fake_bin,
        r#"#!/bin/sh
set -eu
invocation=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--invocation" ]; then
    shift
    invocation="${1:-}"
    break
  fi
  shift
done
if [ -z "$invocation" ]; then
  echo "missing invocation" >&2
  exit 2
fi
invocations_dir=$(dirname "$invocation")
node_dir=$(dirname "$invocations_dir")
runtime_id="${PLOKE_PROTOTYPE1_RUNTIME_ID:?missing runtime id}"
channel_dir="$node_dir/channels/$runtime_id"
mkdir -p "$channel_dir"
cat >> "$channel_dir/child-to-parent.jsonl" <<JSON
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"${PLOKE_PROTOTYPE1_CAMPAIGN_ID:?missing campaign}","node_id":"${PLOKE_PROTOTYPE1_NODE_ID:?missing node}","runtime_id":"$runtime_id","message_id":"00000000-0000-4000-8000-000000000001","recorded_at":0,"body_hash":"40ec7f71ea684c8b976e79e8e425f87779e6de57f4821dcfc8066dbcad2defe0","body":"ready"}
JSON
sleep 1
exit 0
"#,
    );
    let _env = crate::test_support::env_guard_os(vec![("PATH", path)]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(1, 1);
    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let baseline = CompleteBaseline::complete(
        "campaign".to_string(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: parent_identity
                .instance_id()
                .expect("test parent instance")
                .to_string(),
            registration_path: None,
            record_path: tmp.path().join("baseline-record.json.gz"),
            metrics: test_metrics(false, true, 0),
        }],
    )
    .expect("complete baseline");

    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let outcome = run_planned_child(
        "campaign".to_string(),
        manifest_path.clone(),
        repo_root.clone(),
        journal_path.clone(),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Spawn,
        Duration::from_secs(30),
        0,
        child,
    )
    .expect("spawn child and observe ready channel message");

    assert_eq!(outcome.outcome, "spawned");
    assert_eq!(outcome.node_status, Prototype1NodeStatus::Running);
    let runtime = outcome
        .child_runtime
        .as_deref()
        .expect("spawned child runtime id");
    let channel_path = node
        .node_dir
        .join("channels")
        .join(runtime)
        .join("child-to-parent.jsonl");
    let channel = fs::read_to_string(&channel_path).expect("child ready channel record");
    assert!(channel.contains(r#""body":"ready""#));

    let entries = PrototypeJournal::new(journal_path)
        .load_entries()
        .expect("load transition journal");
    assert!(entries.iter().any(|entry| {
        matches!(
            entry,
            JournalEntry::SpawnChild(spawn)
                if spawn.refs.node_id == node.node_id
                    && spawn.phase == crate::cli::prototype1_state::journal::SpawnPhase::Observed
                    && matches!(
                        spawn.result,
                        Some(crate::cli::prototype1_state::journal::SpawnObservation::Acknowledged)
                    )
        )
    }));
}

#[cfg(unix)]
#[tokio::test]
async fn child_spawn_observes_failed_result() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let fake_bin = tmp.path().join("fake-bin");

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(1, 1);
    let batch =
        publish_broad_harness_child_plan_request(&manifest_path, &repo_root, parent, budget)
            .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let runner_result = crate::intervention::Prototype1RunnerResult {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: "campaign".to_string(),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Failed,
        disposition: crate::intervention::Prototype1RunnerDisposition::TreatmentFailed,
        treatment_campaign_id: None,
        evaluation_artifact_path: None,
        detail: Some("fake spawned child terminal failure".to_string()),
        exit_code: Some(1),
        stdout_excerpt: None,
        stderr_excerpt: None,
        recorded_at: "2026-05-25T00:00:00Z".to_string(),
    };
    let terminal = crate::cli::prototype1_state::channel::ToParent::Result {
        runner_result: runner_result.clone(),
        treatment: None,
    };
    let channel_body = |message: &crate::cli::prototype1_state::channel::ToParent| {
        use sha2::{Digest, Sha256};

        let bytes = serde_json::to_vec(message).expect("serialize channel body");
        (
            String::from_utf8(bytes.clone()).expect("channel body utf8"),
            format!("{:x}", Sha256::digest(&bytes)),
        )
    };
    let (ready_body, ready_hash) =
        channel_body(&crate::cli::prototype1_state::channel::ToParent::Ready);
    let (evaluating_body, evaluating_hash) =
        channel_body(&crate::cli::prototype1_state::channel::ToParent::Evaluating);
    let (terminal_body, terminal_hash) = channel_body(&terminal);
    let result_json = serde_json::to_string(&runner_result).expect("serialize runner result");
    let path = install_fake_cargo(
        &fake_bin,
        &format!(
            r#"#!/bin/sh
set -eu
invocation=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--invocation" ]; then
    shift
    invocation="${{1:-}}"
    break
  fi
  shift
done
if [ -z "$invocation" ]; then
  echo "missing invocation" >&2
  exit 2
fi
invocations_dir=$(dirname "$invocation")
node_dir=$(dirname "$invocations_dir")
runtime_id="${{PLOKE_PROTOTYPE1_RUNTIME_ID:?missing runtime id}}"
channel_dir="$node_dir/channels/$runtime_id"
mkdir -p "$channel_dir" "$node_dir/results"
cat >> "$channel_dir/child-to-parent.jsonl" <<JSON
{{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"${{PLOKE_PROTOTYPE1_CAMPAIGN_ID:?missing campaign}}","node_id":"${{PLOKE_PROTOTYPE1_NODE_ID:?missing node}}","runtime_id":"$runtime_id","message_id":"00000000-0000-4000-8000-000000000001","recorded_at":0,"body_hash":"{ready_hash}","body":{ready_body}}}
JSON
cat >> "$channel_dir/child-to-parent.jsonl" <<JSON
{{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"${{PLOKE_PROTOTYPE1_CAMPAIGN_ID:?missing campaign}}","node_id":"${{PLOKE_PROTOTYPE1_NODE_ID:?missing node}}","runtime_id":"$runtime_id","message_id":"00000000-0000-4000-8000-000000000002","recorded_at":0,"body_hash":"{evaluating_hash}","body":{evaluating_body}}}
JSON
cat > "$node_dir/results/$runtime_id.json" <<'RESULT'
{result_json}
RESULT
cat >> "$channel_dir/child-to-parent.jsonl" <<JSON
{{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"${{PLOKE_PROTOTYPE1_CAMPAIGN_ID:?missing campaign}}","node_id":"${{PLOKE_PROTOTYPE1_NODE_ID:?missing node}}","runtime_id":"$runtime_id","message_id":"00000000-0000-4000-8000-000000000003","recorded_at":0,"body_hash":"{terminal_hash}","body":{terminal_body}}}
JSON
exit 0
"#
        ),
    );
    let _env = crate::test_support::env_guard_os(vec![("PATH", path)]);

    let baseline = CompleteBaseline::complete(
        "campaign".to_string(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: parent_identity
                .instance_id()
                .expect("test parent instance")
                .to_string(),
            registration_path: None,
            record_path: tmp.path().join("baseline-record.json.gz"),
            metrics: test_metrics(false, true, 0),
        }],
    )
    .expect("complete baseline");

    let outcome = run_planned_child(
        "campaign".to_string(),
        manifest_path.clone(),
        repo_root.clone(),
        prototype1_transition_journal_path(&manifest_path),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Complete,
        Duration::from_secs(30),
        0,
        child,
    )
    .expect("observe spawned child terminal result");

    assert_eq!(outcome.outcome, "completed:Reject");
    assert_eq!(outcome.node_status, Prototype1NodeStatus::Failed);
    assert!(outcome.child_runtime.is_some());
    assert!(outcome.evaluation_report.is_none());
}

#[test]
fn broad_harness_batch_rejects_below_minimum_admitted_transactions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let admission_binding = test_broad_request_admission_binding();
    let budget = Prototype1ChildBudget::new(3, 3);
    let mut slots = Vec::new();
    for _ in 0..budget.max {
        let publication = publish_broad_edit_harness_request(
            &manifest_path,
            &repo_root,
            &parent_identity,
            Prototype1ChildBudget::new(1, 1),
            admission_binding.clone(),
        )
        .expect("publish broad slot");
        slots.push(HarnessRequestSlot {
            request_path: publication.request_path,
            published: publication.published,
        });
    }
    let awaiting_parent = parent.awaiting_harness_plan_for_request((&slots[0].published).into());
    let admitted = slots
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, slot)| {
            let changed_paths = vec![allowed[index].clone(), allowed[index + 1].clone()];
            admit_broad_slot_for_test(&repo_root, slot, &changed_paths, &format!("slot-{index}"))
        })
        .collect::<Vec<_>>();
    let batch = HarnessRequestBatch {
        parent: awaiting_parent,
        slots,
        child_budget: budget,
        patch_generation_parallel_cap: 2,
    };

    let result = publish_broad_harness_child_plan_from_admitted_batch(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        batch,
        admitted,
    );
    let err = match result {
        Ok(_) => panic!("below-minimum broad fanout must not seal a child plan"),
        Err(err) => err,
    };

    let PrepareError::InvalidBatchSelection { detail } = err else {
        panic!("unexpected error variant");
    };
    assert!(detail.contains("fewer than required minimum 3"));
}

#[test]
fn broad_harness_materialization_rejects_post_admission_drift() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let changed_paths = {
        let allowed = write_broad_surface_targets(&repo_root);
        commit_indexed_repo(&repo_root, "broad surface fixture");
        vec![allowed[0].clone(), allowed[1].clone()]
    };
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let publication = publish_broad_edit_harness_request(
        &manifest_path,
        &repo_root,
        parent.identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
    )
    .expect("published request");
    let awaiting_parent = parent.awaiting_harness_plan_for_request((&publication.published).into());
    let receipt = HarnessRequestReceipt {
        parent: awaiting_parent,
        request_path: publication.request_path,
        published: publication.published,
    };
    let candidate_root = receipt.published.workspace_path().to_path_buf();
    GitWorktreeBackend
        .prepare_broad_harness_workspace(&repo_root, &receipt.published)
        .expect("prepare broad harness workspace");
    for relpath in &changed_paths {
        write_surface_target(
            &candidate_root,
            relpath,
            &format!("candidate edit for {}\n", relpath.display()),
        );
    }
    let submitted = submitted_broad_harness_result_for_paths(&receipt.published, &changed_paths);
    let admitted = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            &repo_root,
            EditSurfaceAdmission::new(
                receipt.published.admission_binding().coordinate().clone(),
                SurfacePolicyId::new(receipt.published.admission_binding().policy_id().as_str()),
            ),
            &receipt.published,
            &submitted,
        )
        .expect("admit broad harness result");
    let child_plan = publish_broad_harness_child_plan_from_admitted(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        receipt,
        admitted,
    )
    .expect("multi-file admitted transaction should mint one child artifact");
    let child = &child_plan.plan.body().children()[0];
    let evidence = child.harness_evidence().expect("harness evidence");

    write_surface_target(&candidate_root, &changed_paths[0], "post-admission drift\n");

    let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let c1 = C1::from_child_plan(
        "campaign",
        manifest_path.clone(),
        child.node_record().clone(),
        child.runner_request().clone(),
        child.resolved().clone(),
        repo_root,
    )
    .expect("load c1");
    let err = MaterializeBranch::new()
        .transition_with_harness(c1, evidence, &mut journal)
        .expect_err("post-admission candidate drift must reject");

    assert!(format!("{err:?}").contains("HarnessArtifactMeasurement"));
}

#[test]
fn below_min_rejected_attempts_are_persisted_and_recoverable_from_existing_child_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let rejected = surface_attempt::Evidence::rejected(
        TUI_EDIT_SURFACE_PRODUCER_ID,
        "proposal-rejected",
        "run-rejected",
        "workspace_except_ploke_eval",
        PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
        "backend rejected deterministic proposal",
    );

    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    persist_rejected_surface_attempt_child_plan(&manifest_path, parent, vec![rejected.clone()])
        .expect("persist rejected attempt child plan");

    let resumed_parent = ready_parent_for_test(&manifest_path, &repo_root);
    let receipt = receive_existing_child_plan(
        ChildPlanEnv {
            campaign_id: "campaign",
            manifest_path: &manifest_path,
            repo_root: &repo_root,
        },
        resumed_parent,
    )
    .expect("receive existing child plan");

    assert!(
        receipt.plan.body().children().is_empty(),
        "rejected-attempt-only path must not fabricate child artifacts"
    );
    assert_eq!(
        receipt.plan.body().rejected_surface_attempts(),
        std::slice::from_ref(&rejected)
    );
    assert_eq!(
        receipt.rejected_surface_attempts,
        vec![rejected],
        "rejected attempts should survive receive_existing_child_plan"
    );

    let parent_identity = test_parent_identity();
    let parent_selection = ParentSelection::new(
        &manifest_path,
        &parent_identity,
        &[],
        &receipt.rejected_surface_attempts,
    );
    let projection = parent_selection
        .current_generation_candidates()
        .expect("rejected-only plan should still project payload evidence");
    assert!(
        projection
            .considered
            .iter()
            .all(|payload| payload.artifact.is_none()),
        "rejected-only projection must not fabricate child artifacts"
    );
    assert!(
        projection
            .considered
            .iter()
            .any(|payload| payload.has_parent_readable_surface_attempt()),
        "rejected-only projection should produce parent-readable attempt evidence"
    );
}

#[test]
fn child_plan_replay_rejects_wrong_parent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let rejected = surface_attempt::Evidence::rejected(
        TUI_EDIT_SURFACE_PRODUCER_ID,
        "proposal-rejected",
        "run-rejected",
        "workspace_except_ploke_eval",
        PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
        "backend rejected deterministic proposal",
    );
    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new())
        .with_rejected_surface_attempts(vec![rejected]);
    let at = files.message_at();
    let mut json = serde_json::to_value(&files).expect("child plan serializes");
    json.as_object_mut()
        .expect("child plan json object")
        .insert("parent_node_id".to_string(), "node-other".into());
    fs::create_dir_all(at.path().parent().expect("child plan parent"))
        .expect("create child plan dir");
    write_json_file_pretty(at.path(), &json).expect("write mismatched child plan");

    let (result, trace) = collect_traces(|| {
        receive_existing_child_plan(
            ChildPlanEnv {
                campaign_id: "campaign",
                manifest_path: &manifest_path,
                repo_root: &repo_root,
            },
            parent,
        )
    });
    dump_trace_if_requested(&trace);
    let err = match result {
        Ok(_) => panic!("wrong-parent child plan must not be accepted"),
        Err(err) => err,
    };
    let PrepareError::InvalidBatchSelection { detail } = err else {
        panic!("unexpected error variant");
    };
    assert!(
        detail.contains("child plan is addressed to parent node 'node-other'"),
        "unexpected detail: {detail}"
    );
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=Parent<Ready>->Parent<Planned>",
            "phase=retry_replay",
            "record_access=read",
            "record_kind=child_plan_file",
            "outcome=committed",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=ChildPlan->Parent<Selectable>",
            "phase=message_receive",
            "record_access=read",
            "record_kind=child_plan_file",
            "outcome=failed",
        ],
    ));
}

#[test]
fn child_plan_replay_rejects_malformed_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let at = crate::cli::prototype1_state::inner::At::<ChildPlanFile>::resolve((
        manifest_path.clone(),
        parent_identity.node_id().to_string(),
    ));
    fs::create_dir_all(at.path().parent().expect("child plan parent"))
        .expect("create child plan dir");
    fs::write(at.path(), b"{ not valid child plan json").expect("write malformed child plan");

    let (result, trace) = collect_traces(|| {
        receive_existing_child_plan(
            ChildPlanEnv {
                campaign_id: "campaign",
                manifest_path: &manifest_path,
                repo_root: &repo_root,
            },
            parent,
        )
    });
    dump_trace_if_requested(&trace);
    let err = match result {
        Ok(_) => panic!("malformed child plan must not be accepted"),
        Err(err) => err,
    };
    let PrepareError::InvalidBatchSelection { detail } = err else {
        panic!("unexpected error variant");
    };
    assert!(
        detail.contains("could not decode child plan message"),
        "unexpected detail: {detail}"
    );
    assert!(trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=Parent<Ready>->Parent<Planned>",
            "phase=retry_replay",
            "record_access=read",
            "record_kind=child_plan_file",
            "outcome=failed",
        ],
    ));
    assert!(!trace_contains(
        &trace,
        &[
            "event=typestate_transition",
            "transition=ChildPlan->Parent<Selectable>",
            "phase=message_receive",
        ],
    ));
}

fn test_edit_surface_admission(
    surface: Prototype1EditSurface,
    artifact_id: crate::loop_graph::ArtifactId,
) -> crate::cli::prototype1_state::backend::EditSurfaceAdmission {
    let policy = match surface {
        Prototype1EditSurface::PlokeTuiTools => TUI_EDIT_SURFACE_POLICY_ID.to_string(),
        other => serde_name(&other).to_string(),
    };
    crate::cli::prototype1_state::backend::EditSurfaceAdmission::new(
        crate::loop_graph::Coordinate {
            runtime_id: crate::loop_graph::RuntimeId::new(),
            target: crate::loop_graph::OperationTarget::Artifact { artifact_id },
        },
        crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(policy),
    )
}

fn checked_edit_surface_delta_for_test() -> crate::cli::prototype1_state::edit_surface::ArtifactDelta
{
    use crate::cli::prototype1_state::edit_surface::{
        graph, graph::View as _, harness, harness::Harness as _, surface,
    };
    use crate::loop_graph::ArtifactId;

    let path = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let target = graph::Target::new(&path, "code_edit_tool");
    let file_hash = surface::Hash::new("before-file-hash");
    let base = surface::Ref::new(ArtifactId::new("artifact:base"), surface::Hash::new("base"));
    let after = surface::Ref::new(
        ArtifactId::new("artifact:after"),
        surface::Hash::new("after"),
    );
    let artifact = surface::Artifact::new(base.clone(), [(path.clone(), file_hash.clone())]);
    let view = graph::Mock::new(
        vec![graph::Node::new(target.clone(), path.clone(), 0, 4)],
        [],
    );
    let projection = view.project(&artifact).expect("project graph");
    let bounds = view
        .bounds(&projection, &[graph::Rule::Include(target.clone())])
        .expect("derive bounds");
    let span = view
        .resolve(&projection, &bounds, &target)
        .expect("resolve bounded target");
    let admission =
        test_edit_surface_admission(Prototype1EditSurface::PlokeTuiTools, base.id().clone());
    let grant = surface::Grant::for_coordinate(
        admission.coordinate().clone(),
        admission.policy().clone(),
        base.clone(),
        bounds,
        surface::Area::new([span.clone()]),
    )
    .expect("grant");
    let touch = surface::Touch::new(span, "new");
    let harness = harness::Mock::new(view);
    let (proposal, _run) = harness
        .propose(harness::Input {
            proposal: "proposal-1",
            run: "run-1",
            base: &base,
            after,
            touches: vec![touch],
        })
        .expect("proposal");
    let check = grant.check(proposal.draft()).expect("surface check");
    let applied = harness
        .apply_checked(proposal, check)
        .expect("checked apply");
    applied.delta().clone()
}

fn test_node(
    campaign_root: &Path,
    node_id: &str,
    branch_id: &str,
    candidate_id: &str,
) -> Prototype1NodeRecord {
    let node_dir = campaign_root.join("prototype1").join("nodes").join(node_id);
    Prototype1NodeRecord {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        node_id: node_id.to_string(),
        parent_node_id: None,
        generation: 1,
        instance_id: "clap-rs__clap-3670".to_string(),
        source_state_id: "baseline-run".to_string(),
        operation_target: None,
        base_artifact_id: None,
        patch_id: None,
        derived_artifact_id: None,
        parent_branch_id: None,
        branch_id: branch_id.to_string(),
        candidate_id: candidate_id.to_string(),
        target_relpath: PathBuf::from("crates/ploke-core/tool_text/read_file.md"),
        node_dir: node_dir.clone(),
        workspace_root: PathBuf::from("/tmp/repo"),
        binary_path: node_dir.join("bin/ploke-eval"),
        runner_request_path: node_dir.join("runner-request.json"),
        runner_result_path: node_dir.join("runner-result.json"),
        status: Prototype1NodeStatus::Planned,
        created_at: "2026-04-26T00:00:00Z".to_string(),
        updated_at: "2026-04-26T00:00:00Z".to_string(),
    }
}

fn test_resolved(node: &Prototype1NodeRecord) -> crate::intervention::ResolvedTreatmentBranch {
    crate::intervention::ResolvedTreatmentBranch {
        instance_id: node.instance_id.clone(),
        source_state_id: node.source_state_id.clone(),
        parent_branch_id: node.parent_branch_id.clone(),
        target_relpath: node.target_relpath.clone(),
        source_content: "old".to_string(),
        source_content_hash: "old-hash".to_string(),
        selected_branch_id: Some(node.branch_id.clone()),
        branch: TreatmentBranchNode {
            branch_id: node.branch_id.clone(),
            candidate_id: node.candidate_id.clone(),
            patch_id: None,
            branch_label: "candidate 1".to_string(),
            synthesized_spec_id: "spec-1".to_string(),
            proposed_content: "new".to_string(),
            proposed_content_hash: "new-hash".to_string(),
            generation_target: None,
            generation_coordinate: None,
            status: TreatmentBranchStatus::Selected,
            apply_id: None,
            applied_content_hash: None,
            derived_artifact_id: None,
        },
    }
}

fn test_parent_identity() -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
            .to_string(),
        campaign_id: "campaign".to_string(),
        parent_id: "node-parent".to_string(),
        node_id: "node-parent".to_string(),
        generation: 0,
        instance_id: Some("clap-rs__clap-3670".to_string()),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: "branch-parent".to_string(),
        artifact_branch: Some("prototype1-node-parent".to_string()),
        created_at: "2026-05-06T00:00:00Z".to_string(),
    })
}

fn test_metrics(
    oracle_eligible: bool,
    convergence: bool,
    failed_tool_calls: usize,
) -> OperationalRunMetrics {
    OperationalRunMetrics {
        tool_calls_total: 5,
        tool_calls_failed: failed_tool_calls,
        patch_attempted: true,
        patch_apply_state: if convergence {
            crate::PatchApplyState::Applied
        } else {
            crate::PatchApplyState::No
        },
        submission_artifact_state: if oracle_eligible {
            crate::record::SubmissionArtifactState::Nonempty
        } else {
            crate::record::SubmissionArtifactState::Missing
        },
        patch_projection_check_state: if oracle_eligible {
            ploke_records::evaluation::PatchProjectionCheckState::Passed
        } else {
            ploke_records::evaluation::PatchProjectionCheckState::NotApplicable
        },
        partial_patch_failures: 0,
        same_file_patch_retry_count: 0,
        same_file_patch_max_streak: 0,
        aborted: false,
        aborted_repair_loop: false,
        nonempty_valid_patch: convergence,
        convergence,
        oracle_eligible,
    }
}

fn test_evaluation_report(node: &Prototype1NodeRecord) -> Prototype1BranchEvaluationReport {
    Prototype1BranchEvaluationReport {
        baseline_campaign_id: "baseline".to_string(),
        branch_id: node.branch_id.clone(),
        treatment_campaign_id: "treatment".to_string(),
        evaluation_procedure_id: Some(
            crate::cli::prototype1_state::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID
                .to_string(),
        ),
        evaluator_identity: Some(Prototype1EvaluatorIdentity {
            id: "test-evaluator".to_string(),
            version: "v1".to_string(),
        }),
        eval_set_identity: Some(Prototype1EvalSetIdentity {
            id: "test-eval-set".to_string(),
            kind: "closure_instance_slice".to_string(),
            authority: "typed_child_channel".to_string(),
            explicit: true,
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_sources: Vec::new(),
            eval_policy: EvalCampaignPolicy {
                include_partial: false,
                stop_on_error: false,
                limit: None,
                include_dataset_labels: Vec::new(),
                exclude_dataset_labels: Vec::new(),
                budget: EvalBudget::default(),
                batch_prefix: Some("test-batch".to_string()),
            },
            instance_ids: vec![node.instance_id.clone()],
            missing_treatment_instance_ids: Vec::new(),
            note: None,
        }),
        branch_registry_path: PathBuf::from("/tmp/prototype1/branches.json"),
        evaluation_artifact_path: PathBuf::from(format!("evaluations/{}.json", node.branch_id)),
        treatment_campaign_manifest: PathBuf::from("/tmp/treatment/campaign.json"),
        treatment_closure_state_path: PathBuf::from("/tmp/treatment/closure.json"),
        overall_disposition: BranchDisposition::Keep,
        reasons: Vec::new(),
        compared_instances: vec![Prototype1ComparedInstanceReport {
            instance_id: node.instance_id.clone(),
            baseline_registration_path: Some(PathBuf::from("/tmp/baseline/registration.json")),
            treatment_registration_path: Some(PathBuf::from("/tmp/treatment/registration.json")),
            baseline_record_path: None,
            treatment_record_path: None,
            baseline_metrics: Some(test_metrics(false, false, 0)),
            treatment_metrics: Some(test_metrics(true, true, 0)),
            oracle_evaluation: None,
            evaluation: None,
            status: "compared".to_string(),
        }],
    }
}

fn test_treatment_evidence(node: &Prototype1NodeRecord) -> Prototype1TreatmentEvidence {
    Prototype1TreatmentEvidence {
        baseline_campaign_id: "baseline".to_string(),
        branch_id: node.branch_id.clone(),
        treatment_campaign_id: "treatment".to_string(),
        treatment_campaign_manifest: PathBuf::from("/tmp/treatment/campaign.json"),
        treatment_closure_state_path: PathBuf::from("/tmp/treatment/closure-state.json"),
        eval_policy: test_eval_policy(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        instances: vec![Prototype1TreatmentInstanceEvidence {
            instance_id: node.instance_id.clone(),
            registration_path: Some(PathBuf::from("/tmp/treatment/registration.json")),
            record_path: Some(PathBuf::from("/tmp/treatment/record.json.gz")),
            metrics: Some(test_metrics(true, true, 0)),
            oracle_evaluation: None,
            status: "complete".to_string(),
        }],
    }
}

fn test_eval_policy() -> EvalCampaignPolicy {
    EvalCampaignPolicy {
        include_partial: false,
        stop_on_error: false,
        limit: None,
        include_dataset_labels: Vec::new(),
        exclude_dataset_labels: Vec::new(),
        budget: EvalBudget::default(),
        batch_prefix: Some("test-batch".to_string()),
    }
}

fn test_closure_state_without_record(instance_id: &str) -> crate::closure::ClosureState {
    crate::closure::ClosureState {
        schema_version: "closure-state.v1".to_string(),
        campaign_id: "baseline".to_string(),
        updated_at: "2026-05-10T00:00:00Z".to_string(),
        config: crate::closure::ClosureConfig {
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            model_id: None,
            route_source: None,
            provider_slug: None,
            registry_path: None,
            dataset_sources: Vec::new(),
            required_procedures: Vec::new(),
            instances_root: PathBuf::from("/tmp/instances"),
            batches_root: PathBuf::from("/tmp/batches"),
            framework: crate::spec::FrameworkConfig::default(),
        },
        registry: crate::closure::RegistryClosureSummary {
            expected_total: 1,
            mapped_total: 1,
            missing_total: 0,
            ambiguous_total: 0,
            status: ClosureClass::Complete,
        },
        eval: crate::closure::EvalClosureSummary {
            expected_total: 1,
            complete_total: 1,
            failed_total: 0,
            missing_total: 0,
            partial_total: 0,
            in_progress_total: 0,
            status: ClosureClass::Complete,
            last_transition_at: None,
        },
        protocol: crate::closure::ProtocolClosureSummary {
            expected_total: 1,
            full_total: 0,
            partial_total: 0,
            failed_total: 0,
            missing_total: 1,
            incompatible_total: 0,
            ineligible_total: 0,
            in_progress_total: 0,
            status: ClosureClass::Missing,
            required_procedures: Vec::new(),
            status_by_procedure: BTreeMap::new(),
            last_transition_at: None,
        },
        instances: vec![crate::closure::ClosureInstanceRow {
            instance_id: instance_id.to_string(),
            dataset_label: "test".to_string(),
            repo_family: "test".to_string(),
            registry_status: crate::closure::RegistryInstanceStatus::Mapped,
            eval_status: ClosureClass::Complete,
            protocol_status: ClosureClass::Missing,
            eval_failure: None,
            protocol_failure: None,
            artifacts: crate::closure::ClosureArtifactRefs {
                registration_path: Some(PathBuf::from("/tmp/baseline/registration.json")),
                ..Default::default()
            },
            protocol_procedures: BTreeMap::new(),
            protocol_counts: None,
            last_event_at: None,
        }],
    }
}

#[test]
fn initial_parent_baseline_rejects_complete_projection_without_record() {
    let parent = test_parent_identity();
    let closure = test_closure_state_without_record("clap-rs__clap-3670");

    let err = complete_baseline_from_closure(&parent, &closure, &test_eval_policy())
        .expect_err("complete baseline requires record_path");

    assert!(
        err.to_string().contains("has no record_path"),
        "unexpected error: {err}"
    );
}

#[test]
fn selected_child_treatment_promotes_to_parent_baseline() {
    let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
            .to_string(),
        campaign_id: "baseline".to_string(),
        parent_id: "node-child".to_string(),
        node_id: "node-child".to_string(),
        generation: 1,
        instance_id: Some("clap-rs__clap-3670".to_string()),
        previous_parent_id: Some("node-parent".to_string()),
        parent_node_id: Some("node-parent".to_string()),
        branch_id: "branch-child".to_string(),
        artifact_branch: Some("prototype1-node-child".to_string()),
        created_at: "2026-05-06T00:00:00Z".to_string(),
    });
    let node = test_node(
        Path::new("/tmp/campaign"),
        "node-child",
        "branch-child",
        "candidate-1",
    );
    let mut report = test_evaluation_report(&node);
    report.compared_instances[0].treatment_record_path =
        Some(PathBuf::from("/tmp/treatment/record.json.gz"));

    let baseline =
        complete_baseline_from_selected_treatment(&parent, &report).expect("promote baseline");

    assert_eq!(baseline.parent_node_id(), parent.node_id());
    assert_eq!(baseline.parent_branch_id(), "branch-child");
    assert_eq!(baseline.instances().len(), 1);
    assert_eq!(
        baseline.instances()[0].record_path,
        PathBuf::from("/tmp/treatment/record.json.gz")
    );
    assert_eq!(
        baseline.instances()[0].metrics,
        report.compared_instances[0]
            .treatment_metrics
            .clone()
            .expect("metrics")
    );
}

#[test]
fn parent_compares_treatment_evidence_against_owned_baseline() {
    let parent = test_parent_identity();
    let node = test_node(
        Path::new("/tmp/campaign"),
        "node-child",
        "branch-child",
        "candidate-1",
    );
    let baseline = CompleteBaseline::complete(
        "baseline".to_string(),
        parent.node_id().to_string(),
        parent.branch_id().to_string(),
        "baseline-eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: node.instance_id.clone(),
            registration_path: Some(PathBuf::from("/tmp/baseline/registration.json")),
            record_path: PathBuf::from("/tmp/baseline/record.json.gz"),
            metrics: test_metrics(false, false, 0),
        }],
    )
    .expect("complete baseline");
    let treatment = test_treatment_evidence(&node);

    let report = build_prototype1_branch_evaluation_report(
        "baseline",
        &node.branch_id,
        Path::new("/tmp/prototype1/branches.json"),
        Path::new("/tmp/prototype1/evaluations/branch-child.json"),
        &baseline,
        &treatment,
    )
    .expect("parent comparison");

    assert_eq!(report.overall_disposition, BranchDisposition::Keep);
    assert_eq!(report.compared_instances.len(), 1);
    assert_eq!(report.compared_instances[0].status, "compared");
    assert_eq!(
        report.compared_instances[0].baseline_record_path,
        Some(PathBuf::from("/tmp/baseline/record.json.gz"))
    );
    assert_eq!(
        report.compared_instances[0].treatment_record_path,
        Some(PathBuf::from("/tmp/treatment/record.json.gz"))
    );
}

fn test_surface_evidence(target_relpath: PathBuf) -> SurfaceEvidence {
    let source_hash = format!("{:x}", Sha256::digest("old".as_bytes()));
    let proposed_hash = format!("{:x}", Sha256::digest("new".as_bytes()));
    let generator_surface = GitWorktreeBackend
        .generator_surface_for_surface_touches(
            &target_relpath,
            "old",
            &[SurfaceTouch {
                target_relpath: target_relpath.clone(),
                target_name: "code_edit:0".to_string(),
                span_relpath: target_relpath.clone(),
                start: 0,
                end: 3,
                base_hash: "base-hash".to_string(),
                replacement: "new".to_string(),
                replacement_hash: format!("{:x}", Sha256::digest("new".as_bytes())),
            }],
        )
        .expect("generator surface");
    let transition = CheckedSurfaceTransition {
        target_relpath: target_relpath.clone(),
        base: SurfaceArtifactRef {
            artifact_id: crate::loop_graph::ArtifactId::new("artifact:base-test"),
            hash: "base-hash".to_string(),
        },
        after: SurfaceArtifactRef {
            artifact_id: crate::loop_graph::ArtifactId::new("artifact:after-test"),
            hash: "after-hash".to_string(),
        },
        patch_id: crate::loop_graph::PatchId::new("patch:test"),
    };
    let grant = crate::cli::prototype1_state::history::grant::Grant::<
        crate::cli::prototype1_state::history::grant::Checked,
    >::checked(
        crate::loop_graph::Coordinate {
            runtime_id: crate::loop_graph::RuntimeId(uuid::Uuid::nil()),
            target: crate::loop_graph::OperationTarget::Artifact {
                artifact_id: transition.base.artifact_id.clone(),
            },
        },
        ProcedureRef::new(TUI_EDIT_SURFACE_POLICY_ID),
        SurfaceWritable {
            target_relpath: target_relpath.clone(),
        },
        &transition,
    )
    .expect("checked surface grant");
    SurfaceEvidence::checked(
        TUI_EDIT_SURFACE_PRODUCER_ID,
        "proposal-test",
        "run-test",
        CheckedSurface { grant, transition },
        source_hash,
        proposed_hash,
        crate::cli::prototype1_state::edit_surface::request_policy::ProposalProducer::NonRouter,
        generator_surface,
        vec![SurfaceTouch {
            target_relpath: target_relpath.clone(),
            target_name: "code_edit:0".to_string(),
            span_relpath: target_relpath,
            start: 0,
            end: 3,
            base_hash: "base-hash".to_string(),
            replacement: "new".to_string(),
            replacement_hash: format!("{:x}", Sha256::digest("new".as_bytes())),
        }],
    )
    .expect("surface evidence")
}

fn bind_test_tui_surface_fields(
    node: &mut Prototype1NodeRecord,
    resolved: &mut crate::intervention::ResolvedTreatmentBranch,
) {
    let runtime_id = crate::loop_graph::RuntimeId(uuid::Uuid::nil()).to_string();
    let base = crate::loop_graph::ArtifactId::new("artifact:base-test");
    let after = crate::loop_graph::ArtifactId::new("artifact:after-test");
    let patch = crate::loop_graph::PatchId::new("patch:test");
    let source_hash = format!("{:x}", Sha256::digest("old".as_bytes()));
    let proposed_hash = format!("{:x}", Sha256::digest("new".as_bytes()));
    node.instance_id = runtime_id;
    node.operation_target = Some(crate::loop_graph::OperationTarget::Artifact {
        artifact_id: base.clone(),
    });
    node.base_artifact_id = Some(base.clone());
    node.patch_id = Some(patch.clone());
    node.derived_artifact_id = Some(after.clone());
    resolved.source_content = "old".to_string();
    resolved.source_content_hash = source_hash;
    resolved.branch.proposed_content = "new".to_string();
    resolved.branch.proposed_content_hash = proposed_hash;
    resolved.branch.patch_id = Some(patch);
    resolved.branch.generation_target =
        Some(crate::loop_graph::OperationTarget::Artifact { artifact_id: base });
    resolved.branch.apply_id = Some("proposal-test".to_string());
    resolved.branch.derived_artifact_id = Some(after);
}

fn test_completed_outcome(
    mut node: Prototype1NodeRecord,
    resolved: crate::intervention::ResolvedTreatmentBranch,
    plan_index: usize,
) -> PlannedChildOutcome {
    node.status = Prototype1NodeStatus::Succeeded;
    let report = test_evaluation_report(&node);
    let selection_input = selection_input_from_child_report(&node, &report);
    PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: "completed:Keep".to_string(),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved,
        child_runtime: Some(format!("runtime:{}", node.node_id)),
        evaluation_report: Some(report),
        selection_input: Some(selection_input),
        surface: None,
        artifact_surface: Some(ArtifactSurface::test(&node.node_id)),
        node,
    }
}

#[test]
fn historical_node_150_channel_treatment_reaches_current_generation_handoff() {
    const NODE_ID: &str = "node-15006265e24b3b9b";

    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let parent_identity: ParentIdentity = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/parent_identity.json"
    ));
    let closure: crate::closure::ClosureState = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/baseline_closure_state.json"
    ));
    let child_plan: ChildPlanFiles = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/child_plan_node-f4cf695decef97df.json"
    ));
    let node: Prototype1NodeRecord = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/node.json"
    ));
    let runner_request: crate::intervention::Prototype1RunnerRequest = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/runner-request.json"
    ));
    let runner_result: crate::intervention::Prototype1RunnerResult = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/runner-result.json"
    ));

    let (plan_index, child) = child_plan
        .children()
        .iter()
        .enumerate()
        .find(|(_, child)| child.node_id() == NODE_ID)
        .expect("node-150 child plan entry");
    assert_eq!(child_plan.parent_node_id(), parent_identity.node_id());
    assert_eq!(child.node_record().node_id, node.node_id);
    assert_eq!(child.node_record().branch_id, node.branch_id);
    assert_eq!(child.runner_request().node_id, runner_request.node_id);
    assert_eq!(child.runner_request().branch_id, runner_request.branch_id);
    assert_eq!(
        child.runner_request().runner_args,
        runner_request.runner_args
    );
    assert_eq!(child.runner_request().workspace_root, PathBuf::from("."));
    assert!(
        runner_request
            .workspace_root
            .ends_with("prototype1/workspaces/edit-harness/node-f4cf695decef97df-r6")
    );
    assert_eq!(runner_result.node_id, node.node_id);
    assert_eq!(runner_result.branch_id, node.branch_id);
    assert_eq!(runner_result.status, node.status);

    let terminal =
        include_str!("../../../tests/fixtures/prototype1-node-150-handoff/child-to-parent.jsonl")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<
                    crate::cli::prototype1_state::channel::Envelope<
                        crate::cli::prototype1_state::channel::ToParent,
                    >,
                >(line)
                .expect("channel envelope")
            })
            .find_map(|envelope| match envelope.body() {
                crate::cli::prototype1_state::channel::ToParent::Result {
                    runner_result,
                    treatment,
                } => Some((
                    envelope.runtime_id().to_string(),
                    runner_result.clone(),
                    treatment
                        .clone()
                        .expect("successful child result carries treatment"),
                )),
                _ => None,
            })
            .expect("terminal channel result");
    assert_eq!(terminal.1, runner_result);
    assert_eq!(terminal.2.branch_id, node.branch_id);
    assert_eq!(
        terminal.2.treatment_campaign_id,
        "p1-gemini35-flash-direct-15g2x3-20260525-035000-treatment-branch-c56614c6e6a63aa9-1779711014414"
    );
    let treatment_record_path = tmp.path().join("treatment-record.json.gz");
    fs::write(
        &treatment_record_path,
        include_bytes!(
            "../../../tests/fixtures/prototype1-node-150-handoff/treatment-record.json.gz"
        ),
    )
    .expect("write treatment record fixture");
    let treatment_record =
        crate::record::read_compressed_record(&treatment_record_path).expect("treatment record");
    assert_eq!(
        Some(treatment_record.operational_metrics()),
        terminal.2.instances[0].metrics
    );

    let baseline_record_path = tmp.path().join("baseline-record.json.gz");
    fs::write(
        &baseline_record_path,
        include_bytes!(
            "../../../tests/fixtures/prototype1-node-150-handoff/baseline-record.json.gz"
        ),
    )
    .expect("write baseline record fixture");
    let baseline_record =
        crate::record::read_compressed_record(&baseline_record_path).expect("baseline record");
    let baseline_metrics = baseline_record.operational_metrics();
    let baseline_row = closure
        .instances
        .iter()
        .find(|row| {
            terminal
                .2
                .instances
                .iter()
                .any(|treatment| treatment.instance_id == row.instance_id)
        })
        .expect("matching baseline closure row");
    let instance_ids = vec![baseline_row.instance_id.clone()];
    let parent_baseline = CompleteBaseline::complete(
        closure.campaign_id.clone(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        prototype1_eval_set_id(
            &closure.campaign_id,
            &closure.campaign_id,
            closure.config.benchmark_family,
            &closure.config.dataset_sources,
            &terminal.2.eval_policy,
            &instance_ids,
        ),
        vec![BaselineInstance {
            instance_id: baseline_row.instance_id.clone(),
            registration_path: baseline_row.artifacts.registration_path.clone(),
            record_path: baseline_record_path,
            metrics: baseline_metrics,
        }],
    )
    .expect("complete baseline from real closure row and record");

    let branch_log_gate = Mutex::new(());
    let report = compare_observed_child_treatment(
        parent_identity.campaign_id(),
        &manifest_path,
        &parent_baseline,
        child.resolved(),
        &terminal.2,
        &branch_log_gate,
    )
    .expect("parent compares historical child treatment");
    assert_eq!(report.branch_id, node.branch_id);
    assert_eq!(report.overall_disposition, BranchDisposition::Keep);

    let outcome = PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: format!("completed:{:?}", report.overall_disposition),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved: child.resolved().clone(),
        child_runtime: Some(terminal.0),
        evaluation_report: Some(report.clone()),
        selection_input: Some(selection_input_from_child_report(&node, &report)),
        surface: child.surface().cloned(),
        artifact_surface: Some(
            child
                .harness_evidence()
                .expect("historical child has broad harness evidence")
                .artifact_surface()
                .clone(),
        ),
        node,
    };
    let profile = toml::from_str::<profile::Prototype1RunProfile>(
        r#"
schema_version = "prototype1-run-profile.v1"
name = "historical-node-150-handoff"

[selection]
strategy = "generation-local"
evidence = "operational"
seed = 0
"#,
    )
    .expect("profile parses");
    profile.validate().expect("profile validates");

    let (decision, material) = select_successor_for_profile(
        &manifest_path,
        &parent_identity,
        std::slice::from_ref(&outcome),
        &[],
        &profile,
    )
    .expect("selector runs")
    .expect("selector chooses node-150");
    assert_eq!(decision.candidate_node_id, NODE_ID);
    assert_eq!(
        decision.selected_branch_id.as_deref(),
        Some(outcome.node.branch_id.as_str())
    );
    assert!(material.selected_from_generation_outcomes);

    let (selection, trace) = collect_traces(|| {
        select_artifact_for_handoff(&decision, &material)
            .expect("current generation selection admits an Artifact carrier")
    });
    dump_trace_if_requested(&trace);

    assert_eq!(selection.selected().node().node_id, NODE_ID);
    assert_eq!(selection.selected().branch_id(), outcome.node.branch_id);
    assert_eq!(
        selection.selected().source(),
        state_selection::Source::CurrentGeneration
    );
    assert_eq!(
        selection.selected().primary_runtime_id(),
        outcome.child_runtime.as_deref()
    );
    assert_eq!(
        selection.selected().resolved().branch.branch_id,
        outcome.resolved.branch.branch_id
    );
    assert!(trace_contains(
        &trace,
        &[
            "span:select_artifact_for_handoff",
            "selected_from_generation_outcomes=true",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "event:ploke_exec",
            "hydrated selected Artifact payload for successor handoff",
            "node_id=node-15006265e24b3b9b",
            "source=CurrentGeneration",
        ],
    ));
}

#[test]
fn current_generation_selector_trace_follows_child_channel_evidence_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    let resolved = test_resolved(&node);
    let outcomes = vec![test_completed_outcome(node, resolved, 0)];
    let parent_identity = test_parent_identity();
    let parent_selection = ParentSelection::new(&manifest_path, &parent_identity, &outcomes, &[]);

    let (projection, trace) = collect_traces(|| {
        parent_selection
            .current_generation_candidates()
            .expect("current generation candidate projection")
    });
    dump_trace_if_requested(&trace);

    assert_eq!(projection.considered.len(), 1);
    let payload = &projection.considered[0];
    assert!(payload.artifact.is_some());
    assert!(
        payload.decision_grade_eligibility().eligible,
        "unexpected decision-grade gaps: {:?}",
        payload.decision_grade_eligibility().identity_gaps
    );
    let sealed = payload.sealed_evidence.as_ref().expect("sealed evidence");
    assert_eq!(sealed.evaluations.len(), 1);
    assert_eq!(
        sealed.evaluations[0].primary_report_citation.ref_id,
        "inline:child-channel:evaluation-report:branch-child"
    );

    assert!(trace_contains(
        &trace,
        &[
            "span:current_generation_candidates",
            "evidence_boundary=child_outcome_channel",
            "parent_node_id=node-parent",
            "child_count=1",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "span:current_generation_candidate_evidence",
            "evidence_boundary=child_outcome_channel",
            "node_id=node-child",
            "branch_id=branch-child",
            "has_evaluation_report=true",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "event:ploke_exec",
            "sealed current-generation candidate evidence from typed child outcome",
            "evaluation_count=1",
            "branch_count=1",
        ],
    ));
    assert!(
        trace
            .iter()
            .all(|line| !line.contains("FsEvidenceStore") && !line.contains("history_preview")),
        "trace should stay on the typed child outcome path: {trace:#?}"
    );
}

#[test]
fn current_generation_candidates_include_edit_surface_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    node.target_relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    bind_test_tui_surface_fields(&mut node, &mut resolved);
    let mut outcome = test_completed_outcome(node.clone(), resolved, 0);
    outcome.surface = Some(test_surface_evidence(node.target_relpath.clone()));
    let parent_identity = test_parent_identity();
    let parent_selection = ParentSelection::new(
        &manifest_path,
        &parent_identity,
        std::slice::from_ref(&outcome),
        &[],
    );

    let projection = parent_selection
        .current_generation_candidates()
        .expect("current generation candidate projection");

    let payload = &projection.considered[0];
    let artifact = payload.artifact.as_ref().expect("candidate artifact");
    let surface = artifact.surface.as_ref().expect("surface evidence");
    assert_eq!(surface.producer_id, TUI_EDIT_SURFACE_PRODUCER_ID);
    assert_eq!(surface.proposal_id, "proposal-test");
    assert_eq!(surface.target_relpath, node.target_relpath);
    assert_eq!(surface.touches.len(), 1);
    assert!(surface.delta_id.starts_with("surface-delta:"));
    assert!(
        payload.has_parent_readable_surface_attempt(),
        "applied surface evidence should project parent-readable attempt evidence"
    );

    let serialized = serde_json::to_value(payload).expect("payload json");
    assert_eq!(
        serialized["artifact"]["surface"]["producer_id"],
        TUI_EDIT_SURFACE_PRODUCER_ID
    );
    assert_eq!(
        serialized["artifact"]["surface"]["touches"][0]["replacement"],
        "new"
    );
    assert!(
        serialized["artifact"]["surface"]["delta_digest"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(
        serialized["surface_attempt"]["outcome"]["kind"], "applied",
        "surface attempt evidence should be parent-readable on payload"
    );
}

#[test]
fn current_generation_candidates_include_rejected_edit_surface_attempt_payload() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    let outcome = test_completed_outcome(
        node,
        test_resolved(&test_node(
            tmp.path(),
            "node-child",
            "branch-child",
            "candidate-1",
        )),
        0,
    );
    let parent_identity = test_parent_identity();
    let rejected = surface_attempt::Evidence::rejected(
        TUI_EDIT_SURFACE_PRODUCER_ID,
        "proposal-rejected",
        "run-rejected",
        "workspace_except_ploke_eval",
        PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
        "one or more touched spans were rejected",
    );
    let parent_selection = ParentSelection::new(
        &manifest_path,
        &parent_identity,
        std::slice::from_ref(&outcome),
        std::slice::from_ref(&rejected),
    );

    let projection = parent_selection
        .current_generation_candidates()
        .expect("current generation candidate projection");
    let rejected_payload = projection
        .considered
        .iter()
        .find(|payload| payload.surface_attempt.as_ref() == Some(&rejected))
        .expect("rejected attempt payload");

    assert!(
        rejected_payload.artifact.is_none(),
        "rejected attempt payload must not carry a candidate artifact"
    );
    assert!(
        rejected_payload.has_parent_readable_surface_attempt(),
        "rejected attempt payload should be parent-readable"
    );
}

#[test]
fn payload_surface_attempt_rejected_is_parent_readable_without_artifact() {
    let payload = EvaluationPayload::builder(
        SubjectRef::new("candidate:rejected-attempt:plan_index=0"),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .surface_attempt_evidence(surface_attempt::Evidence::rejected(
        TUI_EDIT_SURFACE_PRODUCER_ID,
        "proposal-rejected",
        "run-rejected",
        "workspace_except_ploke_eval",
        PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
        "one or more touched spans were rejected",
    ))
    .build();

    assert!(
        payload.has_parent_readable_surface_attempt(),
        "rejected attempt evidence must be visible from EvaluationPayload"
    );
    assert!(
        payload.artifact.is_none(),
        "rejected attempt evidence must not imply a derived artifact"
    );

    let serialized = serde_json::to_value(&payload).expect("payload json");
    assert_eq!(
        serialized["surface_attempt"]["producer_id"],
        TUI_EDIT_SURFACE_PRODUCER_ID
    );
    assert_eq!(serialized["surface_attempt"]["outcome"]["kind"], "rejected");
    assert_eq!(
        serialized["surface_attempt"]["outcome"]["reason"],
        "one or more touched spans were rejected"
    );
}

#[test]
fn payload_without_surface_attempt_is_not_parent_readable_attempt_evidence() {
    let payload = EvaluationPayload::builder(
        SubjectRef::new("candidate:projection-only:plan_index=0"),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .projection_failure(
        SelectionProjectionFailure::committed(
            SelectionProjectionFailureKind::MissingSelectionInput,
            None,
            Some(
                "log-only rejected apply mention should not count as typed attempt evidence"
                    .to_string(),
            ),
        )
        .expect("projection failure"),
    )
    .build();

    assert!(
        !payload.has_parent_readable_surface_attempt(),
        "log/projection-only state must not count as parent-readable attempt evidence"
    );
}

#[test]
fn current_generation_candidates_reject_deterministic_missing_surface_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    let outcome = test_completed_outcome(node, resolved, 0);
    let parent_identity = test_parent_identity();
    let parent_selection = ParentSelection::new(
        &manifest_path,
        &parent_identity,
        std::slice::from_ref(&outcome),
        &[],
    );

    let err = match parent_selection.current_generation_candidates() {
        Ok(_) => panic!("deterministic producer must fail closed without evidence"),
        Err(err) => err,
    };

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(
        err.to_string()
            .contains("missing checked edit-surface evidence")
    );
}

#[test]
fn requested_tui_surface_child_rejects_legacy_plan_child() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    let child = ChildFiles::from_resolved(
        "campaign",
        node,
        test_resolved(&test_node(
            tmp.path(),
            "node-child",
            "branch-child",
            "candidate-1",
        )),
        false,
    );

    let err = match validate_requested_tui_surface_child(&child) {
        Ok(_) => panic!("tui generator must not reuse legacy child plan entries"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("cannot use child"));
    assert!(err.to_string().contains("produced by 'spec-1'"));
}

#[test]
fn requested_tui_surface_child_rejects_missing_generator_surface_provenance() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    node.target_relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    bind_test_tui_surface_fields(&mut node, &mut resolved);
    let mut surface = test_surface_evidence(node.target_relpath.clone());
    surface.generator_surface = None;
    let child = ChildFiles::from_resolved("campaign", node, resolved, false).with_surface(surface);

    let err = match validate_requested_tui_surface_child(&child) {
        Ok(_) => panic!("missing generator provenance must reject"),
        Err(err) => err,
    };

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(
        err.to_string()
            .contains("missing generator_surface provenance")
    );
}

#[test]
fn requested_tui_surface_child_rejects_mismatched_generator_surface_provenance() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    node.target_relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    bind_test_tui_surface_fields(&mut node, &mut resolved);
    let mut surface = test_surface_evidence(node.target_relpath.clone());
    let generator_surface = surface
        .generator_surface
        .as_mut()
        .expect("surface generator provenance");
    generator_surface.source_version = "v2".to_string();
    let child = ChildFiles::from_resolved("campaign", node, resolved, false).with_surface(surface);

    let err = match validate_requested_tui_surface_child(&child) {
        Ok(_) => panic!("mismatched generator provenance must reject"),
        Err(err) => err,
    };

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(err.to_string().contains("carried generator_surface"));
}

#[test]
fn requested_tui_surface_child_rejects_router_backed_proposal_producer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    node.target_relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    bind_test_tui_surface_fields(&mut node, &mut resolved);
    let mut surface = test_surface_evidence(node.target_relpath.clone());
    surface.proposal_producer =
            crate::cli::prototype1_state::edit_surface::request_policy::ProposalProducer::Router {
                request_policy:
                    crate::cli::prototype1_state::edit_surface::request_policy::Receipt {
                        schema_version: 1,
                        base_artifact_id: surface.base.artifact_id.clone(),
                        objective:
                            crate::cli::prototype1_state::edit_surface::request_policy::ObjectiveBinding {
                                summary: "router proposal".to_string(),
                                target_metric: "invalid_edit_surface_candidates".to_string(),
                                writable_intent: "semantic_resolution".to_string(),
                            },
                        proposal:
                            crate::cli::prototype1_state::edit_surface::request_policy::ProposalBinding {
                                proposal_id: surface.proposal_id.clone(),
                                run_id: surface.run_id.clone(),
                            },
                        router: "openrouter".to_string(),
                        model:
                            crate::cli::prototype1_state::edit_surface::request_policy::Effective {
                                value: "openai/gpt-5".to_string(),
                                origin:
                                    crate::cli::prototype1_state::edit_surface::request_policy::PolicyOrigin::Explicit,
                            },
                        response_format:
                            crate::cli::prototype1_state::edit_surface::request_policy::Effective {
                                value:
                                    crate::cli::prototype1_state::edit_surface::request_policy::ResponseFormat::JsonObject,
                                origin:
                                    crate::cli::prototype1_state::edit_surface::request_policy::PolicyOrigin::Explicit,
                            },
                        stop:
                            crate::cli::prototype1_state::edit_surface::request_policy::Effective {
                                value: vec!["STOP".to_string()],
                                origin:
                                    crate::cli::prototype1_state::edit_surface::request_policy::PolicyOrigin::Default,
                            },
                        stream:
                            crate::cli::prototype1_state::edit_surface::request_policy::Effective {
                                value: false,
                                origin:
                                    crate::cli::prototype1_state::edit_surface::request_policy::PolicyOrigin::Default,
                            },
                        parameters:
                            crate::cli::prototype1_state::edit_surface::request_policy::ParameterPolicy::default(),
                        provider: None,
                        request_payload_hash: Some(
                            crate::cli::prototype1_state::edit_surface::request_policy::PayloadHash::unknown(
                                "deterministic path does not capture router request payload",
                            ),
                        ),
                        response_payload_hash: Some(
                            crate::cli::prototype1_state::edit_surface::request_policy::PayloadHash::unknown(
                                "deterministic path does not capture router response payload",
                            ),
                        ),
                        client_policy_hash: "not-checked-on-deterministic-path".to_string(),
                    },
            };
    let child = ChildFiles::from_resolved("campaign", node, resolved, false).with_surface(surface);

    let err = match validate_requested_tui_surface_child(&child) {
        Ok(_) => panic!("router-backed deterministic provenance must reject"),
        Err(err) => err,
    };

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(
        err.to_string()
            .contains("Router-backed proposal provenance")
    );
}

#[test]
fn current_generation_candidates_reject_surface_artifact_mismatch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    node.target_relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    bind_test_tui_surface_fields(&mut node, &mut resolved);
    node.derived_artifact_id = Some(crate::loop_graph::ArtifactId::new("artifact:wrong-after"));
    let mut outcome = test_completed_outcome(node.clone(), resolved, 0);
    outcome.surface = Some(test_surface_evidence(node.target_relpath.clone()));
    let parent_identity = test_parent_identity();
    let parent_selection = ParentSelection::new(
        &manifest_path,
        &parent_identity,
        std::slice::from_ref(&outcome),
        &[],
    );

    let err = match parent_selection.current_generation_candidates() {
        Ok(_) => panic!("mismatched surface Artifact must fail before sealing"),
        Err(err) => err,
    };

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(err.to_string().contains("derived_artifact_id"));
}

#[test]
fn history_handoff_rejects_missing_artifact_payload_before_seal() {
    let selected = SubjectRef::new("candidate:node-historical:plan_index=0");
    let payload = EvaluationPayload::builder(
        selected.clone(),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .sealed_candidate_evidence(
        crate::cli::prototype1_state::history::SealedCandidateEvidence {
            schema_version: 2,
            coordinate: crate::cli::prototype1_state::history::CandidateCoordinate {
                node_id: "node-historical".to_string(),
                parent_node_id: Some("node-parent".to_string()),
                branch_id: Some("branch-historical".to_string()),
                generation: Some(1),
                plan_index: Some(0),
                primary_runtime_id: Some("runtime:node-historical".to_string()),
            },
            lifecycle: crate::cli::prototype1_state::history::CandidateLifecycle {
                planner_outcome: "completed".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: Vec::new(),
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        },
    )
    .build();
    let material = SelectionSealMaterial {
        procedure: ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
        scope: SelectionScope::all_admitted_candidates(),
        selected_candidate: selected,
        selected_occurrence_id: None,
        selected_membership_id: None,
        considered: vec![payload.clone()],
        considered_sources: Vec::new(),
        projection_failures: Vec::new(),
        traversal: Some(TraversalEvidence {
            seed: 1,
            strategy: StrategyKind::default(),
            selected_source: None,
        }),
        metrics: selection_metrics_for(&[payload.clone()], &[]),
        selected_from_generation_outcomes: false,
    };
    let decision = SuccessorDecision {
        procedure_id: crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID.to_string(),
        candidate_node_id: "node-historical".to_string(),
        selected_branch_id: Some("branch-historical".to_string()),
        branch_disposition: "keep".to_string(),
        outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
        findings: Vec::new(),
        rationale: Vec::new(),
    };

    let err = select_artifact_for_handoff(&decision, &material)
        .expect_err("historical candidate without sealed Artifact payload must not hand off");

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(
        err.to_string()
            .contains("has no sealed Artifact payload for handoff hydration")
    );
}

#[test]
fn history_handoff_rejects_missing_artifact_surface_before_seal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let node = test_node(
        tmp.path(),
        "node-historical",
        "branch-historical",
        "candidate-1",
    );
    let resolved = test_resolved(&node);
    let selected = SubjectRef::new("candidate:node-historical:plan_index=0");
    let payload = EvaluationPayload::builder(
        selected.clone(),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .sealed_candidate_evidence(
        crate::cli::prototype1_state::history::SealedCandidateEvidence {
            schema_version: 2,
            coordinate: crate::cli::prototype1_state::history::CandidateCoordinate {
                node_id: "node-historical".to_string(),
                parent_node_id: Some("node-parent".to_string()),
                branch_id: Some("branch-historical".to_string()),
                generation: Some(1),
                plan_index: Some(0),
                primary_runtime_id: Some("runtime:node-historical".to_string()),
            },
            lifecycle: crate::cli::prototype1_state::history::CandidateLifecycle {
                planner_outcome: "completed".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: Vec::new(),
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        },
    )
    .candidate_artifact(CandidateArtifact::new(node.clone(), resolved))
    .build();
    let material = SelectionSealMaterial {
        procedure: ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
        scope: SelectionScope::all_admitted_candidates(),
        selected_candidate: selected,
        selected_occurrence_id: None,
        selected_membership_id: None,
        considered: vec![payload.clone()],
        considered_sources: Vec::new(),
        projection_failures: Vec::new(),
        traversal: Some(TraversalEvidence {
            seed: 1,
            strategy: StrategyKind::default(),
            selected_source: None,
        }),
        metrics: selection_metrics_for(&[payload.clone()], &[]),
        selected_from_generation_outcomes: false,
    };
    let decision = SuccessorDecision {
        procedure_id: crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID.to_string(),
        candidate_node_id: "node-historical".to_string(),
        selected_branch_id: Some("branch-historical".to_string()),
        branch_disposition: "keep".to_string(),
        outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
        findings: Vec::new(),
        rationale: Vec::new(),
    };

    let err = select_artifact_for_handoff(&decision, &material)
        .expect_err("historical candidate without artifact surface must not hand off");

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(
        err.to_string()
            .contains("no admitted artifact surface measurement")
    );
}

#[test]
fn history_handoff_rejects_unresolvable_runtime_before_seal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let node = test_node(
        tmp.path(),
        "node-historical",
        "branch-historical",
        "candidate-1",
    );
    let resolved = test_resolved(&node);
    let selected = SubjectRef::new("candidate:node-historical:plan_index=0");
    let payload = EvaluationPayload::builder(
        selected.clone(),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .sealed_candidate_evidence(
        crate::cli::prototype1_state::history::SealedCandidateEvidence {
            schema_version: 2,
            coordinate: crate::cli::prototype1_state::history::CandidateCoordinate {
                node_id: "node-historical".to_string(),
                parent_node_id: Some("node-parent".to_string()),
                branch_id: Some("branch-historical".to_string()),
                generation: Some(1),
                plan_index: Some(0),
                primary_runtime_id: None,
            },
            lifecycle: crate::cli::prototype1_state::history::CandidateLifecycle {
                planner_outcome: "completed".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: Vec::new(),
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        },
    )
    .candidate_artifact(
        CandidateArtifact::new(node.clone(), resolved)
            .with_artifact_surface(ArtifactSurface::test("node-historical")),
    )
    .build();
    let material = SelectionSealMaterial {
        procedure: ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
        scope: SelectionScope::all_admitted_candidates(),
        selected_candidate: selected,
        selected_occurrence_id: None,
        selected_membership_id: None,
        considered: vec![payload.clone()],
        considered_sources: Vec::new(),
        projection_failures: Vec::new(),
        traversal: Some(TraversalEvidence {
            seed: 1,
            strategy: StrategyKind::default(),
            selected_source: None,
        }),
        metrics: selection_metrics_for(&[payload.clone()], &[]),
        selected_from_generation_outcomes: false,
    };
    let decision = SuccessorDecision {
        procedure_id: crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID.to_string(),
        candidate_node_id: "node-historical".to_string(),
        selected_branch_id: Some("branch-historical".to_string()),
        branch_disposition: "keep".to_string(),
        outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
        findings: Vec::new(),
        rationale: Vec::new(),
    };
    let err = select_artifact_for_handoff(&decision, &material)
        .expect_err("historical candidate without runtime identity must not hand off");

    assert!(matches!(err, PrepareError::InvalidBatchSelection { .. }));
    assert!(err.to_string().contains("lacks sealed runtime identity"));
}

#[test]
fn history_handoff_selection_carries_resolved_artifact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let node = test_node(
        tmp.path(),
        "node-historical",
        "branch-historical",
        "candidate-1",
    );
    let resolved = test_resolved(&node);
    let selected = SubjectRef::new("candidate:node-historical:plan_index=0");
    let payload = EvaluationPayload::builder(
        selected.clone(),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .sealed_candidate_evidence(
        crate::cli::prototype1_state::history::SealedCandidateEvidence {
            schema_version: 2,
            coordinate: crate::cli::prototype1_state::history::CandidateCoordinate {
                node_id: "node-historical".to_string(),
                parent_node_id: Some("node-parent".to_string()),
                branch_id: Some("branch-historical".to_string()),
                generation: Some(1),
                plan_index: Some(0),
                primary_runtime_id: Some("runtime:node-historical".to_string()),
            },
            lifecycle: crate::cli::prototype1_state::history::CandidateLifecycle {
                planner_outcome: "completed".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: Vec::new(),
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        },
    )
    .candidate_artifact(
        CandidateArtifact::new(node.clone(), resolved)
            .with_artifact_surface(ArtifactSurface::test("node-historical")),
    )
    .build();
    let material = SelectionSealMaterial {
        procedure: ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
        scope: SelectionScope::all_admitted_candidates(),
        selected_candidate: selected,
        selected_occurrence_id: None,
        selected_membership_id: None,
        considered: vec![payload.clone()],
        considered_sources: Vec::new(),
        projection_failures: Vec::new(),
        traversal: Some(TraversalEvidence {
            seed: 1,
            strategy: StrategyKind::default(),
            selected_source: None,
        }),
        metrics: selection_metrics_for(&[payload.clone()], &[]),
        selected_from_generation_outcomes: false,
    };
    let decision = SuccessorDecision {
        procedure_id: crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID.to_string(),
        candidate_node_id: "node-historical".to_string(),
        selected_branch_id: Some("branch-historical".to_string()),
        branch_disposition: "keep".to_string(),
        outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
        findings: Vec::new(),
        rationale: Vec::new(),
    };

    let (selection, trace) = collect_traces(|| {
        select_artifact_for_handoff(&decision, &material)
            .expect("historical selection admits an Artifact carrier")
    });
    dump_trace_if_requested(&trace);

    assert_eq!(selection.selected().node().node_id, "node-historical");
    assert_eq!(selection.selected().branch_id(), "branch-historical");
    assert_eq!(
        selection.selected().source(),
        state_selection::Source::History
    );
    assert_eq!(
        selection.selected().primary_runtime_id(),
        Some("runtime:node-historical")
    );
    assert!(trace_contains(
        &trace,
        &[
            "span:select_artifact_for_handoff",
            "selected_candidate=candidate:node-historical:plan_index=0",
            "candidate_node_id=node-historical",
            "selected_from_generation_outcomes=false",
        ],
    ));
    assert!(trace_contains(
        &trace,
        &[
            "event:ploke_exec",
            "hydrated selected Artifact payload for successor handoff",
            "node_id=node-historical",
            "branch_id=branch-historical",
            "source=History",
            "primary_runtime_id=Some(\"runtime:node-historical\")",
        ],
    ));
}

#[test]
fn observation_log_loads_chat_http_attempts_from_trace_spans() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log_path = tmp.path().join("prototype1_observation_test.jsonl");
    fs::write(
            &log_path,
            concat!(
                r#"{"timestamp":"2026-05-02T00:00:00Z","target":"chat_http","event":"chat_http_request_start","request_id":7,"attempt":1,"max_attempts":3,"model":"model-a","span":{"campaign_id":"campaign-a","node_id":"node-a","branch_id":"branch-a","generation":2}}"#,
                "\n",
                r#"{"timestamp":"2026-05-02T00:00:01Z","target":"chat_http","event":"chat_http_retry_scheduled","request_id":7,"attempt":1,"max_attempts":3,"phase":"body","status":503,"elapsed_ms":1000,"backoff_ms":250,"spans":[{"name":"prototype1.child.evaluate.eval_closure","campaign_id":"campaign-a","branch_id":"branch-a","generation":2}]}"#,
                "\n",
                r#"{"timestamp":"2026-05-02T00:00:03Z","target":"chat_http","event":"chat_http_request_completed","request_id":7,"attempt":2,"max_attempts":3,"status":200,"elapsed_ms":2000,"spans":[{"name":"prototype1.child.evaluate.eval_closure","campaign_id":"campaign-a","branch_id":"branch-a","generation":2}]}"#,
                "\n",
            ),
        )
        .expect("write log");

    let mut evidence = ObservationEvidence::default();
    parse_observation_log("campaign-a", &log_path, &mut evidence);

    assert!(evidence.steps.is_empty());
    assert_eq!(evidence.provider_http.len(), 3);
    assert_eq!(evidence.provider_http[0].node_id.as_deref(), Some("node-a"));
    assert_eq!(
        evidence.provider_http[1].branch_id.as_deref(),
        Some("branch-a")
    );

    let requests = provider_http_requests(&evidence.provider_http);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].retry_count(), 1);
    assert_eq!(requests[0].elapsed_ms(), 3250);
}

#[test]
fn tool_result_trace_projection_parses_turn_trace_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let trace_path = tmp.path().join("agent-turn-trace.json");
    fs::write(
        &trace_path,
        r#"{
                "events": [
                    {"ToolRequested": {
                        "request_id": "req-001",
                        "parent_id": "parent-001",
                        "call_id": "call-001",
                        "tool": "read_file",
                        "arguments": "{\"file\":\"src/lib.rs\"}"
                    }},
                    {"ToolCompleted": {
                        "request_id": "req-001",
                        "parent_id": "parent-001",
                        "call_id": "call-001",
                        "tool": "read_file",
                        "content": "ok",
                        "ui_payload": null,
                        "latency_ms": 17
                    }},
                    {"ToolFailed": {
                        "request_id": "req-002",
                        "parent_id": "parent-001",
                        "call_id": "call-002",
                        "tool": "apply_code_edit",
                        "error": "failed",
                        "ui_payload": null,
                        "latency_ms": 31
                    }}
                ]
            }"#,
    )
    .expect("write trace");

    let trace = parse_turn_trace(trace_path).expect("parse trace");

    assert_eq!(trace.tool_completed, 1);
    assert_eq!(trace.tool_failed, 1);
    assert_eq!(trace.tool_latency_ms_total, 48);
    assert_eq!(trace.tool_latency_ms_max, 31);
    assert_eq!(trace.event_counts.get("ToolRequested"), Some(&1));
}

#[test]
fn observation_log_loads_provider_attempt_event() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log_path = tmp
        .path()
        .join("prototype1_observation_provider_attempt.jsonl");
    let provider_attempt = ProviderAttempt {
        request_id: 9,
        attempt: 2,
        max_attempts: 2,
        started_at: Duration::from_millis(0),
        request_sent: Some(Duration::from_millis(5)),
        headers_received: Some(Duration::from_millis(20)),
        output_started: None,
        output_progress: None,
        output_completed: None,
        failed: Some(Duration::from_millis(200_000)),
        status: Some(200),
        response_bytes: None,
        outcome: ProviderAttemptOutcome::Failed,
        failure_phase: Some(ploke_llm::ProviderFailurePhase::Body),
        body_failure: Some(HttpBodyFailure::Timeout),
        retry_decision: ploke_llm::ProviderRetryDecision::Exhausted,
        backoff: Some(Duration::from_millis(500)),
    };
    let line = serde_json::json!({
        "timestamp": "2026-05-02T00:00:04Z",
        "target": "chat_http",
        "event": "provider_attempt",
        "request_id": 9,
        "attempt": 2,
        "max_attempts": 2,
        "span": {
            "name": "prototype1.chat_request",
            "campaign_id": "",
            "node_id": "",
            "branch_id": "",
            "generation": "",
            "runtime_id": ""
        },
        "spans": [
            {
                "name": "prototype1.runtime",
                "campaign_id": "campaign-a",
                "node_id": "node-a",
                "branch_id": "branch-a",
                "generation": 2,
                "runtime_id": "runtime-a",
                "role": "child",
                "runtime_phase": "child_evaluation"
            },
            {
                "name": "prototype1.chat_request",
                "campaign_id": "",
                "node_id": "",
                "branch_id": "",
                "generation": "",
                "runtime_id": ""
            }
        ],
        "provider_attempt": serde_json::to_string(&provider_attempt).expect("provider attempt json"),
    });
    let scoped_line = serde_json::json!({
        "timestamp": "2026-05-02T00:00:03Z",
        "level": "INFO",
        "message": "prototype1 result step finished",
        "duration_ms": 0,
        "spans": [{
            "name": "prototype1.child.evaluate.eval_closure",
            "campaign_id": "campaign-a",
            "branch_id": "branch-a",
            "generation": 2
        }]
    });
    fs::write(&log_path, format!("{scoped_line}\n{line}\n")).expect("write log");

    let mut evidence = ObservationEvidence::default();
    parse_observation_log("campaign-a", &log_path, &mut evidence);

    assert_eq!(evidence.provider_http.len(), 1);
    assert!(evidence.provider_http[0].provider_attempt.is_some());
    assert_eq!(
        evidence.provider_http[0].campaign_id.as_deref(),
        Some("campaign-a")
    );
    assert_eq!(evidence.provider_http[0].node_id.as_deref(), Some("node-a"));
    assert_eq!(
        evidence.provider_http[0].branch_id.as_deref(),
        Some("branch-a")
    );
    assert_eq!(evidence.provider_http[0].generation, Some(2));
    assert_eq!(
        evidence.provider_http[0].runtime_id.as_deref(),
        Some("runtime-a")
    );
    assert_eq!(evidence.provider_http[0].role.as_deref(), Some("child"));
    assert_eq!(
        evidence.provider_http[0].runtime_phase.as_deref(),
        Some("child_evaluation")
    );
    let requests = provider_http_requests(&evidence.provider_http);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].timeout_count(), 1);
    assert_eq!(requests[0].elapsed_ms(), 200_500);
}

#[test]
fn observation_log_reports_malformed_legacy_provider_attempt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log_path = tmp
        .path()
        .join("prototype1_observation_provider_attempt_malformed.jsonl");
    let line = serde_json::json!({
        "timestamp": "2026-05-02T00:00:04Z",
        "target": "chat_http",
        "event": "provider_attempt",
        "request_id": 9,
        "attempt": 2,
        "max_attempts": 2,
        "spans": [{
            "name": "prototype1.runtime",
            "campaign_id": "campaign-a",
            "node_id": "node-a",
            "branch_id": "branch-a",
            "generation": 2,
            "runtime_id": "runtime-a",
            "role": "child",
            "runtime_phase": "child_evaluation"
        }],
        "provider_attempt": "{\"request_id\":9",
    });
    fs::write(&log_path, format!("{line}\n")).expect("write log");

    let mut evidence = ObservationEvidence::default();
    parse_observation_log("campaign-a", &log_path, &mut evidence);

    assert_eq!(evidence.provider_http.len(), 1);
    let event = &evidence.provider_http[0];
    assert_eq!(event.event, "provider_attempt_parse_failure");
    assert!(event.provider_attempt.is_none());
    assert_eq!(
        event
            .failure
            .as_deref()
            .map(|value| value.starts_with("malformed_legacy_provider_attempt:")),
        Some(true)
    );
    match &event.observation {
        ProviderHttpObservation::MalformedProviderAttempt(failure) => {
            assert_eq!(failure.raw, "{\"request_id\":9");
            assert!(failure.error.contains("EOF"));
        }
        other => panic!("expected malformed legacy provider attempt, got {other:?}"),
    }

    let requests = provider_http_requests(&evidence.provider_http);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].error_count(), 1);
}

#[test]
fn observation_log_loads_provider_attempt_timeline_fields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log_path = tmp
        .path()
        .join("prototype1_observation_provider_attempt_timeline.jsonl");
    fs::write(
            &log_path,
            concat!(
                r#"{"timestamp":"2026-05-02T00:00:04Z","target":"chat_http","event":"provider_attempt","request_id":11,"attempt":1,"max_attempts":2,"started_at_ms":0,"request_sent_ms":5,"headers_received_ms":20,"failed_ms":300000,"status":200,"outcome":"failed","failure_phase":"body","body_failure":"timeout","retry_decision":"scheduled","backoff_ms":250,"spans":[{"name":"prototype1.runtime","campaign_id":"campaign-a","node_id":"node-a","branch_id":"branch-a","generation":2,"runtime_id":"runtime-a","role":"child","runtime_phase":"child_evaluation"}]}"#,
                "\n",
            ),
        )
        .expect("write log");

    let mut evidence = ObservationEvidence::default();
    parse_observation_log("campaign-a", &log_path, &mut evidence);

    assert_eq!(evidence.provider_http.len(), 1);
    let event = &evidence.provider_http[0];
    let attempt = event.provider_attempt.as_ref().expect("provider attempt");
    assert_eq!(attempt.request_id, 11);
    assert_eq!(attempt.body_failure, Some(HttpBodyFailure::Timeout));
    assert_eq!(
        attempt.retry_decision,
        ploke_llm::ProviderRetryDecision::Scheduled
    );

    let requests = provider_http_requests(&evidence.provider_http);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].timeout_count(), 1);
    assert_eq!(requests[0].elapsed_ms(), 300_250);
}

#[test]
fn provider_attempt_watch_row_renders_table_cells() {
    let provider_attempt = ProviderAttempt {
        request_id: 9,
        attempt: 2,
        max_attempts: 2,
        started_at: Duration::from_millis(0),
        request_sent: Some(Duration::from_millis(5)),
        headers_received: Some(Duration::from_millis(20)),
        output_started: None,
        output_progress: None,
        output_completed: None,
        failed: Some(Duration::from_millis(200_000)),
        status: Some(200),
        response_bytes: None,
        outcome: ProviderAttemptOutcome::Failed,
        failure_phase: Some(ploke_llm::ProviderFailurePhase::Body),
        body_failure: Some(HttpBodyFailure::Timeout),
        retry_decision: ploke_llm::ProviderRetryDecision::Exhausted,
        backoff: Some(Duration::from_millis(500)),
    };
    let event = ProviderHttpEvent {
        source: PathBuf::from("/tmp/attempts.jsonl"),
        timestamp: Some("2026-05-02T00:00:04Z".to_string()),
        campaign_id: Some("campaign-a".to_string()),
        node_id: Some("node-abcdef1234567890".to_string()),
        branch_id: Some("branch-abcdef1234567890".to_string()),
        generation: Some(2),
        runtime_id: Some("runtime-a".to_string()),
        role: Some("child".to_string()),
        runtime_phase: Some("child_evaluation".to_string()),
        request_id: 9,
        attempt: 2,
        max_attempts: Some(2),
        event: "provider_attempt".to_string(),
        phase: None,
        status: Some(200),
        elapsed_ms: Some(200_000),
        backoff_ms: Some(500),
        model: None,
        request_bytes: None,
        response_bytes: None,
        is_timeout: Some(true),
        failure: None,
        provider_attempt: Some(provider_attempt.clone()),
        observation: ProviderHttpObservation::AttemptTimeline(provider_attempt),
    };

    let header = provider_attempt_watch_header(false);
    let row = provider_attempt_watch_row(&event, false).expect("row");

    assert!(header.contains("request"));
    assert!(row.contains("node-abcdef"));
    assert!(row.contains("branch-abcde"));
    assert!(row.contains("2/2"));
    assert!(row.contains("200.000s"));
    assert!(row.contains("failed"));
    assert!(row.contains("exhausted"));
    assert!(row.to_lowercase().contains("timeout"));
}

#[test]
fn provider_attempt_phone_row_keeps_fixed_columns() {
    let mut provider_attempt = ProviderAttempt {
        request_id: 15,
        attempt: 1,
        max_attempts: 1,
        started_at: Duration::from_millis(0),
        request_sent: Some(Duration::from_millis(5)),
        headers_received: None,
        output_started: None,
        output_progress: None,
        output_completed: None,
        failed: Some(Duration::from_millis(300_000)),
        status: None,
        response_bytes: None,
        outcome: ProviderAttemptOutcome::Failed,
        failure_phase: Some(ploke_llm::ProviderFailurePhase::Body),
        body_failure: Some(HttpBodyFailure::Timeout),
        retry_decision: ploke_llm::ProviderRetryDecision::Exhausted,
        backoff: None,
    };
    let mut event = ProviderHttpEvent {
        source: PathBuf::from("/tmp/attempts.jsonl"),
        timestamp: Some("2026-05-02T00:00:04Z".to_string()),
        campaign_id: Some("campaign-a".to_string()),
        node_id: Some("node-abcdef1234567890".to_string()),
        branch_id: None,
        generation: Some(2),
        runtime_id: Some("runtime-a".to_string()),
        role: Some("child".to_string()),
        runtime_phase: Some("child_evaluation".to_string()),
        request_id: 15,
        attempt: 1,
        max_attempts: Some(1),
        event: "provider_attempt".to_string(),
        phase: None,
        status: None,
        elapsed_ms: Some(300_000),
        backoff_ms: None,
        model: None,
        request_bytes: None,
        response_bytes: None,
        is_timeout: Some(true),
        failure: None,
        provider_attempt: Some(provider_attempt.clone()),
        observation: ProviderHttpObservation::AttemptTimeline(provider_attempt.clone()),
    };

    let timeout_row = provider_attempt_phone_row(&event).expect("timeout row");
    provider_attempt.request_id = 2;
    provider_attempt.failed = None;
    provider_attempt.output_completed = Some(Duration::from_millis(1_000));
    provider_attempt.status = Some(200);
    provider_attempt.outcome = ProviderAttemptOutcome::Completed;
    provider_attempt.failure_phase = None;
    provider_attempt.body_failure = None;
    provider_attempt.retry_decision = ploke_llm::ProviderRetryDecision::None;
    event.request_id = 2;
    event.provider_attempt = Some(provider_attempt.clone());
    event.observation = ProviderHttpObservation::AttemptTimeline(provider_attempt);
    let completed_row = provider_attempt_phone_row(&event).expect("completed row");

    assert_eq!(timeout_row, "00:00 g  2 nabcd r 15/1 300s TO");
    assert_eq!(completed_row, "00:00 g  2 nabcd r  2/1   1s OK");
    assert!(timeout_row.len() <= 34);
    assert_eq!(timeout_row.len(), completed_row.len());
}
