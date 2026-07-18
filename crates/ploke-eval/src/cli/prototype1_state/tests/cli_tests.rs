use super::*;

use crate::cli::prototype1_state::cli_facing::{CandidateGenerationConfig, ParentSelectionOutcome};
use crate::cli::prototype1_state::edit_surface::harness_request::{
    PublishedBroadHarnessRequest, RequestAdmissionBinding,
};
use crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId;
use crate::cli::prototype1_state::eval_store;
use crate::cli::prototype1_state::typestate::{self, StepInput};
use crate::cli::prototype1_state::walk::{
    controller::WalkController,
    phase::WalkPhase,
    protocol::{SessionVersion, WalkStartConfig},
};
use crate::cli::{
    InspectOutputFormat, Prototype1CandidateGenerator,
    Prototype1ChildScheduleMode as CliPrototype1ChildScheduleMode, Prototype1LoopCommand,
    Prototype1LoopStopAfter, Prototype1StateCommand, Prototype1StateStopAfter,
    Prototype1SuccessorSelection, Prototype1TraversalMetrics,
};
use crate::intervention::Prototype1NodeRecord;
use ploke_llm::request::models::ModelRouteSource;
use ploke_records::identity::ParentIdentityRecord;
use std::str::FromStr;
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

use crate::cli::prototype1_state::c1::MaterializeBranchError;
use crate::cli::prototype1_state::c3::SpawnChildError;
use crate::intervention::{
    CommitError, CommitPhase, PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1RunnerResult,
    Prototype1SearchPolicy, RecordStore, TreatmentBranchNode, TreatmentBranchStatus,
};
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};
use ploke_records::ids::CampaignId;
use std::sync::LazyLock;

static CLI_TEST_CAMPAIGN: LazyLock<CampaignId> = LazyLock::new(|| CampaignId::from("campaign"));

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

fn hex_fixture_bytes(path: &Path) -> Vec<u8> {
    let encoded = fs::read_to_string(path).expect("read hex fixture");
    let compact = encoded
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect::<String>();
    assert_eq!(compact.len() % 2, 0, "hex fixture is complete");
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex pair is UTF-8");
            u8::from_str_radix(pair, 16).expect("decode hex fixture")
        })
        .collect()
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
        stop_after: Some(Prototype1StateStopAfter::Complete),
        successor_selection: Some(Prototype1SuccessorSelection::HistoryScoreChildProp),
        successor_selection_seed: Some(0),
        successor_selection_metrics: Some(Prototype1TraversalMetrics::Operational),
        candidate_generator: Some(Prototype1CandidateGenerator::BroadHarnessRequest),
        format: InspectOutputFormat::Table,
    }
}

#[test]
fn r0_to_r1_requires_campaign_or_parent_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut command = state_command_without_ids();
    command.repo_root = Some(tmp.path().to_path_buf());

    let error = typestate::R0::new(command)
        .advance(r0_to_r1)
        .expect_err("missing campaign and parent identity should fail during R0 -> R1");

    match error {
        PrepareError::InvalidBatchSelection { detail } => {
            assert!(detail.contains("--campaign was omitted"));
            assert!(detail.contains("no parent identity exists"));
            assert!(detail.contains("parent_identity.json"));
        }
        other => panic!("unexpected R0 -> R1 error: {other:?}"),
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

fn load_test_node_record(manifest_path: &Path, node_id: &str) -> Prototype1NodeRecord {
    let path = crate::intervention::prototype1_node_record_path(manifest_path, node_id);
    let bytes = fs::read(&path)
        .unwrap_or_else(|source| panic!("read node record '{}': {source}", path.display()));
    serde_json::from_slice(&bytes).expect("decode node record")
}

fn parent_identity_for(node_id: &str, generation: u32) -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
            .to_string(),
        campaign_id: CampaignId::from("campaign"),
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
            campaign_id: identity.campaign_id().clone(),
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
            oracle_targets: Vec::new(),
            selected_source: Some(TraversalCandidateSource::History),
            child_counts: std::collections::BTreeMap::new(),
        }),
        metrics: selection_metrics_for(&[], &[]),
        selected_from_generation_outcomes: false,
    }
}

fn selection_material_from_current_generation() -> SelectionSealMaterial {
    SelectionSealMaterial {
        selected_from_generation_outcomes: true,
        ..selection_material_from_history()
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
fn parent_selection_outcome_hydrates_current_generation_entry_exactly() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let node = test_node(
        tmp.path(),
        "node-current",
        "branch-current",
        "candidate-current",
    );
    let selected = SubjectRef::new("candidate:node-current:plan_index=0");
    let payload = EvaluationPayload::builder(
        selected.clone(),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .selection_input(selection_input_from_child_report(
        &node,
        &test_evaluation_report(&node),
    ))
    .expect("selection input binds")
    .build();
    let sources = vec![TraversalCandidateSource::CurrentGeneration];
    let mut material = selection_material_from_current_generation();
    material.selected_candidate = selected;
    material.considered = vec![payload.clone()];
    material.considered_sources = sources.clone();
    material.metrics = selection_metrics_for(std::slice::from_ref(&payload), &sources);
    material
        .traversal
        .as_mut()
        .expect("traversal evidence")
        .selected_source = Some(TraversalCandidateSource::CurrentGeneration);
    let original = ParentSelectionOutcome::Selected {
        decision: successor_decision_for(&node),
        material,
    }
    .entry()
    .expect("selected entry");

    let hydrated =
        ParentSelectionOutcome::from_entry(original.clone()).expect("selected entry hydrates");
    assert_eq!(hydrated.entry().expect("hydrated entry rebuilds"), original);
    let (_, material) = hydrated.selected().expect("hydrated selected outcome");
    assert!(material.selected_from_generation_outcomes);
    assert_eq!(
        material
            .traversal
            .as_ref()
            .and_then(|traversal| traversal.selected_source),
        Some(TraversalCandidateSource::CurrentGeneration)
    );
    match ParentSelectionOutcome::from_admitted_entry(
        original.clone(),
        crate::successor_selection::PatchGate::ReviewedAdmissible,
        None,
    ) {
        Ok(_) => panic!("receipt admitted under a different patch gate must fail closed"),
        Err(PrepareError::InvalidBatchSelection { detail }) => {
            assert!(
                detail.contains(
                    "persisted selection patch gate does not match admitted profile: recorded=disabled, admitted=reviewed-admissible"
                ),
                "unexpected patch gate mismatch error: {detail}"
            );
        }
        Err(other) => panic!("expected invalid selection error, got {other:?}"),
    }

    let mut missing = original;
    missing
        .traversal
        .as_mut()
        .expect("traversal evidence")
        .selected_source = None;
    match ParentSelectionOutcome::from_entry(missing) {
        Ok(_) => panic!("selected receipt without selected_source must fail closed"),
        Err(PrepareError::InvalidBatchSelection { detail }) => {
            assert!(
                detail.contains("persisted selected receipt has no traversal candidate source"),
                "unexpected hydration error: {detail}"
            );
        }
        Err(other) => panic!("expected invalid selection error, got {other:?}"),
    }
}

fn persisted_continuation_decision(
    manifest_path: &Path,
    parent: &ParentIdentity,
    policy: &Prototype1SearchPolicy,
    decision: &SuccessorDecision,
    material: &SelectionSealMaterial,
    node: &Prototype1NodeRecord,
) -> Result<Prototype1ContinuationDecision, PrepareError> {
    let continuation =
        preview_successor_continuation(manifest_path, parent, policy, decision, material, node)?;
    record_continuation_decision(manifest_path, parent, &continuation)?;
    Ok(continuation)
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
    let decision = persisted_continuation_decision(
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

    let decision = persisted_continuation_decision(
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
fn historical_selection_rejects_already_active_parent_cycle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = test_manifest_path(tmp.path());
    let parent = parent_identity_for("node-current", 1);
    append_parent_started(&manifest_path, parent.clone());
    let mut node = test_node(
        tmp.path(),
        parent.node_id(),
        parent.branch_id(),
        "candidate-current",
    );
    node.generation = parent.generation();
    node.parent_node_id = Some("node-root".to_string());
    write_test_node(&manifest_path, &node);

    let policy = Prototype1SearchPolicy {
        max_generations: 15,
        max_total_nodes: 96,
        ..Prototype1SearchPolicy::default()
    };

    let decision = persisted_continuation_decision(
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
        Prototype1ContinuationDisposition::StopHistoricalTraversalCycle
    );
    assert!(!decision.disposition.allows_successor());
}

#[test]
fn continuation_decision_mirrors_owned_eval_store_row_without_successor_authority() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = test_manifest_path(tmp.path());
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
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

    let decision = persisted_continuation_decision(
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
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let rows = db
        .raw_query_params(
            r#"
?[
    campaign_id,
    parent_id,
    disposition,
    selected_branch_id,
    next_generation,
    total_nodes,
    policy_ref
] :=
    *eval_continuation_decision {
        campaign_id,
        parent_id,
        disposition,
        selected_branch_id,
        next_generation,
        total_nodes,
        policy_ref
    }
"#,
            std::collections::BTreeMap::new(),
        )
        .expect("query continuation rows");
    assert_eq!(rows.rows.len(), 1);
    let row = rows.row_refs().next().expect("continuation row");
    assert_eq!(
        row.get::<String>("campaign_id").expect("campaign"),
        parent.campaign_id().to_string()
    );
    assert_eq!(
        row.get::<String>("parent_id").expect("parent"),
        parent.parent_id()
    );
    assert_eq!(
        row.get::<String>("disposition").expect("disposition"),
        "continue_historical_traversal"
    );
    assert_eq!(
        row.get::<String>("selected_branch_id")
            .expect("selected branch"),
        "branch-history"
    );
    assert_eq!(
        row.get::<i64>("next_generation").expect("generation"),
        i64::from(node.generation)
    );
    assert_eq!(row.get::<i64>("total_nodes").expect("total nodes"), 1);
    assert_eq!(
        row.get::<String>("policy_ref").expect("policy"),
        "prototype1.search_policy"
    );

    let journal_entries = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path))
        .load_entries()
        .expect("load journal entries");
    assert!(
        journal_entries
            .iter()
            .all(|entry| !matches!(entry, JournalEntry::Successor(_))),
        "passive eval-store continuation rows must not replace successor transition authority"
    );
}

#[test]
fn history_candidate_filter_excludes_active_parent_before_sampling() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let parent = parent_identity_for("node-current", 2);
    let mut active = test_node(
        tmp.path(),
        parent.node_id(),
        parent.branch_id(),
        "candidate-current",
    );
    active.generation = parent.generation();
    let other = test_node(tmp.path(), "node-other", "branch-other", "candidate-other");
    let candidates = HistoryCandidates {
        scope: SelectionScope::all_admitted_candidates(),
        candidates: vec![
            test_history_candidate(&active, "active"),
            test_history_candidate(&other, "other"),
        ],
    };

    let filtered = without_active_parent_candidate(candidates, &parent);

    assert_eq!(filtered.candidates.len(), 1);
    assert_eq!(
        filtered.candidates[0]
            .payload
            .selection_input
            .as_ref()
            .expect("selection input")
            .candidate
            .node_id,
        "node-other"
    );
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

    let decision = persisted_continuation_decision(
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
fn generation_cap_stops_direct_child_handoff_at_max_generation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = test_manifest_path(tmp.path());
    let parent = parent_identity_for("node-current", 2);
    append_parent_started(&manifest_path, parent.clone());
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.generation = 3;
    node.parent_node_id = Some(parent.node_id().to_string());
    write_test_node(&manifest_path, &node);
    let policy = Prototype1SearchPolicy {
        max_generations: 3,
        max_total_nodes: 96,
        ..Prototype1SearchPolicy::default()
    };

    let decision = persisted_continuation_decision(
        &manifest_path,
        &parent,
        &policy,
        &successor_decision_for(&node),
        &selection_material_from_current_generation(),
        &node,
    )
    .expect("continuation decision");

    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::StopMaxGenerations
    );
    assert!(!decision.disposition.allows_successor());
}

fn test_history_candidate(
    node: &Prototype1NodeRecord,
    label: &str,
) -> crate::cli::prototype1_state::history::HistoryCandidate {
    let payload = EvaluationPayload::builder(
        SubjectRef::new(format!("candidate:{}:plan_index=0", node.node_id)),
        ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
    )
    .selection_input(crate::successor_selection::SelectionInput::new(
        crate::successor_selection::CandidateRef {
            node_id: node.node_id.clone(),
            branch_id: node.branch_id.clone(),
            generation: node.generation,
        },
        crate::BranchDisposition::Reject,
        PathBuf::from(format!("evaluations/{}.json", node.branch_id)),
        Vec::new(),
    ))
    .expect("selection input")
    .build();
    let payload_hash = payload.payload_hash().expect("payload hash");
    crate::cli::prototype1_state::history::HistoryCandidate {
        source: crate::cli::prototype1_state::history::HistoryCandidateSource {
            block_hash: crate::cli::prototype1_state::history::BlockHash::from(
                HistoryHash::of_bytes(label.as_bytes()),
            ),
            block_height: 1,
            lineage_id: crate::cli::prototype1_state::history::LineageId::new("lineage:test"),
            entry_id: crate::cli::prototype1_state::history::EntryId::new(),
        },
        decision_scope: SelectionScope::all_admitted_candidates(),
        selected_by_decision: false,
        payload,
        payload_hash,
        candidate_set_root: None,
        candidate_set_membership: None,
    }
}

#[test]
fn candidate_generation_config_dispatches_broad_harness_surface() {
    let mut command = state_command_without_ids();
    command.candidate_generator = Some(Prototype1CandidateGenerator::BroadHarnessRequest);
    let config = CandidateGenerationConfig::from_command(&command);

    assert_eq!(config, CandidateGenerationConfig::BroadHarnessRequest);
}

#[test]
fn candidate_generation_config_dispatches_deterministic_tui_tools_fixture() {
    let mut command = state_command_without_ids();
    command.candidate_generator = Some(Prototype1CandidateGenerator::DeterministicTuiTools);
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

[storage.eval]
backend = "dual-strict"

[target]
instance = "BurntSushi__ripgrep-2209"

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
        shape.eval_storage_backend,
        profile::EvalStorageBackend::DualStrict
    );
    assert_eq!(
        shape.successor_oracle_mode,
        crate::successor_selection::OracleMode::RecordOnly
    );
    assert!(shape.successor_oracle_require_evidence);
    assert_eq!(
        shape.successor_oracle_gate,
        crate::successor_selection::OracleGate::Disabled
    );
    assert_eq!(
        shape.successor_oracle_targets,
        vec!["BurntSushi__ripgrep-2209".to_string()]
    );
}

#[test]
fn state_run_shape_rejects_profile_without_commitment() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let profile_path = tmp.path().join("prototype1/run-profile.toml");
    fs::create_dir_all(profile_path.parent().expect("profile parent"))
        .expect("create profile parent");
    fs::write(
        &profile_path,
        r#"schema_version = "prototype1-run-profile.v1"
name = "partial-admission"
"#,
    )
    .expect("write partial profile");

    let error = Prototype1StateRunShape::resolve(&state_command_without_ids(), &manifest_path)
        .expect_err("profile-only campaign must not fall back to command defaults");

    assert!(error.to_string().contains("has no admission commitment"));
}

#[test]
fn state_run_shape_defaults_eval_storage_backend_to_fs() {
    let command = state_command_without_ids();
    let shape = Prototype1StateRunShape::from_command(&command);

    assert_eq!(shape.eval_storage_backend, profile::EvalStorageBackend::Fs);
}

fn write_direct_google_registry(eval_home: &Path) {
    let models_dir = eval_home.join("models");
    fs::create_dir_all(&models_dir).expect("models dir");
    fs::write(
        models_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "data": [{
                "id": "google/gemini-3.5-flash",
                "name": "gemini-3.5-flash",
                "created": 0,
                "description": "Direct Google test row",
                "architecture": {
                    "input_modalities": ["text"],
                    "modality": "text->text",
                    "output_modalities": ["text"],
                    "tokenizer": "Gemini"
                },
                "top_provider": {
                    "is_moderated": false,
                    "context_length": null,
                    "max_completion_tokens": null
                },
                "pricing": {
                    "prompt": 0.0,
                    "completion": 0.0
                },
                "canonical_slug": "google/gemini-3.5-flash",
                "context_length": 1048576,
                "hugging_face_id": null,
                "per_request_limits": null,
                "supported_parameters": ["tools"],
                "route_source": "direct_google"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
}

fn setup_preview_command(batch: PathBuf, profile: PathBuf) -> Prototype1LoopCommand {
    Prototype1LoopCommand {
        batch: Some(batch),
        batch_id: None,
        dataset: None,
        dataset_key: None,
        all: false,
        instance: Vec::new(),
        specific: Vec::new(),
        limit: None,
        prepare_batch_id: None,
        campaign: Some(CampaignId::from("setup-preview-campaign")),
        profile: Some(profile.display().to_string()),
        repo_cache: None,
        instances_root: None,
        batches_root: None,
        max_turns: 40,
        max_tool_calls: 200,
        wall_clock_secs: 1800,
        eval_max_tokens: crate::campaign::DEFAULT_EVAL_MAX_TOKENS,
        index_debug_snapshots: true,
        use_default_model: false,
        model_id: Some("google/gemini-3.5-flash".to_string()),
        provider: Some("google".to_string()),
        route_source: Some(ModelRouteSource::DirectGoogle),
        embedding_model_id: Some("perplexity/pplx-embed-v1-4b".to_string()),
        embedding_route: None,
        embedding_provider: Some("perplexity".to_string()),
        stop_on_error: false,
        protocol_model_id: Some("google/gemini-3.5-flash".to_string()),
        protocol_provider: Some("google".to_string()),
        protocol_route_source: Some(ModelRouteSource::DirectGoogle),
        source_campaign: None,
        source_branch_id: None,
        max_generations: 1,
        max_total_nodes: 32,
        min_children: 2,
        max_children: 6,
        child_schedule_mode: CliPrototype1ChildScheduleMode::FullBatch,
        stop_on_first_keep: false,
        require_keep_for_continuation: true,
        explore_from_rejected: true,
        stop_after: Prototype1LoopStopAfter::InterventionApply,
        dry_run: false,
        format: InspectOutputFormat::Json,
    }
}

fn snapshot_setup_tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(path).expect("read snapshot directory") {
            let entry = entry.expect("snapshot directory entry");
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path under root")
                .to_path_buf();
            if path.is_dir() {
                entries.insert(relative, None);
                visit(root, &path, entries);
            } else if path.is_file() {
                entries.insert(relative, Some(fs::read(path).expect("snapshot file")));
            }
        }
    }

    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

#[test]
fn setup_authority_tracks_partial_model_sources() {
    let mut command =
        setup_preview_command(PathBuf::from("batch.json"), PathBuf::from("profile.toml"));
    command.model_id = None;
    command.use_default_model = false;
    command.route_source = None;
    command.provider = None;
    command.protocol_model_id = None;
    command.protocol_route_source = None;
    command.protocol_provider = None;
    command.embedding_provider = None;
    let mut run_profile = toml::from_str::<profile::Prototype1RunProfile>(
        r#"schema_version = "prototype1-run-profile.v1"
name = "partial-model-authority"

[model]
route_source = "direct-google"
provider = "google"

[protocol.model]
route_source = "direct-google"
provider = "google"
"#,
    )
    .expect("profile parses");

    let authority = setup_authority(&command, &run_profile);
    assert_eq!(authority.eval_model, "active_model_registry");
    assert_eq!(authority.eval_route, "run_profile.model");
    assert_eq!(authority.eval_provider, "run_profile.model");
    assert_eq!(authority.protocol_model, "resolved_eval_model");
    assert_eq!(authority.protocol_route, "run_profile.protocol.model");
    assert_eq!(authority.protocol_provider, "run_profile.protocol.model");
    assert_eq!(authority.embedding_route, "campaign.eval.default");
    assert_eq!(authority.embedding_model, "command.embedding_model_id");
    assert_eq!(authority.embedding_provider, "runtime_provider_resolution");

    run_profile.model = profile::ModelDefaults::default();
    let authority = setup_authority(&command, &run_profile);
    assert_eq!(authority.eval_model, "active_model_registry");
    assert_eq!(authority.protocol_model, "protocol_model_selection");
}

#[test]
fn prototype1_setup_preview_is_non_writing_and_admits_exact_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", tmp.path().as_os_str().into())]);
    write_direct_google_registry(tmp.path());
    let dataset_file = tmp.path().join("dataset.jsonl");
    let dataset_text = r#"{"instance_id":"BurntSushi__ripgrep-2209"}"#;
    fs::write(&dataset_file, dataset_text).expect("write dataset");
    let batch = crate::spec::PreparedMsbBatch {
        batch_id: "setup-preview-batch".to_string(),
        dataset_file: dataset_file.clone(),
        dataset_url: None,
        repo_cache: tmp.path().join("repo-cache"),
        instances_root: tmp.path().join("instances"),
        output_dir: tmp.path().join("batches/setup-preview-batch"),
        budget: crate::spec::EvalBudget::default(),
        instances: vec!["BurntSushi__ripgrep-2209".to_string()],
        campaign: None,
    };
    let batch_manifest = tmp.path().join("prepared-batch.json");
    fs::write(
        &batch_manifest,
        serde_json::to_vec_pretty(&batch).expect("serialize batch"),
    )
    .expect("write batch");
    let profile_path = tmp.path().join("run-profile.toml");
    let profile_text = r#"schema_version = "prototype1-run-profile.v1"
name = "setup-preview"
"#;
    fs::write(&profile_path, profile_text).expect("write profile");
    let command = setup_preview_command(batch_manifest.clone(), profile_path.clone());
    let before = snapshot_setup_tree(tmp.path());

    let plan = preview_prototype1_parent_setup(&command).expect("preview setup");

    assert_eq!(snapshot_setup_tree(tmp.path()), before);
    assert_eq!(
        plan.schema_version,
        super::PROTOTYPE1_SETUP_PLAN_SCHEMA_VERSION
    );
    assert_eq!(plan.plan_sha256.len(), 64);
    assert!(!plan.content.campaign.manifest_path().exists());
    assert!(!plan.content.profile.profile_path().exists());
    assert_eq!(
        plan.content
            .campaign
            .resolved
            .eval
            .embedding_model_id
            .as_deref(),
        Some("perplexity/pplx-embed-v1-4b")
    );
    assert_eq!(
        plan.content
            .campaign
            .resolved
            .eval
            .embedding_provider_slug
            .as_deref(),
        Some("perplexity")
    );
    assert_eq!(
        plan.content.campaign.resolved.eval.embedding_route,
        EmbeddingRoute::OpenRouter
    );
    assert_eq!(plan.content.embedding_route, EmbeddingRoute::OpenRouter);
    assert_eq!(
        plan.content.authority.embedding_route,
        "campaign.eval.default"
    );
    let plan_json = serde_json::to_value(&plan).expect("serialize setup preview");
    assert_eq!(
        plan_json["content"]["embedding_route"],
        serde_json::json!("openrouter")
    );

    let mut invalid_route = setup_preview_command(batch_manifest.clone(), profile_path.clone());
    invalid_route.embedding_route = Some(EmbeddingRoute::DirectOpenAi);
    let error = preview_prototype1_parent_setup(&invalid_route)
        .expect_err("direct OpenAI setup must reject an OpenRouter provider preference");
    assert!(
        error
            .to_string()
            .contains("does not accept OpenRouter provider")
    );
    invalid_route.embedding_provider = None;
    let error = preview_prototype1_parent_setup(&invalid_route)
        .expect_err("direct OpenAI setup must reject a non-OpenAI model");
    assert!(error.to_string().contains("requires an OpenAI model"));

    let mut direct_route = setup_preview_command(batch_manifest.clone(), profile_path.clone());
    direct_route.embedding_route = Some(EmbeddingRoute::DirectOpenAi);
    direct_route.embedding_model_id = None;
    direct_route.embedding_provider = None;
    let direct_plan = preview_prototype1_parent_setup(&direct_route)
        .expect("direct OpenAI route should enter the reviewed setup plan");
    assert_eq!(
        direct_plan.content.campaign.resolved.eval.embedding_route,
        EmbeddingRoute::DirectOpenAi
    );
    assert_ne!(direct_plan.plan_sha256, plan.plan_sha256);
    let planned_manifest = serde_json::to_value(plan.content.campaign.manifest_plan.manifest())
        .expect("serialize planned manifest");
    let planned_json = plan
        .content
        .campaign
        .manifest_plan
        .normalized_json()
        .to_string();
    let planned_config =
        serde_json::to_value(&plan.content.campaign.resolved).expect("serialize planned config");
    let planned_toml = plan.content.profile.normalized_toml().to_string();
    let planned_sha = plan.content.profile.sha256().to_string();
    let planned_slice = plan.content.campaign.slice_dataset.clone();

    fs::write(
        &profile_path,
        profile_text.replace("setup-preview", "setup-drift"),
    )
    .expect("write profile drift");
    let error = resolve_expected_setup(&command, &plan.plan_sha256)
        .expect_err("profile drift must invalidate preview");
    assert!(error.to_string().contains("setup plan changed"));
    assert!(!plan.content.campaign.manifest_path().exists());
    fs::write(&profile_path, profile_text).expect("restore profile");

    fs::write(
        &dataset_file,
        r#"{"instance_id":"BurntSushi__ripgrep-2209","drift":true}"#,
    )
    .expect("write slice drift");
    let error = resolve_expected_setup(&command, &plan.plan_sha256)
        .expect_err("slice drift must invalidate preview");
    assert!(error.to_string().contains("setup plan changed"));
    assert!(!plan.content.campaign.manifest_path().exists());
    fs::write(&dataset_file, dataset_text).expect("restore dataset");

    let mut changed_batch = batch.clone();
    changed_batch.budget.max_turns += 1;
    fs::write(
        &batch_manifest,
        serde_json::to_vec_pretty(&changed_batch).expect("serialize batch drift"),
    )
    .expect("write batch drift");
    let error = resolve_expected_setup(&command, &plan.plan_sha256)
        .expect_err("batch drift must invalidate preview");
    assert!(error.to_string().contains("setup plan changed"));
    assert!(!plan.content.campaign.manifest_path().exists());
    fs::write(
        &batch_manifest,
        serde_json::to_vec_pretty(&batch).expect("serialize restored batch"),
    )
    .expect("restore batch");

    let admitted_plan = resolve_expected_setup(&command, &plan.plan_sha256)
        .expect("separate admission planning matches preview");
    assert_eq!(snapshot_setup_tree(tmp.path()), before);

    let campaign = admit_prototype1_loop_campaign(admitted_plan.content.campaign)
        .expect("admit planned campaign");
    let admitted = profile::admit_run_profile_plan(admitted_plan.content.profile)
        .expect("admit planned profile");

    let stored_manifest = crate::campaign::load_campaign_manifest(&campaign.campaign_id)
        .expect("load admitted manifest");
    assert_eq!(
        serde_json::to_value(stored_manifest).expect("serialize stored manifest"),
        planned_manifest
    );
    assert_eq!(
        fs::read_to_string(&campaign.manifest_path).expect("read admitted manifest"),
        planned_json
    );
    assert_eq!(
        serde_json::to_value(&campaign.resolved).expect("serialize admitted config"),
        planned_config
    );
    assert_eq!(
        fs::read_to_string(&campaign.slice_dataset_path).expect("read admitted slice"),
        planned_slice
    );
    assert_eq!(
        fs::read_to_string(&admitted.commitment.profile_path).expect("read admitted profile"),
        planned_toml
    );
    assert_eq!(admitted.commitment.sha256, planned_sha);
}

#[tokio::test]
async fn fresh_setup_reconstruction_claims_first_walk_session_at_r3() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eval_home = tmp.path().join("eval-home");
    let repo_root = tmp.path().join("repo");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", eval_home.as_os_str().into())]);
    write_direct_google_registry(&eval_home);

    init_indexed_repo(&repo_root);
    fs::write(repo_root.join("README.md"), "fresh walk setup\n").expect("write seed file");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "seed fresh walk setup");

    let dataset_file = eval_home.join("dataset.jsonl");
    fs::create_dir_all(&eval_home).expect("create eval home");
    fs::write(
        &dataset_file,
        r#"{"instance_id":"BurntSushi__ripgrep-2209","org":"BurntSushi","repo":"ripgrep","number":2209,"title":"Fix multiline replacement","body":"body one","base":{"sha":"abc123"},"fix_patch":"diff --git a/crates/printer/src/util.rs b/crates/printer/src/util.rs\n--- a/crates/printer/src/util.rs\n+++ b/crates/printer/src/util.rs\n@@ -1 +1 @@\n-old\n+new\n"}"#,
    )
    .expect("write dataset");
    let batch = crate::spec::PreparedMsbBatch {
        batch_id: "fresh-walk-batch".to_string(),
        dataset_file,
        dataset_url: None,
        repo_cache: eval_home.join("repo-cache"),
        instances_root: eval_home.join("instances"),
        output_dir: eval_home.join("batches/fresh-walk-batch"),
        budget: crate::spec::EvalBudget::default(),
        instances: vec!["BurntSushi__ripgrep-2209".to_string()],
        campaign: None,
    };
    let batch_path = eval_home.join("prepared-batch.json");
    fs::write(
        &batch_path,
        serde_json::to_vec_pretty(&batch).expect("serialize batch"),
    )
    .expect("write batch");
    let profile_path = eval_home.join("run-profile.toml");
    fs::write(
        &profile_path,
        r#"schema_version = "prototype1-run-profile.v1"
name = "fresh-walk-start"

[control]
mode = "step"

[storage.eval]
backend = "dual-strict"
"#,
    )
    .expect("write profile");
    let mut command = setup_preview_command(batch_path, profile_path);
    command.campaign = Some(CampaignId::from("fresh-walk-campaign"));
    let plan =
        preview_prototype1_parent_setup_at(&command, repo_root.clone()).expect("preview setup");
    let setup =
        prepare_prototype1_parent_setup_at(&command, Some(&plan.plan_sha256), repo_root.clone())
            .expect("prepare completed setup");
    let parent = load_parent_identity_optional(&repo_root)
        .expect("load admitted identity")
        .expect("stored admitted identity");
    assert_eq!(parent.node_id(), setup.node_id);
    let manifest = plan.content.campaign.manifest_path();
    assert_session_absent(manifest, &parent);

    let mut controller = WalkController::new(
        repo_root
            .canonicalize()
            .expect("canonical fresh setup root"),
    );
    controller
        .refresh_from_disk()
        .expect("reconstruct completed setup");
    assert_eq!(controller.phase(), WalkPhase::R4c);

    let advance = controller
        .start_version(
            WalkStartConfig {
                campaign: Some(parent.campaign_id().clone()),
                repo_root: None,
            },
            WalkPhase::R3,
            false,
            &SessionVersion::empty(),
        )
        .await
        .expect("fresh reconstructed setup should admit its first session claim");
    assert_eq!(advance.from(), WalkPhase::R3);
    assert_eq!(advance.to(), WalkPhase::R3);
    assert!(
        advance
            .transition_edges()
            .expect("transition edges")
            .is_empty()
    );
    let version = advance.exact_version().expect("exact session version");
    assert!(version.session_id().is_some());
    assert_eq!(version.phase(), WalkPhase::R3);
    assert!(version.journal_revision() > 0);

    let store = crate::cli::prototype1_state::session::Store::for_manifest(manifest);
    let paths = store.paths(&parent);
    let session = store
        .inspect(&parent)
        .expect("inspect claimed session")
        .expect("claimed session exists");
    assert!(session.active.is_none());
    assert_eq!(session.cursor.expect("session cursor").phase, WalkPhase::R3);
    assert!(paths.journal().exists());

    let cross_runtime = match controller
        .step_version(Some(WalkPhase::R14b), false, true, &version)
        .await
    {
        Ok(_) => panic!("one bounded operation must not cross successor authority"),
        Err(error) => error,
    };
    let detail = cross_runtime.to_string();
    assert!(
        detail.contains("crosses the R13b successor runtime boundary"),
        "{detail}"
    );
    let session = store
        .inspect(&parent)
        .expect("inspect session after rejected cross-runtime target")
        .expect("session remains after rejected cross-runtime target");
    assert!(session.active.is_none());
    assert_eq!(session.journal_revision, version.journal_revision());
    assert_eq!(
        session
            .cursor
            .expect("session cursor after rejected cross-runtime target")
            .phase,
        WalkPhase::R3
    );

    let advanced = controller
        .step_version(Some(WalkPhase::R4a), false, false, &version)
        .await
        .expect("bounded walk step should stop on its committed target");
    assert_eq!(advanced.from(), WalkPhase::R3);
    assert_eq!(advanced.to(), WalkPhase::R4a);
    assert_eq!(
        advanced
            .transition_edges()
            .expect("bounded transition edge"),
        [crate::cli::prototype1_state::edge::ControlEdge::R3ToR4a]
    );
    let advanced_version = advanced.exact_version().expect("advanced session version");
    assert_eq!(
        advanced_version.journal_revision(),
        version.journal_revision() + 4,
        "one committed edge must append exactly Acquired, Began, Finished, and Released; reaching the requested target must not acquire a second no-op lease"
    );

    let session = store
        .inspect(&parent)
        .expect("inspect advanced session")
        .expect("advanced session exists");
    assert!(session.active.is_none());
    assert_eq!(
        session.journal_revision,
        advanced_version.journal_revision()
    );
    assert_eq!(
        session.cursor.expect("advanced session cursor").phase,
        WalkPhase::R4a
    );
}

