use super::*;

use crate::cli::prototype1_state::cli_facing::CandidateGenerationConfig;
use crate::cli::prototype1_state::edit_surface::harness_request::{
    PublishedBroadHarnessRequest, RequestAdmissionBinding,
};
use crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId;
use crate::cli::prototype1_state::eval_store;
use crate::cli::prototype1_state::typestate::{self, StepInput};
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

use crate::intervention::{
    CommitPhase, PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1RunnerResult,
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
            selected_source: Some(TraversalCandidateSource::History),
            child_counts: std::collections::BTreeMap::new(),
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
        Prototype1ContinuationDisposition::StopHistoricalTraversalCycle
    );
    assert!(!decision.disposition.allows_successor());
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

[storage.eval]
backend = "dual-strict"

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
}

#[test]
fn state_run_shape_defaults_eval_storage_backend_to_fs() {
    let command = state_command_without_ids();
    let shape = Prototype1StateRunShape::from_command(&command);

    assert_eq!(shape.eval_storage_backend, profile::EvalStorageBackend::Fs);
}

#[test]
fn prototype1_setup_campaign_manifest_preserves_embedding_overrides() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let _guard =
        crate::test_support::env_guard_os(vec![("PLOKE_EVAL_HOME", tmp.path().as_os_str().into())]);
    let models_dir = tmp.path().join("models");
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
        index_debug_snapshots: true,
        use_default_model: false,
        model_id: Some("google/gemini-3.5-flash".to_string()),
        provider: Some("google".to_string()),
        route_source: Some(ModelRouteSource::DirectGoogle),
        embedding_model_id: Some("perplexity/pplx-embed-v1-4b".to_string()),
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
        config,
        journal_path.clone(),
        journal,
    );
    let r4c = typestate::R4cReady::from_collected_parent(collected, ready);

    R4cFixture {
        r4c,
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

    let err = run_broad_headless_tui_attempt_with_options(&slot, &options)
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

    let err = run_broad_headless_tui_attempt_with_options(&slot, &options)
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
fn tui_edit_surface_parent_selection_publishes_child_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("campaign.json");
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

    write_broad_headless_tui_turn_live_bundle(&slot, &run, "diagnose the run", "test/model")
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
fn live_google_broad_headless_canary_base_dir() -> PathBuf {
    live_google_broad_headless_canary_base_dir_from_override(
        std::env::var_os("PLOKE_EVAL_LIVE_TUI_CANARY_DIR").map(PathBuf::from),
    )
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

#[cfg(feature = "live_api_tests")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "live direct-Google broad headless-TUI contract preflight; requires Google ADC/Vertex quota"]
async fn live_google_direct_broad_headless_tui_rejects_applied_edit_missing_declared_validation() {
    // regr:googlevertex:23-05-26_19-10 resolved 2026-06-02.
    //
    // This is the live contract preflight for the Prototype 1 published-request
    // -> cli_facing runner -> tui_adapter -> vanilla ploke-tui llm_manager ->
    // direct Google/Vertex OpenAI-compatible route. It intentionally asks for an
    // edit without the request-declared validation commands, so the applied edit
    // must be rejected as AppliedValidationMissing instead of being published.
    // It is intentionally ignored because it spends live Google provider calls.
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

    let base = live_google_broad_headless_canary_base_dir();
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

    let publication = publish_broad_edit_harness_request_with_graph_limit(
        &manifest_path,
        &repo_root,
        &test_parent_identity(),
        Prototype1ChildBudget::new(1, 1),
        test_broad_request_admission_binding(),
        DEFAULT_GRAPH_NEAREST_ITEMS,
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
    let err = run_broad_headless_tui_attempt_with_options(&slot, &options)
        .await
        .expect_err("live canary must reject applied edit missing declared validation");
    let detail = err.to_string();
    assert!(
        detail.contains("missing requested validation after applying proposal"),
        "expected missing-validation rejection for '{}', got {detail}; artifacts at {}",
        slot.request_path.display(),
        artifact_root.display()
    );
    assert!(
        detail.contains("cargo check -p ploke-eval")
            && detail.contains("cargo test -p ploke-eval edit_surface"),
        "expected declared validation commands in rejection, got {detail}; artifacts at {}",
        artifact_root.display()
    );
    assert!(
        !slot.published.submitted_result_path().exists(),
        "missing-validation live run must not publish submitted result at {}; artifacts at {}",
        slot.published.submitted_result_path().display(),
        artifact_root.display()
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
    let terminal = diagnostics
        .terminal
        .as_ref()
        .expect("headless diagnostics should include terminal");
    assert!(
        matches!(
            terminal,
            tui_adapter::evidence::Terminal::AppliedValidationMissing { missing, changed_paths, .. }
                if missing.iter().any(|item| item == "cargo check -p ploke-eval")
                    && missing.iter().any(|item| item == "cargo test -p ploke-eval edit_surface")
                    && changed_paths.iter().any(|path| path.ends_with("src/lib.rs"))
        ),
        "expected AppliedValidationMissing terminal in diagnostics; got {:?}; artifacts at {}",
        terminal,
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
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
            route_source: ModelRouteSource::DirectGoogle,
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
            campaign_id: &CLI_TEST_CAMPAIGN,
            manifest_path: &manifest_path,
            repo_root: &repo_root,
            broad_tui: profile::BroadTui::default(),
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
                embedding_model_id: None,
                embedding_provider_slug: None,
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
        embedding_model_id: None,
        embedding_provider_slug: None,
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
    let parent = test_parent_identity();
    let node = test_node(
        Path::new("/tmp/campaign"),
        "node-child",
        "branch-child",
        "candidate-1",
    );
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

    let report = build_prototype1_branch_evaluation_report(
        &CampaignId::from("baseline"),
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
