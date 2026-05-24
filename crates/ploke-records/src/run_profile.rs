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
pub struct RunProfileRecord {
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub storage: Storage,
    #[serde(default)]
    pub target: Target,
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
pub struct Storage {
    pub worktree_root: PathBuf,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            worktree_root: PathBuf::from("~/.ploke-eval/worktrees"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct Generation {
    pub source: GenerationSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<GenerationSurface>,
}

impl Default for Generation {
    fn default() -> Self {
        Self {
            source: GenerationSource::EditSurface,
            surface: Some(GenerationSurface::WorkspaceExceptPlokeEval),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GenerationSource {
    Legacy,
    EditSurface,
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
pub struct Selection {
    pub strategy: SelectionStrategy,
    pub evidence: SelectionEvidence,
    #[serde(default)]
    pub metrics: Metrics,
    #[serde(default)]
    pub oracle: Oracle,
    pub seed: u64,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            strategy: SelectionStrategy::HistoryScoreChildProp,
            evidence: SelectionEvidence::Operational,
            metrics: Metrics::default(),
            oracle: Oracle::default(),
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
pub struct Oracle {
    #[serde(default)]
    pub mode: OracleMode,
    #[serde(default = "default_oracle_require_evidence")]
    pub require_evidence: bool,
}

impl Default for Oracle {
    fn default() -> Self {
        Self {
            mode: OracleMode::RecordOnly,
            require_evidence: default_oracle_require_evidence(),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Protocol {
    #[serde(default = "default_protocol_max_tokens")]
    pub max_tokens: u32,
    #[serde(default, skip_serializing_if = "ProtocolReasoning::is_omit")]
    pub reasoning: ProtocolReasoning,
}

impl Default for Protocol {
    fn default() -> Self {
        Self {
            max_tokens: default_protocol_max_tokens(),
            reasoning: ProtocolReasoning::default(),
        }
    }
}

fn default_protocol_max_tokens() -> u32 {
    2000
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolReasoning {
    #[serde(default)]
    pub mode: ProtocolReasoningMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<ProtocolReasoningEffort>,
}

impl ProtocolReasoning {
    pub fn is_omit(&self) -> bool {
        *self == Self::default()
    }
}

impl Default for ProtocolReasoning {
    fn default() -> Self {
        Self {
            mode: ProtocolReasoningMode::Omit,
            effort: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolReasoningMode {
    #[default]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Execution {
    pub stop_after: ExecutionStopAfter,
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
            trace_jsonl: TraceJsonl::Inherit,
            debug_tools: false,
            mbe: Mbe::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

[search]
max_generations = 15
max_total_nodes = 96
children = { min = 6, max = 6 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
explore_from_rejected = true

[generation]
source = "edit-surface"
surface = "workspace-except-ploke-eval"

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
max_tokens = 2000

[protocol.reasoning]
mode = "omit"

[execution]
stop_after = "complete"
trace_jsonl = "auto"
debug_tools = true
mbe = { enabled = true, python = "python3", workers = 2 }
"#;

    #[test]
    fn run_profile_toml_roundtrips_current_shape() {
        let profile: RunProfileRecord = toml::from_str(PROFILE).expect("profile parses");

        assert_eq!(profile.schema_version, RUN_PROFILE_SCHEMA_VERSION);
        assert_eq!(
            profile.search.children,
            ChildBudgetRecord { min: 6, max: 6 }
        );
        assert_eq!(profile.search.schedule, ChildScheduleModeRecord::FullBatch);
        assert_eq!(profile.generation.source, GenerationSource::EditSurface);
        assert_eq!(
            profile.generation.surface,
            Some(GenerationSurface::WorkspaceExceptPlokeEval)
        );
        assert_eq!(
            profile.target.instances,
            vec!["BurntSushi__ripgrep-2209".to_string()]
        );
        assert_eq!(profile.execution.trace_jsonl, TraceJsonl::Auto);
        assert_eq!(profile.selection.oracle.mode, OracleMode::RecordOnly);
        assert!(profile.selection.oracle.require_evidence);
        assert!(profile.selection.metrics.persist);
        assert!(profile.selection.metrics.imp_at_k.enabled);
        assert_eq!(profile.selection.metrics.imp_at_k.budget_k, 50);
        assert_eq!(profile.protocol.max_tokens, 2000);
        assert_eq!(profile.protocol.reasoning, ProtocolReasoning::default());
        assert!(profile.execution.mbe.enabled);
        assert_eq!(profile.execution.mbe.python, "python3");
        assert_eq!(profile.execution.mbe.workers, 2);

        let encoded = toml::to_string(&profile).expect("profile serializes");
        let decoded: RunProfileRecord = toml::from_str(&encoded).expect("roundtrip parses");

        assert_eq!(decoded, profile);
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
    }

    #[test]
    fn run_profile_toml_defaults_protocol_max_tokens() {
        let profile = PROFILE.replace(
            "\n[protocol]\nmax_tokens = 2000\n\n[protocol.reasoning]\nmode = \"omit\"\n",
            "\n",
        );
        let parsed: RunProfileRecord = toml::from_str(&profile).expect("profile parses");

        assert_eq!(parsed.protocol.max_tokens, default_protocol_max_tokens());
        assert_eq!(parsed.protocol.reasoning, ProtocolReasoning::default());
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

        assert_eq!(parsed.protocol.reasoning.mode, ProtocolReasoningMode::Effort);
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
    fn run_profile_toml_parses_newer_generation_sources() {
        let broad: RunProfileRecord = toml::from_str(
            r#"
schema_version = "prototype1-run-profile.v1"
name = "broad"

[generation]
source = "broad-harness"
"#,
        )
        .expect("broad-harness profile parses");
        assert_eq!(broad.generation.source, GenerationSource::BroadHarness);

        let broad_request: RunProfileRecord = toml::from_str(
            r#"
schema_version = "prototype1-run-profile.v1"
name = "broad-request"

[generation]
source = "broad-harness-request"
"#,
        )
        .expect("broad-harness-request profile parses");
        assert_eq!(
            broad_request.generation.source,
            GenerationSource::BroadHarnessRequest
        );

        let deterministic: RunProfileRecord = toml::from_str(
            r#"
schema_version = "prototype1-run-profile.v1"
name = "deterministic"

[generation]
source = "deterministic-tui-tools"
"#,
        )
        .expect("deterministic-tui-tools profile parses");
        assert_eq!(
            deterministic.generation.source,
            GenerationSource::DeterministicTuiTools
        );
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