#[tokio::test]
async fn prototype1_setup_recovers_and_completed_retry_is_read_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eval_home = tmp.path().join("eval-home");
    let repo_root = tmp.path().join("repo");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", eval_home.as_os_str().into())]);
    write_direct_google_registry(&eval_home);

    init_indexed_repo(&repo_root);
    let old_identity = parent_identity_for("old-parent", 0);
    let identity_path = repo_root.join(parent_identity_relpath());
    fs::create_dir_all(identity_path.parent().expect("identity parent"))
        .expect("create identity parent");
    fs::write(
        &identity_path,
        serde_json::to_vec_pretty(&old_identity).expect("serialize old identity"),
    )
    .expect("write old identity");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "seed old identity");

    let dataset_file = eval_home.join("dataset.jsonl");
    fs::write(
        &dataset_file,
        r#"{"instance_id":"BurntSushi__ripgrep-2209","org":"BurntSushi","repo":"ripgrep","number":2209,"title":"Fix multiline replacement","body":"body one","base":{"sha":"abc123"},"fix_patch":"diff --git a/crates/printer/src/util.rs b/crates/printer/src/util.rs\n--- a/crates/printer/src/util.rs\n+++ b/crates/printer/src/util.rs\n@@ -1 +1 @@\n-old\n+new\n"}"#,
    )
    .expect("write dataset");
    let batch = crate::spec::PreparedMsbBatch {
        batch_id: "setup-recovery-batch".to_string(),
        dataset_file,
        dataset_url: None,
        repo_cache: eval_home.join("repo-cache"),
        instances_root: eval_home.join("instances"),
        output_dir: eval_home.join("batches/setup-recovery-batch"),
        budget: crate::spec::EvalBudget::default(),
        instances: vec!["BurntSushi__ripgrep-2209".to_string()],
        campaign: None,
    };
    let batch_manifest = eval_home.join("prepared-batch.json");
    fs::write(
        &batch_manifest,
        serde_json::to_vec_pretty(&batch).expect("serialize batch"),
    )
    .expect("write batch");
    let profile_path = eval_home.join("run-profile.toml");
    fs::write(
        &profile_path,
        "schema_version = \"prototype1-run-profile.v1\"\nname = \"setup-recovery\"\n\n[storage.eval]\nbackend = \"dual-strict\"\n",
    )
    .expect("write profile");
    let mut command = setup_preview_command(batch_manifest, profile_path);
    command.campaign = Some(CampaignId::from("setup-recovery-campaign"));
    let plan =
        preview_prototype1_parent_setup_at(&command, repo_root.clone()).expect("preview setup");
    let expected_sha = plan.plan_sha256.clone();

    let admission_path = setup_admission_path(plan.content.campaign.manifest_path());
    let receipt_root = admission_path.parent().expect("receipt root");
    fs::create_dir_all(receipt_root).expect("create receipt root");
    let meaningful = receipt_root.join("unexpected.json");
    fs::write(&meaningful, b"{}\n").expect("write meaningful unreceipted artifact");
    let lock_path = GitWorktreeBackend
        .setup_lock_path(&repo_root)
        .expect("setup lock path");
    {
        let _lock = acquire_setup_lock(&lock_path).expect("setup lock");
        let error = load_or_capture_setup_admission(&plan, &GitWorktreeBackend, &admission_path)
            .expect_err("meaningful unreceipted state must fail closed");
        assert!(error.to_string().contains("automatic adoption is unsafe"));
    }
    assert!(meaningful.exists(), "meaningful state must not be removed");
    fs::remove_file(&meaningful).expect("remove meaningful test artifact");

    let staging = receipt_root.join(".setup-admission.json.tmp-99999-1");
    fs::write(&staging, b"partial receipt").expect("write receipt staging remnant");
    let admission = {
        let _lock = acquire_setup_lock(&lock_path).expect("setup lock after simulated crash");
        load_or_capture_setup_admission(&plan, &GitWorktreeBackend, &admission_path)
            .expect("recover crash before receipt publication")
    };
    assert!(!staging.exists(), "stale receipt staging must be removed");
    let campaign = ensure_prototype1_loop_campaign(plan.content.campaign.clone())
        .expect("persist campaign partial");
    let admitted_at = recorded_at_rfc3339(admission.intent.started_at).expect("receipt timestamp");
    let admitted = profile::ensure_run_profile_plan(plan.content.profile.clone(), &admitted_at)
        .expect("persist profile partial");
    let closure = ensure_setup_closure_state(&campaign.resolved).expect("persist closure partial");
    ensure_setup_owner_db(&campaign, &admitted, &closure).expect("persist owner DB partial");
    let root = RootParentSetup {
        node: admission.intent.node.clone(),
        request: admission.intent.request.clone(),
    };
    ensure_root_parent_node(
        &campaign.campaign_id,
        &campaign.manifest_path,
        &root,
        plan.content.search_policy.clone(),
    )
    .expect("persist root partial");
    assert!(matches!(admission.state, SetupAdmissionState::Admitting));
    assert_eq!(
        GitWorktreeBackend
            .active_branch(&repo_root)
            .expect("branch before recovery"),
        plan.content.checkout.branch
    );

    GitWorktreeBackend
        .checkout_fresh_parent_branch(&repo_root, &plan.content.artifact_branch)
        .expect("simulate crash after branch switch");
    let recovered_plan = resolve_expected_setup_at(&command, &expected_sha, repo_root.clone())
        .expect("public retry must recover the receipt-bound plan");
    assert_eq!(recovered_plan.plan_sha256, expected_sha);
    assert_eq!(recovered_plan.content.checkout, plan.content.checkout);

    let report =
        prepare_prototype1_parent_setup_at(&command, Some(&expected_sha), repo_root.clone())
            .expect("recover partial setup through public setup service");
    assert!(matches!(
        report.admission_state,
        SetupAdmissionState::Complete { .. }
    ));
    let admitted_identity = load_parent_identity_optional(&repo_root)
        .expect("load admitted identity")
        .expect("stored admitted identity");
    assert_ne!(admitted_identity, old_identity);
    assert_eq!(admitted_identity.node_id(), report.node_id);

    let before = snapshot_setup_tree(&eval_home);
    let head = GitWorktreeBackend
        .head_commit(&repo_root)
        .expect("bootstrap head");
    let retried =
        prepare_prototype1_parent_setup_at(&command, Some(&expected_sha), repo_root.clone())
            .expect("retry completed setup");

    assert!(matches!(
        retried.admission_state,
        SetupAdmissionState::Complete { .. }
    ));
    assert_eq!(retried.node_id, report.node_id);
    assert_eq!(snapshot_setup_tree(&eval_home), before);
    assert_eq!(
        GitWorktreeBackend
            .head_commit(&repo_root)
            .expect("unchanged bootstrap head"),
        head
    );

    let status = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["switch", "-c", "setup-drift"])
        .status()
        .expect("create drift branch");
    assert!(status.success(), "create drift branch failed");
    let error =
        prepare_prototype1_parent_setup_at(&command, Some(&expected_sha), repo_root.clone())
            .expect_err("completed setup must reject active branch drift");
    assert!(error.to_string().contains("active branch"));
    let status = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["switch", &report.artifact_branch])
        .status()
        .expect("restore setup branch");
    assert!(status.success(), "restore setup branch failed");

    let scheduler_path = prototype1_scheduler_path(plan.content.campaign.manifest_path());
    let scheduler_bytes = fs::read(&scheduler_path).expect("read scheduler");
    fs::remove_file(&scheduler_path).expect("remove scheduler");
    let error =
        prepare_prototype1_parent_setup_at(&command, Some(&expected_sha), repo_root.clone())
            .expect_err("completed setup must reject missing scheduler");
    assert!(error.to_string().contains("scheduler.json"));
    assert!(
        !scheduler_path.exists(),
        "completed retry must not repair state"
    );
    crate::durable_io::write_atomic(&scheduler_path, &scheduler_bytes).expect("restore scheduler");

    let mismatch = Prototype1StateCommand {
        campaign: Some(report.campaign_id.clone()),
        node_id: None,
        repo_root: Some(repo_root.clone()),
        init_parent_identity: false,
        identity_branch: None,
        identity_instance: None,
        handoff_invocation: None,
        stop_after: None,
        successor_selection: None,
        successor_selection_seed: None,
        successor_selection_metrics: None,
        candidate_generator: Some(Prototype1CandidateGenerator::Legacy),
        format: InspectOutputFormat::Table,
    };
    let error =
        crate::cli::prototype1_state::driver::advance::run_to_terminal(mismatch, false, false)
            .await
            .expect_err("profile assertions cannot override admitted configuration");
    assert!(error.to_string().contains("--candidate-generator"));
    assert!(error.to_string().contains("BroadHarnessRequest"));

    let lease = crate::cli::prototype1_state::driver::control::claim_controller(
        &repo_root,
        profile::RunMode::Continuous,
    )
    .expect("completed setup admits the continuous controller");
    assert_eq!(
        lease.cursor().phase,
        crate::cli::prototype1_state::walk::phase::WalkPhase::R3
    );
    let conflict = crate::cli::prototype1_state::driver::control::claim_active(&repo_root)
        .expect_err("ancillary mutation must not bypass the active controller lease");
    assert!(
        conflict
            .to_string()
            .contains("controller session claim conflicted"),
        "{conflict}"
    );
    let intent = lease
        .intent_with_live_api(false, false)
        .expect("R3 edge intent");
    let (lease, receipt, state) =
        match crate::cli::prototype1_state::driver::control::advance_controlled(lease, intent)
            .await
            .expect("R3 edge is controlled")
        {
            crate::cli::prototype1_state::driver::control::ControlAdvance::Finished {
                finished:
                    crate::cli::prototype1_state::session::Finished::Terminal { lease, receipt, .. },
                state,
            } => (lease, receipt, state),
            other => panic!("unexpected R3 controller result: {other:?}"),
        };
    assert_eq!(
        state.phase(),
        crate::cli::prototype1_state::walk::phase::WalkPhase::R4a,
        "a newly committed edge must return its canonical typed post-state"
    );
    assert_eq!(
        lease.cursor().phase,
        crate::cli::prototype1_state::walk::phase::WalkPhase::R4a
    );
    let evidence = receipt
        .evidence
        .as_ref()
        .expect("committed edge has certified evidence");
    assert_eq!(
        evidence.edge(),
        crate::cli::prototype1_state::edge::ControlEdge::R3ToR4a
    );
    assert_eq!(
        evidence.cursor().expect("certified cursor"),
        lease.cursor().clone()
    );
    lease.release().expect("release controlled setup session");

    let session = crate::cli::prototype1_state::session::Store::for_manifest(
        plan.content.campaign.manifest_path(),
    )
    .inspect(&admitted_identity)
    .expect("inspect controlled setup session")
    .expect("session exists");
    assert!(session.active.is_none());
    assert_eq!(
        session.cursor.expect("committed cursor").phase,
        crate::cli::prototype1_state::walk::phase::WalkPhase::R4a
    );

    let mut drifted =
        crate::cli::prototype1_state::setup_admission::load_setup_admission(&admission_path)
            .expect("load completed receipt")
            .expect("completed receipt exists");
    drifted.intent.batch_manifest = eval_home.join("other-batch.json");
    write_json_atomic(&admission_path, &drifted).expect("write drifted receipt");
    let error = prepare_prototype1_parent_setup_at(&command, Some(&expected_sha), repo_root)
        .expect_err("receipt fields must remain bound to the reviewed plan");
    assert!(error.to_string().contains("deterministic intent"));
}

fn assert_session_absent(manifest: &Path, parent: &ParentIdentity) {
    let store = crate::cli::prototype1_state::session::Store::for_manifest(manifest);
    let paths = store.paths(parent);
    assert!(
        store
            .inspect(parent)
            .expect("inspect successor session")
            .is_none(),
        "rejected successor evidence must not create a session"
    );
    assert!(
        !paths.journal().exists(),
        "rejected successor evidence must not create a session journal"
    );
}

#[test]
fn successor_transfer_claims_exact_origin_and_rejects_drift() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eval_home = tmp.path().join("eval-home");
    let repo_root = tmp.path().join("repo");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", eval_home.as_os_str().into())]);

    init_indexed_repo(&repo_root);
    let predecessor = parent_identity_for("node-predecessor", 0);
    let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
            .to_string(),
        campaign_id: predecessor.campaign_id().clone(),
        parent_id: "node-successor".to_string(),
        node_id: "node-successor".to_string(),
        generation: 1,
        instance_id: Some("clap-rs__clap-3670".to_string()),
        previous_parent_id: Some(predecessor.parent_id().to_string()),
        parent_node_id: Some(predecessor.node_id().to_string()),
        branch_id: "branch-node-successor".to_string(),
        artifact_branch: Some("prototype1-node-successor".to_string()),
        created_at: "2026-05-06T00:00:00Z".to_string(),
    });
    let branch = parent
        .artifact_branch()
        .expect("successor artifact branch")
        .to_string();
    let status = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["switch", "-c", &branch])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("create successor branch");
    assert!(status.success(), "create successor branch failed");
    write_parent_identity(&repo_root, &parent).expect("write successor parent identity");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "seed successor parent");
    let head = GitWorktreeBackend
        .head_commit(&repo_root)
        .expect("successor checkout head");

    let manifest = campaign_manifest_path(parent.campaign_id()).expect("campaign manifest path");
    let profile = toml::from_str::<profile::Prototype1RunProfile>(
        "schema_version = \"prototype1-run-profile.v1\"\nname = \"successor-transfer\"\n",
    )
    .expect("run profile parses");
    let admitted = profile::admit_run_profile(
        &manifest,
        &profile::OperatorRunProfile {
            source_path: eval_home.join("successor-transfer.toml"),
            profile,
        },
    )
    .expect("run profile admitted");
    let journal_path = prototype1_transition_journal_path(&manifest);
    let node_dir = journal_path
        .parent()
        .expect("prototype1 root")
        .join("nodes")
        .join(parent.node_id());
    let ready_path = node_dir.join("successor-ready.json");
    let binary_path = std::env::current_exe().expect("current test executable");
    let runtime_other = RuntimeId::new();
    let runtime_wrong = RuntimeId::new();
    let runtime_path = RuntimeId::new();
    let runtime_checkout = RuntimeId::new();
    let runtime_exact = RuntimeId::new();
    let runtime_timeout = RuntimeId::new();

    let invocation_for = |runtime_id| {
        let path = invocation::invocation_path(&node_dir, runtime_id);
        let invocation = crate::cli::prototype1_state::invocation::Invocation {
            schema_version: invocation::SCHEMA_VERSION.to_string(),
            role: crate::cli::prototype1_state::invocation::Role::Successor,
            campaign_id: parent.campaign_id().clone(),
            node_id: parent.node_id().to_string(),
            runtime_id,
            journal_path: journal_path.clone(),
            channel_root: Some(invocation::channel_root(&node_dir, runtime_id)),
            node: None,
            request: None,
            resolved: None,
            active_parent_root: Some(repo_root.clone()),
            run_profile: Some(admitted.commitment.clone()),
            predecessor_attempt: Some(
                crate::cli::prototype1_state::successor::PredecessorAttempt::new(
                    crate::cli::prototype1_state::session::SessionId::for_test(1),
                    crate::cli::prototype1_state::event::TransitionId::new(),
                    crate::cli::prototype1_state::session::Fence::for_test(1),
                    true,
                    true,
                ),
            ),
            created_at: Utc::now().to_rfc3339(),
        };
        write_json_atomic(&path, &invocation).expect("write successor invocation");
        (path, invocation)
    };
    let spawn_for =
        |runtime_id, invocation_path: PathBuf| crate::cli::prototype1_state::successor::Record {
            runtime_id: Some(runtime_id),
            recorded_at: RecordedAt::now(),
            campaign_id: parent.campaign_id().clone(),
            node_id: parent.node_id().to_string(),
            state: crate::cli::prototype1_state::successor::State::Spawned {
                pid: std::process::id(),
                incarnation: crate::cli::prototype1_state::invocation::process_incarnation(
                    std::process::id(),
                )
                .expect("capture successor process"),
                active_parent_root: repo_root.clone(),
                binary_path: binary_path.clone(),
                invocation_path,
                ready_path: ready_path.clone(),
                streams: journal::Streams {
                    stdout: node_dir.join("successor.stdout"),
                    stderr: node_dir.join("successor.stderr"),
                },
            },
        };
    let checkout_for = |installed_commit: String| journal::ActiveCheckoutAdvancedEntry {
        recorded_at: RecordedAt::now(),
        campaign_id: parent.campaign_id().clone(),
        previous_parent_identity: Some(predecessor.clone()),
        selected_parent_identity: parent.clone(),
        active_parent_root: repo_root.clone(),
        selected_branch: branch.clone(),
        installed_commit,
    };
    let mut journal = PrototypeJournal::new(journal_path.clone());
    let exact_checkout = checkout_for(head.to_string());

    let (wrong_path, _) = invocation_for(runtime_wrong);
    journal
        .append(JournalEntry::Successor(spawn_for(
            runtime_other,
            wrong_path.clone(),
        )))
        .expect("append wrong-runtime spawn");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &wrong_path,
    )
    .expect_err("wrong runtime evidence must be rejected");
    assert!(
        error.to_string().contains("exact Spawned runtime"),
        "{error}"
    );
    assert_session_absent(&manifest, &parent);

    journal
        .append(JournalEntry::ActiveCheckoutAdvanced(exact_checkout.clone()))
        .expect("append path-test checkout");
    let (path_drift, _) = invocation_for(runtime_path);
    journal
        .append(JournalEntry::Successor(spawn_for(
            runtime_path,
            node_dir.join("invocations/other.json"),
        )))
        .expect("append wrong-path spawn");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &path_drift,
    )
    .expect_err("wrong invocation evidence must be rejected");
    assert!(error.to_string().contains("Spawned paths"), "{error}");
    assert_session_absent(&manifest, &parent);

    let bad_checkout = checkout_for("wrong-installed-commit".to_string());
    let (checkout_path, _) = invocation_for(runtime_checkout);
    let checkout_spawn = spawn_for(runtime_checkout, checkout_path.clone());
    journal
        .append(JournalEntry::ActiveCheckoutAdvanced(bad_checkout))
        .expect("append wrong checkout");
    journal
        .append(JournalEntry::Successor(checkout_spawn))
        .expect("append checkout-test spawn");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &checkout_path,
    )
    .expect_err("wrong checkout evidence must be rejected");
    assert!(
        error
            .to_string()
            .contains("does not match controller epoch Git HEAD"),
        "{error}"
    );
    assert_session_absent(&manifest, &parent);

    let (exact_path, exact_invocation) = invocation_for(runtime_exact);
    let exact_spawn = spawn_for(runtime_exact, exact_path.clone());
    journal
        .append(JournalEntry::ActiveCheckoutAdvanced(exact_checkout.clone()))
        .expect("append exact checkout before spawn");
    journal
        .append(JournalEntry::Successor(exact_spawn.clone()))
        .expect("append exact spawn after checkout");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Step,
        &exact_path,
    )
    .expect_err("mode mismatch must be rejected");
    assert!(
        error
            .to_string()
            .contains("admitted run profile requires Continuous"),
        "{error}"
    );
    assert_session_absent(&manifest, &parent);

    let expected_origin = crate::cli::prototype1_state::control_evidence::SuccessorOrigin::new(
        exact_path
            .canonicalize()
            .expect("canonical invocation path"),
        exact_invocation,
        exact_checkout.clone(),
        exact_spawn,
        &parent,
        &admitted.commitment,
        &repo_root,
    )
    .expect("exact successor origin");
    let expected_cursor = crate::cli::prototype1_state::control_evidence::successor_cursor(
        &expected_origin,
        &parent,
        &admitted.commitment,
        &repo_root,
    )
    .expect("successor R3 cursor");
    let lease = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &exact_path,
    )
    .expect("exact successor transfer claims session");
    assert_eq!(lease.runtime_id(), Some(runtime_exact));
    assert_eq!(lease.mode(), profile::RunMode::Continuous);
    assert_eq!(lease.cursor(), &expected_cursor);
    assert_eq!(
        lease.cursor().phase,
        crate::cli::prototype1_state::walk::phase::WalkPhase::R3
    );
    assert_eq!(
        lease.handoff_path(),
        Some(expected_origin.invocation_path())
    );
    let store = crate::cli::prototype1_state::session::Store::for_manifest(&manifest);
    let snapshot = store
        .inspect(&parent)
        .expect("inspect claimed successor session")
        .expect("successor session exists");
    assert_eq!(
        snapshot
            .active
            .as_ref()
            .and_then(|owner| owner.runtime_id()),
        Some(runtime_exact)
    );
    assert_eq!(snapshot.cursor.as_ref(), Some(&expected_cursor));
    lease.release().expect("release successor session");

    let session_path = store.paths(&parent).journal().to_path_buf();
    let session_text = fs::read_to_string(&session_path).expect("read successor session");
    let mut created = serde_json::from_str::<serde_json::Value>(
        session_text.lines().next().expect("Created session entry"),
    )
    .expect("decode Created session entry");
    assert_eq!(
        created["schema_version"],
        serde_json::json!("prototype1-control-session.v5")
    );
    assert_eq!(created["origin"]["kind"], serde_json::json!("successor"));
    created["cursor"]["phase"] = serde_json::json!("r4c");
    let tampered =
        crate::cli::prototype1_state::session::Store::new(eval_home.join("tampered-control"));
    let tampered_path = tampered.paths(&parent).journal().to_path_buf();
    fs::create_dir_all(tampered_path.parent().expect("tampered session parent"))
        .expect("create tampered session parent");
    fs::write(
        &tampered_path,
        format!(
            "{}\n",
            serde_json::to_string(&created).expect("encode tampered Created entry")
        ),
    )
    .expect("write tampered Created entry");
    let error = tampered
        .inspect(&parent)
        .expect_err("tampered successor origin cursor must be rejected");
    assert!(
        error
            .to_string()
            .contains("session origin cursor does not match its admitted authority"),
        "{error}"
    );

    fs::remove_dir_all(store.paths(&parent).root()).expect("remove completed test session");
    journal
        .append(JournalEntry::Successor(
            crate::cli::prototype1_state::successor::Record {
                runtime_id: Some(runtime_exact),
                recorded_at: RecordedAt::now(),
                campaign_id: parent.campaign_id().clone(),
                node_id: parent.node_id().to_string(),
                state: crate::cli::prototype1_state::successor::State::Ready {
                    pid: std::process::id(),
                    ready_path: ready_path.clone(),
                    controller: None,
                },
            },
        ))
        .expect("append successor ready");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &exact_path,
    )
    .expect_err("matching Ready evidence must prevent fresh session creation");
    assert!(error.to_string().contains("legacy Ready"), "{error}");
    assert_session_absent(&manifest, &parent);

    let (timeout_path, _) = invocation_for(runtime_timeout);
    journal
        .append(JournalEntry::ActiveCheckoutAdvanced(exact_checkout))
        .expect("append timeout checkout before spawn");
    journal
        .append(JournalEntry::Successor(spawn_for(
            runtime_timeout,
            timeout_path.clone(),
        )))
        .expect("append timeout spawn after checkout");
    let lease = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &timeout_path,
    )
    .expect("Spawned timeout runtime claims initial session");
    assert_eq!(lease.runtime_id(), Some(runtime_timeout));
    lease.release().expect("release timeout runtime session");
    journal
        .append(JournalEntry::Successor(
            crate::cli::prototype1_state::successor::Record {
                runtime_id: Some(runtime_timeout),
                recorded_at: RecordedAt::now(),
                campaign_id: parent.campaign_id().clone(),
                node_id: parent.node_id().to_string(),
                state: crate::cli::prototype1_state::successor::State::TimedOut {
                    waited_ms: 1,
                    ready_path: ready_path.clone(),
                },
            },
        ))
        .expect("append successor timeout");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &timeout_path,
    )
    .expect_err("TimedOut runtime must not reclaim its session");
    assert!(error.to_string().contains("cannot reclaim"), "{error}");
    assert!(error.to_string().contains("TimedOut"), "{error}");
    assert!(
        store
            .inspect(&parent)
            .expect("inspect released timeout session")
            .is_some(),
        "terminal lifecycle rejection must preserve its existing session"
    );

    fs::remove_dir_all(store.paths(&parent).root()).expect("remove timeout test session");
    let error = crate::cli::prototype1_state::driver::control::claim_successor(
        &repo_root,
        profile::RunMode::Continuous,
        &timeout_path,
    )
    .expect_err("TimedOut runtime must not create a fresh session");
    assert!(
        error.to_string().contains("cannot create a fresh"),
        "{error}"
    );
    assert!(error.to_string().contains("TimedOut"), "{error}");
    assert_session_absent(&manifest, &parent);
}

#[test]
fn prototype1_setup_preview_rejects_mutating_or_ambiguous_inputs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", tmp.path().as_os_str().into())]);
    let before = snapshot_setup_tree(tmp.path());

    let mut inline = setup_preview_command(
        tmp.path().join("missing-batch.json"),
        tmp.path().join("missing-profile.toml"),
    );
    inline.batch = None;
    inline.dataset = Some(tmp.path().join("dataset.jsonl"));
    let error = preview_prototype1_parent_setup(&inline)
        .expect_err("preview must reject inline preparation");
    assert!(
        error
            .to_string()
            .contains("requires an existing --batch or --batch-id")
    );

    let mut dry_run = setup_preview_command(
        tmp.path().join("missing-batch.json"),
        tmp.path().join("missing-profile.toml"),
    );
    dry_run.dry_run = true;
    let error = preview_prototype1_parent_setup(&dry_run)
        .expect_err("legacy dry-run must not masquerade as preview");
    assert!(error.to_string().contains("use prototype1-setup --preview"));

    let mut missing_profile = setup_preview_command(
        tmp.path().join("missing-batch.json"),
        tmp.path().join("missing-profile.toml"),
    );
    missing_profile.profile = None;
    let error = prepare_prototype1_parent_setup(&missing_profile, None)
        .expect_err("setup must require an explicit profile");
    assert!(error.to_string().contains("requires an explicit --profile"));

    let mut legacy_search = setup_preview_command(
        tmp.path().join("missing-batch.json"),
        tmp.path().join("missing-profile.toml"),
    );
    legacy_search.max_generations = 2;
    let error = preview_prototype1_parent_setup(&legacy_search)
        .expect_err("setup must reject legacy search authority");
    assert!(error.to_string().contains("run profile"));

    let mut legacy_stop = setup_preview_command(
        tmp.path().join("missing-batch.json"),
        tmp.path().join("missing-profile.toml"),
    );
    legacy_stop.stop_after = Prototype1LoopStopAfter::BaselineEval;
    let error = preview_prototype1_parent_setup(&legacy_stop)
        .expect_err("setup must reject legacy stop authority");
    assert!(error.to_string().contains("[execution].stop_after"));

    let mut batch_selector = setup_preview_command(
        tmp.path().join("missing-batch.json"),
        tmp.path().join("missing-profile.toml"),
    );
    batch_selector.specific.push("ripgrep".to_string());
    let error = preview_prototype1_parent_setup(&batch_selector)
        .expect_err("prepared batch must own its cohort");
    assert!(error.to_string().contains("prepared batch manifest"));
    assert_eq!(snapshot_setup_tree(tmp.path()), before);
}

#[test]
fn prototype1_setup_campaign_manifest_preserves_embedding_overrides() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", tmp.path().as_os_str().into())]);
    write_direct_google_registry(tmp.path());

    let dataset_file = tmp.path().join("dataset.jsonl");
    fs::write(
        &dataset_file,
        r#"{"instance_id":"BurntSushi__ripgrep-2209"}"#,
    )
    .expect("write dataset");
    let prepared_batch = crate::spec::PreparedMsbBatch {
        batch_id: "embedding-override-batch".to_string(),
        dataset_file: dataset_file.clone(),
        dataset_url: None,
        repo_cache: tmp.path().join("repo-cache"),
        instances_root: tmp.path().join("instances"),
        output_dir: tmp.path().join("batches").join("embedding-override-batch"),
        budget: crate::spec::EvalBudget::default(),
        instances: vec!["BurntSushi__ripgrep-2209".to_string()],
        campaign: None,
    };
    let command = Prototype1LoopCommand {
        batch: None,
        batch_id: None,
        dataset: None,
        dataset_key: None,
        all: false,
        instance: Vec::new(),
        specific: Vec::new(),
        limit: None,
        prepare_batch_id: None,
        campaign: Some(CampaignId::from("embedding-override-campaign")),
        profile: None,
        repo_cache: None,
        instances_root: None,
        batches_root: None,
        max_turns: 40,
        max_tool_calls: 200,
        wall_clock_secs: 1800,
        eval_max_tokens: crate::campaign::DEFAULT_EVAL_MAX_TOKENS,
        index_debug_snapshots: true,
        use_default_model: false,
        model_id: Some("google/gemini-3.5-flash".to_string()),
        provider: Some("google".to_string()),
        route_source: Some(ModelRouteSource::DirectGoogle),
        embedding_model_id: Some("perplexity/pplx-embed-v1-4b".to_string()),
        embedding_route: Some(EmbeddingRoute::OpenRouter),
        embedding_provider: Some("perplexity".to_string()),
        stop_on_error: false,
        protocol_model_id: None,
        protocol_provider: None,
        protocol_route_source: None,
        source_campaign: None,
        source_branch_id: None,
        max_generations: 1,
        max_total_nodes: 32,
        min_children: 2,
        max_children: 6,
        child_schedule_mode: CliPrototype1ChildScheduleMode::FullBatch,
        stop_on_first_keep: false,
        require_keep_for_continuation: true,
        explore_from_rejected: true,
        stop_after: Prototype1LoopStopAfter::BaselineEval,
        dry_run: false,
        format: InspectOutputFormat::Json,
    };

    let campaign = prepare_prototype1_loop_campaign(&command, &prepared_batch, None)
        .expect("campaign prepares");
    let manifest =
        crate::campaign::load_campaign_manifest(&CampaignId::from("embedding-override-campaign"))
            .expect("manifest");

    assert_eq!(
        manifest.eval.embedding_model_id.as_deref(),
        Some("perplexity/pplx-embed-v1-4b")
    );
    assert_eq!(
        manifest.eval.embedding_provider_slug.as_deref(),
        Some("perplexity")
    );
    assert_eq!(
        campaign.resolved.eval.embedding_model_id.as_deref(),
        Some("perplexity/pplx-embed-v1-4b")
    );
    assert_eq!(
        campaign.resolved.eval.embedding_provider_slug.as_deref(),
        Some("perplexity")
    );
    assert_eq!(
        manifest.eval.max_tokens,
        Some(crate::campaign::DEFAULT_EVAL_MAX_TOKENS)
    );
    assert_eq!(campaign.resolved.eval.max_tokens, manifest.eval.max_tokens);
}

