//! Passive Prototype 1 run-profile records.
//!
//! These DTOs mirror the current `run-profile.toml` and
//! `run-profile.commitment.json` files. They do not admit a profile, validate
//! runtime policy, compute digests, or mutate campaign storage.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::record::{Record, RecordFamily, RecordFormat};
use crate::scheduler::{ChildBudgetRecord, ChildScheduleModeRecord};

pub const RUN_PROFILE_SCHEMA_VERSION: &str = "prototype1-run-profile.v1";
pub const RUN_PROFILE_COMMITMENT_SCHEMA_VERSION: &str = "prototype1-run-profile-commitment.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunProfileRecord {
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub storage: Storage,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub model: ModelDefaults,
    #[serde(default)]
    pub search: Search,
    #[serde(default)]
    pub generation: Generation,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default)]
    pub execution: Execution,
    #[serde(default)]
    pub control: Control,
}

impl Record for RunProfileRecord {
    const FAMILY: RecordFamily = RecordFamily::RunProfile;
    const SCHEMA: &'static str = RUN_PROFILE_SCHEMA_VERSION;
    const FORMAT: RecordFormat = RecordFormat::Toml;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    #[serde(default = "default_worktree_root")]
    pub worktree_root: PathBuf,
    #[serde(default)]
    pub eval: EvalStorage,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            worktree_root: default_worktree_root(),
            eval: EvalStorage::default(),
        }
    }
}