#[test]
fn prototype1_eval_set_id_includes_embedding_overrides() {
    let sources = vec![crate::target_registry::RegistryDatasetSource {
        key: None,
        path: PathBuf::from("slice.jsonl"),
        label: "prototype1/test-batch".to_string(),
        url: None,
    }];
    let instance_ids = vec!["BurntSushi__ripgrep-2209".to_string()];
    let base_policy = test_eval_policy();
    let baseline_campaign = CampaignId::from("baseline");
    let treatment_campaign = CampaignId::from("treatment");
    let base_id = prototype1_eval_set_id(
        &baseline_campaign,
        &treatment_campaign,
        crate::target_registry::BenchmarkFamily::MultiSweBenchRust,
        &sources,
        &base_policy,
        &instance_ids,
    );

    let mut embedding_policy = base_policy.clone();
    embedding_policy.embedding_model_id = Some("perplexity/pplx-embed-v1-4b".to_string());
    embedding_policy.embedding_provider_slug = Some("perplexity".to_string());
    let embedding_id = prototype1_eval_set_id(
        &baseline_campaign,
        &treatment_campaign,
        crate::target_registry::BenchmarkFamily::MultiSweBenchRust,
        &sources,
        &embedding_policy,
        &instance_ids,
    );

    assert_ne!(base_id, embedding_id);

    let mut direct_policy = embedding_policy;
    direct_policy.embedding_route = EmbeddingRoute::DirectOpenAi;
    direct_policy.embedding_provider_slug = None;
    let direct_id = prototype1_eval_set_id(
        &baseline_campaign,
        &treatment_campaign,
        crate::target_registry::BenchmarkFamily::MultiSweBenchRust,
        &sources,
        &direct_policy,
        &instance_ids,
    );

    assert_ne!(embedding_id, direct_id);

    let mut token_policy = direct_policy;
    token_policy.max_tokens = Some(crate::campaign::DEFAULT_EVAL_MAX_TOKENS);
    let token_id = prototype1_eval_set_id(
        &baseline_campaign,
        &treatment_campaign,
        crate::target_registry::BenchmarkFamily::MultiSweBenchRust,
        &sources,
        &token_policy,
        &instance_ids,
    );

    assert_ne!(direct_id, token_id);
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
        &CLI_TEST_CAMPAIGN,
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
    let child = ChildFiles::from_resolved(
        &CLI_TEST_CAMPAIGN,
        node.clone(),
        test_resolved(&node),
        false,
    );

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

struct R4cFixture {
    r4c: typestate::R4cReady<Prototype1StateRunShape, ResolvedCampaignConfig>,
    config: ResolvedCampaignConfig,
    manifest_path: PathBuf,
    repo_root: PathBuf,
    journal_path: PathBuf,
    parent: ParentIdentity,
}

fn r4c_fixture(root: &Path, backend: profile::EvalStorageBackend) -> R4cFixture {
    let manifest_path = root.join("campaign.json");
    let repo_root = root.join("repo");
    fs::create_dir_all(&repo_root).expect("repo dir");
    let parent = test_parent_identity();
    let ready = ready_parent_for_test(&manifest_path, &repo_root);
    let mut command = state_command_without_ids();
    command.campaign = Some(parent.campaign_id().clone());
    command.repo_root = Some(repo_root.clone());
    let mut shape = Prototype1StateRunShape::from_command(&command);
    shape.eval_storage_backend = backend;
    let config = ResolvedCampaignConfig {
        campaign_id: parent.campaign_id().clone(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        model_id: "test-model".to_string(),
        provider_slug: None,
        route_source: ModelRouteSource::DirectGoogle,
        required_procedures: Vec::new(),
        instances_root: root.join("instances"),
        batches_root: root.join("batches"),
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: crate::FrameworkConfig::default(),
    };
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let journal = PrototypeJournal::new(&journal_path);
    let collected = typestate::context::Collected::new(
        command,
        repo_root.clone(),
        parent.campaign_id().clone(),
        manifest_path.clone(),
        shape,
        config.clone(),
        journal_path.clone(),
        journal,
    );
    let r4c = typestate::R4cReady::from_collected_parent(collected, ready);

    R4cFixture {
        r4c,
        config,
        manifest_path,
        repo_root,
        journal_path,
        parent,
    }
}

#[test]
fn prototype1_transition_contract_r4c_to_r5_fs_records_parent_start() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let fixture = r4c_fixture(tmp.path(), profile::EvalStorageBackend::Fs);
    let parent = fixture.parent.clone();
    let manifest_path = fixture.manifest_path.clone();
    let repo_root = fixture.repo_root.clone();
    let journal_path = fixture.journal_path.clone();

    let r5: typestate::R5<Prototype1StateRunShape, ResolvedCampaignConfig> =
        crate::cli::prototype1_state::live_edges::r4c_to_r5(fixture.r4c)
            .expect("R4c -> R5 succeeds in fs mode");

    let parts = r5.into_parts();
    assert_eq!(parts.parent.identity(), &parent);
    assert_eq!(parts.collected.into_parts().journal_path, journal_path);
    let entries = PrototypeJournal::new(&journal_path)
        .load_entries()
        .expect("journal loads");
    assert_eq!(entries.len(), 2);
    match &entries[0] {
        JournalEntry::ParentStarted(entry) => {
            assert_eq!(entry.campaign_id, parent.campaign_id().clone());
            assert_eq!(entry.parent_identity, parent);
            assert_eq!(entry.repo_root, repo_root);
            assert_eq!(entry.handoff_runtime_id, None);
            assert_eq!(entry.pid, std::process::id());
            assert!(entry.recorded_at.0 > 0);
        }
        other => panic!("unexpected first R4c -> R5 entry: {other:?}"),
    }
    match &entries[1] {
        JournalEntry::Resource(sample) => {
            assert_eq!(sample.campaign_id, parent.campaign_id().clone());
            assert_eq!(sample.parent_id, parent.parent_id());
            assert_eq!(sample.phase, journal::resource::Phase::ParentStart);
            assert_eq!(sample.status, journal::resource::Status::Missing);
            assert_eq!(sample.path, repo_root.join("target"));
        }
        other => panic!("unexpected second R4c -> R5 entry: {other:?}"),
    }
    let campaign_root = prototype1_campaign_root(&manifest_path);
    assert!(!campaign_root.join("history").exists());
    assert!(!campaign_root.join("messages").exists());
    assert!(!repo_root.join("target").exists());
}

#[test]
fn prototype1_transition_contract_r4c_to_r5_db_backends_record_parent_start_rows() {
    for backend in [
        profile::EvalStorageBackend::DbMirror,
        profile::EvalStorageBackend::Database,
        profile::EvalStorageBackend::DualStrict,
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fixture = r4c_fixture(tmp.path(), backend);
        let parent = fixture.parent.clone();
        let manifest_path = fixture.manifest_path.clone();
        let journal_path = fixture.journal_path.clone();

        let r5: typestate::R5<Prototype1StateRunShape, ResolvedCampaignConfig> =
            crate::cli::prototype1_state::live_edges::r4c_to_r5(fixture.r4c)
                .unwrap_or_else(|err| panic!("R4c -> R5 succeeds for {backend:?}: {err:?}"));

        let parts = r5.into_parts();
        assert_eq!(parts.parent.identity(), &parent);
        assert_eq!(parts.collected.into_parts().journal_path, journal_path);
        let entries = PrototypeJournal::new(&journal_path)
            .load_entries()
            .expect("journal loads");
        assert_eq!(entries.len(), 2, "{backend:?} preserves JSONL evidence");
        let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
        assert!(db_path.is_file(), "{backend:?} writes owner eval DB");
        let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "campaign_id".to_string(),
            cozo::DataValue::from(parent.campaign_id().to_string()),
        );
        let events = db
            .raw_query_params(
                r#"
?[event_id, transition, store_scope, source_class, evidence_class, validation_status] :=
    *eval_transition_event {
        event_id,
        campaign_id,
        transition,
        store_scope,
        source_class,
        evidence_class,
        validation_status
    },
    campaign_id = $campaign_id
"#,
                params.clone(),
            )
            .expect("query transition rows");
        assert_eq!(events.rows.len(), 1);
        let event = events.row_refs().next().expect("event row");
        assert_eq!(
            event.get::<String>("transition").expect("transition"),
            "r4c_to_r5"
        );
        assert_eq!(event.get::<String>("store_scope").expect("scope"), "parent");
        assert_eq!(
            event.get::<String>("source_class").expect("source"),
            "direct_write"
        );
        assert_eq!(
            event.get::<String>("evidence_class").expect("evidence"),
            "typed_transition"
        );
        assert_eq!(
            event.get::<String>("validation_status").expect("status"),
            "valid"
        );

        let refs = db
            .raw_query_params(
                r#"
?[record_ref_id, family, evidence_class] :=
    *eval_record_ref { record_ref_id, campaign_id, family, evidence_class },
    campaign_id = $campaign_id
"#,
                params,
            )
            .expect("query record refs");
        assert_eq!(refs.rows.len(), 2);
        let mut classes = std::collections::BTreeMap::new();
        for row in refs.row_refs() {
            classes.insert(
                row.get::<String>("family").expect("family"),
                row.get::<String>("evidence_class").expect("class"),
            );
        }
        assert_eq!(
            classes.get("parent_started").map(String::as_str),
            Some("typed_transition")
        );
        assert_eq!(
            classes.get("resource_parent_start").map(String::as_str),
            Some("diagnostic")
        );
    }
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

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
async fn rag_unavailable_headless_tui_setup_writes_typed_diagnostics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let _env = crate::test_support::env_guard_os(vec![(
        "PLOKE_EVAL_FORCE_HEADLESS_TUI_RAG_UNAVAILABLE",
        "1".into(),
    )]);
    init_indexed_repo(&repo_root);
    write_surface_target(
        &repo_root,
        Path::new("Cargo.toml"),
        "[package]\nname = \"rag-unavailable-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
    );
    write_surface_target(&repo_root, Path::new("src/lib.rs"), "pub fn canary() {}\n");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "rag unavailable fixture");

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
    )
    .expect("published broad harness request");
    let slot = HarnessRequestSlot {
        request_path: publication.request_path,
        published: publication.published,
    };
    let options = BroadTuiAttemptOptions {
        model: None,
        max_attempts: Some(1),
        timeout_secs: Some(60),
    };

    let err = run_broad_headless_tui_attempt_with_options(
        &slot,
        &options,
        None,
        profile::EvalStorageBackend::Fs,
    )
    .await
    .expect_err("RAG/BM25 setup failure must stop child planning");

    let tui_adapter::BroadAttemptError::Setup { phase, detail } = err else {
        panic!("expected typed setup blocker, got {err:?}");
    };
    assert_eq!(phase, "bm25_ready");
    assert_eq!(detail, "RAG service is unavailable");

    let diagnostics_path =
        broad_headless_tui_diagnostics_path(slot.published.submitted_result_path());
    let diagnostics = fs::read_to_string(&diagnostics_path).expect("diagnostics persisted");
    let summary: tui_adapter::evidence::Summary =
        serde_json::from_str(&diagnostics).expect("typed diagnostics decode");
    assert!(matches!(
        summary.terminal,
        Some(tui_adapter::evidence::Terminal::SetupUnavailable { phase, reason })
            if phase == "bm25_ready" && reason == "RAG service is unavailable"
    ));
    let rejection = slot_rejection(&slot);
    assert!(
        rejection.contains("setup unavailable during bm25_ready: RAG service is unavailable"),
        "slot rejection should join typed diagnostics, got: {rejection}"
    );
    assert!(
        !rejection.contains("produced no submitted result or diagnostics"),
        "slot rejection must not degrade to missing diagnostics: {rejection}"
    );
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

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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

    let err = run_broad_headless_tui_attempt_with_options(
        &slot,
        &options,
        None,
        profile::EvalStorageBackend::Fs,
    )
    .await
    .expect_err("workspace preparation failures must stop child planning");

    let tui_adapter::BroadAttemptError::Setup { phase, detail } = err else {
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
    let _slot_env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "9".into())]);
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
    let batch: HarnessRequestBatch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
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
    let parent_identity = batch.parent.identity().clone();
    let result = publish_broad_harness_child_plan_from_admitted_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
        Vec::new(),
    );
    let err = match result {
        Ok(_) => panic!("zero accepted broad harness batch must not seal children"),
        Err(err) => err,
    };
    let PrepareError::ChildPlanBelowMinimum {
        runnable_children,
        required_min,
        attempted_slots,
        accepted_results,
        child_plan_path,
    } = err
    else {
        panic!("unexpected error: {err:?}");
    };
    assert_eq!(runnable_children, 0);
    assert_eq!(required_min, 2);
    assert_eq!(attempted_slots, 9);
    assert_eq!(accepted_results, 0);
    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    assert_eq!(child_plan_path, files.message_at().path().to_path_buf());
    let bytes = fs::read(files.message_at().path())
        .expect("failed batch must persist a rejected-attempt child plan");
    let body: ChildPlanFiles = serde_json::from_slice(&bytes).expect("decode child plan");
    assert!(body.children().is_empty());
    assert!(
        !body.rejected_surface_attempts().is_empty(),
        "failed batch should persist parent-readable rejected attempt evidence"
    );

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
    let planned_result = resolve_profile_child_plan(
        &CLI_TEST_CAMPAIGN,
        &manifest_path,
        &repo_root,
        resumed_parent,
        &run_profile,
        budget,
        ModelRouteSource::DirectGoogle,
    )
    .await;
    let planned: PlannedChildren = planned_result
        .expect("failed broad harness batch should be recoverable as rejected evidence");

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

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
fn applied_timed_out_headless_tui_blocks_submitted_result_with_typed_detail() {
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
    commit_indexed_repo(&repo_root, "typed applied timeout fixture");

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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

    let proposal_id = uuid::Uuid::from_u128(0x45a2_e262_0000_0000_0000_000000000002);
    let attempts = vec![tui_adapter::HeadlessAttempt::applied_for_test(
        1,
        proposal_id,
        vec![candidate_path.clone()],
    )];
    let applied = tui_adapter::HeadlessRun::from_parts_for_test(attempts.clone(), None)
        .applied_edit()
        .expect("applied edit evidence");
    let run = tui_adapter::HeadlessRun::from_parts_for_test(
        attempts,
        Some(tui_adapter::HeadlessTerminal::AppliedTimedOut { secs: 900, applied }),
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
    .expect_err("typed applied-timeout run must not publish submission");
    let detail = err.to_string();
    assert!(
        detail.contains("timed out after 900 seconds after applying proposal"),
        "unexpected error: {detail}"
    );
    assert!(
        detail.contains("refusing to publish submitted broad-harness result"),
        "unexpected error: {detail}"
    );
    assert!(
        !slot.published.submitted_result_path().exists(),
        "typed applied-timeout run must not write submitted result at {}",
        slot.published.submitted_result_path().display()
    );
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
fn deterministic_tui_tools_child_plan_is_disabled_until_real_patch_generation_exists() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 1);

    let err = match publish_deterministic_tui_tools_child_plan(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        parent,
        budget,
    ) {
        Ok(_) => panic!("deterministic no-op child planning must fail loudly"),
        Err(err) => err,
    };

    let PrepareError::InvalidBatchSelection { detail } = err else {
        panic!("unexpected error variant");
    };
    assert!(detail.contains("deterministic-tui-tools is disabled"));
    assert!(detail.contains("generate real patches"));
}

#[test]
#[ignore = "obsolete deterministic no-op child publication path is intentionally disabled"]
fn tui_edit_surface_parent_selection_publishes_child_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 1);

    let receipt = publish_deterministic_tui_tools_child_plan(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
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
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(CLI_TEST_CAMPAIGN.to_string()),
    );
    params.insert(
        "producer_id".to_string(),
        cozo::DataValue::from("node-parent".to_string()),
    );
    let rows = db
        .raw_query_params(
            r#"
?[
    family,
    schema_version,
    store_scope,
    producer_role,
    source_class,
    evidence_class,
    visibility_scope,
    validation_status,
    source_ref,
    content_sha256,
    payload_json
] :=
    *eval_record_ref {
        campaign_id,
        family,
        schema_version,
        store_scope,
        producer_role,
        producer_id,
        source_class,
        evidence_class,
        visibility_scope,
        validation_status,
        source_ref,
        content_sha256,
        payload_json
    },
    campaign_id = $campaign_id,
    producer_id = $producer_id,
    family = "scheduler_node"
"#,
            params,
        )
        .expect("query deterministic parent scheduler-node refs");
    assert_eq!(rows.rows.len(), 1);
    let row = rows.row_refs().next().expect("record ref row");
    assert_eq!(
        row.get::<String>("family").expect("family"),
        "scheduler_node"
    );
    assert_eq!(
        row.get::<String>("schema_version").expect("schema"),
        PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION
    );
    assert_eq!(row.get::<String>("store_scope").expect("scope"), "parent");
    assert_eq!(row.get::<String>("producer_role").expect("role"), "parent");
    assert_eq!(
        row.get::<String>("source_class").expect("source"),
        "compatibility_import"
    );
    assert_eq!(
        row.get::<String>("evidence_class").expect("evidence"),
        "compatibility"
    );
    assert_eq!(
        row.get::<String>("visibility_scope").expect("visibility"),
        "parent_visible"
    );
    assert_eq!(
        row.get::<String>("validation_status").expect("status"),
        "valid"
    );
    assert!(
        row.get::<String>("source_ref")
            .expect("source ref")
            .contains("node.json:L1")
    );
    assert!(
        !row.get::<String>("content_sha256")
            .expect("hash")
            .is_empty(),
        "record ref carries payload hash"
    );
    assert!(
        row.get::<String>("payload_json")
            .expect("payload")
            .contains("\"running\""),
        "payload remains a compatibility ref for deterministic parent Running projection"
    );
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

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(CLI_TEST_CAMPAIGN.to_string()),
    );
    let refs = db
        .raw_query_params(
            r#"
?[family, producer_id] :=
    *eval_record_ref { campaign_id, family, producer_id },
    campaign_id = $campaign_id
"#,
            params,
        )
        .expect("query all record refs");
    let mut nodes = BTreeSet::new();
    for row in refs.row_refs() {
        let family = row.get::<String>("family").expect("family");
        let producer = row.get::<String>("producer_id").expect("producer");
        if family == "scheduler_node" {
            nodes.insert(producer.clone());
        }
        assert_ne!(
            family, "child_plan_file",
            "child-plan persistence must use normalized eval_child_plan rows, not eval_record_ref payloads"
        );
    }
    for child in body.children() {
        assert!(
            nodes.contains(child.node_id()),
            "child scheduler node '{}' should still be mirrored until scheduler-node normalization lands",
            child.node_id()
        );
    }

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(CLI_TEST_CAMPAIGN.to_string()),
    );
    params.insert(
        "parent_node_id".to_string(),
        cozo::DataValue::from(body.parent_node_id().to_string()),
    );
    let plans = db
        .raw_query_params(
            r#"
?[plan_id, child_generation, child_count, rejected_count, message_sha256] :=
    *eval_child_plan {
        plan_id,
        campaign_id,
        parent_node_id,
        child_generation,
        child_count,
        rejected_count,
        message_sha256
    },
    campaign_id = $campaign_id,
    parent_node_id = $parent_node_id
"#,
            params,
        )
        .expect("query normalized child plan rows");
    assert_eq!(
        plans.rows.len(),
        1,
        "child plan should have one normalized row"
    );
    let plan = plans.row_refs().next().expect("child plan row");
    let plan_id = plan.get::<String>("plan_id").expect("plan id");
    assert_eq!(plan.get::<i64>("child_generation").expect("generation"), 1);
    assert_eq!(plan.get::<i64>("child_count").expect("child count"), 1);
    assert_eq!(
        plan.get::<i64>("rejected_count").expect("rejected count"),
        0
    );
    assert!(
        !plan
            .get::<String>("message_sha256")
            .expect("message hash")
            .is_empty(),
        "child-plan row carries message content hash"
    );

    let mut params = std::collections::BTreeMap::new();
    params.insert("plan_id".to_string(), cozo::DataValue::from(plan_id));
    let children = db
        .raw_query_params(
            r#"
?[child_node_id, branch_id, child_index, status, runner_request_path] :=
    *eval_child_plan_child {
        plan_id,
        child_node_id,
        branch_id,
        child_index,
        status,
        runner_request_path
    },
    plan_id = $plan_id
"#,
            params,
        )
        .expect("query normalized child-plan children");
    assert_eq!(children.rows.len(), body.children().len());
    let child_row = children.row_refs().next().expect("child row");
    assert_eq!(
        child_row.get::<String>("child_node_id").expect("child id"),
        body.children()[0].node_id()
    );
    assert_eq!(
        child_row.get::<String>("status").expect("status"),
        "planned"
    );
    assert!(
        child_row
            .get::<String>("runner_request_path")
            .expect("request path")
            .ends_with("runner-request.json")
    );
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

    let first = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &parent_identity,
        budget,
        admission_binding.clone(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
    )
    .expect("first publication");
    let second = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &parent_identity,
        budget,
        admission_binding,
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
    let _env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "9".into())]);
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(2, 3);

    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
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

#[tokio::test]
async fn pre_child_planning_review_writes_prompt_and_artifact_before_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let _env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "2".into())]);
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 2);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    let first = &batch.slots[0].published;
    let artifact_path = first
        .request()
        .planning
        .artifact_path
        .clone()
        .expect("request carries planner artifact path");

    run_pre_child_planning_review(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        &batch,
    )
    .await
    .expect("test planner stub writes artifact");

    let prompt_path = pre_child_planning_prompt_path(&artifact_path);
    assert!(prompt_path.exists());
    assert!(artifact_path.exists());
    let artifact: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&artifact_path).expect("artifact text"))
            .expect("artifact json");
    assert_eq!(artifact["schema_version"], PRE_CHILD_PLANNING_SCHEMA);
    assert_eq!(artifact["status"], "test_stub");
    assert_eq!(artifact["request_id"], first.request_id());
    assert_eq!(artifact["request_hash"], first.request_hash());
    assert_eq!(
        artifact["structured_response"]["target_pipeline"],
        "test-stub broad harness pipeline"
    );
}

#[test]
fn broad_batch_default_cap_respects_small_max() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let _env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "6".into())]);
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(1, 2);

    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
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
    let _env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "9".into())]);
    write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(2, 3).with_parallel_targets(2);

    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
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
fn turn_live_bundle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(&repo_root, Path::new("src/lib.rs"), "pub fn canary() {}\n");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "turn live bundle fixture");

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
    )
    .expect("published broad harness request");
    let slot = HarnessRequestSlot {
        request_path: publication.request_path,
        published: publication.published,
    };
    let run = tui_adapter::HeadlessRun::from_parts_for_test(
        Vec::new(),
        Some(tui_adapter::HeadlessTerminal::CompletedWithoutEdit {
            outcome: "completed".to_string(),
            summary: "diagnostic bundle".to_string(),
        }),
    );

    write_broad_headless_tui_turn_live_bundle(
        &slot,
        &run,
        "diagnose the run",
        "test/model",
        Some(&CLI_TEST_CAMPAIGN),
        profile::EvalStorageBackend::DualStrict,
    )
    .expect("write turn-live bundle");

    let dir = broad_headless_tui_turn_live_dir(slot.published.submitted_result_path());
    let trace_path = dir.join("agent-turn-trace.json");
    let summary_path = dir.join("agent-turn-summary.json");
    let full_response_path = dir.join(ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE);
    assert!(trace_path.exists(), "missing {}", trace_path.display());
    assert!(summary_path.exists(), "missing {}", summary_path.display());
    assert!(
        full_response_path.exists(),
        "missing {}",
        full_response_path.display()
    );

    let trace: ploke_records::agent_turn::AgentTurnTraceRecord =
        serde_json::from_slice(&fs::read(&trace_path).expect("read turn-live trace"))
            .expect("decode turn-live trace");
    assert_eq!(trace.0.task_id, slot.published.request_id());
    assert_eq!(trace.0.selected_model, "test/model");
    assert_eq!(trace.0.issue_prompt, "diagnose the run");

    let db_path = eval_store::owner_eval_db_file_for_record_path(&trace_path)
        .expect("turn-live owner db path");
    assert!(db_path.is_file(), "missing {}", db_path.display());
    let db = eval_store::load_owner_eval_database(&db_path).expect("reload turn-live owner db");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "task_id".to_string(),
        cozo::DataValue::from(slot.published.request_id().to_string()),
    );
    let rows = db
        .raw_query_params(
            r#"
?[turn_id, campaign_id, task_id, selected_model] :=
    *eval_agent_turn { turn_id, campaign_id, task_id, selected_model },
    task_id = $task_id
"#,
            params,
        )
        .expect("query turn-live agent turn row");
    assert_eq!(rows.rows.len(), 1);
    let row = rows.row_refs().next().expect("turn-live row");
    assert_eq!(
        row.get::<String>("campaign_id").expect("campaign"),
        "campaign"
    );
    assert_eq!(
        row.get::<String>("selected_model").expect("model"),
        "test/model"
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
fn live_google_broad_headless_canary_base_dir_from_override(
    override_dir: Option<PathBuf>,
) -> PathBuf {
    override_dir.unwrap_or_else(|| {
        crate::layout::ploke_eval_home()
            .unwrap_or_else(|_| PathBuf::from(".ploke-eval"))
            .join("probes")
            .join("live-google-broad-headless")
    })
}

#[cfg(feature = "live_api_tests")]
#[test]
fn live_google_broad_headless_canary_default_root_is_durable() {
    let root = live_google_broad_headless_canary_base_dir_from_override(None);

    assert!(
        root.ends_with(Path::new("probes").join("live-google-broad-headless")),
        "default broad-headless Google preflight root should live under ploke-eval probes, got {}",
        root.display()
    );
    assert!(
        !root.starts_with(std::env::temp_dir()),
        "default broad-headless Google preflight root should not use /tmp: {}",
        root.display()
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

    let executor = run_broad_headless_tui_attempt_with_options(
        &slot,
        &options,
        None,
        profile::EvalStorageBackend::Fs,
    )
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
    let node = test_node(tmp.path(), "node-det", "branch-det", "candidate-det");
    let mut resolved = test_resolved(&node);
    resolved.branch.synthesized_spec_id = TUI_EDIT_SURFACE_PRODUCER_ID.to_string();
    let child = ChildFiles::from_resolved(&CLI_TEST_CAMPAIGN, node, resolved, false);

    let err = CandidateGenerationConfig::BroadHarnessRequest
        .validate_received_child_plan(&[child])
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
    let child = ChildFiles::from_resolved(&CLI_TEST_CAMPAIGN, node, resolved, false);

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
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
    let repo_root = tmp.path().join("repo");
    let changed_paths = {
        let allowed = write_broad_surface_targets(&repo_root);
        commit_indexed_repo(&repo_root, "broad surface fixture");
        vec![allowed[0].clone(), allowed[1].clone()]
    };
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        parent.identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
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

    {
        let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "campaign_id".to_string(),
            cozo::DataValue::from(CLI_TEST_CAMPAIGN.to_string()),
        );
        params.insert(
            "parent_node_id".to_string(),
            cozo::DataValue::from(child_plan.plan.body().parent_node_id().to_string()),
        );
        let plans = db
            .raw_query_params(
                r#"
?[plan_id, child_count, rejected_count, message_sha256] :=
    *eval_child_plan {
        plan_id,
        campaign_id,
        parent_node_id,
        child_count,
        rejected_count,
        message_sha256
    },
    campaign_id = $campaign_id,
    parent_node_id = $parent_node_id
"#,
                params,
            )
            .expect("query normalized broad-harness child plan");
        assert_eq!(plans.rows.len(), 1);
        let plan = plans.row_refs().next().expect("plan row");
        let plan_id = plan.get::<String>("plan_id").expect("plan id");
        assert_eq!(plan.get::<i64>("child_count").expect("child count"), 1);
        assert_eq!(
            plan.get::<i64>("rejected_count").expect("rejected count"),
            0
        );
        assert!(
            !plan
                .get::<String>("message_sha256")
                .expect("message hash")
                .is_empty()
        );

        let mut params = std::collections::BTreeMap::new();
        params.insert("plan_id".to_string(), cozo::DataValue::from(plan_id));
        let children = db
            .raw_query_params(
                r#"
?[child_node_id, child_index, status, target_relpath, harness_present, surface_present] :=
    *eval_child_plan_child {
        plan_id,
        child_node_id,
        child_index,
        status,
        target_relpath,
        harness_present,
        surface_present
    },
    plan_id = $plan_id
"#,
                params,
            )
            .expect("query normalized broad-harness child rows");
        assert_eq!(children.rows.len(), 1);
        let row = children.row_refs().next().expect("child row");
        assert_eq!(
            row.get::<String>("child_node_id").expect("child id"),
            child.node_id()
        );
        assert_eq!(row.get::<i64>("child_index").expect("index"), 0);
        assert_eq!(row.get::<String>("status").expect("status"), "planned");
        assert_eq!(
            row.get::<String>("target_relpath").expect("target"),
            child.node_record().target_relpath.display().to_string()
        );
        assert!(row.get::<bool>("harness_present").expect("harness"));
        assert!(
            !row.get::<bool>("surface_present").expect("surface"),
            "broad-harness children carry request-bound harness evidence, not deterministic surface evidence"
        );

        let refs = db
            .raw_query_params(
                r#"
?[count(record_ref_id)] :=
    *eval_record_ref { record_ref_id, family },
    family = "child_plan_file"
"#,
                std::collections::BTreeMap::new(),
            )
            .expect("query child-plan record refs");
        let count = refs
            .row_refs()
            .next()
            .expect("count row")
            .get::<i64>("count(record_ref_id)")
            .expect("count");
        assert_eq!(
            count, 0,
            "child plan should not use eval_record_ref payloads"
        );
    }

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
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "artifact_id".to_string(),
        cozo::DataValue::from(admitted_derived.to_string()),
    );
    let artifacts = db
        .raw_query_params(
            r#"
?[
    artifact_id,
    source,
    store_scope,
    created_by,
    parent_artifact_id,
    tree_hash
] :=
    *eval_artifact {
        artifact_id,
        source,
        store_scope,
        created_by,
        parent_artifact_id,
        tree_hash
    },
    artifact_id = $artifact_id
"#,
            params.clone(),
        )
        .expect("query artifact provenance");
    assert_eq!(artifacts.rows.len(), 1);
    let artifact_row = artifacts.row_refs().next().expect("artifact row");
    assert_eq!(
        artifact_row.get::<String>("source").expect("source"),
        "broad_harness"
    );
    assert_eq!(
        artifact_row
            .get::<String>("store_scope")
            .expect("store scope"),
        "parent"
    );
    assert_eq!(
        artifact_row
            .get::<String>("created_by")
            .expect("created by"),
        child.node_record().node_id
    );
    assert_eq!(
        artifact_row
            .get::<String>("parent_artifact_id")
            .expect("parent artifact"),
        evidence
            .artifact()
            .expect("artifact evidence")
            .base_artifact_id
            .to_string()
    );
    assert!(
        !artifact_row
            .get::<String>("tree_hash")
            .expect("tree hash")
            .is_empty()
    );
    let surfaces = db
        .raw_query_params(
            r#"
?[
    artifact_id,
    surface_hash,
    source_ref
] :=
    *eval_artifact_surface {
        artifact_id,
        surface_hash,
        source_ref
    },
    artifact_id = $artifact_id
"#,
            params.clone(),
        )
        .expect("query artifact surface");
    assert_eq!(surfaces.rows.len(), 1);
    let surface_row = surfaces.row_refs().next().expect("surface row");
    assert!(
        !surface_row
            .get::<String>("surface_hash")
            .expect("surface hash")
            .is_empty()
    );
    assert!(
        surface_row
            .get::<String>("source_ref")
            .expect("source ref")
            .contains(&candidate_root.display().to_string())
    );
    let refs = db
        .raw_query_params(
            r#"
?[
    artifact_id,
    kind,
    source_ref,
    content_sha256
] :=
    *eval_artifact_ref {
        artifact_id,
        kind,
        source_ref,
        content_sha256
    },
    artifact_id = $artifact_id
"#,
            params,
        )
        .expect("query artifact refs");
    assert_eq!(refs.rows.len(), 1);
    let ref_row = refs.row_refs().next().expect("artifact ref row");
    assert_eq!(
        ref_row.get::<String>("kind").expect("kind"),
        "broad_harness_child_artifact"
    );
    assert!(
        !ref_row
            .get::<String>("content_sha256")
            .expect("content hash")
            .is_empty()
    );

    let patch_id = child
        .node_record()
        .patch_id
        .as_ref()
        .expect("child patch id")
        .to_string();
    let apply_id = child
        .resolved()
        .branch
        .apply_id
        .as_ref()
        .expect("child apply id")
        .clone();
    let base_artifact = evidence
        .artifact()
        .expect("artifact evidence")
        .base_artifact_id
        .to_string();
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "patch_id".to_string(),
        cozo::DataValue::from(patch_id.clone()),
    );
    let operations = db
        .raw_query_params(
            r#"
?[
    operation_id,
    generator_id,
    target_kind,
    target_ref,
    procedure_id,
    output_artifact_id,
    output_patch_id
] :=
    *eval_operation {
        operation_id,
        generator_id,
        target_kind,
        target_ref,
        procedure_id,
        output_artifact_id,
        output_patch_id
    },
    output_patch_id = $patch_id
"#,
            params.clone(),
        )
        .expect("query operation provenance");
    assert_eq!(operations.rows.len(), 1);
    let operation_row = operations.row_refs().next().expect("operation row");
    assert!(
        !operation_row
            .get::<String>("operation_id")
            .expect("operation id")
            .is_empty()
    );
    assert_eq!(
        operation_row
            .get::<String>("generator_id")
            .expect("generator"),
        child.node_record().instance_id
    );
    assert_eq!(
        operation_row
            .get::<String>("target_kind")
            .expect("target kind"),
        "artifact"
    );
    assert_eq!(
        operation_row
            .get::<String>("target_ref")
            .expect("target ref"),
        base_artifact
    );
    assert_eq!(
        operation_row
            .get::<String>("procedure_id")
            .expect("procedure id"),
        child.resolved().branch.synthesized_spec_id
    );
    assert_eq!(
        operation_row
            .get::<String>("output_artifact_id")
            .expect("output artifact"),
        admitted_derived.to_string()
    );
    assert_eq!(
        operation_row
            .get::<String>("output_patch_id")
            .expect("output patch"),
        patch_id
    );

    let patches = db
        .raw_query_params(
            r#"
?[
    patch_id,
    base_artifact_id,
    creator_id,
    target_relpath,
    patch_ref,
    content_sha256,
    status
] :=
    *eval_patch {
        patch_id,
        base_artifact_id,
        creator_id,
        target_relpath,
        patch_ref,
        content_sha256,
        status
    },
    patch_id = $patch_id
"#,
            params.clone(),
        )
        .expect("query patch provenance");
    assert_eq!(patches.rows.len(), 1);
    let patch_row = patches.row_refs().next().expect("patch row");
    assert_eq!(
        patch_row
            .get::<String>("base_artifact_id")
            .expect("base artifact"),
        base_artifact
    );
    assert_eq!(
        patch_row.get::<String>("creator_id").expect("creator"),
        child.node_record().instance_id
    );
    assert_eq!(
        patch_row
            .get::<String>("target_relpath")
            .expect("target relpath"),
        child.resolved().target_relpath.display().to_string()
    );
    assert_eq!(
        patch_row.get::<String>("patch_ref").expect("patch ref"),
        apply_id
    );
    assert_eq!(
        patch_row
            .get::<String>("content_sha256")
            .expect("content hash"),
        eval_store::content_sha256(&child.resolved().branch.proposed_content)
    );
    assert_eq!(
        patch_row.get::<String>("status").expect("status"),
        "applied"
    );

    let mut params = std::collections::BTreeMap::new();
    params.insert("apply_id".to_string(), cozo::DataValue::from(apply_id));
    let applies = db
        .raw_query_params(
            r#"
?[
    apply_id,
    patch_id,
    runtime_id,
    artifact_id,
    outcome,
    output_artifact_id
] :=
    *eval_apply_event {
        apply_id,
        patch_id,
        runtime_id,
        artifact_id,
        outcome,
        output_artifact_id
    },
    apply_id = $apply_id
"#,
            params,
        )
        .expect("query apply event");
    assert_eq!(applies.rows.len(), 1);
    let apply_row = applies.row_refs().next().expect("apply event row");
    assert_eq!(
        apply_row.get::<String>("patch_id").expect("patch"),
        patch_id
    );
    assert_eq!(
        apply_row.get::<String>("runtime_id").expect("runtime"),
        child.node_record().instance_id
    );
    assert_eq!(
        apply_row
            .get::<String>("artifact_id")
            .expect("input artifact"),
        base_artifact
    );
    assert_eq!(
        apply_row.get::<String>("outcome").expect("outcome"),
        "applied"
    );
    assert_eq!(
        apply_row
            .get::<String>("output_artifact_id")
            .expect("output artifact"),
        admitted_derived.to_string()
    );
}

#[test]
fn broad_harness_eval_store_rows_do_not_replace_missing_candidate_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let changed_paths = {
        let allowed = write_broad_surface_targets(&repo_root);
        commit_indexed_repo(&repo_root, "broad surface fixture");
        vec![allowed[0].clone(), allowed[1].clone()]
    };
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        parent.identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
    let base_artifact = admitted.base_artifact_id().to_string();
    let admitted_derived = admitted.derived_artifact_id().clone();
    let surface_hash =
        eval_store::artifact_surface_hash(admitted.artifact_surface()).expect("surface hash");

    let child_plan = publish_broad_harness_child_plan_from_admitted(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        receipt,
        admitted,
    )
    .expect("multi-file admitted transaction should mint one child artifact");
    let child = &child_plan.plan.body().children()[0];
    let evidence = child.harness_evidence().expect("harness evidence");
    let patch_id = child
        .node_record()
        .patch_id
        .as_ref()
        .expect("child patch id")
        .to_string();
    let apply_id = child
        .resolved()
        .branch
        .apply_id
        .as_ref()
        .expect("child apply id")
        .clone();
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    eval_store::write_artifact_provenance_to_owner_db(
        &db_path,
        eval_store::ArtifactProvenanceEvidence {
            artifact: eval_store::ArtifactEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                artifact_id: admitted_derived.to_string(),
                tree_hash: Some(format!("{:?}", evidence.artifact_surface().tree_key())),
                git_branch: None,
                git_commit: None,
                source: "broad_harness".to_string(),
                store_scope: "parent".to_string(),
                created_by: Some(child.node_record().node_id.clone()),
                parent_artifact_id: Some(base_artifact.clone()),
            },
            surface: Some(eval_store::ArtifactSurfaceEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                artifact_id: admitted_derived.to_string(),
                immutable_root: None,
                mutated_root: None,
                ambient_root: None,
                surface_hash: Some(surface_hash.clone()),
                source_ref: Some(format!(
                    "broad_harness:workspace:{}",
                    candidate_root.display()
                )),
                recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
            }),
            refs: vec![eval_store::ArtifactRefEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                artifact_id: Some(admitted_derived.to_string()),
                kind: "broad_harness_child_artifact".to_string(),
                source_ref: format!("broad_harness:workspace:{}", candidate_root.display()),
                content_sha256: Some(surface_hash),
                recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
            }],
        },
    )
    .expect("seed artifact provenance rows");
    eval_store::write_operation_provenance_to_owner_db(
        &db_path,
        eval_store::OperationProvenanceEvidence {
            operation: eval_store::OperationEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                generator_id: child.node_record().instance_id.clone(),
                target_kind: "artifact".to_string(),
                target_ref: base_artifact.clone(),
                procedure_id: Some(child.resolved().branch.synthesized_spec_id.clone()),
                output_artifact_id: Some(admitted_derived.to_string()),
                output_patch_id: Some(patch_id.clone()),
                recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
            },
            patch: eval_store::PatchEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                patch_id: patch_id.clone(),
                base_artifact_id: Some(base_artifact),
                creator_id: Some(child.node_record().instance_id.clone()),
                tool_call_id: None,
                target_relpath: Some(child.resolved().target_relpath.display().to_string()),
                patch_ref: Some(apply_id.clone()),
                content_sha256: Some(eval_store::content_sha256(
                    &child.resolved().branch.proposed_content,
                )),
                status: Some("applied".to_string()),
            },
            apply_event: eval_store::ApplyEventEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                apply_id: apply_id.clone(),
                patch_id: patch_id.clone(),
                runtime_id: Some(child.node_record().instance_id.clone()),
                artifact_id: child
                    .node_record()
                    .base_artifact_id
                    .as_ref()
                    .map(|id| id.to_string()),
                outcome: "applied".to_string(),
                output_artifact_id: Some(admitted_derived.to_string()),
                recorded_at: "2026-06-23T00:00:00Z".to_string(),
            },
        },
    )
    .expect("seed operation provenance rows");
    fs::remove_dir_all(&candidate_root).expect("remove candidate workspace");

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
        .expect_err("DB artifact rows cannot replace the candidate workspace");

    match err {
        CommitError::Transition(MaterializeBranchError::HarnessWorkspaceMissing {
            node_id,
            path,
        }) => {
            assert_eq!(node_id, child.node_record().node_id);
            assert_eq!(path, candidate_root);
        }
        other => panic!("unexpected materialization error: {other:?}"),
    }
    let entries = journal.load_entries().expect("journal entries");
    assert!(
        entries.is_empty(),
        "workspace authority failure must occur before C1 journal commits"
    );
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "artifact_id".to_string(),
        cozo::DataValue::from(admitted_derived.to_string()),
    );
    let rows = db
        .raw_query_params(
            r#"
?[artifact_id] :=
    *eval_artifact { artifact_id },
    artifact_id = $artifact_id
"#,
            params,
        )
        .expect("query seeded artifact row");
    assert_eq!(rows.rows.len(), 1);
    let mut params = std::collections::BTreeMap::new();
    params.insert("apply_id".to_string(), cozo::DataValue::from(apply_id));
    let rows = db
        .raw_query_params(
            r#"
?[apply_id, patch_id] :=
    *eval_apply_event { apply_id, patch_id },
    apply_id = $apply_id
"#,
            params,
        )
        .expect("query seeded apply event row");
    assert_eq!(rows.rows.len(), 1);
}

#[test]
fn broad_harness_child_plan_skips_missing_source_admitted_result_when_min_remains() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let budget = Prototype1ChildBudget::new(2, 3);
    let broad_tui = profile::BroadTui {
        fresh_slots_per_child: Some(1),
        ..profile::BroadTui::default()
    };
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        broad_tui,
    )
    .expect("publish broad harness batch");
    assert_eq!(batch.slots.len(), 3);

    let first =
        admit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "one");
    let missing_source = PathBuf::from("crates/ploke-selection-score/tests/raser.rs");
    let bad = admit_broad_slot_for_test(
        &repo_root,
        &batch.slots[1],
        &[missing_source.clone()],
        "bad",
    );
    let third =
        admit_broad_slot_for_test(&repo_root, &batch.slots[2], &[allowed[1].clone()], "three");

    let receipt = publish_broad_harness_child_plan_from_admitted_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui,
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
        vec![first, bad, third],
    )
    .expect("missing-source candidate should be rejected while enough children remain");

    let children = receipt.plan.body().children();
    assert_eq!(children.len(), 2);
    assert!(
        children
            .iter()
            .all(|child| child.node_record().target_relpath != missing_source)
    );
    assert!(
        receipt.rejected_surface_attempts.iter().any(|attempt| {
            matches!(
                &attempt.outcome,
                surface_attempt::Outcome::Rejected { reason }
                    if reason.contains("could not read source file")
                        && reason.contains("raser.rs")
            )
        }),
        "missing-source admission should be retained as rejected evidence: {:?}",
        receipt.rejected_surface_attempts
    );
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
    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &parent_identity,
        Prototype1ChildBudget::new(1, 1),
        admission_binding.clone(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
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
    let _env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "9".into())]);
    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(3, 3).with_parallel_targets(2);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
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

    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("three admitted broad transactions should seal one three-child plan");

    let children = receipt.plan.body().children();
    assert_eq!(children.len(), 3);
    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let bytes = fs::read(files.message_at().path()).expect("published child plan should persist");
    let body: ChildPlanFiles = serde_json::from_slice(&bytes).expect("decode child plan");
    assert_eq!(body.parent_node_id(), parent_identity.node_id());
    assert_eq!(body.child_generation(), parent_identity.generation() + 1);
    assert_eq!(body.children().len(), 3);
    assert!(body.rejected_surface_attempts().is_empty());
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
    let batch: HarnessRequestBatch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    assert_eq!(batch.patch_generation_parallel_cap, 2);
    assert_eq!(
        batch.slots.len(),
        2,
        "test-scoped slot limit keeps this fanout proof focused"
    );
    assert_eq!(count_broad_requests(&manifest_path), 2);

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await;
    let err = match result {
        Ok(_) => panic!("fixture-backed parallel slots should not produce runnable children"),
        Err(err) => err,
    };
    let PrepareError::ChildPlanBelowMinimum {
        runnable_children,
        required_min,
        attempted_slots,
        accepted_results,
        ..
    } = err
    else {
        panic!("unexpected error: {err:?}");
    };
    assert_eq!(runnable_children, 0);
    assert_eq!(required_min, 2);
    assert_eq!(attempted_slots, 2);
    assert_eq!(accepted_results, 0);

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