fn default_worktree_root() -> PathBuf {
    PathBuf::from("~/.ploke-eval/worktrees")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalStorage {
    #[serde(default)]
    pub backend: EvalStorageBackend,
}

impl Default for EvalStorage {
    fn default() -> Self {
        Self {
            backend: EvalStorageBackend::Fs,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvalStorageBackend {
    Fs,
    /// Files remain authority; owner eval DB rows are a mirror/query surface.
    DbMirror,
    /// Compatibility spelling for the current mirror mode until DB-backed reads exist.
    Database,
    DualStrict,
}

impl Default for EvalStorageBackend {
    fn default() -> Self {
        Self::Fs
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_source: Option<ModelRouteSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Retired profile-local eval cap retained only for immutable v1 evidence.
    #[serde(
        rename = "max_tokens",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub legacy_max_tokens: Option<u32>,
}

impl ModelDefaults {
    fn is_empty(&self) -> bool {
        self.id.is_none()
            && self.route_source.is_none()
            && self.provider.is_none()
            && self.legacy_max_tokens.is_none()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelRouteSource {
    #[serde(rename = "openrouter", alias = "open-router", alias = "open_router")]
    OpenRouter,
    #[serde(rename = "direct-google", alias = "direct_google", alias = "google")]
    DirectGoogle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub max_generations: u32,
    pub max_total_nodes: u32,
    pub children: ChildBudgetRecord,
    pub schedule: ChildScheduleModeRecord,
    pub stop_on_first_keep: bool,
    pub require_keep_for_continuation: bool,
    pub explore_from_rejected: bool,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            max_generations: 1,
            max_total_nodes: 32,
            children: ChildBudgetRecord::default(),
            schedule: ChildScheduleModeRecord::default(),
            stop_on_first_keep: false,
            require_keep_for_continuation: true,
            explore_from_rejected: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    pub source: GenerationSource,
    /// Historical v1 profiles may carry this retired generation selector.
    ///
    /// It remains part of the passive wire shape so immutable admitted profiles
    /// can be inspected and hashed without dropping a known v1 field. New
    /// operator admission rejects it in `ploke-eval`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<GenerationSurface>,
}

impl Default for Generation {
    fn default() -> Self {
        Self {
            source: GenerationSource::BroadHarnessRequest,
            surface: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GenerationSource {
    Legacy,
    /// Retained for passive decoding of immutable v1 profiles only.
    EditSurface,
    /// Retained for passive decoding of immutable v1 profiles only.
    BroadHarness,
    BroadHarnessRequest,
    DeterministicTuiTools,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GenerationSurface {
    PlokeTuiTools,
    WorkspaceExceptPlokeEval,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub strategy: SelectionStrategy,
    pub evidence: SelectionEvidence,
    #[serde(default)]
    pub metrics: Metrics,
    #[serde(default)]
    pub oracle: Oracle,
    #[serde(default, skip_serializing_if = "Patch::is_disabled")]
    pub patch: Patch,
    pub seed: u64,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            strategy: SelectionStrategy::HistoryScoreChildProp,
            evidence: SelectionEvidence::Operational,
            metrics: Metrics::default(),
            oracle: Oracle::default(),
            patch: Patch::default(),
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionStrategy {
    GenerationLocal,
    HistoryFrontierMax,
    HistoryScoreChildProp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionEvidence {
    Operational,
    OperationalAndProtocol,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    #[serde(default = "default_metrics_persist")]
    pub persist: bool,
    #[serde(default)]
    pub score_profile: ScoreProfile,
    #[serde(default)]
    pub imp_at_k: ImpAtK,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            persist: default_metrics_persist(),
            score_profile: ScoreProfile::default(),
            imp_at_k: ImpAtK::default(),
        }
    }
}

fn default_metrics_persist() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ScoreProfile {
    #[default]
    OperationalQualityV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImpAtK {
    #[serde(default = "default_imp_at_k_enabled")]
    pub enabled: bool,
    #[serde(default = "default_imp_at_k_budget")]
    pub budget_k: usize,
    #[serde(default)]
    pub archive_scope: ArchiveScope,
    #[serde(default)]
    pub score_points_per_imp_point: i64,
    #[serde(default)]
    pub require_for_score: bool,
}

impl Default for ImpAtK {
    fn default() -> Self {
        Self {
            enabled: default_imp_at_k_enabled(),
            budget_k: default_imp_at_k_budget(),
            archive_scope: ArchiveScope::default(),
            score_points_per_imp_point: 0,
            require_for_score: false,
        }
    }
}

fn default_imp_at_k_enabled() -> bool {
    true
}

fn default_imp_at_k_budget() -> usize {
    50
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ArchiveScope {
    #[default]
    SelectionScope,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Oracle {
    #[serde(default)]
    pub mode: OracleMode,
    #[serde(default = "default_oracle_require_evidence")]
    pub require_evidence: bool,
    #[serde(default)]
    pub gate: OracleGate,
}

impl Default for Oracle {
    fn default() -> Self {
        Self {
            mode: OracleMode::RecordOnly,
            require_evidence: default_oracle_require_evidence(),
            gate: OracleGate::Disabled,
        }
    }
}

fn default_oracle_require_evidence() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OracleMode {
    #[default]
    RecordOnly,
    RelativeScore,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OracleGate {
    #[default]
    Disabled,
    AllResolved,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(default)]
    pub gate: PatchGate,
}

impl Patch {
    pub fn is_disabled(&self) -> bool {
        self.gate.is_disabled()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PatchGate {
    #[default]
    Disabled,
    ReviewedAdmissible,
}

impl PatchGate {
    pub fn is_disabled(&self) -> bool {
        *self == Self::Disabled
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ReviewedAdmissible => "reviewed-admissible",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    #[serde(default, skip_serializing_if = "ModelDefaults::is_empty")]
    pub model: ModelDefaults,
    /// Retired flat protocol routing keys retained only for immutable v1 evidence.
    #[serde(rename = "model_id", default, skip_serializing_if = "Option::is_none")]
    pub legacy_model_id: Option<String>,
    #[serde(
        rename = "route_source",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub legacy_route_source: Option<ModelRouteSource>,
    #[serde(rename = "provider", default, skip_serializing_if = "Option::is_none")]
    pub legacy_provider: Option<String>,
    #[serde(default = "default_protocol_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_protocol_tool_review_parallelism")]
    pub tool_review_parallelism: usize,
    #[serde(default, skip_serializing_if = "ProtocolReasoning::is_auto")]
    pub reasoning: ProtocolReasoning,
}

impl Default for Protocol {
    fn default() -> Self {
        Self {
            model: ModelDefaults::default(),
            legacy_model_id: None,
            legacy_route_source: None,
            legacy_provider: None,
            max_tokens: default_protocol_max_tokens(),
            tool_review_parallelism: default_protocol_tool_review_parallelism(),
            reasoning: ProtocolReasoning::default(),
        }
    }
}

fn default_protocol_max_tokens() -> u32 {
    4096
}

fn default_protocol_tool_review_parallelism() -> usize {
    8
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProtocolReasoning {
    #[serde(default)]
    pub mode: ProtocolReasoningMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<ProtocolReasoningEffort>,
}

impl ProtocolReasoning {
    pub fn is_auto(&self) -> bool {
        *self == Self::default()
    }

    pub fn is_omit(&self) -> bool {
        self.mode == ProtocolReasoningMode::Omit && self.effort.is_none()
    }
}

impl Default for ProtocolReasoning {
    fn default() -> Self {
        Self {
            mode: ProtocolReasoningMode::Auto,
            effort: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolReasoningMode {
    #[default]
    Auto,
    Omit,
    Effort,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolReasoningEffort {
    Xhigh,
    High,
    Medium,
    Low,
    Minimal,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub stop_after: ExecutionStopAfter,
    #[serde(
        rename = "observe_child_stale_after_secs",
        default = "default_observe_child_stale_after_secs"
    )]
    pub child_stale_secs: u64,
    #[serde(default)]
    pub broad_tui: BroadTui,
    #[serde(default)]
    pub trace_jsonl: TraceJsonl,
    #[serde(default)]
    pub debug_tools: bool,
    #[serde(default)]
    pub mbe: Mbe,
}

impl Default for Execution {
    fn default() -> Self {
        Self {
            stop_after: ExecutionStopAfter::Complete,
            child_stale_secs: default_observe_child_stale_after_secs(),
            broad_tui: BroadTui::default(),
            trace_jsonl: TraceJsonl::Inherit,
            debug_tools: false,
            mbe: Mbe::default(),
        }
    }
}

fn default_observe_child_stale_after_secs() -> u64 {
    20 * 60
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BroadTui {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fresh_slots_per_child: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_nearest: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Mbe {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mbe_python")]
    pub python: String,
    #[serde(default = "default_mbe_workers")]
    pub workers: u32,
}

impl Default for Mbe {
    fn default() -> Self {
        Self {
            enabled: false,
            python: default_mbe_python(),
            workers: default_mbe_workers(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStopAfter {
    Materialize,
    Build,
    Spawn,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TraceJsonl {
    #[default]
    Inherit,
    Auto,
    Off,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Control {
    #[serde(default)]
    pub mode: RunMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_cap: Option<u32>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            mode: RunMode::Continuous,
            parallel_cap: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    #[default]
    Continuous,
    Step,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunProfileCommitmentRecord {
    pub schema_version: String,
    pub profile_path: PathBuf,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    pub admitted_at: String,
}

impl Record for RunProfileCommitmentRecord {
    const FAMILY: RecordFamily = RecordFamily::RunProfileCommitment;
    const SCHEMA: &'static str = RUN_PROFILE_COMMITMENT_SCHEMA_VERSION;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

fn default_mbe_python() -> String {
    "python".to_string()
}

fn default_mbe_workers() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = r#"
schema_version = "prototype1-run-profile.v1"
name = "overnight-edit-surface"

[storage]
worktree_root = "~/.ploke-eval/worktrees"

[target]
dataset_key = "ripgrep"
instance = "BurntSushi__ripgrep-2209"
instances = ["BurntSushi__ripgrep-2209"]

[model]
id = "google/gemini-3.5-flash"
route_source = "direct-google"
provider = "google"

[search]
max_generations = 15
max_total_nodes = 96
children = { min = 6, max = 6, parallel_targets = 3 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
explore_from_rejected = true

[generation]
source = "broad-harness-request"

[selection]
strategy = "history-score-child-prop"
evidence = "operational-and-protocol"
seed = 0

[selection.metrics]
persist = true
score_profile = "operational-quality-v1"

[selection.metrics.imp_at_k]
enabled = true
budget_k = 50
archive_scope = "selection-scope"
score_points_per_imp_point = 0
require_for_score = false

[selection.oracle]
mode = "record-only"
require_evidence = true

[protocol]
max_tokens = 4096
tool_review_parallelism = 2

[protocol.model]
id = "openai/gpt-5.1"
route_source = "openrouter"
provider = "openai"

[protocol.reasoning]
mode = "omit"

[execution]
stop_after = "complete"
observe_child_stale_after_secs = 1200
trace_jsonl = "auto"
debug_tools = true
mbe = { enabled = true, python = "python3", workers = 2 }

[execution.broad_tui]
max_attempts = 2
fresh_slots_per_child = 2
graph_nearest = 13
"#;

    #[test]
    fn run_profile_toml_roundtrips_current_shape() {
        let profile: RunProfileRecord = toml::from_str(PROFILE).expect("profile parses");

        assert_eq!(profile.schema_version, RUN_PROFILE_SCHEMA_VERSION);
        assert_eq!(
            profile.search.children,
            ChildBudgetRecord {
                min: 6,
                max: 6,
                parallel_targets: Some(3),
            }
        );
        assert_eq!(profile.search.schedule, ChildScheduleModeRecord::FullBatch);
        assert_eq!(
            profile.generation.source,
            GenerationSource::BroadHarnessRequest
        );
        assert_eq!(
            profile.target.instances,
            vec!["BurntSushi__ripgrep-2209".to_string()]
        );
        assert_eq!(
            profile.model.route_source,
            Some(ModelRouteSource::DirectGoogle)
        );
        assert_eq!(profile.model.provider.as_deref(), Some("google"));
        assert_eq!(profile.execution.trace_jsonl, TraceJsonl::Auto);
        assert_eq!(profile.execution.child_stale_secs, 1200);
        assert_eq!(profile.execution.broad_tui.max_attempts, Some(2));
        assert_eq!(profile.execution.broad_tui.fresh_slots_per_child, Some(2));
        assert_eq!(profile.execution.broad_tui.graph_nearest, Some(13));
        assert_eq!(profile.selection.oracle.mode, OracleMode::RecordOnly);
        assert!(profile.selection.oracle.require_evidence);
        assert!(profile.selection.metrics.persist);
        assert!(profile.selection.metrics.imp_at_k.enabled);
        assert_eq!(profile.selection.metrics.imp_at_k.budget_k, 50);
        assert_eq!(profile.protocol.max_tokens, 4096);
        assert_eq!(profile.protocol.tool_review_parallelism, 2);
        assert_eq!(
            profile.protocol.model.route_source,
            Some(ModelRouteSource::OpenRouter)
        );
        assert_eq!(profile.protocol.model.provider.as_deref(), Some("openai"));
        assert!(profile.protocol.reasoning.is_omit());
        assert_eq!(profile.storage.eval.backend, EvalStorageBackend::Fs);
        assert!(profile.execution.mbe.enabled);
        assert_eq!(profile.execution.mbe.python, "python3");
        assert_eq!(profile.execution.mbe.workers, 2);

        let encoded = toml::to_string(&profile).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, profile);
    }

    #[test]
    fn run_profile_toml_defaults_match_runtime_profile() {
        let profile: RunProfileRecord = toml::from_str(
            r#"
schema_version = "prototype1-run-profile.v1"
name = "runtime-defaults"
"#,
        )
        .expect("profile parses");

        assert_eq!(profile.model, ModelDefaults::default());
        assert_eq!(profile.search.children.parallel_targets, None);
        assert_eq!(
            profile.generation.source,
            GenerationSource::BroadHarnessRequest
        );
        assert_eq!(profile.protocol.model, ModelDefaults::default());
        assert_eq!(profile.protocol.max_tokens, 4096);
        assert_eq!(profile.protocol.tool_review_parallelism, 8);
        assert_eq!(profile.protocol.reasoning, ProtocolReasoning::default());
        assert_eq!(profile.execution.child_stale_secs, 1200);

        let encoded = toml::to_string(&profile).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, profile);
    }

    #[test]
    fn run_profile_toml_defaults_eval_storage_to_fs() {
        let parsed: RunProfileRecord = toml::from_str(PROFILE).expect("profile parses");

        assert_eq!(parsed.storage.eval.backend, EvalStorageBackend::Fs);
    }

    #[test]
    fn run_profile_toml_roundtrips_eval_storage_backend() {
        let profile = PROFILE.replace(
            "[target]",
            "[storage.eval]\nbackend = \"db-mirror\"\n\n[target]",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");
        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded.storage.eval.backend, EvalStorageBackend::DbMirror);
        assert!(encoded.contains("backend = \"db-mirror\""));
    }

    #[test]
    fn run_profile_toml_accepts_legacy_database_backend() {
        let profile = PROFILE.replace(
            "[target]",
            "[storage.eval]\nbackend = \"database\"\n\n[target]",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.storage.eval.backend, EvalStorageBackend::Database);
    }

    #[test]
    fn run_profile_toml_defaults_oracle_policy_to_record_only() {
        let profile = PROFILE.replace(
            "\n[selection.oracle]\nmode = \"record-only\"\nrequire_evidence = true\n",
            "\n",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.selection.oracle.mode, OracleMode::RecordOnly);
        assert!(parsed.selection.oracle.require_evidence);
        assert_eq!(parsed.selection.oracle.gate, OracleGate::Disabled);
    }

    #[test]
    fn run_profile_toml_defaults_patch_gate_to_disabled() {
        let parsed: RunProfileRecord = toml::from_str(PROFILE).expect("profile parses");

        assert_eq!(parsed.selection.patch.gate, PatchGate::Disabled);
    }

    #[test]
    fn run_profile_toml_roundtrips_reviewed_admissible_patch_gate() {
        let profile = PROFILE.replace(
            "[selection.metrics]",
            "[selection.patch]\ngate = \"reviewed-admissible\"\n\n[selection.metrics]",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.selection.patch.gate, PatchGate::ReviewedAdmissible);

        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, parsed);
    }

    #[test]
    fn run_profile_toml_rejects_unknown_patch_policy_fields() {
        let profile = PROFILE.replace(
            "[selection.metrics]",
            "[selection.patch]\ngate = \"disabled\"\nunknown = true\n\n[selection.metrics]",
        );
        let error =
            toml::from_str::<RunProfileRecord>(&profile).expect_err("unknown patch field rejected");

        assert!(error.to_string().contains("unknown field `unknown`"));
    }

    #[test]
    fn run_profile_toml_defaults_protocol_max_tokens() {
        let profile = PROFILE
            .replace("max_tokens = 4096\n", "")
            .replace("tool_review_parallelism = 2\n", "");
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.protocol.max_tokens, default_protocol_max_tokens());
        assert_eq!(
            parsed.protocol.tool_review_parallelism,
            default_protocol_tool_review_parallelism()
        );
        assert!(parsed.protocol.reasoning.is_omit());
    }

    #[test]
    fn run_profile_toml_roundtrips_protocol_reasoning_effort() {
        let profile = PROFILE
            .replace("mode = \"omit\"", "mode = \"effort\"")
            .replace(
                "[protocol.reasoning]\nmode = \"effort\"",
                "[protocol.reasoning]\nmode = \"effort\"\neffort = \"low\"",
            );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(
            parsed.protocol.reasoning.mode,
            ProtocolReasoningMode::Effort
        );
        assert_eq!(
            parsed.protocol.reasoning.effort,
            Some(ProtocolReasoningEffort::Low)
        );

        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, parsed);
    }

    #[test]
    fn run_profile_toml_roundtrips_relative_oracle_policy() {
        let profile = PROFILE
            .replace("mode = \"record-only\"", "mode = \"relative-score\"")
            .replace("require_evidence = true", "require_evidence = false");
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.selection.oracle.mode, OracleMode::RelativeScore);
        assert!(!parsed.selection.oracle.require_evidence);

        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, parsed);
    }

    #[test]
    fn run_profile_toml_roundtrips_all_resolved_oracle_gate() {
        let profile = PROFILE.replace(
            "require_evidence = true",
            "require_evidence = true\ngate = \"all-resolved\"",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.selection.oracle.gate, OracleGate::AllResolved);

        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, parsed);
    }

    #[test]
    fn partial_search_requires_explore_from_rejected() {
        let profile = r#"
schema_version = "prototype1-run-profile.v1"
name = "missing-search-field"

[search]
max_generations = 15
max_total_nodes = 96
children = { min = 6, max = 6 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
"#;

        toml::from_str::<RunProfileRecord>(profile)
            .expect_err("partial search table must require explore_from_rejected");
    }

    #[test]
    fn search_explore_from_rejected_false_roundtrips() {
        let profile = PROFILE.replace(
            "explore_from_rejected = true",
            "explore_from_rejected = false",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert!(!parsed.search.explore_from_rejected);

        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded.search.explore_from_rejected, false);
        assert_eq!(decoded, parsed);
    }

    #[test]
    fn run_profile_toml_accepts_runtime_generation_sources() {
        for (source, expected) in [
            ("legacy", GenerationSource::Legacy),
            (
                "broad-harness-request",
                GenerationSource::BroadHarnessRequest,
            ),
            (
                "deterministic-tui-tools",
                GenerationSource::DeterministicTuiTools,
            ),
        ] {
            let profile = PROFILE.replace("broad-harness-request", source);
            let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

            assert_eq!(parsed.generation.source, expected);
        }
    }

    #[test]
    fn run_profile_toml_preserves_historical_generation_sources() {
        for (source, expected) in [
            ("edit-surface", GenerationSource::EditSurface),
            ("broad-harness", GenerationSource::BroadHarness),
        ] {
            let profile = PROFILE.replace("broad-harness-request", source);
            let parsed = toml::from_str::<RunProfileRecord>(&profile)
                .expect("known historical generation source must remain readable");
            let encoded = toml::to_string(&parsed).expect("historical profile serializes");

            assert_eq!(parsed.generation.source, expected);
            assert!(encoded.contains(&format!("source = \"{source}\"")));
        }
    }

    #[test]
    fn run_profile_toml_preserves_historical_generation_surfaces() {
        for (surface, expected) in [
            ("ploke-tui-tools", GenerationSurface::PlokeTuiTools),
            (
                "workspace-except-ploke-eval",
                GenerationSurface::WorkspaceExceptPlokeEval,
            ),
        ] {
            let profile = PROFILE.replace(
                "source = \"broad-harness-request\"",
                &format!("source = \"broad-harness-request\"\nsurface = \"{surface}\""),
            );
            let parsed = toml::from_str::<RunProfileRecord>(&profile)
                .expect("known historical generation surface must remain readable");
            let encoded = toml::to_string(&parsed).expect("historical profile serializes");

            assert_eq!(parsed.generation.surface, Some(expected));
            assert!(encoded.contains(&format!("surface = \"{surface}\"")));
        }
    }

    #[test]
    fn run_profile_toml_rejects_unknown_generation_surface() {
        let profile = PROFILE.replace(
            "source = \"broad-harness-request\"",
            "source = \"broad-harness-request\"\nsurface = \"workspace-except-eval\"",
        );
        let error = toml::from_str::<RunProfileRecord>(&profile)
            .expect_err("unknown generation surface must be rejected")
            .to_string();

        assert!(error.contains("unknown variant"));
        assert!(error.contains("workspace-except-eval"));
    }

    #[test]
    fn run_profile_toml_rejects_typos_in_current_key_scopes() {
        for (valid, typo) in [
            (
                "route_source = \"direct-google\"",
                "route_soruce = \"direct-google\"",
            ),
            (
                "tool_review_parallelism = 2",
                "tool_review_parallellism = 2",
            ),
            (
                "observe_child_stale_after_secs = 1200",
                "observe_child_stale_after_sec = 1200",
            ),
        ] {
            let profile = PROFILE.replacen(valid, typo, 1);
            let error = toml::from_str::<RunProfileRecord>(&profile)
                .expect_err("misspelled current profile key must be rejected")
                .to_string();
            let key = typo.split_once(" =").expect("typo assignment").0;

            assert!(error.contains("unknown field"));
            assert!(error.contains(key));
        }
    }

    #[test]
    fn run_profile_toml_roundtrips_multi_instance_target_cohort() {
        let profile = PROFILE.replace(
            "instances = [\"BurntSushi__ripgrep-2209\"]",
            "instances = [\"BurntSushi__ripgrep-2209\", \"BurntSushi__ripgrep-454\"]",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(
            parsed.target.instances,
            vec![
                "BurntSushi__ripgrep-2209".to_string(),
                "BurntSushi__ripgrep-454".to_string()
            ]
        );

        let encoded = toml::to_string(&parsed).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, parsed);
    }

    #[test]
    fn run_profile_commitment_json_roundtrips_current_shape() {
        let commitment = RunProfileCommitmentRecord {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: PathBuf::from("/tmp/campaign/prototype1/run-profile.toml"),
            sha256: "0123456789abcdef".to_string(),
            source_path: Some(PathBuf::from("/tmp/profiles/overnight.toml")),
            admitted_at: "2026-05-11T12:00:00Z".to_string(),
        };

        let encoded = serde_json::to_string_pretty(&commitment).expect("commitment serializes");
        let decoded: RunProfileCommitmentRecord =
            serde_json::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, commitment);
    }
}