#[tokio::test(flavor = "current_thread")]
async fn provider_unavailable_after_partial_admissions_persists_failed_child_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let provider_fixture = tmp.path().join("provider-unavailable.headless-tui.json");
    write_json_file_pretty(
        &provider_fixture,
        &serde_json::json!({
            "attempts": [],
            "terminal": {
                "terminal": "provider_unavailable",
                "reason": "test provider unavailable"
            }
        }),
    )
    .expect("write provider fixture");
    let _env = crate::test_support::env_guard_os(vec![
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "5".into()),
        (
            "PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE",
            provider_fixture.into_os_string(),
        ),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(5, 5).with_parallel_targets(1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    assert_eq!(batch.slots.len(), 5);

    for (index, slot) in batch.slots.iter().take(2).enumerate() {
        submit_broad_slot_for_test(
            &repo_root,
            slot,
            &[allowed[index].clone()],
            &format!("slot-{index}"),
        );
    }

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await;
    let err = match result {
        Ok(_) => panic!("provider failure should still block child planning"),
        Err(err) => err,
    };
    let PrepareError::ProviderUnavailable { detail, .. } = err else {
        panic!("unexpected error variant: {err:?}");
    };
    assert!(detail.contains("test provider unavailable"));

    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let plan_path = files.message_at().path().to_path_buf();
    let bytes = fs::read(&plan_path).expect("failed child-plan should be persisted");
    let body: ChildPlanFiles = serde_json::from_slice(&bytes).expect("decode child plan");
    assert!(body.children().is_empty());
    let attempts = body.rejected_surface_attempts();
    assert_eq!(attempts.len(), 3, "{attempts:#?}");
    let reasons = attempts
        .iter()
        .filter_map(|attempt| match &attempt.outcome {
            surface_attempt::Outcome::Rejected { reason } => Some(reason.as_str()),
            surface_attempt::Outcome::Applied => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reasons.len(), 3);
    assert!(
        reasons
            .iter()
            .filter(|reason| reason.contains("was admitted, but was not materialized as a child"))
            .count()
            >= 2,
        "{reasons:#?}"
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("provider unavailable")),
        "{reasons:#?}"
    );

    let request_count_before = count_broad_requests(&manifest_path);
    let resumed_parent = ready_parent_for_test(&manifest_path, &repo_root);
    let receipt = receive_existing_child_plan(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        resumed_parent,
    )
    .expect("failed child plan should be reusable");
    assert!(receipt.plan.body().children().is_empty());
    assert_eq!(receipt.rejected_surface_attempts.len(), 3);
    assert_eq!(
        count_broad_requests(&manifest_path),
        request_count_before,
        "retry should reuse persisted failed child plan instead of minting fresh slots"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn provider_unavailable_after_min_admitted_returns_published_plan() {
    // At or above the child minimum, a later provider-unavailable slot does not
    // block the batch: it publishes the admitted children and drops the fatal
    // error (logging it). Uses a non-direct-google route to show the
    // resumable carve-out only applies below the minimum.
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let provider_fixture = tmp.path().join("provider-unavailable.headless-tui.json");
    write_json_file_pretty(
        &provider_fixture,
        &serde_json::json!({
            "attempts": [],
            "terminal": {
                "terminal": "provider_unavailable",
                "reason": "test provider unavailable"
            }
        }),
    )
    .expect("write provider fixture");
    let _env = crate::test_support::env_guard_os(vec![
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "5".into()),
        (
            "PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE",
            provider_fixture.into_os_string(),
        ),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(2, 5).with_parallel_targets(1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    assert_eq!(batch.slots.len(), 5);

    for (index, slot) in batch.slots.iter().take(2).enumerate() {
        submit_broad_slot_for_test(
            &repo_root,
            slot,
            &[allowed[index].clone()],
            &format!("slot-{index}"),
        );
    }

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::OpenRouter,
        },
        batch,
    )
    .await;
    let receipt = result.expect("batch at/above minimum should publish children");
    assert_eq!(
        receipt.plan.body().children().len(),
        2,
        "two admitted slots should materialize as children"
    );

    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let plan_path = files.message_at().path().to_path_buf();
    let bytes = fs::read(&plan_path).expect("published child plan should persist");
    let body: ChildPlanFiles = serde_json::from_slice(&bytes).expect("decode child plan");
    assert_eq!(body.children().len(), 2);
    assert!(
        body.rejected_surface_attempts().iter().any(|attempt| {
            matches!(
                &attempt.outcome,
                surface_attempt::Outcome::Rejected { reason }
                    if reason.contains("provider unavailable")
            )
        }),
        "published child plan should retain evidence for the dropped provider error: {:?}",
        body.rejected_surface_attempts()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn database_setup_fatal_after_min_admitted_returns_published_plan() {
    // DatabaseSetup is not subject to the provider-unavailable resumable
    // carve-out. At or above the minimum it still publishes children and drops
    // the fatal error (logging it).
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let _env = crate::test_support::env_guard_os(vec![
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "5".into()),
        (
            "PLOKE_EVAL_BROAD_TUI_DATABASE_SETUP_FIXTURE",
            "test database setup unavailable".into(),
        ),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(2, 5).with_parallel_targets(1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    assert_eq!(batch.slots.len(), 5);

    for (index, slot) in batch.slots.iter().take(2).enumerate() {
        submit_broad_slot_for_test(
            &repo_root,
            slot,
            &[allowed[index].clone()],
            &format!("slot-{index}"),
        );
    }

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::OpenRouter,
        },
        batch,
    )
    .await;
    let receipt = result.expect("database-setup fatal at/above minimum should publish children");
    assert_eq!(receipt.plan.body().children().len(), 2);

    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let plan_path = files.message_at().path().to_path_buf();
    let bytes = fs::read(&plan_path).expect("published child plan should persist");
    let body: ChildPlanFiles = serde_json::from_slice(&bytes).expect("decode child plan");
    assert_eq!(body.children().len(), 2);
    assert!(
        body.rejected_surface_attempts().iter().any(|attempt| {
            matches!(
                &attempt.outcome,
                surface_attempt::Outcome::Rejected { reason }
                    if reason.contains("database setup unavailable")
            )
        }),
        "published child plan should retain evidence for the dropped DatabaseSetup error: {:?}",
        body.rejected_surface_attempts()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn provider_unavailable_with_parallel_slots_aborts_without_corrupting_plan() {
    // cap > 1: the first provider-unavailable slot triggers abort_all over the
    // other in-flight slot. The persisted plan must stay deterministic (zero
    // children, no partial/orphaned submitted result) and record only the
    // observed slot as parent-readable evidence.
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let probe_dir = tmp.path().join("slot-probe");
    let provider_fixture = tmp.path().join("provider-unavailable.headless-tui.json");
    write_json_file_pretty(
        &provider_fixture,
        &serde_json::json!({
            "attempts": [],
            "terminal": {
                "terminal": "provider_unavailable",
                "reason": "test provider unavailable"
            }
        }),
    )
    .expect("write provider fixture");
    let _env = crate::test_support::env_guard_os(vec![
        (
            "PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE",
            provider_fixture.into_os_string(),
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
    let budget = Prototype1ChildBudget::new(2, 2).with_parallel_targets(2);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    assert_eq!(batch.patch_generation_parallel_cap, 2);
    assert_eq!(batch.slots.len(), 2);

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await;
    let err = match result {
        Ok(_) => panic!("provider failure should block child planning"),
        Err(err) => err,
    };
    assert!(
        matches!(err, PrepareError::ProviderUnavailable { .. }),
        "unexpected error variant: {err:?}"
    );

    for slot_index in [0, 1] {
        assert!(
            probe_dir.join(format!("start-{slot_index}")).exists(),
            "slot {slot_index} should have started concurrently before the abort"
        );
    }

    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let plan_path = files.message_at().path().to_path_buf();
    let bytes = fs::read(&plan_path).expect("failed child-plan should be persisted");
    let body: ChildPlanFiles = serde_json::from_slice(&bytes).expect("decode child plan");
    assert!(
        body.children().is_empty(),
        "aborted parallel batch must not materialize any children"
    );
    assert_eq!(
        body.rejected_surface_attempts().len(),
        1,
        "only the observed (non-aborted) slot should appear as parent-readable evidence: {:#?}",
        body.rejected_surface_attempts()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn provider_unavailable_with_google_direct_permanently_fails_parent() {
    // Direct-google + below minimum: keep the terminal-failure contract. The
    // parent is projected to Failed, a reuse-terminal child-plan is persisted,
    // and a later resume reuses it WITHOUT minting fresh broad requests.
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let provider_fixture = tmp.path().join("provider-unavailable.headless-tui.json");
    write_json_file_pretty(
        &provider_fixture,
        &serde_json::json!({
            "attempts": [],
            "terminal": {
                "terminal": "provider_unavailable",
                "reason": "test provider unavailable"
            }
        }),
    )
    .expect("write provider fixture");
    let _env = crate::test_support::env_guard_os(vec![
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "5".into()),
        (
            "PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE",
            provider_fixture.into_os_string(),
        ),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(3, 5).with_parallel_targets(1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");

    for (index, slot) in batch.slots.iter().take(2).enumerate() {
        submit_broad_slot_for_test(
            &repo_root,
            slot,
            &[allowed[index].clone()],
            &format!("slot-{index}"),
        );
    }

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await;
    let err = match result {
        Ok(_) => panic!("below-minimum direct-google provider failure should fail the parent"),
        Err(err) => err,
    };
    assert!(
        matches!(err, PrepareError::ProviderUnavailable { .. }),
        "unexpected error variant: {err:?}"
    );

    let node_record = load_test_node_record(&manifest_path, parent_identity.node_id());
    assert_eq!(
        node_record.status,
        Prototype1NodeStatus::Failed,
        "direct-google below-minimum provider failure should project the parent to Failed"
    );

    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let plan_path = files.message_at().path().to_path_buf();
    assert!(
        plan_path.exists(),
        "a reuse-terminal failed child-plan should be persisted"
    );

    let request_count_before = count_broad_requests(&manifest_path);
    let resumed_parent = ready_parent_for_test(&manifest_path, &repo_root);
    let receipt = receive_existing_child_plan(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        resumed_parent,
    )
    .expect("failed child plan should be reusable");
    assert!(receipt.plan.body().children().is_empty());
    assert_eq!(
        count_broad_requests(&manifest_path),
        request_count_before,
        "resume should reuse the persisted failed plan instead of minting fresh slots"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn provider_unavailable_without_google_direct_keeps_parent_resumable() {
    // Non-direct-google + below minimum: surface the provider blocker but keep
    // the parent resumable. No reuse-terminal plan is persisted, the parent is
    // NOT projected to Failed, already-observed slot evidence is preserved on
    // disk, and a later resume re-mints fresh broad requests.
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let provider_fixture = tmp.path().join("provider-unavailable.headless-tui.json");
    write_json_file_pretty(
        &provider_fixture,
        &serde_json::json!({
            "attempts": [],
            "terminal": {
                "terminal": "provider_unavailable",
                "reason": "test provider unavailable"
            }
        }),
    )
    .expect("write provider fixture");
    let _env = crate::test_support::env_guard_os(vec![
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "5".into()),
        (
            "PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE",
            provider_fixture.into_os_string(),
        ),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(3, 5).with_parallel_targets(1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");

    let mut submitted_result_paths = Vec::new();
    for (index, slot) in batch.slots.iter().take(2).enumerate() {
        submit_broad_slot_for_test(
            &repo_root,
            slot,
            &[allowed[index].clone()],
            &format!("slot-{index}"),
        );
        submitted_result_paths.push(slot.published.submitted_result_path().to_path_buf());
    }

    let result = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::OpenRouter,
        },
        batch,
    )
    .await;
    let err = match result {
        Ok(_) => panic!("provider failure should surface even on a resumable route"),
        Err(err) => err,
    };
    assert!(
        matches!(err, PrepareError::ProviderUnavailable { .. }),
        "unexpected error variant: {err:?}"
    );

    let node_record = load_test_node_record(&manifest_path, parent_identity.node_id());
    assert_ne!(
        node_record.status,
        Prototype1NodeStatus::Failed,
        "non-direct-google provider failure must not permanently fail the parent"
    );

    let files = ChildPlanFiles::for_parent(&manifest_path, &parent_identity, Vec::new());
    let plan_path = files.message_at().path().to_path_buf();
    assert!(
        !plan_path.exists(),
        "resumable path must not persist a reuse-terminal child-plan"
    );

    // Already-observed slot evidence (submitted results) must not be erased.
    for path in &submitted_result_paths {
        assert!(
            path.exists(),
            "admitted slot submitted result should be preserved on the resumable path: {}",
            path.display()
        );
    }

    let request_count_before = count_broad_requests(&manifest_path);
    let resumed_parent = ready_parent_for_test(&manifest_path, &repo_root);
    let resumed_batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        resumed_parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("resume should re-publish a fresh broad batch");
    assert!(!resumed_batch.slots.is_empty());
    assert!(
        count_broad_requests(&manifest_path) > request_count_before,
        "resume should re-mint fresh broad requests because no reuse-terminal plan blocks it"
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
        ("PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT", "9".into()),
    ]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(3, 3).with_parallel_targets(2);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
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
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("three admitted children");
    let children = receipt.plan.body().children().to_vec();
    assert_eq!(children.len(), 3);

    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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
        &CLI_TEST_CAMPAIGN,
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

fn local_node(mut node: Prototype1NodeRecord, manifest_path: &Path) -> Prototype1NodeRecord {
    let prototype_root = manifest_path
        .parent()
        .expect("test manifest has parent")
        .join("prototype1");
    node.node_dir = prototype_root.join("nodes").join(&node.node_id);
    node.workspace_root = prototype_root
        .join("workspaces/edit-harness")
        .join(node.parent_node_id.as_deref().unwrap_or("parent"));
    node.binary_path = node.node_dir.join("bin/ploke-eval");
    node.runner_request_path = node.node_dir.join("runner-request.json");
    node.runner_result_path = node.node_dir.join("runner-result.json");
    node
}

#[tokio::test]
async fn child_build_promotes_binary_and_cleans_scratch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
    let repo_root = tmp.path().join("repo");
    let fake_bin = tmp.path().join("fake-bin");
    let path = install_fake_cargo(&fake_bin, "#!/bin/sh\nexit 0\n");
    let _env = crate::test_support::env_guard_os(vec![("PATH", path)]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(1, 1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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
        CampaignId::from("campaign"),
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

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "node_id".to_string(),
        cozo::DataValue::from(node.node_id.clone()),
    );
    let builds = db
        .raw_query_params(
            r#"
?[
    build_id,
    node_id,
    artifact_id,
    phase,
    outcome,
    binary_ref
] :=
    *eval_build_event {
        build_id,
        node_id,
        artifact_id,
        phase,
        outcome,
        binary_ref
    },
    node_id = $node_id
"#,
            params,
        )
        .expect("query build event");
    assert_eq!(builds.rows.len(), 1);
    let build_row = builds.row_refs().next().expect("build row");
    assert_eq!(build_row.get::<String>("phase").expect("phase"), "promote");
    assert_eq!(
        build_row.get::<String>("outcome").expect("outcome"),
        "built"
    );
    assert_eq!(
        build_row.get::<String>("artifact_id").expect("artifact id"),
        node.derived_artifact_id
            .expect("derived artifact")
            .to_string()
    );
    let binary_ref_id = build_row
        .get::<String>("binary_ref")
        .expect("binary ref id");
    assert!(!binary_ref_id.is_empty());

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "binary_ref_id".to_string(),
        cozo::DataValue::from(binary_ref_id),
    );
    let binaries = db
        .raw_query_params(
            r#"
?[
    binary_ref_id,
    artifact_id,
    source_ref,
    content_sha256
] :=
    *eval_binary_ref {
        binary_ref_id,
        artifact_id,
        source_ref,
        content_sha256
    },
    binary_ref_id = $binary_ref_id
"#,
            params,
        )
        .expect("query binary ref");
    assert_eq!(binaries.rows.len(), 1);
    let binary_row = binaries.row_refs().next().expect("binary row");
    assert_eq!(
        binary_row.get::<String>("source_ref").expect("source ref"),
        outcome.binary_path.display().to_string()
    );
    assert!(
        !binary_row
            .get::<String>("content_sha256")
            .expect("binary hash")
            .is_empty()
    );
}

#[tokio::test]
async fn binary_ref_rows_do_not_replace_missing_promoted_binary() {
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
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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
        CampaignId::from("campaign"),
        manifest_path.clone(),
        repo_root,
        prototype1_transition_journal_path(&manifest_path),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Build,
        Duration::from_secs(30),
        0,
        child.clone(),
    )
    .expect("build child with fake cargo");
    assert!(outcome.binary_path.is_file());
    let binary_hash = eval_store::file_sha256(&outcome.binary_path).expect("binary hash");
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    eval_store::write_build_provenance_to_owner_db(
        &db_path,
        eval_store::BuildProvenanceEvidence {
            binary_ref: eval_store::BinaryRefEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                artifact_id: child
                    .node_record()
                    .derived_artifact_id
                    .as_ref()
                    .map(|id| id.to_string()),
                built_by: None,
                source_ref: outcome.binary_path.display().to_string(),
                content_sha256: Some(binary_hash.clone()),
                protocol_digest: None,
                recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
            },
            build_event: eval_store::BuildEventEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                node_id: child.node_record().node_id.clone(),
                runtime_id: None,
                artifact_id: child
                    .node_record()
                    .derived_artifact_id
                    .as_ref()
                    .map(|id| id.to_string()),
                phase: "promote".to_string(),
                outcome: "built".to_string(),
                binary_ref: None,
                log_ref: None,
                recorded_at: "2026-06-23T00:00:00Z".to_string(),
            },
        },
    )
    .expect("seed binary provenance rows");
    eval_store::write_build_provenance_to_owner_db(
        &db_path,
        eval_store::BuildProvenanceEvidence {
            binary_ref: eval_store::BinaryRefEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                artifact_id: child
                    .node_record()
                    .derived_artifact_id
                    .as_ref()
                    .map(|id| id.to_string()),
                built_by: Some("runtime-seeded-spawn".to_string()),
                source_ref: outcome.binary_path.display().to_string(),
                content_sha256: Some(binary_hash),
                protocol_digest: None,
                recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
            },
            build_event: eval_store::BuildEventEvidence {
                campaign_id: CLI_TEST_CAMPAIGN.clone(),
                node_id: child.node_record().node_id.clone(),
                runtime_id: Some("runtime-seeded-spawn".to_string()),
                artifact_id: child
                    .node_record()
                    .derived_artifact_id
                    .as_ref()
                    .map(|id| id.to_string()),
                phase: "spawn".to_string(),
                outcome: "acknowledged".to_string(),
                binary_ref: None,
                log_ref: None,
                recorded_at: "2026-06-23T00:00:00Z".to_string(),
            },
        },
    )
    .expect("seed spawn binary provenance rows");
    fs::remove_file(&outcome.binary_path).expect("remove promoted binary");

    let stored = load_test_node_record(&manifest_path, child.node_id());
    let c3: crate::cli::prototype1_state::c2::C3 = Prototype {
        campaign_id: CLI_TEST_CAMPAIGN.clone(),
        campaign_manifest_path: manifest_path.clone(),
        node: stored.clone(),
        request: child.runner_request().clone(),
        resolved: child.resolved().clone(),
        artifact: Artifact {
            repo_root: stored.workspace_root.clone(),
            target_relpath: child.resolved().target_relpath.clone(),
            source_content_hash: ContentHash(child.resolved().source_content_hash.clone()),
            current_content_hash: ContentHash(
                child.resolved().branch.proposed_content_hash.clone(),
            ),
            proposed_content_hash: ContentHash(
                child.resolved().branch.proposed_content_hash.clone(),
            ),
            _lineage: std::marker::PhantomData::<ChildLineage>,
        },
        binary: Binary {
            parent_running: true,
            child_path: stored.binary_path.clone(),
            child_runtime: None,
            _lineage: std::marker::PhantomData::<ParentLineage>,
            _child: std::marker::PhantomData::<Present>,
            _ack: std::marker::PhantomData::<crate::cli::prototype1_state::c1::Unacknowledged>,
        },
    };
    let mut journal = PrototypeJournal::new(tmp.path().join("spawn-negative-journal.jsonl"));
    let err = SpawnChild::new()
        .transition(c3, &mut journal)
        .expect_err("DB binary refs cannot replace the promoted binary file");

    match err {
        CommitError::Transition(SpawnChildError::MissingChildBinary { path }) => {
            assert_eq!(path, outcome.binary_path);
        }
        other => panic!("unexpected spawn error: {other:?}"),
    }
    assert!(
        journal.load_entries().expect("spawn journal").is_empty(),
        "missing binary must fail before spawn journal entries or invocation bootstrap writes"
    );
}

#[tokio::test]
async fn historical_late_child_result_blocks_direct_reentry() {
    const NODE_ID: &str = "node-7809adc3fc6e4aad";
    const RUNTIME_ID: &str = "a7691248-60cb-419f-9a3a-52670fcb7e08";
    const BRANCH_ID: &str = "branch-3e52c562999775da";

    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let child_plan: ChildPlanFiles = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-late-child-result/child-plan-node-0dae679bb16a4604.json"
    ));
    let (plan_index, child) = child_plan
        .children()
        .iter()
        .enumerate()
        .find(|(_, child)| child.node_id() == NODE_ID)
        .expect("historical late child plan entry");
    let recorded_node: Prototype1NodeRecord = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-late-child-result/node.json"
    ));
    let runner_result: Prototype1RunnerResult = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-late-child-result/runner-result.json"
    ));

    assert_eq!(child.node_record().node_id, NODE_ID);
    assert_eq!(child.node_record().branch_id, BRANCH_ID);
    assert_eq!(recorded_node.node_id, NODE_ID);
    assert_eq!(recorded_node.status, Prototype1NodeStatus::BinaryBuilt);
    assert_eq!(runner_result.node_id, NODE_ID);
    assert_eq!(runner_result.status, Prototype1NodeStatus::Succeeded);

    let terminal =
        include_str!(
            "../../../tests/fixtures/prototype1-late-child-result/child-to-parent-a7691248-60cb-419f-9a3a-52670fcb7e08.jsonl"
        )
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<
                crate::cli::prototype1_state::channel::Envelope<
                    crate::cli::prototype1_state::channel::ToParent,
                >,
            >(line)
            .expect("historical late-child channel envelope")
        })
        .find_map(|envelope| match envelope.body() {
            crate::cli::prototype1_state::channel::ToParent::Result {
                runner_result,
                treatment,
            } => Some((
                envelope.runtime_id().to_string(),
                runner_result.clone(),
                treatment.clone(),
            )),
            _ => None,
        })
        .expect("historical late-child terminal channel result");
    assert_eq!(terminal.0, RUNTIME_ID);
    assert_eq!(terminal.1, runner_result);
    assert!(
        terminal.2.is_some(),
        "successful late child result carried treatment evidence"
    );

    let mut stored = local_node(recorded_node, &manifest_path);
    write_node_projection(&stored).expect("write historical contaminated node projection");
    crate::intervention::write_runner_result_at(&stored.runner_result_path, &runner_result)
        .expect("write historical terminal runner result");
    let runtime_result_path = stored
        .node_dir
        .join("results")
        .join(format!("{RUNTIME_ID}.json"));
    fs::create_dir_all(runtime_result_path.parent().expect("runtime result parent"))
        .expect("create runtime result dir");
    fs::write(
        &runtime_result_path,
        include_str!(
            "../../../tests/fixtures/prototype1-late-child-result/runtime-result-a7691248-60cb-419f-9a3a-52670fcb7e08.json"
        ),
    )
    .expect("write historical per-runtime result");

    let runtime_id = crate::loop_graph::RuntimeId::from_str(RUNTIME_ID).expect("runtime id");
    let mut observe_before = None;
    let mut parent_identity = None;
    for line in include_str!(
        "../../../tests/fixtures/prototype1-late-child-result/transition-journal.jsonl"
    )
    .lines()
    .filter(|line| !line.trim().is_empty())
    {
        let entry: JournalEntry = serde_json::from_str(line).expect("historical journal entry");
        match entry {
            JournalEntry::ParentStarted(entry)
                if entry.parent_identity.node_id() == child_plan.parent_node_id() =>
            {
                parent_identity.get_or_insert(entry.parent_identity);
            }
            JournalEntry::ObserveChild(mut entry)
                if entry.refs.node_id == NODE_ID
                    && entry.runtime_id == runtime_id
                    && entry.phase == CommitPhase::Before =>
            {
                entry.runner_result_path = runtime_result_path.clone();
                observe_before = Some(entry);
            }
            _ => {}
        }
    }
    let mut journal = PrototypeJournal::new(journal_path.clone());
    journal
        .append(JournalEntry::ObserveChild(
            observe_before.expect("historical observe_child before entry"),
        ))
        .expect("append historical observe replay entry");
    let replay = journal
        .replay_observe_child_at(
            crate::cli::prototype1_state::event::RecordedAt(1_781_028_186_753),
            Duration::from_secs(1),
        )
        .expect("replay historical observe child");
    assert_eq!(replay.len(), 1);
    assert!(matches!(
        &replay[0].outcome,
        crate::cli::prototype1_state::journal::CompletionOutcome::Pending {
            disposition:
                crate::cli::prototype1_state::journal::PendingCompletion::TerminalResultWrittenUnobserved(
                    crate::cli::prototype1_state::event::ObservedChildTerminal::Succeeded
                ),
        }
    ));
    assert!(
        !prototype1_branch_evaluation_path(&manifest_path, BRANCH_ID).exists(),
        "historical late child has no selection-grade branch evaluation"
    );

    let parent_identity = parent_identity.expect("historical parent identity");
    let baseline = CompleteBaseline::complete(
        parent_identity.campaign_id().clone(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: parent_identity
                .instance_id()
                .expect("historical parent instance")
                .to_string(),
            registration_path: None,
            record_path: tmp.path().join("baseline-record.json.gz"),
            metrics: test_metrics(false, true, 0),
        }],
    )
    .expect("complete baseline");

    let err = run_planned_child(
        parent_identity.campaign_id().clone(),
        manifest_path.clone(),
        tmp.path().join("repo"),
        journal_path.clone(),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Build,
        Duration::from_secs(30),
        plan_index,
        child.clone(),
    )
    .expect_err("historical late child direct re-entry must not rebuild");

    match err {
        PrepareError::InvalidBatchSelection { detail } => {
            assert!(detail.contains("terminal runner result"));
            assert!(detail.contains("repair terminal node state"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    stored = load_node_record(
        &manifest_path,
        NODE_ID,
        OperatorProjectionRead::cli_operator(),
    )
    .expect("reload historical late child");
    assert_eq!(stored.status, Prototype1NodeStatus::BinaryBuilt);
    assert_eq!(stored.updated_at, "2026-06-09T18:03:04.433855206+00:00");
    assert!(
        !stored.node_dir.join("target").exists(),
        "blocked historical re-entry must not create a child build target"
    );
    assert_eq!(
        PrototypeJournal::new(journal_path)
            .load_entries()
            .expect("reload historical replay journal")
            .len(),
        1,
        "blocked re-entry must not append materialize/build records"
    );
}

#[tokio::test]
async fn terminal_child_blocks_reentry() {
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
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let mut stored = project_node_status(&node, Prototype1NodeStatus::BinaryBuilt);
    stored.updated_at = "2026-06-09T18:03:04.433855206+00:00".to_string();
    write_node_projection(&stored).expect("write contaminated node projection");
    let runner_result = crate::intervention::Prototype1RunnerResult {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign"),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Succeeded,
        disposition: crate::intervention::Prototype1RunnerDisposition::Succeeded,
        treatment_campaign_id: Some(CampaignId::from("treatment")),
        evaluation_artifact_path: None,
        detail: None,
        exit_code: Some(0),
        stdout_excerpt: None,
        stderr_excerpt: None,
        recorded_at: "2026-06-09T17:55:54.420975107+00:00".to_string(),
    };
    crate::intervention::write_runner_result_at(&stored.runner_result_path, &runner_result)
        .expect("write terminal runner result");
    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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

    let err = run_planned_child(
        CampaignId::from("campaign"),
        manifest_path.clone(),
        repo_root,
        prototype1_transition_journal_path(&manifest_path),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Build,
        Duration::from_secs(30),
        0,
        child,
    )
    .expect_err("direct re-entry must not rebuild a terminal child");

    match err {
        PrepareError::InvalidBatchSelection { detail } => {
            assert!(detail.contains("terminal runner result"));
            assert!(detail.contains("repair terminal node state"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let loaded = load_node_record(
        &manifest_path,
        &node.node_id,
        OperatorProjectionRead::cli_operator(),
    )
    .expect("reload node after blocked re-entry");
    assert_eq!(loaded.status, Prototype1NodeStatus::BinaryBuilt);
    assert_eq!(loaded.updated_at, "2026-06-09T18:03:04.433855206+00:00");
    assert!(
        !loaded.node_dir.join("target").exists(),
        "blocked re-entry must not create a child build target"
    );
}

#[tokio::test]
async fn succeeded_child_without_evaluation_blocks_direct_reentry() {
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
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let mut stored = project_node_status(&node, Prototype1NodeStatus::Succeeded);
    stored.updated_at = "2026-06-09T18:04:04.433855206+00:00".to_string();
    write_node_projection(&stored).expect("write terminal node projection");
    let runner_result = crate::intervention::Prototype1RunnerResult {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign"),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Succeeded,
        disposition: crate::intervention::Prototype1RunnerDisposition::Succeeded,
        treatment_campaign_id: Some(CampaignId::from("treatment")),
        evaluation_artifact_path: None,
        detail: None,
        exit_code: Some(0),
        stdout_excerpt: None,
        stderr_excerpt: None,
        recorded_at: "2026-06-09T17:55:54.420975107+00:00".to_string(),
    };
    crate::intervention::write_runner_result_at(&stored.runner_result_path, &runner_result)
        .expect("write terminal runner result");
    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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

    let err = run_planned_child(
        CampaignId::from("campaign"),
        manifest_path.clone(),
        repo_root,
        prototype1_transition_journal_path(&manifest_path),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Build,
        Duration::from_secs(30),
        0,
        child,
    )
    .expect_err("succeeded child without evaluation must not re-enter as complete");

    match err {
        PrepareError::InvalidBatchSelection { detail } => {
            assert!(detail.contains("missing branch evaluation report"));
            assert!(detail.contains("observe recovery"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let loaded = load_node_record(
        &manifest_path,
        &node.node_id,
        OperatorProjectionRead::cli_operator(),
    )
    .expect("reload node after blocked re-entry");
    assert_eq!(loaded.status, Prototype1NodeStatus::Succeeded);
    assert_eq!(loaded.updated_at, "2026-06-09T18:04:04.433855206+00:00");
    assert!(
        !loaded.node_dir.join("target").exists(),
        "blocked re-entry must not create a child build target"
    );
}

#[tokio::test]
async fn succeeded_child_without_evaluation_recovers_from_terminal_channel() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    let campaign_id = CampaignId::from("campaign");

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(1, 1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let mut stored = project_node_status(&node, Prototype1NodeStatus::Succeeded);
    stored.updated_at = "2026-06-09T18:04:04.433855206+00:00".to_string();
    write_node_projection(&stored).expect("write terminal node projection");

    let treatment_id = CampaignId::from("treatment");
    let run_metrics = test_metrics(false, true, 0);
    let treatment = Prototype1TreatmentEvidence {
        baseline_campaign_id: campaign_id.clone(),
        branch_id: node.branch_id.clone(),
        treatment_campaign_id: treatment_id.clone(),
        treatment_campaign_manifest: tmp.path().join("treatment/campaign.json"),
        treatment_closure_state_path: tmp.path().join("treatment/closure-state.json"),
        eval_policy: EvalCampaignPolicy::default(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        instances: vec![Prototype1TreatmentInstanceEvidence {
            instance_id: node.instance_id.clone(),
            registration_path: None,
            record_path: Some(tmp.path().join("treatment-record.json.gz")),
            metrics: Some(run_metrics.clone()),
            oracle_evaluation: None,
            status: "complete".to_string(),
        }],
    };
    let result = crate::intervention::Prototype1RunnerResult {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.clone(),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: Prototype1NodeStatus::Succeeded,
        disposition: crate::intervention::Prototype1RunnerDisposition::Succeeded,
        treatment_campaign_id: Some(treatment_id.clone()),
        evaluation_artifact_path: None,
        detail: None,
        exit_code: Some(0),
        stdout_excerpt: None,
        stderr_excerpt: None,
        recorded_at: "2026-06-09T17:55:54.420975107+00:00".to_string(),
    };
    crate::intervention::write_runner_result_at(&stored.runner_result_path, &result)
        .expect("write terminal runner result");

    let runtime_id = RuntimeId::new();
    let runtime_path =
        crate::cli::prototype1_state::invocation::result_path(&stored.node_dir, runtime_id);
    crate::intervention::write_runner_result_at(&runtime_path, &result)
        .expect("write attempt runner result");
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let channel_root =
        crate::cli::prototype1_state::invocation::channel_root(&stored.node_dir, runtime_id);
    let payload = project_node_status(&node, Prototype1NodeStatus::BinaryBuilt);
    let invocation = crate::cli::prototype1_state::invocation::ChildInvocation::with_bootstrap(
        campaign_id.clone(),
        payload,
        child.runner_request().clone(),
        child.resolved().clone(),
        runtime_id,
        journal_path.clone(),
        channel_root.clone(),
    )
    .expect("child invocation bootstrap");
    let invocation_path =
        crate::cli::prototype1_state::invocation::invocation_path(&stored.node_dir, runtime_id);
    crate::cli::prototype1_state::invocation::write_child_invocation(&invocation_path, &invocation)
        .expect("write child invocation");

    let ready = crate::cli::prototype1_state::channel::ToParent::Ready;
    let evaluating = crate::cli::prototype1_state::channel::ToParent::Evaluating;
    let terminal = crate::cli::prototype1_state::channel::ToParent::Result {
        runner_result: result.clone(),
        treatment: Some(treatment),
    };
    let messages = [ready, evaluating, terminal];
    let mut lines = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        use sha2::{Digest, Sha256};

        let bytes = serde_json::to_vec(message).expect("serialize channel body");
        let body = String::from_utf8(bytes.clone()).expect("channel body utf8");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let message_id = format!("00000000-0000-4000-8000-{:012}", index + 1);
        lines.push(format!(
            r#"{{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"{campaign_id}","node_id":"{}","runtime_id":"{runtime_id}","message_id":"{message_id}","recorded_at":0,"body_hash":"{hash}","body":{body}}}"#,
            node.node_id
        ));
    }
    fs::create_dir_all(&channel_root).expect("create channel root");
    fs::write(
        channel_root.join("child-to-parent.jsonl"),
        lines.join("\n") + "\n",
    )
    .expect("write terminal channel");

    let baseline = CompleteBaseline::complete(
        campaign_id.clone(),
        parent_identity.node_id().to_string(),
        parent_identity.branch_id().to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: node.instance_id.clone(),
            registration_path: None,
            record_path: tmp.path().join("baseline-record.json.gz"),
            metrics: run_metrics,
        }],
    )
    .expect("complete baseline");

    let path = prototype1_branch_evaluation_path(&manifest_path, &node.branch_id);
    assert!(!path.exists(), "test starts without branch evaluation");
    let outcome = run_planned_child(
        campaign_id.clone(),
        manifest_path.clone(),
        repo_root,
        journal_path.clone(),
        parent_identity,
        baseline,
        Arc::new(Mutex::new(())),
        Prototype1StateStopAfter::Build,
        Duration::from_secs(30),
        0,
        child.clone(),
    )
    .expect("recover missing branch evaluation from terminal channel");

    assert_eq!(outcome.outcome, "completed:Keep");
    assert_eq!(outcome.node_status, Prototype1NodeStatus::Succeeded);
    assert_eq!(outcome.child_runtime, Some(runtime_id.to_string()));
    assert!(outcome.evaluation_report.is_some());
    assert!(outcome.selection_input.is_some());
    assert!(path.exists(), "recovery writes parent comparison report");
    assert!(
        !stored.node_dir.join("target").exists(),
        "stored terminal recovery must not create a child build target"
    );
    let entries = PrototypeJournal::new(journal_path)
        .load_entries()
        .expect("load recovery journal");
    assert!(entries.iter().any(|entry| matches!(
        entry,
        JournalEntry::ObserveChild(observed)
            if observed.refs.node_id == node.node_id
                && observed.runtime_id == runtime_id
                && observed.phase == CommitPhase::After
                && matches!(
                    &observed.result,
                    Some(crate::cli::prototype1_state::journal::ObservedChildResult::TreatmentComplete {
                        treatment_campaign_id,
                    }) if treatment_campaign_id == &treatment_id
                )
    )));

    let reconstructed = reconstruct_child_outcomes_from_store(
        &campaign_id,
        &manifest_path,
        std::slice::from_ref(&child),
    )
    .expect("read-only child outcome reconstruction from terminal channel");
    assert_eq!(reconstructed.len(), 1);
    assert_eq!(reconstructed[0].outcome, "completed:Keep");
    assert_eq!(reconstructed[0].child_runtime, Some(runtime_id.to_string()));
    assert!(reconstructed[0].selection_input.is_some());
}

#[tokio::test]
async fn child_spawn_observes_ready() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
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
: > "$node_dir/completed-$runtime_id"
exit 0
"#,
    );
    let _env = crate::test_support::env_guard_os(vec![("PATH", path)]);

    let allowed = write_broad_surface_targets(&repo_root);
    commit_indexed_repo(&repo_root, "broad surface fixture");
    let parent = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let budget = Prototype1ChildBudget::new(1, 1);
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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
        CampaignId::from("campaign"),
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
    let (child_pid, incarnation) = entries
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::SpawnChild(spawn)
                if spawn.refs.node_id == node.node_id
                    && spawn.phase
                        == crate::cli::prototype1_state::journal::SpawnPhase::Observed =>
            {
                spawn.child_pid.zip(spawn.incarnation.clone())
            }
            _ => None,
        })
        .expect("observed child process identity");
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

    #[cfg(target_os = "linux")]
    {
        let completion_path = node.node_dir.join(format!("completed-{runtime}"));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !completion_path.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "acknowledged child process {child_pid} must reach normal completion"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let observed = crate::cli::prototype1_state::invocation::process_incarnation(child_pid)
                .expect("inspect acknowledged child process");
            if observed.as_ref() != Some(&incarnation) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "acknowledged child process {child_pid} must be reaped after it exits"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "node_id".to_string(),
        cozo::DataValue::from(node.node_id.clone()),
    );
    params.insert("phase".to_string(), cozo::DataValue::from("spawn"));
    let builds = db
        .raw_query_params(
            r#"
?[
    build_id,
    runtime_id,
    artifact_id,
    outcome,
    binary_ref
] :=
    *eval_build_event {
        build_id,
        node_id,
        runtime_id,
        artifact_id,
        phase,
        outcome,
        binary_ref
    },
    node_id = $node_id,
    phase = $phase
"#,
            params,
        )
        .expect("query spawn build event");
    assert_eq!(builds.rows.len(), 1);
    let build_row = builds.row_refs().next().expect("spawn build row");
    assert_eq!(
        build_row.get::<String>("runtime_id").expect("runtime"),
        runtime
    );
    assert_eq!(
        build_row.get::<String>("artifact_id").expect("artifact"),
        node.derived_artifact_id
            .as_ref()
            .expect("derived artifact")
            .to_string()
    );
    assert_eq!(
        build_row.get::<String>("outcome").expect("outcome"),
        "acknowledged"
    );
    let binary_ref_id = build_row
        .get::<String>("binary_ref")
        .expect("binary ref id");
    assert!(!binary_ref_id.is_empty());

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "binary_ref_id".to_string(),
        cozo::DataValue::from(binary_ref_id),
    );
    let binaries = db
        .raw_query_params(
            r#"
?[
    binary_ref_id,
    built_by,
    source_ref,
    content_sha256
] :=
    *eval_binary_ref {
        binary_ref_id,
        built_by,
        source_ref,
        content_sha256
    },
    binary_ref_id = $binary_ref_id
"#,
            params,
        )
        .expect("query spawn binary ref");
    assert_eq!(binaries.rows.len(), 1);
    let binary_row = binaries.row_refs().next().expect("spawn binary row");
    assert_eq!(
        binary_row.get::<String>("built_by").expect("built by"),
        runtime
    );
    assert_eq!(
        binary_row.get::<String>("source_ref").expect("source ref"),
        outcome.binary_path.display().to_string()
    );
    assert!(
        !binary_row
            .get::<String>("content_sha256")
            .expect("binary hash")
            .is_empty()
    );
}

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
    let batch = publish_broad_harness_child_plan_request(
        &manifest_path,
        &repo_root,
        parent,
        budget,
        profile::BroadTui::default(),
    )
    .expect("publish broad harness batch");
    submit_broad_slot_for_test(&repo_root, &batch.slots[0], &[allowed[0].clone()], "slot-0");
    let receipt = admit_broad_harness_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
    )
    .await
    .expect("admit one child");
    let child = receipt.plan.body().children()[0].clone();
    let node = child.node_record().clone();
    let runner_result = crate::intervention::Prototype1RunnerResult {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign"),
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
cp "$node_dir/results/$runtime_id.json" "$node_dir/runner-result.json"
cat >> "$channel_dir/child-to-parent.jsonl" <<JSON
{{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"${{PLOKE_PROTOTYPE1_CAMPAIGN_ID:?missing campaign}}","node_id":"${{PLOKE_PROTOTYPE1_NODE_ID:?missing node}}","runtime_id":"$runtime_id","message_id":"00000000-0000-4000-8000-000000000003","recorded_at":0,"body_hash":"{terminal_hash}","body":{terminal_body}}}
JSON
exit 0
"#
        ),
    );
    let _env = crate::test_support::env_guard_os(vec![("PATH", path)]);

    let baseline = CompleteBaseline::complete(
        CampaignId::from("campaign"),
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
        CampaignId::from("campaign"),
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
        let publication = publish_broad_edit_harness_request_with_graph_limit(
            &manifest_path,
            &repo_root,
            &parent_identity,
            Prototype1ChildBudget::new(1, 1),
            admission_binding.clone(),
            DEFAULT_GRAPH_NEAREST_ITEMS,
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
        broad_tui: profile::BroadTui::default(),
    };

    let result = publish_broad_harness_child_plan_from_admitted_batch(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
        },
        batch,
        admitted,
    );
    let err = match result {
        Ok(_) => panic!("below-minimum broad fanout must not seal a child plan"),
        Err(err) => err,
    };

    let PrepareError::ChildPlanBelowMinimum {
        runnable_children,
        required_min,
        attempted_slots,
        accepted_results,
        ..
    } = err
    else {
        panic!("unexpected error variant: {err:?}");
    };
    assert_eq!(runnable_children, 2);
    assert_eq!(required_min, 3);
    assert_eq!(attempted_slots, 3);
    assert_eq!(accepted_results, 2);
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
    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        parent.identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
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
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
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
    persist_rejected_surface_attempt_child_plan(
        &CLI_TEST_CAMPAIGN,
        &manifest_path,
        parent,
        vec![rejected.clone()],
    )
    .expect("persist rejected attempt child plan");

    let resumed_parent = ready_parent_for_test(&manifest_path, &repo_root);
    let receipt = receive_existing_child_plan(
        ChildPlanEnv {
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            eval_storage_backend: profile::EvalStorageBackend::Fs,
            route_source: ModelRouteSource::DirectGoogle,
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
        vec![rejected.clone()],
        "rejected attempts should survive receive_existing_child_plan"
    );

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(CLI_TEST_CAMPAIGN.to_string()),
    );
    params.insert(
        "parent_node_id".to_string(),
        cozo::DataValue::from(receipt.plan.body().parent_node_id().to_string()),
    );
    let plans = db
        .raw_query_params(
            r#"
?[plan_id, child_count, rejected_count] :=
    *eval_child_plan {
        plan_id,
        campaign_id,
        parent_node_id,
        child_count,
        rejected_count
    },
    campaign_id = $campaign_id,
    parent_node_id = $parent_node_id
"#,
            params,
        )
        .expect("query rejected-only normalized child plan");
    assert_eq!(plans.rows.len(), 1);
    let plan = plans.row_refs().next().expect("child plan row");
    let plan_id = plan.get::<String>("plan_id").expect("plan id");
    assert_eq!(plan.get::<i64>("child_count").expect("child count"), 0);
    assert_eq!(
        plan.get::<i64>("rejected_count").expect("rejected count"),
        1
    );

    let mut params = std::collections::BTreeMap::new();
    params.insert("plan_id".to_string(), cozo::DataValue::from(plan_id));
    let attempts = db
        .raw_query_params(
            r#"
?[producer_id, proposal_id, run_id, policy, target_relpath, outcome, reason] :=
    *eval_child_plan_rejected_attempt {
        plan_id,
        producer_id,
        proposal_id,
        run_id,
        policy,
        target_relpath,
        outcome,
        reason
    },
    plan_id = $plan_id
"#,
            params,
        )
        .expect("query normalized rejected attempts");
    assert_eq!(attempts.rows.len(), 1);
    let attempt = attempts.row_refs().next().expect("attempt row");
    assert_eq!(
        attempt.get::<String>("proposal_id").expect("proposal id"),
        "proposal-rejected"
    );
    assert_eq!(
        attempt.get::<String>("outcome").expect("outcome"),
        "rejected"
    );
    assert_eq!(
        attempt.get::<String>("reason").expect("reason"),
        "backend rejected deterministic proposal"
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
fn prototype1_storage_authority_negative_projection_cannot_replace_child_plan_box() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let projection_path =
        prototype1_campaign_root(&manifest_path).join("eval-store/child-plan-row.json");
    fs::create_dir_all(projection_path.parent().expect("projection parent"))
        .expect("create projection dir");
    write_json_file_pretty(
        &projection_path,
        &serde_json::json!({
            "schema_version": "prototype1-eval-store-projection-test.v1",
            "edge_id": "r7_to_r8",
            "parent_id": parent_identity.parent_id(),
            "message_box_claimed": true
        }),
    )
    .expect("write projection row");

    let (result, trace) = collect_traces(|| {
        receive_existing_child_plan(
            ChildPlanEnv {
                campaign_id: &CLI_TEST_CAMPAIGN,
                manifest_path: &manifest_path,
                repo_root: &repo_root,
                broad_tui: profile::BroadTui::default(),
                eval_storage_backend: profile::EvalStorageBackend::Fs,
                route_source: ModelRouteSource::DirectGoogle,
            },
            parent,
        )
    });
    dump_trace_if_requested(&trace);
    let err = match result {
        Ok(_) => panic!("projection row must not replace child-plan MessageBox"),
        Err(err) => err,
    };
    let PrepareError::ReadManifest { path, source } = err else {
        panic!("unexpected error variant");
    };
    assert_eq!(
        path,
        child_plan_message_path_for_parent(&manifest_path, &parent_identity)
    );
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
    assert!(projection_path.exists());
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
}

#[test]
fn prototype1_storage_authority_negative_record_ref_cannot_replace_child_plan_box() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let repo_root = tmp.path().join("repo");
    write_broad_surface_targets(&repo_root);
    let parent: Parent<Ready> = ready_parent_for_test(&manifest_path, &repo_root);
    let parent_identity = parent.identity().clone();
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    let payload = serde_json::json!({
        "schema_version": "prototype1-eval-store-projection-test.v1",
        "edge_id": "r7_to_r8",
        "parent_id": parent_identity.parent_id(),
        "message_box_claimed": true
    })
    .to_string();
    eval_store::write_record_ref_to_owner_db(
        &db_path,
        eval_store::RecordRefEvidence::compatibility_import(
            CLI_TEST_CAMPAIGN.clone(),
            "child_plan_projection",
            "prototype1-eval-store-projection-test.v1",
            parent_identity.parent_id(),
            "prototype1-eval-store:child-plan-projection",
            0,
            1,
            db_path.display().to_string(),
            payload,
            1000,
        ),
    )
    .expect("write owner eval DB record ref");
    assert!(db_path.is_file());
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let refs = db
        .raw_query_params(
            r#"
?[record_ref_id] :=
    *eval_record_ref { record_ref_id, family },
    family = "child_plan_projection"
"#,
            std::collections::BTreeMap::new(),
        )
        .expect("query record refs");
    assert_eq!(refs.rows.len(), 1);

    let (result, trace) = collect_traces(|| {
        receive_existing_child_plan(
            ChildPlanEnv {
                campaign_id: &CLI_TEST_CAMPAIGN,
                manifest_path: &manifest_path,
                repo_root: &repo_root,
                broad_tui: profile::BroadTui::default(),
                eval_storage_backend: profile::EvalStorageBackend::Fs,
                route_source: ModelRouteSource::DirectGoogle,
            },
            parent,
        )
    });
    dump_trace_if_requested(&trace);
    let err = match result {
        Ok(_) => panic!("eval_record_ref row must not replace child-plan MessageBox"),
        Err(err) => err,
    };
    let PrepareError::ReadManifest { path, source } = err else {
        panic!("unexpected error variant");
    };
    assert_eq!(
        path,
        child_plan_message_path_for_parent(&manifest_path, &parent_identity)
    );
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
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
                campaign_id: &CLI_TEST_CAMPAIGN,
                manifest_path: &manifest_path,
                repo_root: &repo_root,
                broad_tui: profile::BroadTui::default(),
                eval_storage_backend: profile::EvalStorageBackend::Fs,
                route_source: ModelRouteSource::DirectGoogle,
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
                campaign_id: &CLI_TEST_CAMPAIGN,
                manifest_path: &manifest_path,
                repo_root: &repo_root,
                broad_tui: profile::BroadTui::default(),
                eval_storage_backend: profile::EvalStorageBackend::Fs,
                route_source: ModelRouteSource::DirectGoogle,
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
        campaign_id: CampaignId::from("campaign"),
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
        baseline_campaign_id: CampaignId::from("baseline"),
        branch_id: node.branch_id.clone(),
        treatment_campaign_id: CampaignId::from("treatment"),
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
                embedding_route: EmbeddingRoute::OpenRouter,
                embedding_model_id: None,
                embedding_provider_slug: None,
                max_tokens: None,
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
        baseline_campaign_id: CampaignId::from("baseline"),
        branch_id: node.branch_id.clone(),
        treatment_campaign_id: CampaignId::from("treatment"),
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
        embedding_route: EmbeddingRoute::OpenRouter,
        embedding_model_id: None,
        embedding_provider_slug: None,
        max_tokens: None,
    }
}

fn test_closure_state_without_record(instance_id: &str) -> crate::closure::ClosureState {
    crate::closure::ClosureState {
        schema_version: "closure-state.v1".to_string(),
        campaign_id: CampaignId::from("baseline"),
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
fn load_parent_baseline_treats_fresh_missing_closure_as_not_ready() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let _env =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", tmp.path().as_os_str().into())]);
    let fixture = r4c_fixture(tmp.path(), profile::EvalStorageBackend::Fs);
    let campaign_dir = tmp
        .path()
        .join("campaigns")
        .join(fixture.parent.campaign_id());
    fs::create_dir_all(&campaign_dir).expect("campaign dir");

    let mut closure = test_closure_state_without_record("clap-rs__clap-3670");
    closure.campaign_id = fixture.parent.campaign_id().clone();
    closure.eval.complete_total = 0;
    closure.eval.missing_total = 1;
    closure.eval.status = ClosureClass::Missing;
    closure.instances[0].eval_status = ClosureClass::Missing;
    write_json_file_pretty(&campaign_dir.join("closure-state.json"), &closure)
        .expect("write missing closure");

    let baseline = load_parent_baseline(
        fixture.parent.campaign_id(),
        &fixture.config,
        &fixture.manifest_path,
        &fixture.parent,
    )
    .expect("missing pre-baseline closure is not a reconstruction blocker");

    assert!(
        baseline.is_none(),
        "fresh missing closure must reconstruct only through R5"
    );

    closure.eval.missing_total = 0;
    write_json_file_pretty(&campaign_dir.join("closure-state.json"), &closure)
        .expect("write inconsistent closure");
    let error = load_parent_baseline(
        fixture.parent.campaign_id(),
        &fixture.config,
        &fixture.manifest_path,
        &fixture.parent,
    )
    .expect_err("inconsistent missing counts must remain a reconstruction blocker");
    assert!(
        error.to_string().contains("Missing"),
        "unexpected inconsistent closure error: {error}"
    );
}

#[test]
fn selected_child_treatment_promotes_to_parent_baseline() {
    let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
            .to_string(),
        campaign_id: CampaignId::from("baseline"),
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
    let tmp = tempfile::tempdir().expect("tempdir");
    let campaign_root = tmp.path().join("campaign");
    let prototype1_root = campaign_root.join("prototype1");
    let manifest_path = campaign_root.join("campaign.json");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");

    let parent = test_parent_identity();
    let node = test_node(&campaign_root, "node-child", "branch-child", "candidate-1");
    let baseline = CompleteBaseline::complete(
        CampaignId::from("baseline"),
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

    let branch_log_gate = Mutex::new(());
    let report = compare_observed_child_treatment(
        &CampaignId::from("baseline"),
        &manifest_path,
        &baseline,
        &test_resolved(&node),
        &treatment,
        &branch_log_gate,
    )
    .expect("parent comparison");

    let pure_report = build_prototype1_branch_evaluation_report(
        &CampaignId::from("baseline"),
        &node.branch_id,
        &prototype1_root.join("branches.json"),
        &prototype1_root.join("evaluations/branch-child.json"),
        &baseline,
        &treatment,
    )
    .expect("parent comparison");

    assert_eq!(report.overall_disposition, BranchDisposition::Keep);
    assert_eq!(report.branch_id, pure_report.branch_id);
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

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "branch_id".to_string(),
        cozo::DataValue::from(node.branch_id.clone()),
    );
    let rows = db
        .raw_query_params(
            r#"
?[
    evaluation_id,
    baseline_id,
    treatment_id,
    procedure_id,
    evaluator_id,
    eval_set_id,
    disposition,
    record_ref
] :=
    *eval_evaluation {
        evaluation_id,
        branch_id,
        baseline_id,
        treatment_id,
        procedure_id,
        evaluator_id,
        eval_set_id,
        disposition,
        record_ref
    },
    branch_id = $branch_id
"#,
            params,
        )
        .expect("query evaluation rows");
    assert_eq!(rows.rows.len(), 1);
    let row = rows.row_refs().next().expect("evaluation row");
    let evaluation_id = row.get::<String>("evaluation_id").expect("evaluation id");
    assert_eq!(
        row.get::<String>("baseline_id").expect("baseline"),
        "baseline"
    );
    assert_eq!(
        row.get::<String>("treatment_id").expect("treatment"),
        "treatment"
    );
    assert_eq!(
        row.get::<String>("procedure_id").expect("procedure"),
        crate::cli::prototype1_state::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID
    );
    assert_eq!(
        row.get::<String>("evaluator_id").expect("evaluator"),
        "prototype1.branch_evaluation.mechanized"
    );
    assert!(
        !row.get::<String>("eval_set_id")
            .expect("eval set")
            .is_empty()
    );
    assert_eq!(
        row.get::<String>("disposition").expect("disposition"),
        "keep"
    );
    assert!(
        row.get::<String>("record_ref")
            .expect("record ref")
            .contains("prototype1/evaluations/branch-child.json#sha256:")
    );

    let mut instance_params = std::collections::BTreeMap::new();
    instance_params.insert(
        "evaluation_id".to_string(),
        cozo::DataValue::from(evaluation_id),
    );
    let instance_rows = db
        .raw_query_params(
            r#"
?[
    instance_id,
    baseline_ref,
    treatment_ref,
    status,
    outcome
] :=
    *eval_evaluation_instance {
        evaluation_id,
        instance_id,
        baseline_ref,
        treatment_ref,
        status,
        outcome
    },
    evaluation_id = $evaluation_id
"#,
            instance_params,
        )
        .expect("query evaluation instance rows");
    assert_eq!(instance_rows.rows.len(), 1);
    let instance = instance_rows
        .row_refs()
        .next()
        .expect("evaluation instance");
    assert_eq!(
        instance.get::<String>("instance_id").expect("instance"),
        node.instance_id
    );
    assert_eq!(
        instance
            .get::<String>("baseline_ref")
            .expect("baseline ref"),
        "path:/tmp/baseline/record.json.gz"
    );
    assert_eq!(
        instance
            .get::<String>("treatment_ref")
            .expect("treatment ref"),
        "path:/tmp/treatment/record.json.gz"
    );
    assert_eq!(
        instance.get::<String>("status").expect("status"),
        "compared"
    );
    assert_eq!(instance.get::<String>("outcome").expect("outcome"), "keep");
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

fn channel_refs_for_test(node_id: &str, runtime_id: &str) -> ChildChannelEvidenceRefs {
    let terminal_hash = HistoryHash::of_domain_json(
        "prototype1.test.child_channel_terminal_result",
        &(node_id, runtime_id),
    )
    .expect("terminal hash");
    ChildChannelEvidenceRefs {
        runtime_id: runtime_id.to_string(),
        terminal_result: SealedEvidenceCitation {
            ref_id: format!("channel:child-to-parent:terminal-result:{node_id}:{runtime_id}"),
            content_hash: Some(terminal_hash),
            record_name: Some(CHILD_CHANNEL_TERMINAL_RESULT_RECORD.to_string()),
        },
        attempt_result: None,
        invocation: None,
    }
}

fn test_completed_outcome(
    mut node: Prototype1NodeRecord,
    resolved: crate::intervention::ResolvedTreatmentBranch,
    plan_index: usize,
) -> PlannedChildOutcome {
    node.status = Prototype1NodeStatus::Succeeded;
    let report = test_evaluation_report(&node);
    let selection_input = selection_input_from_child_report(&node, &report);
    let runtime_id = format!("runtime:{}", node.node_id);
    PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: "completed:Keep".to_string(),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved,
        child_runtime: Some(runtime_id.clone()),
        channel_evidence: Some(channel_refs_for_test(&node.node_id, &runtime_id)),
        evaluation_report: Some(report),
        selection_input: Some(selection_input),
        surface: None,
        harness: None,
        artifact_surface: Some(ArtifactSurface::test(&node.node_id)),
        node,
    }
}

#[tokio::test]
async fn r12_reject_replay() {
    const NODE_ID: &str = "node-cb1b41e21e01ddc7";
    const BRANCH_ID: &str = "branch-aef83be6f4105a58";

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/prototype1-r12-selected-reject-20260716");
    let profile_text =
        fs::read_to_string(fixture.join("run-profile.toml")).expect("read historical run profile");
    let profile: profile::Prototype1RunProfile =
        toml::from_str(&profile_text).expect("parse historical run profile");
    profile
        .validate()
        .expect("historical profile remains valid");
    assert!(profile.search.require_keep_for_continuation);
    assert!(!profile.search.explore_from_rejected);
    let commitment: serde_json::Value = json_fixture(
        &fs::read_to_string(fixture.join("run-profile.commitment.json"))
            .expect("read historical profile commitment"),
    );
    let profile_hash = format!("{:x}", Sha256::digest(profile_text.as_bytes()));
    assert_eq!(commitment["sha256"], profile_hash);
    assert_eq!(
        profile_hash,
        "7a92bef48096e563b4b1287207a006b320029f0bf88ca5314caa102900340275"
    );

    let parent: ParentIdentity = json_fixture(
        &fs::read_to_string(fixture.join("parent_identity.json"))
            .expect("read historical parent identity"),
    );
    assert_eq!(parent.node_id(), "node-802e115bdf749c6d");
    assert_eq!(parent.generation(), 0);

    let temp = tempfile::tempdir().expect("historical R12 replay tempdir");
    let manifest_path = temp.path().join("campaign.json");
    fs::copy(fixture.join("campaign.json"), &manifest_path)
        .expect("stage historical campaign manifest");
    let repo_root = temp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(
        &repo_root,
        Path::new("README.md"),
        "historical R12 replay\n",
    );
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "historical R12 replay");

    let control = crate::cli::prototype1_state::session::Store::new(temp.path().join("control"));
    let control_path = control.paths(&parent).journal().to_path_buf();
    fs::create_dir_all(control_path.parent().expect("control journal parent"))
        .expect("create control journal directory");
    fs::copy(fixture.join("control-journal.jsonl"), &control_path)
        .expect("stage historical control journal");
    let epoch = crate::cli::prototype1_state::walk::epoch::ServerEpoch::capture(&repo_root)
        .expect("capture replay epoch");
    let control_history = control
        .inspect_history(&parent, epoch)
        .expect("replay historical controller journal")
        .expect("historical controller session exists");
    assert!(control_history.damage.is_none());
    assert_eq!(control_history.version.phase(), WalkPhase::R12);
    assert!(control_history.events.iter().any(|event| {
        matches!(
            event.kind,
            crate::cli::prototype1_state::walk::protocol::WalkSessionEventKind::Acquired {
                fence: 19,
                ..
            }
        )
    }));
    assert!(!control_history.events.iter().any(|event| {
        matches!(
            event.kind,
            crate::cli::prototype1_state::walk::protocol::WalkSessionEventKind::AttemptBegan {
                fence: 19,
                ..
            }
        )
    }));

    let journal_path = prototype1_transition_journal_path(&manifest_path);
    fs::create_dir_all(journal_path.parent().expect("transition journal parent"))
        .expect("create transition journal directory");
    fs::copy(fixture.join("transition-journal.jsonl"), &journal_path)
        .expect("stage historical transition journal");
    let journal = PrototypeJournal::new(&journal_path);
    let journal_before = journal
        .load_entries()
        .expect("load historical transition journal");

    let child_plan_path = child_plan_message_path_for_parent(&manifest_path, &parent);
    fs::create_dir_all(child_plan_path.parent().expect("child plan parent"))
        .expect("create child plan directory");
    let mut child_plan_json: serde_json::Value = json_fixture(
        &fs::read_to_string(fixture.join("child-plan-node-802e115bdf749c6d.json"))
            .expect("read historical child plan fixture"),
    );
    child_plan_json["message"] = serde_json::Value::String(
        child_plan_path
            .to_str()
            .expect("temporary child plan path is UTF-8")
            .to_string(),
    );
    fs::write(
        &child_plan_path,
        serde_json::to_vec_pretty(&child_plan_json).expect("serialize re-homed child plan"),
    )
    .expect("stage historical child plan");
    let child_plan: ChildPlanFiles = json_fixture(
        &fs::read_to_string(&child_plan_path).expect("read staged historical child plan"),
    );
    let (plan_index, child) = child_plan
        .children()
        .iter()
        .enumerate()
        .find(|(_, child)| child.node_id() == NODE_ID)
        .expect("historical selected child plan entry");

    let load_parent = || {
        let unchecked = Parent::<Unchecked>::load(&manifest_path, parent.clone())
            .expect("load historical parent");
        let checked = unchecked
            .check(
                &NoopBackend,
                &manifest_path,
                Check {
                    campaign_id: parent.campaign_id(),
                    active_root: &repo_root,
                },
            )
            .expect("check historical parent");
        let startup = Startup::<Genesis>::from_history(checked.identity(), &manifest_path)
            .expect("historical genesis startup");
        let ready = checked.ready(startup).expect("historical parent ready");
        load_existing_child_plan_for_id(parent.campaign_id(), &manifest_path, ready)
            .expect("replay historical child plan authority")
            .parent
    };
    let planned_parent = load_parent();
    let denied_parent = load_parent();

    let mut node: Prototype1NodeRecord = json_fixture(
        &fs::read_to_string(fixture.join("child-node.json")).expect("read historical child node"),
    );
    let node_dir = manifest_path
        .parent()
        .expect("manifest parent")
        .join("prototype1/nodes")
        .join(NODE_ID);
    node.node_dir = node_dir.clone();
    node.workspace_root = temp.path().join("candidate");
    node.binary_path = node_dir.join("bin/ploke-eval");
    node.runner_request_path = node_dir.join("runner-request.json");
    node.runner_result_path = node_dir.join("runner-result.json");
    fs::create_dir_all(&node_dir).expect("create historical child node directory");
    fs::write(
        crate::intervention::prototype1_node_record_path(&manifest_path, NODE_ID),
        serde_json::to_vec_pretty(&node).expect("serialize re-homed historical node"),
    )
    .expect("stage historical child node");

    let runner_result: Prototype1RunnerResult = json_fixture(
        &fs::read_to_string(fixture.join("child-runner-result.json"))
            .expect("read historical runner result"),
    );
    assert_eq!(runner_result.node_id, node.node_id);
    assert_eq!(runner_result.branch_id, node.branch_id);
    fs::copy(
        fixture.join("child-runner-result.json"),
        &node.runner_result_path,
    )
    .expect("stage historical runner result");

    let report: Prototype1BranchEvaluationReport = json_fixture(
        &fs::read_to_string(fixture.join("branch-aef83be6f4105a58.evaluation.json"))
            .expect("read historical branch evaluation"),
    );
    assert_eq!(report.branch_id, BRANCH_ID);
    assert_eq!(report.overall_disposition, BranchDisposition::Reject);

    let terminal = fs::read_to_string(fixture.join("child-to-parent.jsonl"))
        .expect("read historical child channel")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<
                crate::cli::prototype1_state::channel::Envelope<
                    crate::cli::prototype1_state::channel::ToParent,
                >,
            >(line)
            .expect("historical child channel envelope")
        })
        .find_map(|envelope| match envelope.body() {
            crate::cli::prototype1_state::channel::ToParent::Result {
                runner_result,
                treatment,
            } => Some((
                envelope.runtime_id().to_string(),
                runner_result.clone(),
                treatment.clone(),
            )),
            _ => None,
        })
        .expect("historical child terminal result");
    assert_eq!(terminal.1, runner_result);
    let terminal_body = crate::cli::prototype1_state::channel::ToParent::Result {
        runner_result: terminal.1.clone(),
        treatment: terminal.2,
    };
    let channel_evidence = ChildChannelEvidenceRefs {
        runtime_id: terminal.0.clone(),
        terminal_result: SealedEvidenceCitation {
            ref_id: format!(
                "channel:child-to-parent:terminal-result:{NODE_ID}:{}",
                terminal.0
            ),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_channel_terminal_result.v1",
                    &terminal_body,
                )
                .expect("historical terminal channel hash"),
            ),
            record_name: Some(CHILD_CHANNEL_TERMINAL_RESULT_RECORD.to_string()),
        },
        attempt_result: Some(SealedEvidenceCitation {
            ref_id: format!("child-store:attempt-runner-result:{NODE_ID}:{}", terminal.0),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_attempt_runner_result.v1",
                    &runner_result,
                )
                .expect("historical attempt result hash"),
            ),
            record_name: Some(CHILD_ATTEMPT_RUNNER_RESULT_RECORD.to_string()),
        }),
        invocation: None,
    };
    let outcome = PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: "completed:Reject".to_string(),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved: child.resolved().clone(),
        child_runtime: Some(terminal.0),
        channel_evidence: Some(channel_evidence),
        evaluation_report: Some(report.clone()),
        selection_input: Some(selection_input_from_child_report(&node, &report)),
        surface: child.surface().cloned(),
        harness: child.harness_evidence().cloned(),
        artifact_surface: Some(
            child
                .harness_evidence()
                .expect("historical broad harness evidence")
                .artifact_surface()
                .clone(),
        ),
        node: node.clone(),
    };
    let (selection, material) = select_successor_for_profile(
        &manifest_path,
        &parent,
        std::slice::from_ref(&outcome),
        child_plan.rejected_surface_attempts(),
        &profile,
    )
    .expect("replay historical successor selection")
    .expect("historical rejected child remains traversal-selected");
    assert_eq!(selection.candidate_node_id, NODE_ID);
    assert_eq!(selection.selected_branch_id.as_deref(), Some(BRANCH_ID));
    let denied_selection = selection.clone();
    let denied_material = material.clone();

    let candidate_generation = match profile.generation.source {
        profile::GenerationSource::Legacy => CandidateGenerationConfig::Legacy,
        profile::GenerationSource::BroadHarnessRequest => {
            CandidateGenerationConfig::BroadHarnessRequest
        }
        profile::GenerationSource::DeterministicTuiTools => {
            CandidateGenerationConfig::DeterministicTuiTools
        }
    };
    let run_shape = Prototype1StateRunShape {
        stop_after: profile.execution.state_stop_after(),
        observe_child_stale_after: profile.execution.observe_child_stale_after(),
        broad_tui: profile.execution.broad_tui,
        candidate_generation,
        successor_selection: profile.selection.successor_selection(),
        successor_selection_seed: profile.selection.seed,
        successor_selection_metrics: profile.selection.traversal_metrics(),
        successor_oracle_mode: profile.selection.oracle_mode(),
        successor_oracle_require_evidence: profile.selection.oracle_require_evidence(),
        successor_oracle_gate: profile.selection.oracle_gate(),
        successor_patch_gate: crate::successor_selection::PatchGate::Disabled,
        successor_oracle_targets: profile.target.eval_instances(),
        successor_metrics_policy: profile.selection.metrics_policy(),
        eval_storage_backend: profile.storage.eval.backend,
    };
    let config = ResolvedCampaignConfig {
        campaign_id: parent.campaign_id().clone(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        model_id: profile
            .model
            .id
            .clone()
            .expect("historical profile model id"),
        provider_slug: profile.model.provider.clone(),
        route_source: profile
            .model
            .route_source
            .expect("historical profile route source"),
        required_procedures: Vec::new(),
        instances_root: temp.path().join("instances"),
        batches_root: temp.path().join("batches"),
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: crate::FrameworkConfig::default(),
    };
    let mut command = state_command_without_ids();
    command.campaign = Some(parent.campaign_id().clone());
    command.repo_root = Some(repo_root.clone());
    let facts = typestate::context::Facts {
        complete_search_policy: Some(profile.search_policy()),
        selection: Some(ParentSelectionOutcome::Selected {
            decision: selection,
            material,
        }),
        report: Some(typestate::context::ReportFacts {
            outcome: "historical R12 selected rejected child".to_string(),
            node_id: node.node_id.clone(),
            node_status: node.status,
            workspace_root: node.workspace_root.clone(),
            binary_path: node.binary_path.clone(),
            child_runtime: outcome.child_runtime.clone(),
            successor_runtime: None,
            successor_pid: None,
            successor_ready_path: None,
        }),
        ..Default::default()
    };
    let collected = typestate::context::Collected::new(
        command,
        repo_root.clone(),
        parent.campaign_id().clone(),
        manifest_path.clone(),
        run_shape.clone(),
        config.clone(),
        journal_path.clone(),
        journal,
    )
    .with_facts(facts);
    let r12 = typestate::R12::from_collected_parent(collected, planned_parent);
    assert!(r12.has_successor_selection());
    let preview = r12
        .preview_continuation()
        .expect("preview historical continuation")
        .expect("historical R12 has selected continuation evidence");
    assert_eq!(
        preview.disposition,
        Prototype1ContinuationDisposition::StopSelectedBranchRejected
    );
    assert!(!preview.disposition.allows_successor());

    let mut denied_policy = profile.search_policy();
    denied_policy.explore_from_rejected = true;
    let mut denied_command = state_command_without_ids();
    denied_command.campaign = Some(parent.campaign_id().clone());
    denied_command.repo_root = Some(repo_root.clone());
    let denied_facts = typestate::context::Facts {
        complete_search_policy: Some(denied_policy),
        selection: Some(ParentSelectionOutcome::Selected {
            decision: denied_selection,
            material: denied_material,
        }),
        report: Some(typestate::context::ReportFacts {
            outcome: "historical R12 rejected exploration".to_string(),
            node_id: node.node_id.clone(),
            node_status: node.status,
            workspace_root: node.workspace_root.clone(),
            binary_path: node.binary_path.clone(),
            child_runtime: outcome.child_runtime.clone(),
            successor_runtime: None,
            successor_pid: None,
            successor_ready_path: None,
        }),
        ..Default::default()
    };
    let denied = typestate::context::Collected::new(
        denied_command,
        repo_root.clone(),
        parent.campaign_id().clone(),
        manifest_path.clone(),
        run_shape,
        config,
        journal_path.clone(),
        PrototypeJournal::new(&journal_path),
    )
    .with_facts(denied_facts);
    let denied = typestate::R12::from_collected_parent(denied, denied_parent);
    let denied_preview = denied
        .preview_continuation()
        .expect("preview rejected exploration")
        .expect("rejected exploration has selected continuation evidence");
    assert_eq!(
        denied_preview.disposition,
        Prototype1ContinuationDisposition::ContinueExploreFromRejected
    );
    assert!(denied_preview.disposition.allows_successor());
    assert_eq!(
        crate::cli::prototype1_state::walk::controller::test_r12_target(
            &denied,
            Some(WalkPhase::R13b),
        )
        .expect("continuable R12 must admit the bounded handoff target"),
        WalkPhase::R13b
    );
    let boundary_error = crate::cli::prototype1_state::walk::controller::test_r12_target(
        &denied,
        Some(WalkPhase::R14b),
    )
    .expect_err("step-mode R12 must reject a target beyond transferred authority");
    assert!(
        boundary_error
            .to_string()
            .contains("crosses the R13b successor runtime boundary"),
        "unexpected runtime-boundary error: {boundary_error}"
    );

    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    fs::create_dir_all(db_path.parent().expect("eval db parent")).expect("eval db dir");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(&db_path)
        .expect("seed owner eval db");
    let db_before = fs::read(&db_path).expect("read owner eval DB before denied handoff");
    assert_eq!(
        crate::cli::prototype1_state::walk::controller::test_r12_target(
            &r12,
            Some(WalkPhase::R13a),
        )
        .expect("historical policy stop must admit the operator's R13a target"),
        WalkPhase::R13a
    );
    assert_eq!(
        crate::cli::prototype1_state::walk::controller::test_r12_target(&r12, None)
            .expect("historical policy stop must default to R13a"),
        WalkPhase::R13a
    );
    let branch_error = crate::cli::prototype1_state::walk::controller::test_r12_target(
        &r12,
        Some(WalkPhase::R13b),
    )
    .expect_err("historical policy stop must reject the handoff branch");
    assert!(
        branch_error
            .to_string()
            .contains("continuation does not authorize handoff"),
        "unexpected branch error: {branch_error}"
    );

    let history_root = manifest_path
        .parent()
        .expect("manifest parent")
        .join("prototype1/history");
    assert!(!history_root.exists());
    let head_before = GitWorktreeBackend
        .head_commit(&repo_root)
        .expect("historical replay head");
    let status_before = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("historical replay status");
    assert!(status_before.status.success());

    let denied_error =
        crate::cli::prototype1_state::driver::control::test_r12_denied(&repo_root, denied)
            .expect_err("continuable R12 must reject a stopped-branch permit before mutation");
    assert!(
        denied_error
            .to_string()
            .contains("only an admitted R12->R13b controller attempt"),
        "unexpected denied handoff error: {denied_error}"
    );
    let denied_journal = PrototypeJournal::new(&journal_path)
        .load_entries()
        .expect("load journal after denied handoff");
    assert_eq!(denied_journal.len(), journal_before.len());
    assert_eq!(
        fs::read(&db_path).expect("read owner eval DB after denied handoff"),
        db_before
    );
    assert!(!history_root.exists());
    assert_eq!(
        GitWorktreeBackend
            .head_commit(&repo_root)
            .expect("head after denied handoff"),
        head_before
    );
    let status_denied = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("status after denied handoff");
    assert!(status_denied.status.success());
    assert_eq!(status_denied.stdout, status_before.stdout);

    let step = crate::cli::prototype1_state::driver::control::test_r12_stop(&repo_root, r12)
        .await
        .expect("selected rejected continuation must stop without checkout permission");

    assert_eq!(step.transition().from(), WalkPhase::R12);
    assert_eq!(step.transition().to(), WalkPhase::R13a);
    assert!(matches!(
        step.state(),
        crate::cli::prototype1_state::driver::control::ControlState::R13a(_)
    ));
    let journal_after = PrototypeJournal::new(&journal_path)
        .load_entries()
        .expect("load stopped transition journal");
    assert_eq!(journal_after.len(), journal_before.len() + 1);
    let stopped = journal_after
        .iter()
        .rev()
        .find_map(|entry| match entry {
            JournalEntry::Successor(record) if record.node_id == NODE_ID => Some(record),
            _ => None,
        })
        .expect("stopped successor record");
    let crate::cli::prototype1_state::successor::State::Stopped { decision, .. } = &stopped.state
    else {
        panic!("selected rejected child must record a stopped successor")
    };
    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::StopSelectedBranchRejected
    );
    assert!(!decision.disposition.allows_successor());
    assert!(!history_root.exists());
    assert_eq!(
        GitWorktreeBackend
            .head_commit(&repo_root)
            .expect("unchanged historical replay head"),
        head_before
    );
    let status_after = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("historical replay status after stop");
    assert!(status_after.status.success());
    assert_eq!(status_after.stdout, status_before.stdout);
}

#[test]
fn v15_missing_oracle_replay_fails_closed_under_all_resolved_gate() {
    const NODE_ID: &str = "node-6bae782006db482c";
    const BRANCH_ID: &str = "branch-68afac57e92d5ebd";
    const FIXTURE_HASHES: [(&str, &str); 9] = [
        (
            "branch-68afac57e92d5ebd.evaluation.json",
            "d41b3dd2effebc4dde22cc9b9dddc59d4890b3b37f2f3b44f692f4e1d460bbb4",
        ),
        (
            "campaign.json",
            "2f21c8af424c15603bc0219446b87c161a01f7f2c2ce044cb82aa54d32c00dfb",
        ),
        (
            "child-node.json",
            "5cb0ae5153a198522077ef0946d6c37b4ce74f115450ec275bb86bfa4d70039c",
        ),
        (
            "child-plan-node-9c9dcbeeb3a4d400.json",
            "c3251db8d66075579d114891432fb128715e6d2379ffbe5cf7dafda26e45e3c2",
        ),
        (
            "child-runner-result.json",
            "0ba334192a79524eda1aa227701cc5c41c5eb2a0058ce4ee9e7f379021439368",
        ),
        (
            "child-to-parent.jsonl",
            "64d919e0d0bbc814bab76fb4d8de671056781a1957841ae0a606e252dd4ad372",
        ),
        (
            "parent_identity.json",
            "96f0c9cbb3e00b5d78d7d55e18b6bee1b2a2aa3ae1b7eb8f2828de3532bc5fbc",
        ),
        (
            "run-profile.commitment.json",
            "d693b80d9cf92a1790c8d1019aee9be5e2ba2d1468ddd400be7c8f3a93403bd7",
        ),
        (
            "run-profile.toml",
            "cfda200a2ad71803cb405c6e109bca8c80dd6b2101d4ec4a12abe9eedf1c4f95",
        ),
    ];

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/prototype1-v15-missing-oracle-20260717");
    for (name, expected) in FIXTURE_HASHES {
        let bytes = fs::read(fixture.join(name)).expect("read immutable v15 fixture artifact");
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected, "{name}");
    }
    let manifest_path = fixture.join("campaign.json");
    let profile_text =
        fs::read_to_string(fixture.join("run-profile.toml")).expect("read v15 run profile");
    let profile: profile::Prototype1RunProfile =
        toml::from_str(&profile_text).expect("parse v15 run profile");
    profile.validate().expect("v15 profile remains valid");
    assert_eq!(
        profile.selection.oracle_mode(),
        crate::successor_selection::OracleMode::RecordOnly
    );
    assert!(profile.selection.oracle_require_evidence());
    assert_eq!(
        profile.selection.oracle_gate(),
        crate::successor_selection::OracleGate::Disabled
    );
    assert!(!profile.execution.mbe.enabled);

    let commitment: serde_json::Value = json_fixture(
        &fs::read_to_string(fixture.join("run-profile.commitment.json"))
            .expect("read v15 profile commitment"),
    );
    let profile_hash = format!("{:x}", Sha256::digest(profile_text.as_bytes()));
    assert_eq!(commitment["sha256"], profile_hash);
    assert_eq!(
        profile_hash,
        "cfda200a2ad71803cb405c6e109bca8c80dd6b2101d4ec4a12abe9eedf1c4f95"
    );

    let parent: ParentIdentity = json_fixture(
        &fs::read_to_string(fixture.join("parent_identity.json"))
            .expect("read v15 parent identity"),
    );
    assert_eq!(parent.node_id(), "node-9c9dcbeeb3a4d400");

    let child_plan: ChildPlanFiles = json_fixture(
        &fs::read_to_string(fixture.join("child-plan-node-9c9dcbeeb3a4d400.json"))
            .expect("read v15 child plan"),
    );
    let (plan_index, child) = child_plan
        .children()
        .iter()
        .enumerate()
        .find(|(_, child)| child.node_id() == NODE_ID)
        .expect("v15 selected child plan entry");

    let node: Prototype1NodeRecord = json_fixture(
        &fs::read_to_string(fixture.join("child-node.json")).expect("read v15 child node"),
    );
    assert_eq!(node.node_id, NODE_ID);
    assert_eq!(node.branch_id, BRANCH_ID);

    let runner_result: Prototype1RunnerResult = json_fixture(
        &fs::read_to_string(fixture.join("child-runner-result.json"))
            .expect("read v15 runner result"),
    );
    assert_eq!(runner_result.node_id, node.node_id);
    assert_eq!(runner_result.branch_id, node.branch_id);

    let report: Prototype1BranchEvaluationReport = json_fixture(
        &fs::read_to_string(fixture.join("branch-68afac57e92d5ebd.evaluation.json"))
            .expect("read v15 branch evaluation"),
    );
    assert_eq!(report.branch_id, BRANCH_ID);
    assert_eq!(report.overall_disposition, BranchDisposition::Keep);
    assert_eq!(
        report
            .eval_set_identity
            .as_ref()
            .expect("v15 eval set identity")
            .instance_ids,
        vec!["BurntSushi__ripgrep-2209".to_string()]
    );
    assert_eq!(report.compared_instances.len(), 1);
    let compared = &report.compared_instances[0];
    assert!(
        compared
            .baseline_metrics
            .as_ref()
            .is_some_and(|metrics| metrics.oracle_eligible)
    );
    assert!(
        compared
            .treatment_metrics
            .as_ref()
            .is_some_and(|metrics| metrics.oracle_eligible)
    );
    assert!(compared.oracle_evaluation.is_none());

    let terminal = fs::read_to_string(fixture.join("child-to-parent.jsonl"))
        .expect("read v15 child channel")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<
                crate::cli::prototype1_state::channel::Envelope<
                    crate::cli::prototype1_state::channel::ToParent,
                >,
            >(line)
            .expect("v15 child channel envelope")
        })
        .find_map(|envelope| match envelope.body() {
            crate::cli::prototype1_state::channel::ToParent::Result {
                runner_result,
                treatment,
            } => Some((
                envelope.runtime_id().to_string(),
                runner_result.clone(),
                treatment.clone(),
            )),
            _ => None,
        })
        .expect("v15 child terminal result");
    assert_eq!(terminal.1, runner_result);
    let terminal_body = crate::cli::prototype1_state::channel::ToParent::Result {
        runner_result: terminal.1.clone(),
        treatment: terminal.2,
    };
    let channel_evidence = ChildChannelEvidenceRefs {
        runtime_id: terminal.0.clone(),
        terminal_result: SealedEvidenceCitation {
            ref_id: format!(
                "channel:child-to-parent:terminal-result:{NODE_ID}:{}",
                terminal.0
            ),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_channel_terminal_result.v1",
                    &terminal_body,
                )
                .expect("v15 terminal channel hash"),
            ),
            record_name: Some(CHILD_CHANNEL_TERMINAL_RESULT_RECORD.to_string()),
        },
        attempt_result: Some(SealedEvidenceCitation {
            ref_id: format!("child-store:attempt-runner-result:{NODE_ID}:{}", terminal.0),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_attempt_runner_result.v1",
                    &runner_result,
                )
                .expect("v15 attempt result hash"),
            ),
            record_name: Some(CHILD_ATTEMPT_RUNNER_RESULT_RECORD.to_string()),
        }),
        invocation: None,
    };
    let outcome = PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: "completed:Keep".to_string(),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved: child.resolved().clone(),
        child_runtime: Some(terminal.0),
        channel_evidence: Some(channel_evidence),
        evaluation_report: Some(report.clone()),
        selection_input: Some(selection_input_from_child_report(&node, &report)),
        surface: child.surface().cloned(),
        harness: child.harness_evidence().cloned(),
        artifact_surface: Some(
            child
                .harness_evidence()
                .expect("v15 broad harness evidence")
                .artifact_surface()
                .clone(),
        ),
        node,
    };

    let original = select_successor_for_profile(
        &manifest_path,
        &parent,
        std::slice::from_ref(&outcome),
        child_plan.rejected_surface_attempts(),
        &profile,
    )
    .expect("replay original v15 selection")
    .expect("original v15 policy selected the candidate");
    assert_eq!(original.0.candidate_node_id, NODE_ID);
    assert_eq!(original.0.selected_branch_id.as_deref(), Some(BRANCH_ID));

    let mut strict = profile.clone();
    strict.selection.oracle.gate = crate::successor_selection::OracleGate::AllResolved;
    strict.execution.mbe.enabled = true;
    strict.validate().expect("strict replay profile is valid");
    let error = match select_successor_for_profile(
        &manifest_path,
        &parent,
        std::slice::from_ref(&outcome),
        child_plan.rejected_surface_attempts(),
        &strict,
    ) {
        Err(error) => error,
        Ok(_) => panic!("v15 missing oracle evidence must fail closed"),
    };
    assert!(
        error.to_string().contains("missing oracle evaluation"),
        "unexpected strict replay error: {error}"
    );
    assert!(!fixture.join("prototype1/history").exists());
    assert!(!eval_store::prototype1_eval_store_db_path(&manifest_path).exists());
}

#[test]
fn v25_receipt_replay() {
    const SELECTED_NODE: &str = "node-f4f44e07e0be8caa";
    const SELECTED_BRANCH: &str = "branch-55892858db150ccd";
    const OTHER_BLOCKED_NODE: &str = "node-3247ff02a2053a57";
    const ADMISSIBLE_NODE: &str = "node-2f0b3cda4c2c8d89";
    const SECOND_PATH: &str = "crates/ploke-tui/src/tools/get_code_edges.rs";
    const SECOND_SOURCE_HASH: &str =
        "be0dd87df60e1aa5e1381ad9d9ee7d404863487dd58fa129a3c93a9686e91776";
    const SECOND_PROPOSED_HASH: &str =
        "055dd8d3c5a357ca1177b69125eff2dace008cb741b18523f88ad1f6659f6d66";
    const DECISION_HASH: &str = "26e318146c7ca203b18f924b6b329e96975743dfbe416a92d743730912545de6";
    const ENTRY_SHA256: &str = "b453ddaf8d0ac90527835568f8ad16ad93a0d84158deb0fc035d082802ff2615";
    const SET_ROOT: &str = "6632780e126edc069f4b8d103dc36ea33fdc35b77119c1066a8a36a29108cf6c";
    const PAYLOAD_HASHES: [&str; 3] = [
        "519cb576045f8236e3cc74a663322d9cabdf2f57990f330b0f83986afd7bf211",
        "7ebdf2b794704b4952fc54e0040b45ac524793dc059a7236c26bddb76fde4608",
        "567c194c04882b04c3c7b7cc2f0890086bce94e5e1b5a7bdfe85fb90c060c7fe",
    ];
    const FIXTURE_HASHES: [(&str, &str); 14] = [
        (
            "branch-269c5d245f354a1e.evaluation.json",
            "df45a63dd9af27f2061a6fb82f6194c020082cfd32c5ee5bb45e1d38acd6928d",
        ),
        (
            "branch-42f0d2c400a40cee.evaluation.json",
            "0522142bd9fe8d887fcfda2b52c38bbc47e5b266ce882c7b79318e4f088069b0",
        ),
        (
            "branch-55892858db150ccd.evaluation.json",
            "93e90a839c12ec8eacb74ebfca53b8a56d815fe98e59d0421d1dea75395e98c9",
        ),
        (
            "campaign.json",
            "ab21d10c0efdaa003ac49312de5baef0b39fc282640b1808d769a9bce795fec5",
        ),
        (
            "child-plan-node-461dba1909fb6cf7.json",
            "f68736a57839947494478b10a72dee26da2537e72a1bd34b4d7922e414a4bd64",
        ),
        (
            "node-2f0b3cda4c2c8d89.child-to-parent.jsonl",
            "dddbc063fd326b505a0f08972fc535f73699c9b31f3617422aba8c38a76a71db",
        ),
        (
            "node-2f0b3cda4c2c8d89.json",
            "8d50a7cb7b738602fb844a95287151c4132ef410e21ea45a152f825c8db3c562",
        ),
        (
            "node-3247ff02a2053a57.child-to-parent.jsonl",
            "48c4c039fb5c822bd485a7c46830b18b38a342c9dc4351bb8341cf1c1ad752eb",
        ),
        (
            "node-3247ff02a2053a57.json",
            "f47153b214ee327afcfd5148763f23f5cb3500e670376ec8b8cbe6e65fa27beb",
        ),
        (
            "node-f4f44e07e0be8caa.child-to-parent.jsonl",
            "f1cf77fd595316055dfc9f2dd67ddb2240fd1178164749559e0e29f8954b3949",
        ),
        (
            "node-f4f44e07e0be8caa.json",
            "638c9094635474d7472179499d78549f7a49ac6f926601d1523ed18fa1afec0c",
        ),
        (
            "parent_identity.json",
            "b000745fed82cc75fdd2b1fdd6c1340f473c1d682d2cde77af7be65ad70ad071",
        ),
        (
            "run-profile.commitment.json",
            "cb021229bfde16585bb55eedaf62cab3244ac581270efb5e92ef574d6262807c",
        ),
        (
            "run-profile.toml",
            "c3b2fd9147560917f64bb0d28a833016f0e46080fd9453e73f3883162631b85a",
        ),
    ];

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/prototype1-v25-selection-safety-20260717");
    let manifest_path = fixture.join("campaign.json");
    let history_dir = fixture.join("prototype1/history");
    let reviews_dir = fixture.join("prototype1/reviews");
    let owner_db = eval_store::prototype1_eval_store_db_path(&manifest_path);
    assert!(!history_dir.exists());
    assert!(!reviews_dir.exists());
    assert!(!owner_db.exists());
    for (name, expected) in FIXTURE_HASHES {
        let bytes = fs::read(fixture.join(name)).expect("read immutable v25 fixture artifact");
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected, "{name}");
    }

    let profile_text =
        fs::read_to_string(fixture.join("run-profile.toml")).expect("read v25 run profile");
    let profile: profile::Prototype1RunProfile =
        toml::from_str(&profile_text).expect("parse v25 run profile");
    profile.validate().expect("v25 profile remains valid");
    assert_eq!(
        profile.selection.patch_gate(),
        crate::successor_selection::PatchGate::Disabled
    );
    assert_eq!(
        profile.selection.oracle_gate(),
        crate::successor_selection::OracleGate::AllResolved
    );
    assert!(profile.execution.mbe.enabled);

    let bytes =
        fs::read(fixture.join("selection-decision-entry.json")).expect("read exact v25 receipt");
    assert_eq!(bytes.len(), 573_257);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        ENTRY_SHA256,
        "exact persisted entry_json bytes"
    );
    let entry: SelectionDecisionEntry =
        serde_json::from_slice(&bytes).expect("decode exact persisted v25 receipt");
    entry.validate_shape().expect("validate exact v25 receipt");
    assert_eq!(
        entry
            .decision_hash()
            .expect("hash exact v25 receipt")
            .as_str(),
        DECISION_HASH
    );
    let decision = entry.decision.as_ref().expect("v25 selected decision");
    assert_eq!(decision.candidate_node_id, SELECTED_NODE);
    assert_eq!(
        decision.selected_branch_id.as_deref(),
        Some(SELECTED_BRANCH)
    );
    let candidate_set = entry.candidate_set.as_ref().expect("v25 candidate set");
    assert_eq!(candidate_set.root.as_str(), SET_ROOT);
    assert_eq!(
        candidate_set
            .memberships
            .iter()
            .map(|membership| membership.payload_hash.as_str())
            .collect::<Vec<_>>(),
        PAYLOAD_HASHES
    );
    let traversal = entry.traversal.as_ref().expect("v25 traversal");
    assert_eq!(
        traversal.strategy.patch_gate(),
        crate::successor_selection::PatchGate::Disabled
    );

    let original = traversal_selection::Candidates::from_history(HistoryCandidates {
        scope: entry.scope.clone(),
        candidates: Vec::new(),
    })
    .with_current_generation(entry.scope.clone(), entry.considered.clone())
    .expect("bind exact historical V25 candidates");
    let original = traversal_selection::select_attempt_with_policy(
        original,
        traversal.seed,
        traversal.strategy,
        entry.metrics.policy.clone(),
        &traversal.oracle_targets,
    )
    .expect("replay original V25 production traversal");
    let traversal_selection::SelectionAttempt::Selected(original) = original else {
        panic!("original V25 traversal must reproduce the persisted selection")
    };
    assert_eq!(
        &original.decision,
        entry.decision.as_ref().expect("v25 selected decision")
    );
    assert_eq!(original.decision.candidate_node_id, SELECTED_NODE);

    let outcome =
        ParentSelectionOutcome::from_entry(entry.clone()).expect("hydrate exact v25 receipt");
    assert_eq!(
        outcome.entry().expect("rebuild hydrated v25 receipt"),
        entry,
        "typed hydration preserves every persisted field"
    );
    traversal_selection::validate_patch_gate(
        &entry,
        crate::successor_selection::PatchGate::Disabled,
    )
    .expect("historical disabled patch gate remains compatible");
    let error = traversal_selection::validate_patch_gate(
        &entry,
        crate::successor_selection::PatchGate::ReviewedAdmissible,
    )
    .expect_err("strict patch gate rejects a receipt without candidate reviews");
    assert!(
        error
            .to_string()
            .contains("reviewed-admissible patch gate requires candidate review"),
        "unexpected strict v25 replay error: {error}"
    );

    let plan_bytes = fs::read(fixture.join("child-plan-node-461dba1909fb6cf7.json"))
        .expect("read exact v25 child plan");
    let child_plan: ChildPlanFiles =
        serde_json::from_slice(&plan_bytes).expect("decode exact v25 child plan");
    let mut reviewed = entry.considered.clone();
    for payload in &mut reviewed {
        let candidate = payload
            .selection_input
            .as_ref()
            .expect("v25 selection input")
            .candidate
            .clone();
        let child = child_plan
            .children()
            .iter()
            .find(|child| child.node_id() == candidate.node_id)
            .expect("v25 payload has a child-plan carrier");
        assert_eq!(child.resolved().branch.branch_id, candidate.branch_id);
        let harness = child
            .harness_evidence()
            .expect("v25 broad candidate has typed harness evidence")
            .clone();
        let artifact = payload.artifact.as_mut().expect("v25 candidate artifact");
        assert_eq!(
            artifact.artifact_surface.as_ref(),
            Some(harness.artifact_surface()),
            "receipt and child-plan artifact surfaces agree"
        );
        let harness_artifact = harness.artifact().expect("v25 harness artifact binding");
        assert_eq!(
            artifact.resolved.branch.derived_artifact_id.as_ref(),
            Some(&harness_artifact.derived_artifact_id)
        );
        assert_eq!(
            artifact.node.derived_artifact_id.as_ref(),
            Some(&harness_artifact.derived_artifact_id)
        );
        assert_eq!(
            artifact.node.base_artifact_id.as_ref(),
            Some(&harness_artifact.base_artifact_id)
        );
        artifact.harness = Some(harness);
        artifact.schema_version = artifact.schema_version.max(4);
    }

    // V25 predated persisted patch reviews. This post-incident overlay uses the
    // production reviewer-config identity derived from the exact admitted
    // profile, but never claims that a provider response existed during the
    // historical run.
    let config_tmp = tempfile::tempdir().expect("v25 reviewer config tempdir");
    let config_manifest = config_tmp.path().join("campaign.json");
    fs::copy(&manifest_path, &config_manifest).expect("stage v25 reviewer campaign");
    profile::admit_run_profile(
        &config_manifest,
        &profile::OperatorRunProfile {
            source_path: fixture.join("run-profile.toml"),
            profile: profile.clone(),
        },
    )
    .expect("admit exact v25 reviewer profile");
    let config_hash =
        crate::cli::prototype1_state::candidate_review::admitted_config_hash(&config_manifest)
            .expect("hash production v25 reviewer config");
    let attach_review = |payload: &mut EvaluationPayload,
                         verdict: crate::successor_selection::PatchVerdict,
                         findings: Vec<String>| {
        let candidate = payload
            .selection_input
            .as_ref()
            .expect("v25 selection input")
            .candidate
            .clone();
        let artifact = payload.artifact.as_ref().expect("v25 candidate artifact");
        let harness = artifact
            .harness
            .as_ref()
            .expect("in-memory overlay retains exact harness evidence");
        let artifact_id = artifact
            .resolved
            .branch
            .derived_artifact_id
            .as_ref()
            .expect("v25 derived artifact id")
            .clone();
        let surface_hash = HistoryHash::of_domain_json(
            "prototype1.history.artifact_surface.v1",
            artifact
                .artifact_surface
                .as_ref()
                .expect("v25 artifact surface"),
        )
        .expect("hash v25 artifact surface");
        assert_eq!(
            artifact.node.branch_id.as_str(),
            candidate.branch_id.as_str()
        );
        let evaluations = payload
            .sealed_evidence
            .as_ref()
            .expect("v25 sealed evidence")
            .evaluations
            .iter()
            .filter(|item| item.branch_id == candidate.branch_id)
            .collect::<Vec<_>>();
        let [evaluation] = evaluations.as_slice() else {
            panic!("v25 candidate must have exactly one branch evaluation")
        };
        let evaluation_hash = evaluation
            .evaluation_artifact_citation
            .as_ref()
            .and_then(|citation| citation.content_hash.as_ref())
            .expect("v25 evaluation artifact hash")
            .clone();
        assert_eq!(
            evaluation.primary_report_citation.content_hash.as_ref(),
            Some(&evaluation_hash),
            "v25 evaluation citations bind the same report"
        );

        let mut paths = harness.changed_paths().to_vec();
        let recorded_paths = paths.clone();
        paths.sort();
        paths.dedup();
        assert_eq!(recorded_paths, paths, "v25 change paths are canonical");
        let changes = paths
            .into_iter()
            .map(|relpath| {
                let (source_hash, proposed_hash) = if relpath == artifact.resolved.target_relpath {
                    (
                        artifact.resolved.source_content_hash.clone(),
                        artifact.resolved.branch.proposed_content_hash.clone(),
                    )
                } else {
                    assert_eq!(candidate.node_id, ADMISSIBLE_NODE);
                    assert_eq!(relpath, PathBuf::from(SECOND_PATH));
                    (
                        SECOND_SOURCE_HASH.to_string(),
                        SECOND_PROPOSED_HASH.to_string(),
                    )
                };
                crate::successor_selection::PatchChange {
                    relpath,
                    source_content_hash: Some(source_hash),
                    proposed_content_hash: Some(proposed_hash),
                }
            })
            .collect::<Vec<_>>();
        let change_set_hash = HistoryHash::of_domain_json(
            "prototype1.history.candidate_patch_change_set.v1",
            &changes,
        )
        .expect("hash exact v25 change set");
        let citation_hash = HistoryHash::of_domain_json(
            "prototype1.test.v25_post_incident_patch_review.v1",
            &(
                &candidate,
                &artifact_id,
                &surface_hash,
                &evaluation_hash,
                &config_hash,
                &change_set_hash,
                &changes,
                verdict,
                &findings,
            ),
        )
        .expect("hash post-incident v25 review");
        let confidence = crate::successor_selection::domains::Confidence::High;
        payload.patch_review = Some(crate::successor_selection::PatchReview {
            schema_version: 2,
            procedure_id: crate::successor_selection::PATCH_REVIEW_PROCEDURE_ID.to_string(),
            candidate: candidate.clone(),
            artifact_id,
            artifact_surface_hash: surface_hash,
            evaluation_hash,
            config_hash: config_hash.clone(),
            change_set_hash,
            changes,
            verdict,
            confidence,
            blocking_findings: findings,
            missing_evidence: Vec::new(),
            rationale: vec![
                "post-incident review of the exact V25 artifact and evaluation bindings"
                    .to_string(),
            ],
            citation: SealedEvidenceCitation {
                ref_id: crate::successor_selection::candidate_review_ref(&candidate.branch_id),
                content_hash: Some(citation_hash),
                record_name: Some(crate::successor_selection::PATCH_REVIEW_RECORD_NAME.to_string()),
            },
        });
        payload.schema_version = payload.schema_version.max(5);
    };

    for payload in &mut reviewed {
        let node_id = payload
            .selection_input
            .as_ref()
            .expect("v25 selection input")
            .candidate
            .node_id
            .as_str();
        let (verdict, findings) = match node_id {
            SELECTED_NODE => (
                crate::successor_selection::PatchVerdict::Rejected,
                vec![
                    "uses a process-global Cargo metadata cache keyed only by the focused manifest modification time"
                        .to_string(),
                    "can return stale workspace metadata after workspace-root or sibling-manifest changes"
                        .to_string(),
                ],
            ),
            OTHER_BLOCKED_NODE => (
                crate::successor_selection::PatchVerdict::Rejected,
                vec![
                    "spawns unbounded nested operating-system thread fanout and unwraps join failures"
                        .to_string(),
                    "turns a missing filesystem node into an empty iterator and erases the error"
                        .to_string(),
                ],
            ),
            ADMISSIBLE_NODE => (
                crate::successor_selection::PatchVerdict::Admissible,
                Vec::new(),
            ),
            other => panic!("unexpected V25 candidate '{other}'"),
        };
        attach_review(payload, verdict, findings);
    }
    assert!(reviewed.iter().all(|payload| {
        payload
            .patch_review
            .as_ref()
            .is_some_and(|review| review.config_hash == config_hash)
    }));

    let strict_gate = crate::successor_selection::PatchGate::ReviewedAdmissible;
    let strict_strategy = traversal.strategy.with_patch_gate(strict_gate);
    let candidates = traversal_selection::Candidates::from_history(HistoryCandidates {
        scope: entry.scope.clone(),
        candidates: Vec::new(),
    })
    .with_current_generation(entry.scope.clone(), reviewed.clone())
    .expect("bind reviewed V25 candidates");
    let attempt = traversal_selection::select_attempt_with_policy(
        candidates,
        traversal.seed,
        strict_strategy,
        entry.metrics.policy.clone(),
        &traversal.oracle_targets,
    )
    .expect("run production strict traversal over exact V25 payloads");
    let traversal_selection::SelectionAttempt::Selected(selection) = attempt else {
        panic!("strict V25 overlay must select the admissible patch")
    };
    assert_eq!(selection.decision.candidate_node_id, ADMISSIBLE_NODE);
    assert_ne!(selection.decision.candidate_node_id, SELECTED_NODE);
    assert_ne!(selection.decision.candidate_node_id, OTHER_BLOCKED_NODE);
    assert_eq!(
        selection
            .selected_payload
            .selection_input
            .as_ref()
            .expect("selected V25 input")
            .candidate
            .node_id,
        ADMISSIBLE_NODE
    );

    let mut all_rejected = reviewed;
    let remaining = all_rejected
        .iter_mut()
        .find(|payload| {
            payload
                .selection_input
                .as_ref()
                .is_some_and(|input| input.candidate.node_id == ADMISSIBLE_NODE)
        })
        .expect("v25 admissible overlay");
    attach_review(
        remaining,
        crate::successor_selection::PatchVerdict::Rejected,
        vec![
            "the persisted V25 evidence has no focused regression for the cross-file error propagation"
                .to_string(),
        ],
    );
    let candidates = traversal_selection::Candidates::from_history(HistoryCandidates {
        scope: entry.scope.clone(),
        candidates: Vec::new(),
    })
    .with_current_generation(entry.scope.clone(), all_rejected)
    .expect("bind all-rejected V25 candidates");
    let attempt = traversal_selection::select_attempt_with_policy(
        candidates,
        traversal.seed,
        strict_strategy,
        entry.metrics.policy.clone(),
        &traversal.oracle_targets,
    )
    .expect("run production traversal for all-rejected V25 overlay");
    let traversal_selection::SelectionAttempt::NoSelection(receipt) = attempt else {
        panic!("all-rejected V25 overlay must preserve no selection")
    };
    let no_selection = SelectionDecisionEntry::new_no_selection_with_traversal_metrics(
        ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
        entry.scope.clone(),
        receipt.considered,
        receipt.considered_sources,
        receipt.projection_failures,
        Some(TraversalEvidence {
            seed: traversal.seed,
            strategy: strict_strategy,
            oracle_targets: traversal.oracle_targets.clone(),
            selected_source: None,
            child_counts: receipt.child_counts,
        }),
        receipt.metrics,
    )
    .expect("construct in-memory V25 no-selection receipt");
    traversal_selection::validate_patch_gate(&no_selection, strict_gate)
        .expect("all V25 reviews satisfy the typed strict gate");
    traversal_selection::validate_patch_replay(&no_selection)
        .expect("all-rejected V25 receipt replays through production traversal");
    let traversal_selection::Formula::ScoreChildProp(formula) = &no_selection
        .formula
        .as_ref()
        .expect("V25 no-selection formula")
        .formula;
    assert_eq!(formula.patch_gate, strict_gate);
    assert_eq!(formula.rows.len(), 3);
    for node_id in [SELECTED_NODE, OTHER_BLOCKED_NODE, ADMISSIBLE_NODE] {
        let row = formula
            .rows
            .iter()
            .find(|row| row.node_id.as_deref() == Some(node_id))
            .expect("V25 exclusion row");
        assert!(!row.selectable);
        assert!(!row.selected);
        assert_eq!(
            row.exclusion_reason.as_deref(),
            Some("patch_gate_not_satisfied")
        );
    }

    // Selection accepted only deserialized values and paths for diagnostics;
    // it had no campaign/History/database handle capable of mutation.
    assert!(!history_dir.exists());
    assert!(!reviews_dir.exists());
    assert!(!owner_db.exists());
    assert_eq!(
        fs::read(fixture.join("selection-decision-entry.json"))
            .expect("re-read immutable V25 receipt"),
        bytes
    );
    for (name, expected) in FIXTURE_HASHES {
        let bytes = fs::read(fixture.join(name)).expect("re-read immutable v25 fixture artifact");
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected, "{name}");
    }
}

#[tokio::test]
async fn v16_all_unresolved_replay_persists_no_selection_evidence() {
    const NODE_ID: &str = "node-0d80c1aeea697e6b";
    const BRANCH_ID: &str = "branch-652e6dab480c355a";
    const FIXTURE_HASHES: [(&str, &str); 9] = [
        (
            "branch-652e6dab480c355a.evaluation.json",
            "91a9aea152cff69f171823126ae90a93b939d88ccc87db11be683f3bf6f406ec",
        ),
        (
            "campaign.json",
            "dcb2679451b905181fb99a8fe038acd33ca27b29a97a00c8dfd653d50fdfb57b",
        ),
        (
            "child-node.json",
            "353bdf9fb01e0f2bdde9e02eeaabf34ce497edb0f01043bb3460cd8738422161",
        ),
        (
            "child-plan-node-e4ecdce2d6ee1098.json",
            "ea3bcc6b9528577af591681afa171055158f35b612787bc498e0a67289b7ab6e",
        ),
        (
            "child-runner-result.json",
            "855cfaa8618d4739c4ea64a9f7c636a9634ab5ae146516b397be3586b1b7d2a0",
        ),
        (
            "child-to-parent.jsonl",
            "bc0a7c2377057392a1b4096088138aca8d1bbdb96163664928e93af2903b4e32",
        ),
        (
            "parent_identity.json",
            "dc89d998348061704bdab22269a34e297d80e856ed4a16acc299ae03135f66a3",
        ),
        (
            "run-profile.commitment.json",
            "84844a242dd344e5b4bd47aa0a4c32a721ea91b1a260eb4159d3ed0a45cbbdde",
        ),
        (
            "run-profile.toml",
            "0949cf0e14e78111577e3823f433468301b13062b89eaea83f0533ee946d4d77",
        ),
    ];

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/prototype1-v16-all-unresolved-20260717");
    for (name, expected) in FIXTURE_HASHES {
        let bytes = fs::read(fixture.join(name)).expect("read immutable v16 fixture artifact");
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected, "{name}");
    }
    let temp = tempfile::tempdir().expect("v16 replay tempdir");
    let baseline_bytes = hex_fixture_bytes(&fixture.join("baseline-record.json.gz.hex"));
    assert_eq!(
        format!("{:x}", Sha256::digest(&baseline_bytes)),
        "f4c6adc1451f86ef072d218dd0ef6fb6a66c6ddcfa16405ee5b0122d30db820f"
    );
    let baseline_path = temp.path().join("baseline-record.json.gz");
    fs::write(&baseline_path, baseline_bytes).expect("stage v16 baseline record");
    let treatment_bytes = hex_fixture_bytes(&fixture.join("treatment-record.json.gz.hex"));
    assert_eq!(
        format!("{:x}", Sha256::digest(&treatment_bytes)),
        "09d191acf579a994e6fea9706575ccc8567d4a43139877444ac455967b2e7dd5"
    );
    let treatment_path = temp.path().join("treatment-record.json.gz");
    fs::write(&treatment_path, treatment_bytes).expect("stage v16 treatment record");

    let profile_text =
        fs::read_to_string(fixture.join("run-profile.toml")).expect("read v16 run profile");
    let profile: profile::Prototype1RunProfile =
        toml::from_str(&profile_text).expect("parse v16 run profile");
    profile.validate().expect("v16 profile remains valid");
    assert_eq!(
        profile.selection.oracle_mode(),
        crate::successor_selection::OracleMode::RecordOnly
    );
    assert!(profile.selection.oracle_require_evidence());
    assert_eq!(
        profile.selection.oracle_gate(),
        crate::successor_selection::OracleGate::AllResolved
    );
    assert!(profile.execution.mbe.enabled);

    let commitment: serde_json::Value = json_fixture(
        &fs::read_to_string(fixture.join("run-profile.commitment.json"))
            .expect("read v16 profile commitment"),
    );
    let profile_hash = format!("{:x}", Sha256::digest(profile_text.as_bytes()));
    assert_eq!(commitment["sha256"], profile_hash);
    assert_eq!(
        profile_hash,
        "0949cf0e14e78111577e3823f433468301b13062b89eaea83f0533ee946d4d77"
    );

    let parent: ParentIdentity = json_fixture(
        &fs::read_to_string(fixture.join("parent_identity.json"))
            .expect("read v16 parent identity"),
    );
    assert_eq!(parent.node_id(), "node-e4ecdce2d6ee1098");
    assert_eq!(parent.generation(), 0);

    let child_plan: ChildPlanFiles = json_fixture(
        &fs::read_to_string(fixture.join("child-plan-node-e4ecdce2d6ee1098.json"))
            .expect("read v16 child plan"),
    );
    assert_eq!(child_plan.rejected_surface_attempts().len(), 2);
    let (plan_index, child) = child_plan
        .children()
        .iter()
        .enumerate()
        .find(|(_, child)| child.node_id() == NODE_ID)
        .expect("v16 admitted child plan entry");

    let node: Prototype1NodeRecord = json_fixture(
        &fs::read_to_string(fixture.join("child-node.json")).expect("read v16 child node"),
    );
    assert_eq!(node.node_id, NODE_ID);
    assert_eq!(node.branch_id, BRANCH_ID);

    let runner_result: Prototype1RunnerResult = json_fixture(
        &fs::read_to_string(fixture.join("child-runner-result.json"))
            .expect("read v16 runner result"),
    );
    assert_eq!(runner_result.node_id, node.node_id);
    assert_eq!(runner_result.branch_id, node.branch_id);

    let mut report: Prototype1BranchEvaluationReport = json_fixture(
        &fs::read_to_string(fixture.join("branch-652e6dab480c355a.evaluation.json"))
            .expect("read v16 branch evaluation"),
    );
    assert_eq!(report.branch_id, BRANCH_ID);
    assert_eq!(report.overall_disposition, BranchDisposition::Keep);
    assert_eq!(
        report
            .eval_set_identity
            .as_ref()
            .expect("v16 eval set identity")
            .instance_ids,
        vec!["BurntSushi__ripgrep-2209".to_string()]
    );
    assert_eq!(report.compared_instances.len(), 1);
    let oracle = report.compared_instances[0]
        .oracle_evaluation
        .as_ref()
        .expect("v16 oracle evaluation");
    assert_eq!(oracle.evidence.verdict, crate::mbe::Verdict::Unresolved);
    assert!(oracle.usable_for_selection);
    let instance_report = oracle
        .instance_report
        .as_ref()
        .expect("v16 MBE instance report");
    assert_eq!(instance_report.valid, Some(true));
    assert!(
        instance_report
            .fixed_tests
            .contains_key("regression::r2095")
    );
    assert!(
        instance_report
            .fix_patch_result
            .failed_tests
            .contains("regression::r2208")
    );
    report.compared_instances[0].baseline_record_path = Some(baseline_path);
    report.compared_instances[0].treatment_record_path = Some(treatment_path);

    let terminal = fs::read_to_string(fixture.join("child-to-parent.jsonl"))
        .expect("read v16 child channel")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<
                crate::cli::prototype1_state::channel::Envelope<
                    crate::cli::prototype1_state::channel::ToParent,
                >,
            >(line)
            .expect("v16 child channel envelope")
        })
        .find_map(|envelope| match envelope.body() {
            crate::cli::prototype1_state::channel::ToParent::Result {
                runner_result,
                treatment,
            } => Some((
                envelope.runtime_id().to_string(),
                runner_result.clone(),
                treatment.clone(),
            )),
            _ => None,
        })
        .expect("v16 child terminal result");
    assert_eq!(terminal.1, runner_result);
    let terminal_body = crate::cli::prototype1_state::channel::ToParent::Result {
        runner_result: terminal.1.clone(),
        treatment: terminal.2,
    };
    let channel_evidence = ChildChannelEvidenceRefs {
        runtime_id: terminal.0.clone(),
        terminal_result: SealedEvidenceCitation {
            ref_id: format!(
                "channel:child-to-parent:terminal-result:{NODE_ID}:{}",
                terminal.0
            ),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_channel_terminal_result.v1",
                    &terminal_body,
                )
                .expect("v16 terminal channel hash"),
            ),
            record_name: Some(CHILD_CHANNEL_TERMINAL_RESULT_RECORD.to_string()),
        },
        attempt_result: Some(SealedEvidenceCitation {
            ref_id: format!("child-store:attempt-runner-result:{NODE_ID}:{}", terminal.0),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_attempt_runner_result.v1",
                    &runner_result,
                )
                .expect("v16 attempt result hash"),
            ),
            record_name: Some(CHILD_ATTEMPT_RUNNER_RESULT_RECORD.to_string()),
        }),
        invocation: None,
    };
    let outcome = PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: "completed:Keep".to_string(),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved: child.resolved().clone(),
        child_runtime: Some(terminal.0),
        channel_evidence: Some(channel_evidence),
        evaluation_report: Some(report.clone()),
        selection_input: Some(selection_input_from_child_report(&node, &report)),
        surface: child.surface().cloned(),
        harness: child.harness_evidence().cloned(),
        artifact_surface: Some(
            child
                .harness_evidence()
                .expect("v16 broad harness evidence")
                .artifact_surface()
                .clone(),
        ),
        node,
    };

    let manifest_path = temp.path().join("campaign.json");
    fs::copy(fixture.join("campaign.json"), &manifest_path).expect("stage v16 campaign manifest");
    let manifest: crate::campaign::CampaignManifest = json_fixture(
        &fs::read_to_string(&manifest_path).expect("read staged v16 campaign manifest"),
    );
    let operator = profile::OperatorRunProfile {
        source_path: fixture.join("run-profile.toml"),
        profile: profile.clone(),
    };
    let admitted =
        profile::admit_run_profile(&manifest_path, &operator).expect("admit v16 replay profile");
    let setup_closure = eval_store::sample_closure_state(parent.campaign_id().clone());
    let setup_path = temp.path().join("closure-state.json");
    fs::write(
        &setup_path,
        serde_json::to_vec_pretty(&setup_closure).expect("serialize v16 setup closure"),
    )
    .expect("write v16 setup closure");
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
    eval_store::write_r0_context_to_owner_db(
        &db_path,
        &manifest_path,
        &manifest,
        profile.storage.eval.backend,
        Some(&admitted),
        None,
        &setup_path,
        &setup_closure,
    )
    .expect("seed v16 admitted setup authority");
    let db_before = fs::read(&db_path).expect("read owner DB before pure selection");

    let pure_outcome = selection_outcome_for_profile(
        &manifest_path,
        &parent,
        std::slice::from_ref(&outcome),
        child_plan.rejected_surface_attempts(),
        &profile,
    )
    .expect("compute pure v16 selection outcome");
    assert_eq!(
        fs::read(&db_path).expect("read owner DB after pure selection"),
        db_before,
        "pure selection must not persist eval rows"
    );
    let entry = match pure_outcome {
        ParentSelectionOutcome::NoSelection { entry } => entry,
        ParentSelectionOutcome::Selected { .. } => {
            panic!("v16 unresolved oracle evidence must not select a successor")
        }
    };
    entry
        .validate_shape()
        .expect("v16 no-selection receipt shape");
    assert_eq!(entry.schema_version, 5);
    assert!(entry.decision.is_none());
    assert!(entry.selected_candidate.is_none());
    assert!(entry.selected_occurrence_id.is_none());
    assert!(entry.selected_membership_id.is_none());
    assert_eq!(entry.considered.len(), 1);
    assert_eq!(entry.projection_failures.len(), 4);
    let formula = entry.formula.as_ref().expect("v16 selection formula");
    let crate::successor_selection::traversal::Formula::ScoreChildProp(formula) = &formula.formula;
    assert_eq!(formula.rows.len(), 1);
    let formula_row = &formula.rows[0];
    assert_eq!(formula_row.node_id.as_deref(), Some(NODE_ID));
    assert_eq!(formula_row.branch_id.as_deref(), Some(BRANCH_ID));
    assert_eq!(formula_row.oracle_resolved, Some(0));
    assert_eq!(formula_row.oracle_configured, Some(1));
    assert!(!formula_row.selectable);
    assert_eq!(
        formula_row.exclusion_reason.as_deref(),
        Some("oracle_gate_not_satisfied")
    );
    assert!(!formula_row.selected);
    let receipt_hash = entry.receipt_hash().expect("hash v16 no-selection receipt");
    let expected_hash = receipt_hash.as_str().to_string();
    let mut tampered = entry.clone();
    tampered.projection_failures[0].committed_message =
        Some("tampered projection evidence".to_string());
    assert!(tampered.receipt_hash().is_err());

    let selected = select_successor_for_profile(
        &manifest_path,
        &parent,
        std::slice::from_ref(&outcome),
        child_plan.rejected_surface_attempts(),
        &profile,
    )
    .expect("persist v16 no-selection outcome");
    assert!(selected.is_none());

    let db = eval_store::load_owner_eval_database(&db_path).expect("load v16 owner eval DB");
    let decision_rows = db
        .raw_query_params(
            r#"
?[
    decision_id,
    parent_id,
    procedure_id,
    selected_node_id,
    selected_artifact_id,
    outcome,
    disposition,
    decision_hash
] :=
    *eval_selection_decision {
        decision_id,
        parent_id,
        procedure_id,
        selected_node_id,
        selected_artifact_id,
        outcome,
        disposition,
        decision_hash
    }
"#,
            std::collections::BTreeMap::new(),
        )
        .expect("query v16 selection decision");
    assert_eq!(decision_rows.rows.len(), 1);
    let decision_row = decision_rows.row_refs().next().expect("v16 decision row");
    let decision_id = decision_row
        .get::<String>("decision_id")
        .expect("v16 decision id");
    assert_eq!(
        decision_row.get::<String>("parent_id").expect("v16 parent"),
        parent.parent_id()
    );
    assert_eq!(
        decision_row
            .get::<String>("procedure_id")
            .expect("v16 procedure"),
        crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID
    );
    assert_eq!(
        decision_row
            .get::<Option<String>>("selected_node_id")
            .expect("v16 selected node"),
        None
    );
    assert_eq!(
        decision_row
            .get::<Option<String>>("selected_artifact_id")
            .expect("v16 selected artifact"),
        None
    );
    assert_eq!(
        decision_row.get::<String>("outcome").expect("v16 outcome"),
        "no_selection"
    );
    assert_eq!(
        decision_row
            .get::<Option<String>>("disposition")
            .expect("v16 disposition"),
        None
    );
    assert_eq!(
        decision_row
            .get::<String>("decision_hash")
            .expect("v16 decision hash"),
        expected_hash
    );

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "decision_id".to_string(),
        cozo::DataValue::from(decision_id.clone()),
    );
    let candidate_rows = db
        .raw_query_params(
            r#"
?[node_id, branch_id, selectable, selected, exclusion_ref] :=
    *eval_selection_candidate {
        decision_id,
        node_id,
        branch_id,
        selectable,
        selected,
        exclusion_ref
    },
    decision_id = $decision_id
"#,
            params.clone(),
        )
        .expect("query v16 selection candidate");
    assert_eq!(candidate_rows.rows.len(), 1);
    let candidate_row = candidate_rows.row_refs().next().expect("v16 candidate row");
    assert_eq!(
        candidate_row.get::<String>("node_id").expect("v16 node"),
        NODE_ID
    );
    assert_eq!(
        candidate_row
            .get::<String>("branch_id")
            .expect("v16 branch"),
        BRANCH_ID
    );
    assert!(
        !candidate_row
            .get::<bool>("selectable")
            .expect("v16 selectable")
    );
    assert!(!candidate_row.get::<bool>("selected").expect("v16 selected"));
    assert_eq!(
        candidate_row
            .get::<Option<String>>("exclusion_ref")
            .expect("v16 exclusion")
            .as_deref(),
        Some("oracle_gate_not_satisfied")
    );

    let score_rows = db
        .raw_query_params(
            r#"
?[formula_id, score_json, weight, selected] :=
    *eval_selection_score {
        decision_id,
        formula_id,
        score_json,
        weight,
        selected
    },
    decision_id = $decision_id
"#,
            params.clone(),
        )
        .expect("query v16 selection score");
    assert_eq!(score_rows.rows.len(), 1);
    let score_row = score_rows.row_refs().next().expect("v16 score row");
    assert!(
        score_row
            .get::<String>("formula_id")
            .expect("v16 formula id")
            .starts_with("score_child_prop:")
    );
    assert_eq!(
        score_row.get::<Option<f64>>("weight").expect("v16 weight"),
        None
    );
    assert!(
        !score_row
            .get::<bool>("selected")
            .expect("v16 score selected")
    );
    let score_json: serde_json::Value = serde_json::from_str(
        &score_row
            .get::<String>("score_json")
            .expect("v16 score JSON"),
    )
    .expect("parse v16 score JSON");
    assert_eq!(score_json["oracle_resolved"], 0);
    assert_eq!(score_json["oracle_configured"], 1);
    assert_eq!(score_json["selectable"], false);
    assert_eq!(score_json["exclusion_reason"], "oracle_gate_not_satisfied");
    assert_eq!(score_json["selected"], false);

    let oracle_rows = db
        .raw_query_params(
            r#"
?[mode, require_evidence, gate, targets, formula_id] :=
    *eval_selection_oracle {
        decision_id,
        mode,
        require_evidence,
        gate,
        targets,
        formula_id
    },
    decision_id = $decision_id
"#,
            params.clone(),
        )
        .expect("query v16 selection oracle");
    assert_eq!(oracle_rows.rows.len(), 1);
    let oracle_row = oracle_rows.row_refs().next().expect("v16 oracle row");
    assert_eq!(
        oracle_row.get::<String>("mode").expect("v16 oracle mode"),
        "record-only"
    );
    assert!(
        oracle_row
            .get::<bool>("require_evidence")
            .expect("v16 oracle evidence policy")
    );
    assert_eq!(
        oracle_row.get::<String>("gate").expect("v16 oracle gate"),
        "all-resolved"
    );
    assert_eq!(
        oracle_row
            .get::<Vec<String>>("targets")
            .expect("v16 oracle targets"),
        vec!["BurntSushi__ripgrep-2209".to_string()]
    );
    assert!(
        oracle_row
            .get::<String>("formula_id")
            .expect("v16 oracle formula")
            .starts_with("score_child_prop:")
    );

    let projection_rows = db
        .raw_query_params(
            r#"
?[failure_id, candidate_subject, kind, message] :=
    *eval_selection_projection_failure {
        decision_id,
        failure_id,
        candidate_subject,
        kind,
        message
    },
    decision_id = $decision_id
"#,
            params,
        )
        .expect("query v16 selection projection failures");
    assert_eq!(projection_rows.rows.len(), 4);
    let mut subjects = std::collections::BTreeMap::<String, usize>::new();
    let mut messages = std::collections::BTreeSet::new();
    for row in projection_rows.row_refs() {
        assert!(
            !row.get::<String>("failure_id")
                .expect("v16 projection failure id")
                .is_empty()
        );
        assert_eq!(
            row.get::<String>("kind")
                .expect("v16 projection failure kind"),
            "missing_selection_input"
        );
        let subject = row
            .get::<Option<String>>("candidate_subject")
            .expect("v16 projection subject")
            .expect("v16 rejected surface candidate");
        assert!(subject.starts_with("candidate:rejected_surface_attempt:"));
        *subjects.entry(subject).or_default() += 1;
        messages.insert(
            row.get::<Option<String>>("message")
                .expect("v16 projection message")
                .expect("v16 committed projection message"),
        );
    }
    assert_eq!(subjects.len(), 2);
    assert!(subjects.values().all(|count| *count == 2));
    assert!(
        messages
            .contains("missing_selection_input: rejected_surface_attempt_without_child_runtime")
    );
    assert!(messages.contains("traversal: missing SelectionInput"));
    drop(db);

    let history_root = temp.path().join("prototype1/history");
    assert!(
        !history_root.exists(),
        "passive no-selection evidence must not create History"
    );
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    assert!(
        !journal_path.exists(),
        "selection replay alone must not create a successor handoff journal"
    );
    let journal_entries = PrototypeJournal::new(&journal_path)
        .load_entries()
        .expect("load empty v16 replay journal");
    assert!(
        journal_entries
            .iter()
            .all(|entry| !matches!(entry, JournalEntry::Successor(_)))
    );

    let child_plan_path = child_plan_message_path_for_parent(&manifest_path, &parent);
    fs::create_dir_all(child_plan_path.parent().expect("v16 child-plan parent"))
        .expect("create v16 child-plan directory");
    let mut child_plan_json: serde_json::Value = json_fixture(
        &fs::read_to_string(fixture.join("child-plan-node-e4ecdce2d6ee1098.json"))
            .expect("read v16 child-plan fixture for R12"),
    );
    child_plan_json["message"] = serde_json::Value::String(
        child_plan_path
            .to_str()
            .expect("temporary v16 child-plan path is UTF-8")
            .to_string(),
    );
    fs::write(
        &child_plan_path,
        serde_json::to_vec_pretty(&child_plan_json).expect("serialize re-homed v16 child plan"),
    )
    .expect("stage v16 child plan");

    let repo_root = temp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(&repo_root, Path::new("README.md"), "v16 R12 replay\n");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "v16 R12 replay");
    let head_before = GitWorktreeBackend
        .head_commit(&repo_root)
        .expect("v16 R12 replay head");
    let status_before = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("v16 R12 replay status");
    assert!(status_before.status.success());

    let unchecked =
        Parent::<Unchecked>::load(&manifest_path, parent.clone()).expect("load v16 parent for R12");
    let checked = unchecked
        .check(
            &NoopBackend,
            &manifest_path,
            Check {
                campaign_id: parent.campaign_id(),
                active_root: &repo_root,
            },
        )
        .expect("check v16 parent for R12");
    let startup = Startup::<Genesis>::from_history(checked.identity(), &manifest_path)
        .expect("validate v16 genesis startup");
    let ready = checked.ready(startup).expect("ready v16 parent for R12");
    let planned = load_existing_child_plan_for_id(parent.campaign_id(), &manifest_path, ready)
        .expect("load staged v16 child plan");
    let selectable_parent = planned.parent;

    let mut command = state_command_without_ids();
    command.campaign = Some(parent.campaign_id().clone());
    command.repo_root = Some(repo_root.clone());
    let run_shape = Prototype1StateRunShape::from_profile(&profile);
    let config = ResolvedCampaignConfig {
        campaign_id: parent.campaign_id().clone(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        model_id: profile.model.id.clone().expect("v16 profile model id"),
        provider_slug: profile.model.provider.clone(),
        route_source: profile
            .model
            .route_source
            .expect("v16 profile route source"),
        required_procedures: Vec::new(),
        instances_root: temp.path().join("instances"),
        batches_root: temp.path().join("batches"),
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: crate::FrameworkConfig::default(),
    };
    let facts = typestate::context::Facts {
        complete_search_policy: Some(profile.search_policy()),
        selection: Some(ParentSelectionOutcome::NoSelection { entry }),
        report: Some(typestate::context::ReportFacts {
            outcome: "historical v16 no-selection".to_string(),
            node_id: outcome.node_id.clone(),
            node_status: outcome.node_status,
            workspace_root: outcome.workspace_root.clone(),
            binary_path: outcome.binary_path.clone(),
            child_runtime: outcome.child_runtime.clone(),
            successor_runtime: None,
            successor_pid: None,
            successor_ready_path: None,
        }),
        ..Default::default()
    };
    let collected = typestate::context::Collected::new(
        command,
        repo_root.clone(),
        parent.campaign_id().clone(),
        manifest_path.clone(),
        run_shape,
        config,
        journal_path.clone(),
        PrototypeJournal::new(&journal_path),
    )
    .with_facts(facts);
    let r12 = typestate::R12::from_collected_parent(collected, selectable_parent);
    let step = crate::cli::prototype1_state::driver::control::test_r12_stop(&repo_root, r12)
        .await
        .expect("v16 no-selection R12 must stop");
    assert_eq!(step.transition().from(), WalkPhase::R12);
    assert_eq!(step.transition().to(), WalkPhase::R13a);
    assert!(matches!(
        step.state(),
        crate::cli::prototype1_state::driver::control::ControlState::R13a(_)
    ));

    let journal_entries = PrototypeJournal::new(&journal_path)
        .load_entries()
        .expect("load v16 stopped journal");
    assert_eq!(journal_entries.len(), 1);
    let stopped = journal_entries
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::Successor(record) if record.node_id == parent.node_id() => Some(record),
            _ => None,
        })
        .expect("v16 parent-node stopped record");
    assert_eq!(stopped.runtime_id, None);
    let crate::cli::prototype1_state::successor::State::Stopped {
        decision,
        selection_decision,
        selection_receipt,
    } = &stopped.state
    else {
        panic!("v16 no-selection R12 must persist a stopped successor record")
    };
    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::StopNoSelectedBranch
    );
    assert!(selection_decision.is_none());
    assert_eq!(
        selection_receipt,
        &Some(
            crate::cli::prototype1_state::successor::SelectionReceipt::Completed {
                hash: receipt_hash,
            }
        )
    );
    assert!(journal_entries.iter().all(|entry| {
        !matches!(
            entry,
            JournalEntry::Successor(crate::cli::prototype1_state::successor::Record {
                state: crate::cli::prototype1_state::successor::State::Checkout { .. },
                ..
            })
        )
    }));
    assert!(
        !history_root.exists(),
        "schema-v5 no-selection stop must not create History"
    );
    assert_eq!(
        GitWorktreeBackend
            .head_commit(&repo_root)
            .expect("v16 head after R12 stop"),
        head_before
    );
    let status_after = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("v16 status after R12 stop");
    assert!(status_after.status.success());
    assert_eq!(status_after.stdout, status_before.stdout);
}

#[tokio::test]
async fn r12_rejected_only_records_selection_not_run() {
    let temp = tempfile::tempdir().expect("rejected-only R12 tempdir");
    let manifest_path = temp.path().join("campaign.json");
    let repo_root = temp.path().join("repo");
    init_indexed_repo(&repo_root);
    write_surface_target(&repo_root, Path::new("README.md"), "rejected-only R12\n");
    index_repo(&repo_root);
    commit_indexed_repo(&repo_root, "rejected-only R12");

    let ready = ready_parent_for_test(&manifest_path, &repo_root);
    let parent = ready.identity().clone();
    let rejected = surface_attempt::Evidence::rejected(
        TUI_EDIT_SURFACE_PRODUCER_ID,
        "proposal-rejected",
        "run-rejected",
        "workspace_except_ploke_eval",
        PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"),
        "synthetic rejected-only R12 evidence",
    );
    persist_rejected_surface_attempt_child_plan(
        parent.campaign_id(),
        &manifest_path,
        ready,
        vec![rejected.clone()],
    )
    .expect("persist rejected-only child plan");
    let ready = ready_parent_for_test(&manifest_path, &repo_root);
    let planned = load_existing_child_plan_for_id(parent.campaign_id(), &manifest_path, ready)
        .expect("load rejected-only child plan");
    assert!(planned.children.is_empty());
    assert_eq!(
        planned.rejected_surface_attempts.as_slice(),
        std::slice::from_ref(&rejected)
    );
    let selectable_parent = planned.parent;

    let mut command = state_command_without_ids();
    command.campaign = Some(parent.campaign_id().clone());
    command.repo_root = Some(repo_root.clone());
    let run_shape = Prototype1StateRunShape::from_command(&command);
    let config = ResolvedCampaignConfig {
        campaign_id: parent.campaign_id().clone(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        model_id: "test-model".to_string(),
        provider_slug: None,
        route_source: ModelRouteSource::DirectGoogle,
        required_procedures: Vec::new(),
        instances_root: temp.path().join("instances"),
        batches_root: temp.path().join("batches"),
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: crate::FrameworkConfig::default(),
    };
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let facts = typestate::context::Facts {
        complete_search_policy: Some(Prototype1SearchPolicy::default()),
        selection: None,
        rejected_attempt_payloads: Some(1),
        report: Some(typestate::context::ReportFacts {
            outcome: "synthetic rejected-only R12".to_string(),
            node_id: parent.node_id().to_string(),
            node_status: Prototype1NodeStatus::Running,
            workspace_root: repo_root.clone(),
            binary_path: temp.path().join("unused-ploke-eval"),
            child_runtime: None,
            successor_runtime: None,
            successor_pid: None,
            successor_ready_path: None,
        }),
        ..Default::default()
    };
    let collected = typestate::context::Collected::new(
        command,
        repo_root.clone(),
        parent.campaign_id().clone(),
        manifest_path.clone(),
        run_shape,
        config,
        journal_path.clone(),
        PrototypeJournal::new(&journal_path),
    )
    .with_facts(facts);
    let r12 = typestate::R12::from_collected_parent(collected, selectable_parent);

    let history_root = temp.path().join("prototype1/history");
    assert!(!history_root.exists());
    let head_before = GitWorktreeBackend
        .head_commit(&repo_root)
        .expect("rejected-only R12 head");
    let status_before = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("rejected-only R12 status");
    assert!(status_before.status.success());

    let step = crate::cli::prototype1_state::driver::control::test_r12_stop(&repo_root, r12)
        .await
        .expect("rejected-only R12 must stop");
    assert_eq!(step.transition().from(), WalkPhase::R12);
    assert_eq!(step.transition().to(), WalkPhase::R13a);
    assert!(matches!(
        step.state(),
        crate::cli::prototype1_state::driver::control::ControlState::R13a(_)
    ));

    let journal_entries = PrototypeJournal::new(&journal_path)
        .load_entries()
        .expect("load rejected-only stopped journal");
    assert_eq!(journal_entries.len(), 1);
    let stopped = journal_entries
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::Successor(record) if record.node_id == parent.node_id() => Some(record),
            _ => None,
        })
        .expect("rejected-only parent-node stopped record");
    assert_eq!(stopped.runtime_id, None);
    let crate::cli::prototype1_state::successor::State::Stopped {
        decision,
        selection_decision,
        selection_receipt,
    } = &stopped.state
    else {
        panic!("rejected-only R12 must persist a stopped successor record")
    };
    assert_eq!(
        decision.disposition,
        Prototype1ContinuationDisposition::StopNoSelectedBranch
    );
    assert!(selection_decision.is_none());
    assert_eq!(
        selection_receipt,
        &Some(crate::cli::prototype1_state::successor::SelectionReceipt::NotRun)
    );
    assert!(journal_entries.iter().all(|entry| {
        !matches!(
            entry,
            JournalEntry::Successor(crate::cli::prototype1_state::successor::Record {
                state: crate::cli::prototype1_state::successor::State::Checkout { .. },
                ..
            })
        )
    }));
    assert!(
        !history_root.exists(),
        "rejected-only stop must not create History"
    );
    assert_eq!(
        GitWorktreeBackend
            .head_commit(&repo_root)
            .expect("rejected-only head after R12 stop"),
        head_before
    );
    let status_after = std::process::Command::new("git")
        .current_dir(&repo_root)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("rejected-only status after R12 stop");
    assert!(status_after.status.success());
    assert_eq!(status_after.stdout, status_before.stdout);
}

#[test]
fn historical_node_150_channel_treatment_reaches_current_generation_handoff() {
    const NODE_ID: &str = "node-15006265e24b3b9b";

    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let db_path = eval_store::prototype1_eval_store_db_path(&manifest_path);
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
        CampaignId::from(
            "p1-gemini35-flash-direct-15g2x3-20260525-035000-treatment-branch-c56614c6e6a63aa9-1779711014414"
        )
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
    let mut treatment_closure: crate::closure::ClosureState = json_fixture(include_str!(
        "../../../tests/fixtures/prototype1-node-150-handoff/treatment_closure_state.json"
    ));
    let treatment_row = treatment_closure
        .instances
        .iter_mut()
        .find(|row| row.instance_id == terminal.2.instances[0].instance_id)
        .expect("matching treatment closure row");
    treatment_row.artifacts.record_path = Some(treatment_record_path.clone());
    let treatment_campaign = Prototype1LoopCampaign {
        campaign_id: treatment_closure.campaign_id.clone(),
        manifest_path: terminal.2.treatment_campaign_manifest.clone(),
        closure_state_path: terminal.2.treatment_closure_state_path.clone(),
        slice_dataset_path: treatment_closure
            .config
            .dataset_sources
            .first()
            .expect("treatment closure dataset source")
            .path
            .clone(),
        resolved: ResolvedCampaignConfig {
            campaign_id: treatment_closure.campaign_id.clone(),
            benchmark_family: treatment_closure.config.benchmark_family,
            dataset_sources: treatment_closure.config.dataset_sources.clone(),
            model_id: treatment_closure
                .config
                .model_id
                .clone()
                .expect("treatment closure model id"),
            provider_slug: treatment_closure.config.provider_slug.clone(),
            route_source: treatment_closure
                .config
                .route_source
                .expect("treatment closure route source"),
            required_procedures: treatment_closure.config.required_procedures.clone(),
            instances_root: treatment_closure.config.instances_root.clone(),
            batches_root: treatment_closure.config.batches_root.clone(),
            eval: terminal.2.eval_policy.clone(),
            // The treatment evidence builder reads eval/run records here; protocol
            // policy is not part of this reconstruction contract.
            protocol: ProtocolCampaignPolicy::default(),
            framework: treatment_closure.config.framework.clone(),
        },
    };
    let rebuilt_treatment = build_prototype1_treatment_evidence(
        parent_identity.campaign_id(),
        &node.branch_id,
        &treatment_campaign,
        &treatment_closure,
    )
    .expect("rebuild treatment evidence from historical closure state");
    assert_eq!(
        rebuilt_treatment.treatment_campaign_id,
        terminal.2.treatment_campaign_id
    );
    assert_eq!(rebuilt_treatment.branch_id, terminal.2.branch_id);
    assert_eq!(
        rebuilt_treatment.instances.len(),
        terminal.2.instances.len()
    );
    assert_eq!(
        rebuilt_treatment.instances[0].metrics,
        terminal.2.instances[0].metrics
    );
    assert_eq!(
        rebuilt_treatment.instances[0].status,
        terminal.2.instances[0].status
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

    let terminal_body = crate::cli::prototype1_state::channel::ToParent::Result {
        runner_result: terminal.1.clone(),
        treatment: Some(terminal.2.clone()),
    };
    let channel_evidence = ChildChannelEvidenceRefs {
        runtime_id: terminal.0.clone(),
        terminal_result: SealedEvidenceCitation {
            ref_id: format!(
                "channel:child-to-parent:terminal-result:{}:{}",
                node.node_id, terminal.0
            ),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_channel_terminal_result.v1",
                    &terminal_body,
                )
                .expect("terminal channel hash"),
            ),
            record_name: Some(CHILD_CHANNEL_TERMINAL_RESULT_RECORD.to_string()),
        },
        attempt_result: Some(SealedEvidenceCitation {
            ref_id: format!(
                "child-store:attempt-runner-result:{}:{}",
                node.node_id, terminal.0
            ),
            content_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.child_attempt_runner_result.v1",
                    &terminal.1,
                )
                .expect("attempt result hash"),
            ),
            record_name: Some(CHILD_ATTEMPT_RUNNER_RESULT_RECORD.to_string()),
        }),
        invocation: None,
    };
    let outcome = PlannedChildOutcome {
        plan_index,
        node_id: node.node_id.clone(),
        outcome: format!("completed:{:?}", report.overall_disposition),
        node_status: node.status,
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        resolved: child.resolved().clone(),
        child_runtime: Some(terminal.0.clone()),
        channel_evidence: Some(channel_evidence),
        evaluation_report: Some(report.clone()),
        selection_input: Some(selection_input_from_child_report(&node, &report)),
        surface: child.surface().cloned(),
        harness: child.harness_evidence().cloned(),
        artifact_surface: Some(
            child
                .harness_evidence()
                .expect("historical child has broad harness evidence")
                .artifact_surface()
                .clone(),
        ),
        node,
    };
    let profile_text = r#"
schema_version = "prototype1-run-profile.v1"
name = "historical-node-150-handoff"

[selection]
strategy = "history-score-child-prop"
evidence = "operational"
seed = 0
"#;
    let profile =
        toml::from_str::<profile::Prototype1RunProfile>(profile_text).expect("profile parses");
    profile.validate().expect("profile validates");
    let manifest = crate::campaign::CampaignManifest::new(parent_identity.campaign_id().clone());
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize node-150 campaign manifest"),
    )
    .expect("write node-150 campaign manifest");
    let source_path = tmp.path().join("historical-node-150.toml");
    fs::write(&source_path, profile_text).expect("write node-150 operator profile");
    let admitted = profile::admit_run_profile(
        &manifest_path,
        &profile::OperatorRunProfile {
            source_path,
            profile: profile.clone(),
        },
    )
    .expect("admit node-150 replay profile");
    let closure_path = tmp.path().join("closure-state.json");
    fs::write(
        &closure_path,
        serde_json::to_vec_pretty(&closure).expect("serialize node-150 closure"),
    )
    .expect("write node-150 closure");
    eval_store::write_r0_context_to_owner_db(
        &db_path,
        &manifest_path,
        &manifest,
        profile.storage.eval.backend,
        Some(&admitted),
        None,
        &closure_path,
        &closure,
    )
    .expect("seed node-150 admitted setup authority");

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
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let decision_rows = db
        .raw_query_params(
            r#"
?[
    decision_id,
    parent_id,
    procedure_id,
    selected_node_id,
    outcome,
    disposition,
    decision_hash
] :=
    *eval_selection_decision {
        decision_id,
        parent_id,
        procedure_id,
        selected_node_id,
        outcome,
        disposition,
        decision_hash
    }
"#,
            std::collections::BTreeMap::new(),
        )
        .expect("query selection decision rows");
    assert_eq!(decision_rows.rows.len(), 1);
    let decision_row = decision_rows
        .row_refs()
        .next()
        .expect("selection decision row");
    let decision_id = decision_row
        .get::<String>("decision_id")
        .expect("decision id");
    assert_eq!(
        decision_row.get::<String>("parent_id").expect("parent"),
        parent_identity.parent_id()
    );
    assert_eq!(
        decision_row
            .get::<String>("procedure_id")
            .expect("procedure"),
        crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID
    );
    assert_eq!(
        decision_row
            .get::<String>("selected_node_id")
            .expect("selected node"),
        NODE_ID
    );
    assert_eq!(
        decision_row.get::<String>("outcome").expect("outcome"),
        "accepted"
    );
    assert_eq!(
        decision_row
            .get::<String>("disposition")
            .expect("disposition"),
        "keep"
    );
    assert!(
        !decision_row
            .get::<String>("decision_hash")
            .expect("decision hash")
            .is_empty()
    );

    let mut oracle_params = std::collections::BTreeMap::new();
    oracle_params.insert(
        "decision_id".to_string(),
        cozo::DataValue::from(decision_id.clone()),
    );
    let oracle_rows = db
        .raw_query_params(
            r#"
?[mode, require_evidence, gate, targets, formula_id] :=
    *eval_selection_oracle {
        decision_id,
        mode,
        require_evidence,
        gate,
        targets,
        formula_id
    },
    decision_id = $decision_id
"#,
            oracle_params,
        )
        .expect("query selection oracle row");
    assert_eq!(oracle_rows.rows.len(), 1);
    let oracle_row = oracle_rows.row_refs().next().expect("selection oracle row");
    assert_eq!(
        oracle_row.get::<String>("mode").expect("oracle mode"),
        "record-only"
    );
    assert!(
        oracle_row
            .get::<bool>("require_evidence")
            .expect("oracle evidence policy")
    );
    assert_eq!(
        oracle_row.get::<String>("gate").expect("oracle gate"),
        "disabled"
    );
    assert!(
        oracle_row
            .get::<Vec<String>>("targets")
            .expect("oracle targets")
            .is_empty()
    );
    assert!(
        oracle_row
            .get::<String>("formula_id")
            .expect("oracle formula")
            .starts_with("score_child_prop:")
    );

    let mut candidate_params = std::collections::BTreeMap::new();
    candidate_params.insert(
        "decision_id".to_string(),
        cozo::DataValue::from(decision_id.clone()),
    );
    let candidate_rows = db
        .raw_query_params(
            r#"
?[
    node_id,
    branch_id,
    selectable,
    selected
] :=
    *eval_selection_candidate {
        decision_id,
        node_id,
        branch_id,
        selectable,
        selected
    },
    decision_id = $decision_id
"#,
            candidate_params,
        )
        .expect("query selection candidate rows");
    assert_eq!(candidate_rows.rows.len(), 1);
    let candidate_row = candidate_rows
        .row_refs()
        .next()
        .expect("selection candidate row");
    assert_eq!(
        candidate_row.get::<String>("node_id").expect("node"),
        NODE_ID
    );
    assert_eq!(
        candidate_row.get::<String>("branch_id").expect("branch"),
        outcome.node.branch_id
    );
    assert!(candidate_row.get::<bool>("selectable").expect("selectable"));
    assert!(candidate_row.get::<bool>("selected").expect("selected"));
    let mut finding_params = std::collections::BTreeMap::new();
    finding_params.insert(
        "decision_id".to_string(),
        cozo::DataValue::from(decision_id.clone()),
    );
    finding_params.insert(
        "domain".to_string(),
        cozo::DataValue::from("operational".to_string()),
    );
    let finding_rows = db
        .raw_query_params(
            r#"
?[
    domain,
    verdict,
    confidence,
    member_id
] :=
    *eval_selection_finding {
        decision_id,
        domain,
        verdict,
        confidence,
        member_id
    },
    decision_id = $decision_id,
    domain = $domain
"#,
            finding_params,
        )
        .expect("query selection finding rows");
    assert_eq!(finding_rows.rows.len(), 1);
    let finding_row = finding_rows
        .row_refs()
        .next()
        .expect("selection finding row");
    assert_eq!(
        finding_row.get::<String>("domain").expect("domain"),
        "operational"
    );
    assert_eq!(
        finding_row.get::<String>("verdict").expect("verdict"),
        "better"
    );
    assert_eq!(
        finding_row.get::<String>("confidence").expect("confidence"),
        "high"
    );
    assert!(
        !finding_row
            .get::<String>("member_id")
            .expect("finding member id")
            .is_empty()
    );

    let mut score_params = std::collections::BTreeMap::new();
    score_params.insert(
        "decision_id".to_string(),
        cozo::DataValue::from(decision_id),
    );
    let score_rows = db
        .raw_query_params(
            r#"
?[
    formula_id,
    score_json,
    weight,
    selected
] :=
    *eval_selection_score {
        decision_id,
        formula_id,
        score_json,
        weight,
        selected
    },
    decision_id = $decision_id
"#,
            score_params,
        )
        .expect("query selection score rows");
    assert_eq!(score_rows.rows.len(), 1);
    let score_row = score_rows.row_refs().next().expect("selection score row");
    assert!(
        score_row
            .get::<String>("formula_id")
            .expect("formula")
            .starts_with("score_child_prop:")
    );
    assert!(
        score_row
            .get::<String>("score_json")
            .expect("score json")
            .contains("\"selected\":true")
    );
    assert!(score_row.get::<f64>("weight").expect("weight") > 0.0);
    assert!(score_row.get::<bool>("selected").expect("selected score"));
    let journal_entries = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path))
        .load_entries()
        .expect("load journal entries");
    assert!(
        journal_entries
            .iter()
            .all(|entry| !matches!(entry, JournalEntry::Successor(_))),
        "passive eval-store selection rows must not replace successor transition authority"
    );

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

    fs::remove_file(&db_path).expect("remove owner eval DB for strict-missing check");
    let err = crate::cli::prototype1_state::cli_facing::emit_selection_decision_for_backend(
        &manifest_path,
        &parent_identity,
        &decision,
        &material,
        profile::EvalStorageBackend::DualStrict,
    )
    .expect_err("dual-strict selection persistence requires owner eval DB");
    match err {
        PrepareError::DatabaseSetup { phase, detail } => {
            assert_eq!(phase, "eval_selection_decision_db_missing");
            assert!(detail.contains("dual-strict selection persistence requires owner eval DB"));
        }
        other => panic!("unexpected strict selection DB error: {other:?}"),
    }
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
        "parent-comparison:evaluation-report:branch-child"
    );
    assert!(
        sealed.evaluations[0]
            .primary_report_citation
            .content_hash
            .is_some(),
        "parent comparison citation must carry a report hash"
    );
    let runtime = sealed.runtimes.first().expect("runtime evidence");
    assert_eq!(runtime.runtime_id, "runtime:node-child");
    assert!(
        runtime
            .document_citations
            .iter()
            .any(is_child_channel_terminal_result)
    );
    assert!(
        payload
            .source_refs
            .iter()
            .any(|reference| reference.as_str().contains("terminal-result:node-child"))
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
fn current_generation_candidates_without_channel_refs_are_not_decision_grade() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
    let mut node = test_node(tmp.path(), "node-child", "branch-child", "candidate-1");
    node.parent_node_id = Some("node-parent".to_string());
    let mut outcome = test_completed_outcome(
        node,
        test_resolved(&test_node(
            tmp.path(),
            "node-child",
            "branch-child",
            "candidate-1",
        )),
        0,
    );
    outcome.channel_evidence = None;
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
    let grade = payload.decision_grade_eligibility();

    assert!(
        !grade.eligible,
        "payload must fail closed without channel refs"
    );
    assert!(
        grade
            .identity_gaps
            .iter()
            .any(|gap| gap.starts_with("primary_runtime_terminal_channel_citation_missing")),
        "unexpected gaps: {:?}",
        grade.identity_gaps
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
        &CLI_TEST_CAMPAIGN,
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
            oracle_targets: Vec::new(),
            selected_source: None,
            child_counts: std::collections::BTreeMap::new(),
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
            oracle_targets: Vec::new(),
            selected_source: None,
            child_counts: std::collections::BTreeMap::new(),
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
            oracle_targets: Vec::new(),
            selected_source: None,
            child_counts: std::collections::BTreeMap::new(),
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
            oracle_targets: Vec::new(),
            selected_source: None,
            child_counts: std::collections::BTreeMap::new(),
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
fn stage6_handoff_race_replays_through_production_readers() {
    use crate::cli::prototype1_state::{
        channel::{Envelope, ToParent},
        invocation::{self, InvocationAuthority},
        journal::{JournalEntry, PrototypeJournal},
        session::Store,
        successor::State as SuccessorState,
        walk::{
            epoch::ServerEpoch,
            protocol::{WalkAttemptResult, WalkSessionEventKind},
        },
    };
    use flate2::read::GzDecoder;
    use std::{fs::File, io};

    fn inflate(source: &Path, target: &Path) {
        std::fs::create_dir_all(target.parent().expect("fixture target parent"))
            .expect("create fixture target directory");
        let source = File::open(source).expect("open compressed historical fixture");
        let mut decoder = GzDecoder::new(source);
        let mut target = File::create(target).expect("create inflated historical fixture");
        io::copy(&mut decoder, &mut target).expect("inflate historical fixture");
    }

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/prototype1-stage6-handoff-race-20260715");
    let predecessor: ParentIdentity = json_fixture(
        &std::fs::read_to_string(fixture.join("predecessor-parent-identity.json"))
            .expect("read predecessor identity"),
    );
    let successor: ParentIdentity = json_fixture(
        &std::fs::read_to_string(fixture.join("successor-parent-identity.json"))
            .expect("read successor identity"),
    );
    assert_eq!(predecessor.generation(), 0);
    assert_eq!(successor.generation(), 1);
    assert_eq!(
        successor.previous_parent_id(),
        Some(predecessor.parent_id())
    );

    let invocation_path = fixture.join("successor-invocation.json");
    let InvocationAuthority::Successor(invocation) =
        invocation::load_authority(&invocation_path).expect("load historical successor invocation")
    else {
        panic!("historical invocation must carry successor authority");
    };
    let attempt = invocation
        .predecessor_attempt()
        .expect("successor invocation preserves predecessor attempt");
    assert_eq!(
        attempt.session().to_string(),
        "a3e007a6-07a5-4fc5-aff6-238d522d053f"
    );
    assert_eq!(
        attempt.transition().to_string(),
        "d1d31434-2b01-5b88-be8b-de19c60b0b56"
    );
    assert_eq!(attempt.fence().to_string(), "13");
    assert!(!attempt.allow_live_api());
    assert!(attempt.allow_git_changes());

    let temp = tempfile::tempdir().expect("historical replay tempdir");
    let store = Store::new(temp.path().join("control"));
    inflate(
        &fixture.join("predecessor-control-journal.jsonl.gz"),
        store.paths(&predecessor).journal(),
    );
    inflate(
        &fixture.join("successor-control-journal.jsonl.gz"),
        store.paths(&successor).journal(),
    );
    let epoch = ServerEpoch::capture(temp.path()).expect("capture replay presentation epoch");
    let predecessor_history = store
        .inspect_history(&predecessor, epoch.clone())
        .expect("replay predecessor history")
        .expect("predecessor history exists");
    let successor_history = store
        .inspect_history(&successor, epoch)
        .expect("replay successor history")
        .expect("successor history exists");
    assert!(predecessor_history.damage.is_none());
    assert!(successor_history.damage.is_none());
    assert_eq!(predecessor_history.events.len(), 51);
    assert_eq!(successor_history.events.len(), 43);
    assert_eq!(
        predecessor_history.version.session_id(),
        Some(attempt.session())
    );
    let predecessor_snapshot = store
        .inspect(&predecessor)
        .expect("inspect predecessor session")
        .expect("predecessor session exists");
    let committed = predecessor_snapshot
        .committed_handoff(attempt.session(), attempt.fence())
        .expect("production replay identifies the exact committed handoff");
    assert_eq!(committed.intent().transition_id(), attempt.transition());
    assert!(matches!(
        committed.result(),
        crate::cli::prototype1_state::session::AttemptResult::Committed {
            phase: WalkPhase::R13b,
            ..
        }
    ));

    let began = predecessor_history
        .events
        .iter()
        .find(|event| {
            matches!(
                &event.kind,
                WalkSessionEventKind::AttemptBegan { fence: 13, intent }
                    if intent.transition_id == attempt.transition()
            )
        })
        .expect("replay exact predecessor handoff admission");
    let WalkSessionEventKind::AttemptBegan { intent, .. } = &began.kind else {
        unreachable!("filtered predecessor admission")
    };
    assert_eq!(intent.expected, WalkPhase::R12);
    assert!(intent.targets.contains(&WalkPhase::R13b));
    assert!(!intent.allow_live_api);
    assert!(intent.allow_git_changes);

    let finished = predecessor_history
        .events
        .iter()
        .find(|event| {
            matches!(
                &event.kind,
                WalkSessionEventKind::AttemptFinished { receipt }
                    if receipt.transition_id == attempt.transition()
                        && receipt.fence == 13
                        && matches!(
                            receipt.result,
                            WalkAttemptResult::Committed {
                                phase: WalkPhase::R13b,
                                ..
                            }
                        )
            )
        })
        .expect("replay committed predecessor handoff receipt");
    let released = predecessor_history
        .events
        .iter()
        .find(|event| {
            matches!(
                event.kind,
                WalkSessionEventKind::Released {
                    fence: 13,
                    ready: None
                }
            )
        })
        .expect("replay clean predecessor release");
    let successor_ready = successor_history
        .events
        .iter()
        .find_map(|event| match &event.kind {
            WalkSessionEventKind::Released {
                fence: 1,
                ready: Some(ready),
            } => Some((event, ready)),
            _ => None,
        })
        .expect("replay atomic successor Ready release");
    assert_eq!(
        successor_history.version.session_id(),
        Some(successor_ready.1.commit.session_id)
    );
    assert_eq!(successor_ready.1.commit.cursor.phase, WalkPhase::R4c);
    assert_eq!(
        successor_ready.1.runtime_id.to_string(),
        invocation.runtime_id().to_string()
    );
    assert!(
        successor_ready.0.recorded_at_ms < finished.recorded_at_ms,
        "historical successor Ready preceded predecessor terminal publication"
    );
    assert!(
        finished.recorded_at_ms < released.recorded_at_ms,
        "historical predecessor terminal receipt preceded its clean release"
    );

    let transition_path = temp.path().join("transition-journal.jsonl");
    inflate(
        &fixture.join("transition-journal.jsonl.gz"),
        &transition_path,
    );
    let entries = PrototypeJournal::new(&transition_path)
        .load_entries()
        .expect("replay transition journal");
    let journal_ready = entries
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::Successor(record)
                if record.runtime_id == Some(invocation.runtime_id()) =>
            {
                match &record.state {
                    SuccessorState::Ready {
                        controller: Some(ready),
                        ..
                    } => Some(ready),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect("replay typed successor Ready projection");
    journal_ready
        .validate_persisted()
        .expect("historical Ready receipt remains structurally valid");
    assert_eq!(
        journal_ready.commit().session_id(),
        successor_ready.1.commit.session_id
    );
    let acceptance = entries
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::SuccessorHandoff(handoff)
                if handoff.runtime_id == invocation.runtime_id() =>
            {
                handoff.acceptance.as_ref()
            }
            _ => None,
        })
        .expect("replay typed successor handoff acceptance");
    assert_eq!(acceptance.ready(), journal_ready);
    assert_eq!(acceptance.attempt(), attempt);

    let channel: Envelope<ToParent> = serde_json::from_str(
        std::fs::read_to_string(fixture.join("successor-ready-channel.jsonl"))
            .expect("read historical Ready channel")
            .trim(),
    )
    .expect("decode typed historical Ready channel envelope");
    assert_eq!(channel.runtime_id(), invocation.runtime_id());
    let ToParent::SuccessorReady {
        controller: Some(channel_ready),
        ..
    } = channel.body()
    else {
        panic!("historical channel must carry typed successor Ready");
    };
    assert_eq!(channel_ready, journal_ready);
}
