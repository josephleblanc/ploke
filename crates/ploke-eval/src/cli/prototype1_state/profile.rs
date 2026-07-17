use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::Utc;
use ploke_llm::{ModelId, ProviderKey, request::models::ModelRouteSource};
use ploke_protocol::ProtocolReasoningPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    campaign::{
        ProtocolCampaignPolicy, default_protocol_max_tokens,
        default_protocol_tool_review_parallelism,
    },
    cli::{
        Prototype1CandidateGenerator, Prototype1StateStopAfter, Prototype1SuccessorSelection,
        Prototype1TraversalMetrics,
    },
    intervention::{Prototype1ChildBudget, Prototype1ChildScheduleMode, Prototype1SearchPolicy},
    layout::ploke_eval_home,
    spec::PrepareError,
    successor_selection::{OracleGate, OracleMode, metrics as selection_metrics},
};

pub(crate) const RUN_PROFILE_SCHEMA_VERSION: &str = "prototype1-run-profile.v1";
pub(crate) const RUN_PROFILE_COMMITMENT_SCHEMA_VERSION: &str =
    "prototype1-run-profile-commitment.v1";
pub(crate) const DEFAULT_OBSERVE_CHILD_STALE_AFTER_SECS: u64 = 20 * 60;

const RUN_PROFILE_FILE: &str = "run-profile.toml";
const RUN_PROFILE_COMMITMENT_FILE: &str = "run-profile.commitment.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Prototype1RunProfile {
    pub(crate) schema_version: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) storage: Storage,
    #[serde(default)]
    pub(crate) target: Target,
    #[serde(default)]
    pub(crate) model: ModelDefaults,
    #[serde(default)]
    pub(crate) search: Search,
    #[serde(default)]
    pub(crate) generation: Generation,
    #[serde(default)]
    pub(crate) selection: Selection,
    #[serde(default)]
    pub(crate) protocol: Protocol,
    #[serde(default)]
    pub(crate) execution: Execution,
    #[serde(default)]
    pub(crate) control: Control,
}

impl Prototype1RunProfile {
    // ANCHOR: prototype1_run_profile_validate
    pub(crate) fn validate(&self) -> Result<(), PrepareError> {
        if self.schema_version != RUN_PROFILE_SCHEMA_VERSION {
            return Err(profile_error(format!(
                "unsupported Prototype 1 run profile schema '{}', expected '{}'",
                self.schema_version, RUN_PROFILE_SCHEMA_VERSION
            )));
        }
        self.storage.validate()?;
        self.target.validate()?;
        self.model.validate()?;
        self.search.validate()?;
        self.generation.validate()?;
        self.protocol.validate()?;
        self.execution.validate()?;
        self.control.validate(&self.search)?;
        self.selection.validate(&self.target, &self.execution)?;
        if matches!(self.generation.source, GenerationSource::Legacy)
            && self.target.eval_instances().len() > 1
        {
            return Err(profile_error(
                "legacy generation currently requires exactly one target instance",
            ));
        }
        Ok(())
    }
    // ANCHOR_END: prototype1_run_profile_validate

    pub(crate) fn default_parallel_cap(&self) -> u32 {
        self.search.default_parallel_cap()
    }

    pub(crate) fn patch_generation_parallel_cap(&self) -> u32 {
        self.search.children.parallel_targets()
    }

    pub(crate) fn search_policy(&self) -> Prototype1SearchPolicy {
        Prototype1SearchPolicy {
            max_generations: self.search.max_generations,
            max_total_nodes: self.search.max_total_nodes,
            child_budget: self.search.children,
            child_schedule_mode: self.search.schedule,
            stop_on_first_keep: self.search.stop_on_first_keep,
            require_keep_for_continuation: self.search.require_keep_for_continuation,
            explore_from_rejected: self.search.explore_from_rejected,
        }
    }

    pub(crate) fn protocol_policy(&self) -> ProtocolCampaignPolicy {
        ProtocolCampaignPolicy {
            model_id: self.protocol.model.id.clone(),
            route_source: self.protocol.model.route_source,
            provider_slug: self.protocol.model.provider.clone(),
            max_tokens: self.protocol.max_tokens,
            tool_review_parallelism: self.protocol.tool_review_parallelism,
            reasoning: self.protocol.reasoning,
            ..ProtocolCampaignPolicy::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Storage {
    #[serde(default = "default_worktree_root")]
    pub(crate) worktree_root: PathBuf,
    #[serde(default)]
    pub(crate) eval: EvalStorage,
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

impl Storage {
    fn validate(&self) -> Result<(), PrepareError> {
        self.eval.validate()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvalStorage {
    #[serde(default)]
    pub(crate) backend: EvalStorageBackend,
}

impl Default for EvalStorage {
    fn default() -> Self {
        Self {
            backend: EvalStorageBackend::Fs,
        }
    }
}

impl EvalStorage {
    fn validate(&self) -> Result<(), PrepareError> {
        match self.backend {
            EvalStorageBackend::Fs
            | EvalStorageBackend::DbMirror
            | EvalStorageBackend::Database
            | EvalStorageBackend::DualStrict => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EvalStorageBackend {
    Fs,
    /// Files remain authority; owner eval DB rows are a mirror/query surface.
    DbMirror,
    /// Compatibility spelling for the current mirror mode until DB-backed reads exist.
    Database,
    DualStrict,
}

impl EvalStorageBackend {
    pub(crate) const fn mirrors_owner_db(self) -> bool {
        !matches!(self, Self::Fs)
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Fs => "fs",
            Self::DbMirror => "db-mirror",
            Self::Database => "database",
            Self::DualStrict => "dual-strict",
        }
    }
}

impl Default for EvalStorageBackend {
    fn default() -> Self {
        Self::Fs
    }
}

// ANCHOR: prototype1_model_defaults
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_profile_route_source"
    )]
    pub(crate) route_source: Option<ModelRouteSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) provider: Option<String>,
}

impl ModelDefaults {
    pub(crate) fn is_empty(&self) -> bool {
        self.id.is_none() && self.route_source.is_none() && self.provider.is_none()
    }

    pub(crate) fn parsed_id(&self) -> Result<Option<ModelId>, PrepareError> {
        self.parsed_id_for("profile.model")
    }

    pub(crate) fn parsed_id_for(&self, label: &str) -> Result<Option<ModelId>, PrepareError> {
        self.id
            .as_deref()
            .map(|id| {
                id.parse()
                    .map_err(|err| profile_error(format!("{label}.id '{id}' is invalid: {err}")))
            })
            .transpose()
    }

    fn validate(&self) -> Result<(), PrepareError> {
        self.validate_for("profile.model")
    }

    fn validate_for(&self, label: &str) -> Result<(), PrepareError> {
        self.parsed_id_for(label)?;
        if let Some(provider) = self.provider.as_deref() {
            ProviderKey::new(provider).map_err(|err| {
                profile_error(format!("{label}.provider '{provider}' is invalid: {err}"))
            })?;
        }
        if self
            .route_source
            .is_some_and(|source| source.is_direct_google())
            && let Some(provider) = self.provider.as_deref()
            && provider != "google"
        {
            return Err(profile_error(format!(
                "{label}.route_source = direct-google does not accept OpenRouter provider '{provider}'"
            )));
        }
        Ok(())
    }
}
// ANCHOR_END: prototype1_model_defaults

mod optional_profile_route_source {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::ModelRouteSource;

    pub(super) fn serialize<S>(
        value: &Option<ModelRouteSource>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(ModelRouteSource::DirectGoogle) => serializer.serialize_some("direct-google"),
            Some(ModelRouteSource::OpenRouter) => serializer.serialize_some("openrouter"),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ModelRouteSource>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(value) = Option::<String>::deserialize(deserializer)? else {
            return Ok(None);
        };
        match value.as_str() {
            "openrouter" | "open-router" | "open_router" => Ok(Some(ModelRouteSource::OpenRouter)),
            "direct-google" | "direct_google" | "google" => {
                Ok(Some(ModelRouteSource::DirectGoogle))
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid route source '{other}'; expected openrouter or direct-google"
            ))),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) dataset_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) instances: Vec<String>,
}

impl Target {
    fn validate(&self) -> Result<(), PrepareError> {
        let mut seen = BTreeSet::new();
        for instance in &self.instances {
            if instance.trim().is_empty() {
                return Err(profile_error(
                    "target.instances must not contain empty benchmark instance ids",
                ));
            }
            if !seen.insert(instance) {
                return Err(profile_error(format!(
                    "target.instances contains duplicate benchmark instance '{}'",
                    instance
                )));
            }
        }
        if let Some(primary) = self.instance.as_deref() {
            if primary.trim().is_empty() {
                return Err(profile_error(
                    "target.instance must not be an empty benchmark instance id",
                ));
            }
            if !self.instances.is_empty()
                && !self.instances.iter().any(|instance| instance == primary)
            {
                return Err(profile_error(format!(
                    "target.instance '{}' must be included in target.instances when both are set",
                    primary
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn eval_instances(&self) -> Vec<String> {
        if !self.instances.is_empty() {
            self.instances.clone()
        } else {
            self.instance.clone().into_iter().collect()
        }
    }

    pub(crate) fn primary_instance(&self) -> Option<&str> {
        self.instance
            .as_deref()
            .or_else(|| self.instances.first().map(String::as_str))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Search {
    pub(crate) max_generations: u32,
    pub(crate) max_total_nodes: u32,
    pub(crate) children: Prototype1ChildBudget,
    pub(crate) schedule: Prototype1ChildScheduleMode,
    pub(crate) stop_on_first_keep: bool,
    pub(crate) require_keep_for_continuation: bool,
    pub(crate) explore_from_rejected: bool,
}

impl Search {
    fn validate(&self) -> Result<(), PrepareError> {
        if self.children.min == 0 || self.children.max == 0 {
            return Err(profile_error(
                "profile search.children values must be nonzero",
            ));
        }
        if self.children.min > self.children.max {
            return Err(profile_error(format!(
                "profile search.children.min {} cannot exceed max {}",
                self.children.min, self.children.max
            )));
        }
        if let Some(parallel_targets) = self.children.parallel_targets {
            if parallel_targets == 0 || parallel_targets > self.children.max {
                return Err(profile_error(format!(
                    "profile search.children.parallel_targets {} must be nonzero and no greater than max {}",
                    parallel_targets, self.children.max
                )));
            }
        }
        Ok(())
    }

    fn default_parallel_cap(&self) -> u32 {
        self.schedule
            .fanout_width(self.children, self.children.max as usize) as u32
    }
}

impl Default for Search {
    fn default() -> Self {
        let policy = Prototype1SearchPolicy::default();
        Self {
            max_generations: policy.max_generations,
            max_total_nodes: policy.max_total_nodes,
            children: policy.child_budget,
            schedule: policy.child_schedule_mode,
            stop_on_first_keep: policy.stop_on_first_keep,
            require_keep_for_continuation: policy.require_keep_for_continuation,
            explore_from_rejected: policy.explore_from_rejected,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Generation {
    pub(crate) source: GenerationSource,
    /// Preserved only so historical v1 commitments retain their known wire
    /// shape. Fresh operator admission rejects this retired selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) surface: Option<ploke_records::run_profile::GenerationSurface>,
}

impl Generation {
    fn validate(self) -> Result<(), PrepareError> {
        Ok(())
    }

    pub(crate) fn candidate_generator(self) -> Prototype1CandidateGenerator {
        match self.source {
            GenerationSource::Legacy => Prototype1CandidateGenerator::Legacy,
            GenerationSource::BroadHarnessRequest => {
                Prototype1CandidateGenerator::BroadHarnessRequest
            }
            GenerationSource::DeterministicTuiTools => {
                Prototype1CandidateGenerator::DeterministicTuiTools
            }
        }
    }
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
pub(crate) enum GenerationSource {
    Legacy,
    BroadHarnessRequest,
    DeterministicTuiTools,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub(crate) strategy: SelectionStrategy,
    pub(crate) evidence: SelectionEvidence,
    #[serde(default)]
    pub(crate) metrics: Metrics,
    #[serde(default)]
    pub(crate) oracle: Oracle,
    pub(crate) seed: u64,
}

impl Selection {
    pub(crate) fn successor_selection(self) -> Prototype1SuccessorSelection {
        match self.strategy {
            SelectionStrategy::GenerationLocal => Prototype1SuccessorSelection::GenerationLocal,
            SelectionStrategy::HistoryFrontierMax => {
                Prototype1SuccessorSelection::HistoryFrontierMax
            }
            SelectionStrategy::HistoryScoreChildProp => {
                Prototype1SuccessorSelection::HistoryScoreChildProp
            }
        }
    }

    pub(crate) fn traversal_metrics(self) -> Prototype1TraversalMetrics {
        match self.evidence {
            SelectionEvidence::Operational => Prototype1TraversalMetrics::Operational,
            SelectionEvidence::OperationalAndProtocol => {
                Prototype1TraversalMetrics::OperationalAndProtocol
            }
        }
    }

    pub(crate) fn oracle_mode(self) -> OracleMode {
        self.oracle.mode
    }

    pub(crate) fn oracle_require_evidence(self) -> bool {
        self.oracle.require_evidence
    }

    pub(crate) fn oracle_gate(self) -> OracleGate {
        self.oracle.gate
    }

    pub(crate) fn metrics_policy(self) -> selection_metrics::Policy {
        selection_metrics::Policy {
            persist: self.metrics.persist,
            score_profile: match self.metrics.score_profile {
                ScoreProfile::OperationalQualityV1 => {
                    selection_metrics::ScoreProfile::OperationalQualityV1
                }
            },
            imp_at_k: selection_metrics::ImpAtKPolicy {
                enabled: self.metrics.imp_at_k.enabled,
                budget_k: self.metrics.imp_at_k.budget_k,
                archive_scope: match self.metrics.imp_at_k.archive_scope {
                    ArchiveScope::SelectionScope => selection_metrics::ArchiveScope::SelectionScope,
                },
                score_points_per_imp_point: self.metrics.imp_at_k.score_points_per_imp_point,
                require_for_score: self.metrics.imp_at_k.require_for_score,
            },
        }
    }

    fn validate(self, target: &Target, execution: &Execution) -> Result<(), PrepareError> {
        self.metrics.validate()?;
        if execution.mbe.enabled && target.eval_instances().is_empty() {
            return Err(profile_error(
                "execution.mbe.enabled = true requires target.instance or target.instances",
            ));
        }
        if self.oracle.mode == OracleMode::RelativeScore && self.oracle.require_evidence {
            if !execution.mbe.enabled {
                return Err(profile_error(
                    "selection.oracle.mode = \"relative-score\" with require_evidence = true requires execution.mbe.enabled = true",
                ));
            }
            if target.eval_instances().is_empty() {
                return Err(profile_error(
                    "selection.oracle.mode = \"relative-score\" with require_evidence = true requires target.instance or target.instances",
                ));
            }
        }
        if self.oracle.gate == OracleGate::AllResolved {
            if !self.oracle.require_evidence {
                return Err(profile_error(
                    "selection.oracle.gate = \"all-resolved\" requires selection.oracle.require_evidence = true",
                ));
            }
            if !execution.mbe.enabled {
                return Err(profile_error(
                    "selection.oracle.gate = \"all-resolved\" requires execution.mbe.enabled = true",
                ));
            }
            if target.eval_instances().is_empty() {
                return Err(profile_error(
                    "selection.oracle.gate = \"all-resolved\" requires target.instance or target.instances",
                ));
            }
        }
        Ok(())
    }
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
pub(crate) enum SelectionStrategy {
    GenerationLocal,
    HistoryFrontierMax,
    HistoryScoreChildProp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SelectionEvidence {
    Operational,
    OperationalAndProtocol,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metrics {
    #[serde(default = "default_metrics_persist")]
    pub(crate) persist: bool,
    #[serde(default)]
    pub(crate) score_profile: ScoreProfile,
    #[serde(default)]
    pub(crate) imp_at_k: ImpAtK,
}

impl Metrics {
    fn validate(self) -> Result<(), PrepareError> {
        if self.imp_at_k.enabled && self.imp_at_k.budget_k == 0 {
            return Err(profile_error(
                "selection.metrics.imp_at_k.budget_k must be greater than zero when enabled",
            ));
        }
        if !self.persist
            && self.imp_at_k.enabled
            && (self.imp_at_k.score_points_per_imp_point != 0 || self.imp_at_k.require_for_score)
        {
            return Err(profile_error(
                "selection.metrics.persist = false cannot drive imp@k scoring",
            ));
        }
        Ok(())
    }
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
pub(crate) enum ScoreProfile {
    #[default]
    OperationalQualityV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImpAtK {
    #[serde(default = "default_imp_at_k_enabled")]
    pub(crate) enabled: bool,
    #[serde(default = "default_imp_at_k_budget")]
    pub(crate) budget_k: usize,
    #[serde(default)]
    pub(crate) archive_scope: ArchiveScope,
    #[serde(default)]
    pub(crate) score_points_per_imp_point: i64,
    #[serde(default)]
    pub(crate) require_for_score: bool,
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
pub(crate) enum ArchiveScope {
    #[default]
    SelectionScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Protocol {
    #[serde(default, skip_serializing_if = "ModelDefaults::is_empty")]
    pub(crate) model: ModelDefaults,
    // Defaults through campaign::PROTOTYPE1_PROTOCOL_MIN_SAFE_MAX_TOKENS. Keep
    // omitted-profile protocol closure budgets at or above the live canary floor.
    #[serde(default = "default_protocol_max_tokens")]
    pub(crate) max_tokens: u32,
    #[serde(default = "default_protocol_tool_review_parallelism")]
    pub(crate) tool_review_parallelism: usize,
    #[serde(default, skip_serializing_if = "ProtocolReasoningPolicy::is_auto")]
    pub(crate) reasoning: ProtocolReasoningPolicy,
}

impl Protocol {
    fn validate(&self) -> Result<(), PrepareError> {
        self.model.validate_for("profile.protocol.model")?;
        if self.max_tokens == 0 {
            return Err(profile_error(
                "protocol.max_tokens must be greater than zero",
            ));
        }
        if self.tool_review_parallelism == 0 {
            return Err(profile_error(
                "protocol.tool_review_parallelism must be greater than zero",
            ));
        }
        self.reasoning.validate().map_err(profile_error)?;
        Ok(())
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Self {
            model: ModelDefaults::default(),
            max_tokens: default_protocol_max_tokens(),
            tool_review_parallelism: default_protocol_tool_review_parallelism(),
            reasoning: ProtocolReasoningPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Oracle {
    #[serde(default)]
    pub(crate) mode: OracleMode,
    #[serde(default = "default_oracle_require_evidence")]
    pub(crate) require_evidence: bool,
    #[serde(default)]
    pub(crate) gate: OracleGate,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Execution {
    pub(crate) stop_after: ExecutionStopAfter,
    #[serde(default = "default_observe_child_stale_after_secs")]
    pub(crate) observe_child_stale_after_secs: u64,
    #[serde(default)]
    pub(crate) broad_tui: BroadTui,
    #[serde(default)]
    pub(crate) trace_jsonl: TraceJsonl,
    #[serde(default)]
    pub(crate) debug_tools: bool,
    #[serde(default)]
    pub(crate) mbe: Mbe,
}

impl Execution {
    fn validate(&self) -> Result<(), PrepareError> {
        if self.observe_child_stale_after_secs == 0 {
            return Err(profile_error(
                "execution.observe_child_stale_after_secs must be greater than zero",
            ));
        }
        self.broad_tui.validate()?;
        self.mbe.validate()
    }

    pub(crate) fn state_stop_after(&self) -> Prototype1StateStopAfter {
        match self.stop_after {
            ExecutionStopAfter::Materialize => Prototype1StateStopAfter::Materialize,
            ExecutionStopAfter::Build => Prototype1StateStopAfter::Build,
            ExecutionStopAfter::Spawn => Prototype1StateStopAfter::Spawn,
            ExecutionStopAfter::Complete => Prototype1StateStopAfter::Complete,
        }
    }

    pub(crate) fn observe_child_stale_after(&self) -> Duration {
        Duration::from_secs(self.observe_child_stale_after_secs)
    }
}

impl Default for Execution {
    fn default() -> Self {
        Self {
            stop_after: ExecutionStopAfter::Complete,
            observe_child_stale_after_secs: default_observe_child_stale_after_secs(),
            broad_tui: BroadTui::default(),
            trace_jsonl: TraceJsonl::Inherit,
            debug_tools: false,
            mbe: Mbe::default(),
        }
    }
}

fn default_observe_child_stale_after_secs() -> u64 {
    DEFAULT_OBSERVE_CHILD_STALE_AFTER_SECS
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BroadTui {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_attempts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fresh_slots_per_child: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) graph_nearest: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) timeout_secs: Option<u64>,
}

impl BroadTui {
    fn validate(self) -> Result<(), PrepareError> {
        if self.max_attempts == Some(0) {
            return Err(profile_error(
                "execution.broad_tui.max_attempts must be greater than zero",
            ));
        }
        if self.fresh_slots_per_child == Some(0) {
            return Err(profile_error(
                "execution.broad_tui.fresh_slots_per_child must be greater than zero",
            ));
        }
        if self.graph_nearest == Some(0) {
            return Err(profile_error(
                "execution.broad_tui.graph_nearest must be greater than zero",
            ));
        }
        if self.timeout_secs == Some(0) {
            return Err(profile_error(
                "execution.broad_tui.timeout_secs must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mbe {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default = "default_mbe_python")]
    pub(crate) python: String,
    #[serde(default = "default_mbe_workers")]
    pub(crate) workers: u32,
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

impl Mbe {
    fn validate(&self) -> Result<(), PrepareError> {
        if self.enabled && self.python.trim().is_empty() {
            return Err(profile_error(
                "execution.mbe.python must not be empty when MBE oracle execution is enabled",
            ));
        }
        if self.enabled && self.workers == 0 {
            return Err(profile_error(
                "execution.mbe.workers must be nonzero when MBE oracle execution is enabled",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExecutionStopAfter {
    Materialize,
    Build,
    Spawn,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TraceJsonl {
    Inherit,
    Auto,
    Off,
}

impl Default for TraceJsonl {
    fn default() -> Self {
        Self::Inherit
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Control {
    #[serde(default)]
    pub(crate) mode: RunMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parallel_cap: Option<u32>,
}

impl Control {
    // ANCHOR: prototype1_control_parallel_cap_validate
    fn validate(self, search: &Search) -> Result<(), PrepareError> {
        if let Some(parallel_cap) = self.parallel_cap {
            let derived_parallel_cap = search.default_parallel_cap();
            if parallel_cap == 0 || parallel_cap > derived_parallel_cap {
                return Err(profile_error(format!(
                    "profile control.parallel_cap {} widens admitted fanout {}",
                    parallel_cap, derived_parallel_cap
                )));
            }
        }
        Ok(())
    }
    // ANCHOR_END: prototype1_control_parallel_cap_validate
}

impl Default for Control {
    fn default() -> Self {
        Self {
            mode: RunMode::Continuous,
            parallel_cap: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunMode {
    Continuous,
    Step,
}

impl Default for RunMode {
    fn default() -> Self {
        Self::Continuous
    }
}

fn default_mbe_python() -> String {
    "python".to_string()
}

fn default_mbe_workers() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RunProfileCommitment {
    pub(crate) schema_version: String,
    pub(crate) profile_path: PathBuf,
    pub(crate) sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) admitted_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct OperatorRunProfile {
    pub(crate) source_path: PathBuf,
    pub(crate) profile: Prototype1RunProfile,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RunProfilePlan {
    source_path: PathBuf,
    profile_path: PathBuf,
    profile: Prototype1RunProfile,
    #[serde(skip)]
    normalized_toml: String,
    sha256: String,
}

impl RunProfilePlan {
    pub(crate) fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub(crate) fn profile_path(&self) -> &Path {
        &self.profile_path
    }

    pub(crate) fn profile(&self) -> &Prototype1RunProfile {
        &self.profile
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    #[cfg(test)]
    pub(crate) fn normalized_toml(&self) -> &str {
        &self.normalized_toml
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EffectiveRunControl {
    pub(crate) path: PathBuf,
    pub(crate) mode: RunMode,
    pub(crate) parallel_cap: u32,
    pub(crate) patch_generation_parallel_cap: u32,
    pub(crate) defaulted_from_profile: bool,
    pub(crate) patch_generation_defaulted_from_profile: bool,
}

pub(crate) fn resolve_effective_control(
    path: PathBuf,
    profile: &Prototype1RunProfile,
) -> Result<EffectiveRunControl, PrepareError> {
    let derived_parallel_cap = profile.default_parallel_cap();
    let parallel_cap = profile.control.parallel_cap.unwrap_or(derived_parallel_cap);
    if parallel_cap == 0 || parallel_cap > derived_parallel_cap {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "profile control.parallel_cap {} widens admitted fanout {} at '{}'",
                parallel_cap,
                derived_parallel_cap,
                path.display()
            ),
        });
    }
    Ok(EffectiveRunControl {
        path,
        mode: profile.control.mode,
        parallel_cap,
        patch_generation_parallel_cap: profile.patch_generation_parallel_cap(),
        defaulted_from_profile: profile.control.parallel_cap.is_none(),
        patch_generation_defaulted_from_profile: profile.search.children.parallel_targets.is_none(),
    })
}

#[derive(Debug, Clone)]
pub(crate) struct AdmittedRunProfile {
    pub(crate) commitment: RunProfileCommitment,
    pub(crate) profile: Prototype1RunProfile,
}

pub(crate) fn load_operator_profile(
    name_or_path: &str,
) -> Result<OperatorRunProfile, PrepareError> {
    let source_path = resolve_operator_profile_path(name_or_path)?;
    let text = fs::read_to_string(&source_path).map_err(|source| PrepareError::ReadManifest {
        path: source_path.clone(),
        source,
    })?;
    let profile = parse_operator_profile(&source_path, &text)?;
    Ok(OperatorRunProfile {
        source_path,
        profile,
    })
}

pub(crate) fn plan_run_profile(
    campaign_manifest_path: &Path,
    operator: &OperatorRunProfile,
) -> Result<RunProfilePlan, PrepareError> {
    validate_operator_profile(&operator.profile)?;
    let normalized_toml =
        toml::to_string_pretty(&operator.profile).map_err(|err| profile_error(err.to_string()))?;
    Ok(RunProfilePlan {
        source_path: operator.source_path.clone(),
        profile_path: run_profile_path(campaign_manifest_path),
        profile: operator.profile.clone(),
        sha256: sha256_hex(&normalized_toml),
        normalized_toml,
    })
}

pub(crate) fn admit_run_profile(
    campaign_manifest_path: &Path,
    operator: &OperatorRunProfile,
) -> Result<AdmittedRunProfile, PrepareError> {
    admit_run_profile_plan(plan_run_profile(campaign_manifest_path, operator)?)
}

pub(crate) fn admit_run_profile_plan(
    plan: RunProfilePlan,
) -> Result<AdmittedRunProfile, PrepareError> {
    let RunProfilePlan {
        source_path,
        profile_path,
        profile,
        normalized_toml,
        sha256,
    } = plan;
    let actual_sha = sha256_hex(&normalized_toml);
    if actual_sha != sha256 {
        return Err(profile_error(format!(
            "run profile plan digest mismatch: recorded '{}', resolved '{}'",
            sha256, actual_sha
        )));
    }
    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let stored_path = profile_path.with_file_name(RUN_PROFILE_COMMITMENT_FILE);
    if profile_path.exists() || stored_path.exists() {
        return Err(profile_error(format!(
            "run profile admission requires new profile and commitment paths; found existing state under '{}'",
            profile_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .display()
        )));
    }
    let commitment = RunProfileCommitment {
        schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
        profile_path: profile_path.clone(),
        sha256,
        source_path: Some(source_path),
        admitted_at: Utc::now().to_rfc3339(),
    };
    let commitment_bytes =
        serde_json::to_vec_pretty(&commitment).map_err(PrepareError::Serialize)?;
    if let Err(source) = crate::durable_io::write_atomic(&profile_path, normalized_toml.as_bytes())
    {
        let _ = fs::remove_file(&profile_path);
        return Err(PrepareError::WriteManifest {
            path: profile_path.clone(),
            source,
        });
    }
    if let Err(error) = write_commitment(&profile_path, &commitment_bytes) {
        let _ = fs::remove_file(&profile_path);
        let _ = fs::remove_file(&stored_path);
        return Err(error);
    }
    Ok(AdmittedRunProfile {
        commitment,
        profile,
    })
}

/// Reconcile a setup-owned profile plan without accepting divergent or
/// commitment-first partial state.
pub(crate) fn ensure_run_profile_plan(
    plan: RunProfilePlan,
    admitted_at: &str,
) -> Result<AdmittedRunProfile, PrepareError> {
    let RunProfilePlan {
        source_path,
        profile_path,
        profile,
        normalized_toml,
        sha256,
    } = plan;
    let actual_sha = sha256_hex(&normalized_toml);
    if actual_sha != sha256 {
        return Err(profile_error(format!(
            "run profile plan digest mismatch: recorded '{}', resolved '{}'",
            sha256, actual_sha
        )));
    }
    let stored_path = profile_path.with_file_name(RUN_PROFILE_COMMITMENT_FILE);
    let commitment = RunProfileCommitment {
        schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
        profile_path: profile_path.clone(),
        sha256,
        source_path: Some(source_path),
        admitted_at: admitted_at.to_string(),
    };

    let profile_exists = profile_path.exists();
    let commitment_exists = stored_path.exists();
    if profile_exists {
        let observed =
            fs::read_to_string(&profile_path).map_err(|source| PrepareError::ReadManifest {
                path: profile_path.clone(),
                source,
            })?;
        if observed != normalized_toml {
            return Err(profile_error(format!(
                "setup profile reconciliation conflict at '{}': stored profile differs from the admitted plan",
                profile_path.display()
            )));
        }
    }
    if commitment_exists {
        let observed = load_commitment_from_path(&stored_path)?.ok_or_else(|| {
            profile_error(format!(
                "setup profile reconciliation could not read commitment '{}'",
                stored_path.display()
            ))
        })?;
        if observed != commitment {
            return Err(profile_error(format!(
                "setup profile reconciliation conflict at '{}': stored commitment differs from the admission receipt",
                stored_path.display()
            )));
        }
    }
    if !profile_exists && commitment_exists {
        return Err(profile_error(format!(
            "setup profile reconciliation found commitment '{}' without profile '{}'; automatic recovery is unsafe",
            stored_path.display(),
            profile_path.display()
        )));
    }

    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    if !profile_exists {
        crate::durable_io::write_atomic(&profile_path, normalized_toml.as_bytes()).map_err(
            |source| PrepareError::WriteManifest {
                path: profile_path.clone(),
                source,
            },
        )?;
    }
    if !commitment_exists {
        let bytes = serde_json::to_vec_pretty(&commitment).map_err(PrepareError::Serialize)?;
        write_commitment(&profile_path, &bytes)?;
    }

    let stored_profile =
        fs::read_to_string(&profile_path).map_err(|source| PrepareError::ReadManifest {
            path: profile_path.clone(),
            source,
        })?;
    let observed_profile = parse_profile(&profile_path, &stored_profile)?;
    let observed_commitment = load_commitment_from_path(&stored_path)?.ok_or_else(|| {
        profile_error(format!(
            "setup profile reconciliation did not produce commitment '{}'",
            stored_path.display()
        ))
    })?;
    if observed_commitment != commitment || observed_profile != profile {
        return Err(profile_error(format!(
            "setup profile reconciliation read-back mismatch at '{}'",
            profile_path.display()
        )));
    }
    Ok(AdmittedRunProfile {
        commitment: observed_commitment,
        profile: observed_profile,
    })
}

pub(crate) fn load_admitted_run_profile(
    campaign_manifest_path: &Path,
) -> Result<Option<AdmittedRunProfile>, PrepareError> {
    let profile_path = run_profile_path(campaign_manifest_path);
    let commitment = load_commitment(campaign_manifest_path)?;
    let text = match fs::read_to_string(&profile_path) {
        Ok(text) => Some(text),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: profile_path.clone(),
                source,
            });
        }
    };
    let (text, commitment) = match (text, commitment) {
        (None, None) => return Ok(None),
        (Some(_), None) => {
            return Err(profile_error(format!(
                "campaign run profile '{}' has no admission commitment",
                profile_path.display()
            )));
        }
        (None, Some(_)) => {
            return Err(profile_error(format!(
                "campaign run profile commitment exists but '{}' is missing",
                profile_path.display()
            )));
        }
        (Some(text), Some(commitment)) => (text, commitment),
    };
    if commitment.schema_version != RUN_PROFILE_COMMITMENT_SCHEMA_VERSION {
        return Err(profile_error(format!(
            "campaign run profile commitment at '{}' has unsupported schema '{}'",
            commitment_path(campaign_manifest_path).display(),
            commitment.schema_version
        )));
    }
    if commitment.profile_path != profile_path {
        return Err(profile_error(format!(
            "campaign run profile commitment path '{}' does not match expected '{}'",
            commitment.profile_path.display(),
            profile_path.display()
        )));
    }
    if commitment.sha256 != sha256_hex(&text) {
        return Err(profile_error(format!(
            "campaign run profile digest mismatch for '{}'",
            profile_path.display()
        )));
    }
    let profile = parse_profile(&profile_path, &text)?;
    Ok(Some(AdmittedRunProfile {
        commitment,
        profile,
    }))
}

pub(crate) fn load_admitted_commitment_from_prototype_root(
    prototype1_root: &Path,
) -> Result<Option<RunProfileCommitment>, PrepareError> {
    load_commitment_from_path(&prototype1_root.join(RUN_PROFILE_COMMITMENT_FILE))
}

fn parse_profile(path: &Path, text: &str) -> Result<Prototype1RunProfile, PrepareError> {
    let profile = toml::from_str::<Prototype1RunProfile>(text).map_err(|source| {
        profile_error(format!(
            "failed to parse Prototype 1 run profile '{}': {source}",
            path.display()
        ))
    })?;
    profile.validate()?;
    Ok(profile)
}

fn parse_operator_profile(path: &Path, text: &str) -> Result<Prototype1RunProfile, PrepareError> {
    let profile = parse_profile(path, text)?;
    validate_operator_profile(&profile)?;
    Ok(profile)
}

fn validate_operator_profile(profile: &Prototype1RunProfile) -> Result<(), PrepareError> {
    if profile.generation.surface.is_some() {
        return Err(profile_error(
            "profile.generation.surface is retired; remove it from profiles used for new runs",
        ));
    }
    Ok(())
}

fn resolve_operator_profile_path(name_or_path: &str) -> Result<PathBuf, PrepareError> {
    let path = Path::new(name_or_path);
    if path.is_absolute() || path.components().count() > 1 || name_or_path.ends_with(".toml") {
        return Ok(path.to_path_buf());
    }
    Ok(ploke_eval_home()?
        .join("profiles")
        .join("prototype1")
        .join(format!("{name_or_path}.toml")))
}

fn run_profile_path(campaign_manifest_path: &Path) -> PathBuf {
    prototype1_root(campaign_manifest_path).join(RUN_PROFILE_FILE)
}

fn commitment_path(campaign_manifest_path: &Path) -> PathBuf {
    prototype1_root(campaign_manifest_path).join(RUN_PROFILE_COMMITMENT_FILE)
}

fn prototype1_root(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn write_commitment(profile_path: &Path, bytes: &[u8]) -> Result<(), PrepareError> {
    let path = profile_path.with_file_name(RUN_PROFILE_COMMITMENT_FILE);
    crate::durable_io::write_atomic(&path, bytes)
        .map_err(|source| PrepareError::WriteManifest { path, source })
}

fn load_commitment(
    campaign_manifest_path: &Path,
) -> Result<Option<RunProfileCommitment>, PrepareError> {
    load_commitment_from_path(&commitment_path(campaign_manifest_path))
}

fn load_commitment_from_path(path: &Path) -> Result<Option<RunProfileCommitment>, PrepareError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|source| PrepareError::ParseManifest {
            path: path.to_path_buf(),
            source,
        })
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn profile_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: detail.into(),
    }
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
# Keep this aligned with the 4096 safe floor in
# campaign::PROTOTYPE1_PROTOCOL_MIN_SAFE_MAX_TOKENS. Lower budgets previously
# reproduced direct-Google malformed/truncated structured output before closure.
max_tokens = 4096
tool_review_parallelism = 2

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
    fn run_profile_toml_preserves_runtime_shape() {
        let profile = parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses");

        assert_eq!(
            profile.generation.candidate_generator(),
            Prototype1CandidateGenerator::BroadHarnessRequest
        );
        assert_eq!(
            profile.selection.traversal_metrics(),
            Prototype1TraversalMetrics::OperationalAndProtocol
        );
        assert!(profile.selection.metrics.persist);
        assert!(profile.selection.metrics.imp_at_k.enabled);
        assert_eq!(profile.selection.metrics.imp_at_k.budget_k, 50);
        assert_eq!(profile.selection.oracle_mode(), OracleMode::RecordOnly);
        assert!(profile.selection.oracle_require_evidence());
        assert_eq!(profile.selection.oracle_gate(), OracleGate::Disabled);
        assert_eq!(profile.protocol_policy().max_tokens, 4096);
        assert_eq!(profile.protocol_policy().tool_review_parallelism, 2);
        assert_eq!(
            profile.protocol_policy().reasoning,
            ProtocolReasoningPolicy::omit()
        );
        assert_eq!(
            profile.model.parsed_id().expect("model id parses").as_ref(),
            Some(&"google/gemini-3.5-flash".parse().expect("model id"))
        );
        assert_eq!(
            profile.model.route_source,
            Some(ModelRouteSource::DirectGoogle)
        );
        assert_eq!(profile.model.provider.as_deref(), Some("google"));
        assert_eq!(
            profile.search_policy().child_budget,
            Prototype1ChildBudget::new(6, 6)
        );
        assert_eq!(
            profile.execution.state_stop_after(),
            Prototype1StateStopAfter::Complete
        );
        assert_eq!(
            profile.execution.observe_child_stale_after(),
            Duration::from_secs(1200)
        );
        assert_eq!(profile.execution.broad_tui.max_attempts, Some(2));
        assert_eq!(profile.execution.broad_tui.fresh_slots_per_child, Some(2));
        assert_eq!(profile.execution.broad_tui.graph_nearest, Some(13));
        assert_eq!(profile.storage.eval.backend, EvalStorageBackend::Fs);
        assert!(profile.execution.mbe.enabled);
        assert_eq!(profile.execution.mbe.python, "python3");
        assert_eq!(profile.execution.mbe.workers, 2);
    }

    #[test]
    fn run_profile_storage_eval_backend_defaults_to_fs() {
        let profile = parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses");

        assert_eq!(profile.storage.eval.backend, EvalStorageBackend::Fs);
    }

    #[test]
    fn run_profile_storage_eval_backend_roundtrips_kebab_case() {
        for (backend, expected) in [
            ("db-mirror", EvalStorageBackend::DbMirror),
            ("dual-strict", EvalStorageBackend::DualStrict),
        ] {
            let text = PROFILE.replace(
                "[target]",
                &format!("[storage.eval]\nbackend = \"{backend}\"\n\n[target]"),
            );
            let profile = parse_profile(Path::new("profile.toml"), &text).expect("profile parses");
            let encoded = toml::to_string(&profile).expect("serialize profile");
            let decoded =
                parse_profile(Path::new("profile.toml"), &encoded).expect("roundtrip parses");

            assert_eq!(profile.storage.eval.backend, expected);
            assert_eq!(decoded.storage.eval.backend, expected);
            assert!(encoded.contains("[storage.eval]"));
            assert!(encoded.contains(&format!("backend = \"{backend}\"")));
        }
    }

    #[test]
    fn run_profile_storage_eval_backend_accepts_legacy_database_alias() {
        let text = PROFILE.replace(
            "[target]",
            "[storage.eval]\nbackend = \"database\"\n\n[target]",
        );
        let profile = parse_profile(Path::new("profile.toml"), &text).expect("profile parses");

        assert_eq!(profile.storage.eval.backend, EvalStorageBackend::Database);
    }

    #[test]
    fn run_profile_storage_eval_backend_rejects_unknown() {
        let text = PROFILE.replace(
            "[target]",
            "[storage.eval]\nbackend = \"db-only\"\n\n[target]",
        );
        let err = parse_profile(Path::new("profile.toml"), &text)
            .expect_err("unknown eval storage backend should fail");

        assert!(err.to_string().contains("backend"));
    }

    #[test]
    fn run_profile_execution_rejects_zero_observe_child_stale_after() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace(
                "observe_child_stale_after_secs = 1200",
                "observe_child_stale_after_secs = 0",
            ),
        )
        .expect_err("zero stale-observe threshold should be rejected");

        assert!(
            err.to_string()
                .contains("execution.observe_child_stale_after_secs")
        );
    }

    #[test]
    fn run_profile_execution_defaults_observe_child_stale_after() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("observe_child_stale_after_secs = 1200\n", ""),
        )
        .expect("profile parses");

        assert_eq!(
            profile.execution.observe_child_stale_after(),
            Duration::from_secs(DEFAULT_OBSERVE_CHILD_STALE_AFTER_SECS)
        );
    }

    #[test]
    fn run_profile_execution_rejects_zero_broad_tui_max_attempts() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("max_attempts = 2", "max_attempts = 0"),
        )
        .expect_err("zero broad TUI max attempts should be rejected");

        assert!(err.to_string().contains("execution.broad_tui.max_attempts"));
    }

    #[test]
    fn run_profile_execution_rejects_zero_broad_tui_graph_nearest() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("graph_nearest = 13", "graph_nearest = 0"),
        )
        .expect_err("zero graph-neighborhood limit should be rejected");

        assert!(
            err.to_string()
                .contains("execution.broad_tui.graph_nearest")
        );
    }

    #[test]
    fn run_profile_model_route_defaults_roundtrip_as_kebab_case() {
        let profile = parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses");
        let text = toml::to_string(&profile).expect("serialize profile");

        assert!(text.contains("[model]"));
        assert!(text.contains("route_source = \"direct-google\""));
    }

    #[test]
    fn run_profile_model_rejects_openrouter_provider_on_direct_google_route() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("provider = \"google\"", "provider = \"google-ai-studio\""),
        )
        .expect_err("direct Google route should reject OpenRouter provider pins");

        assert!(
            err.to_string()
                .contains("profile.model.route_source = direct-google")
        );
    }

    #[test]
    fn default_generation_uses_broad_harness_surface() {
        let generation = Generation::default();

        assert_eq!(
            generation.candidate_generator(),
            Prototype1CandidateGenerator::BroadHarnessRequest
        );
    }

    #[test]
    fn runtime_generation_sources_parse_as_shared_run_profile_record() {
        for source in ["legacy", "broad-harness-request", "deterministic-tui-tools"] {
            let text = PROFILE.replace(
                "source = \"broad-harness-request\"",
                &format!("source = \"{source}\""),
            );
            parse_profile(Path::new("profile.toml"), &text)
                .expect("runtime profile parser accepts source");
            toml::from_str::<ploke_records::run_profile::RunProfileRecord>(&text)
                .expect("shared passive profile record accepts runtime source");
        }
    }

    #[test]
    fn shared_passive_run_profile_matches_runtime_wire_shape() {
        let text = PROFILE
            .replace(
                "children = { min = 6, max = 6 }",
                "children = { min = 6, max = 6, parallel_targets = 3 }",
            )
            .replace(
                "tool_review_parallelism = 2\n\n[protocol.reasoning]",
                "tool_review_parallelism = 2\n\n[protocol.model]\nid = \"openai/gpt-5.1\"\nroute_source = \"openrouter\"\nprovider = \"openai\"\n\n[protocol.reasoning]",
            );
        let runtime = parse_profile(Path::new("profile.toml"), &text).expect("runtime parses");
        let passive: ploke_records::run_profile::RunProfileRecord =
            toml::from_str(&text).expect("passive record parses");
        let runtime_wire: toml::Value =
            toml::from_str(&toml::to_string(&runtime).expect("runtime profile serializes"))
                .expect("runtime wire parses");
        let passive_wire: toml::Value =
            toml::from_str(&toml::to_string(&passive).expect("passive profile serializes"))
                .expect("passive wire parses");

        assert_eq!(runtime_wire, passive_wire);
    }

    #[test]
    fn shared_passive_run_profile_matches_runtime_defaults() {
        let text = r#"
schema_version = "prototype1-run-profile.v1"
name = "runtime-defaults"
"#;
        let runtime = parse_profile(Path::new("profile.toml"), text).expect("runtime parses");
        let passive: ploke_records::run_profile::RunProfileRecord =
            toml::from_str(text).expect("passive record parses");
        let runtime_wire: toml::Value =
            toml::from_str(&toml::to_string(&runtime).expect("runtime profile serializes"))
                .expect("runtime wire parses");
        let passive_wire: toml::Value =
            toml::from_str(&toml::to_string(&passive).expect("passive profile serializes"))
                .expect("passive wire parses");

        assert_eq!(runtime_wire, passive_wire);
    }

    #[test]
    fn passive_profile_preserves_retired_token_and_protocol_routing_fields() {
        let text = r#"
schema_version = "prototype1-run-profile.v1"
name = "historical-profile"

[model]
max_tokens = 32768

[protocol]
model_id = "google/gemini-2.5-pro"
route_source = "direct-google"
provider = "google"
"#;
        let passive: ploke_records::run_profile::RunProfileRecord =
            toml::from_str(text).expect("passive history parses legacy fields");

        assert_eq!(passive.model.legacy_max_tokens, Some(32_768));
        assert_eq!(
            passive.protocol.legacy_model_id.as_deref(),
            Some("google/gemini-2.5-pro")
        );
        assert_eq!(passive.protocol.legacy_provider.as_deref(), Some("google"));
        assert_eq!(
            passive.protocol.legacy_route_source,
            Some(ploke_records::run_profile::ModelRouteSource::DirectGoogle)
        );
        let encoded = toml::to_string(&passive).expect("passive history serializes");
        assert!(encoded.contains("max_tokens = 32768"));
        assert!(encoded.contains("model_id = \"google/gemini-2.5-pro\""));
    }

    #[test]
    fn runtime_profile_rejects_retired_eval_and_flat_protocol_authority() {
        for (table, key) in [
            ("[model]\nmax_tokens = 32768", "max_tokens"),
            (
                "[protocol]\nmodel_id = \"google/gemini-2.5-pro\"",
                "model_id",
            ),
        ] {
            let text = format!(
                "schema_version = \"prototype1-run-profile.v1\"\nname = \"retired-authority\"\n\n{table}\n"
            );
            let error = parse_profile(Path::new("profile.toml"), &text)
                .expect_err("runtime profile must reject retired authority")
                .to_string();
            assert!(error.contains(key), "unexpected error: {error}");
        }
    }

    #[test]
    fn admitted_runtime_profile_rejects_hash_committed_legacy_authority() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = temp.path().join("campaign.json");
        let profile_path = run_profile_path(&manifest);
        let text = r#"
schema_version = "prototype1-run-profile.v1"
name = "historical-profile"

[protocol]
model_id = "google/gemini-2.5-pro"
route_source = "direct-google"
provider = "google"
"#;
        let commitment = RunProfileCommitment {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: profile_path.clone(),
            sha256: sha256_hex(text),
            source_path: Some(PathBuf::from("/historical/profile.toml")),
            admitted_at: "2026-06-25T12:00:00Z".to_string(),
        };
        fs::create_dir_all(profile_path.parent().expect("profile parent"))
            .expect("create profile parent");
        fs::write(&profile_path, text).expect("write historical profile");
        fs::write(
            commitment_path(&manifest),
            serde_json::to_vec_pretty(&commitment).expect("serialize commitment"),
        )
        .expect("write commitment");

        let error = load_admitted_run_profile(&manifest)
            .expect_err("legacy fields cannot become runtime authority")
            .to_string();
        assert!(error.contains("model_id"), "unexpected error: {error}");
    }

    #[test]
    fn runtime_rejects_sources_that_passive_history_preserves() {
        for (source, expected) in [
            (
                "edit-surface",
                ploke_records::run_profile::GenerationSource::EditSurface,
            ),
            (
                "broad-harness",
                ploke_records::run_profile::GenerationSource::BroadHarness,
            ),
        ] {
            let text = PROFILE.replace("broad-harness-request", source);
            let runtime_error = parse_profile(Path::new("profile.toml"), &text)
                .expect_err("runtime must reject retired generation source")
                .to_string();
            let passive: ploke_records::run_profile::RunProfileRecord = toml::from_str(&text)
                .expect("passive history must preserve a known v1 generation source");

            assert!(runtime_error.contains(source));
            assert_eq!(passive.generation.source, expected);
        }
    }

    #[test]
    fn shared_passive_run_profile_preserves_historical_generation_surface() {
        let text = PROFILE.replace(
            "source = \"broad-harness-request\"",
            "source = \"broad-harness-request\"\nsurface = \"workspace-except-ploke-eval\"",
        );
        let runtime = parse_profile(Path::new("profile.toml"), &text)
            .expect("runtime must preserve a known historical surface");
        let passive: ploke_records::run_profile::RunProfileRecord =
            toml::from_str(&text).expect("passive record must preserve a known historical surface");
        let runtime_wire = toml::to_string(&runtime).expect("runtime profile serializes");
        let passive_wire = toml::to_string(&passive).expect("passive profile serializes");

        assert!(runtime.generation.surface.is_some());
        assert_eq!(runtime_wire, passive_wire);
        assert!(runtime_wire.contains("surface = \"workspace-except-ploke-eval\""));
    }

    #[test]
    fn operator_profile_rejects_historical_generation_surface() {
        let temp = tempfile::tempdir().expect("tempdir");
        let text = PROFILE.replace(
            "source = \"broad-harness-request\"",
            "source = \"broad-harness-request\"\nsurface = \"workspace-except-ploke-eval\"",
        );
        let path = temp.path().join("operator.toml");
        fs::write(&path, &text).expect("write operator profile");
        let error = load_operator_profile(path.to_str().expect("utf-8 test path"))
            .expect_err("new operator admission must reject a retired generation surface")
            .to_string();

        assert!(error.contains("profile.generation.surface is retired"));
    }

    #[test]
    fn plan_run_profile_rejects_constructed_operator_with_retired_surface() {
        let temp = tempfile::tempdir().expect("tempdir");
        let text = PROFILE.replace(
            "source = \"broad-harness-request\"",
            "source = \"broad-harness-request\"\nsurface = \"workspace-except-ploke-eval\"",
        );
        let operator = OperatorRunProfile {
            source_path: temp.path().join("operator.toml"),
            profile: parse_profile(Path::new("historical.toml"), &text)
                .expect("historical profile remains structurally readable"),
        };
        let plan_error = plan_run_profile(&temp.path().join("campaign.json"), &operator)
            .expect_err("plan boundary must reject a directly constructed legacy operator")
            .to_string();
        assert!(plan_error.contains("profile.generation.surface is retired"));
    }

    #[test]
    fn admitted_profile_preserves_historical_surface_commitment() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = temp.path().join("campaign.json");
        let profile_path = run_profile_path(&manifest);
        let text = PROFILE.replace(
            "source = \"broad-harness-request\"",
            "source = \"broad-harness-request\"\nsurface = \"workspace-except-ploke-eval\"",
        );
        let digest = sha256_hex(&text);
        let commitment = RunProfileCommitment {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: profile_path.clone(),
            sha256: digest.clone(),
            source_path: Some(PathBuf::from("/historical/operator.toml")),
            admitted_at: "2026-06-25T12:00:00Z".to_string(),
        };
        fs::create_dir_all(profile_path.parent().expect("profile directory"))
            .expect("create profile directory");
        fs::write(&profile_path, &text).expect("write historical profile bytes");
        fs::write(
            commitment_path(&manifest),
            serde_json::to_vec_pretty(&commitment).expect("encode commitment"),
        )
        .expect("write commitment");

        let admitted = load_admitted_run_profile(&manifest)
            .expect("load historical commitment")
            .expect("admitted profile");

        assert_eq!(admitted.commitment.sha256, digest);
        assert!(admitted.profile.generation.surface.is_some());
        assert_eq!(
            fs::read_to_string(&profile_path).expect("read preserved profile"),
            text
        );
    }

    #[test]
    fn shared_passive_run_profile_rejects_current_key_typos() {
        for (valid, typo, key) in [
            (
                "worktree_root = \"~/.ploke-eval/worktrees\"",
                "worktree_rooot = \"~/.ploke-eval/worktrees\"",
                "worktree_rooot",
            ),
            (
                "dataset_key = \"ripgrep\"",
                "dataset_keey = \"ripgrep\"",
                "dataset_keey",
            ),
            (
                "route_source = \"direct-google\"",
                "route_soruce = \"direct-google\"",
                "route_soruce",
            ),
            (
                "max_generations = 15",
                "max_generatons = 15",
                "max_generatons",
            ),
            (
                "children = { min = 6, max = 6 }",
                "children = { min = 6, max = 6, parallel_targtes = 3 }",
                "parallel_targtes",
            ),
            (
                "strategy = \"history-score-child-prop\"",
                "stratgey = \"history-score-child-prop\"",
                "stratgey",
            ),
            ("persist = true", "perist = true", "perist"),
            ("budget_k = 50", "budget_kk = 50", "budget_kk"),
            (
                "require_evidence = true",
                "require_evidnce = true",
                "require_evidnce",
            ),
            (
                "tool_review_parallelism = 2",
                "tool_review_parallellism = 2",
                "tool_review_parallellism",
            ),
            ("mode = \"omit\"", "modde = \"omit\"", "modde"),
            (
                "observe_child_stale_after_secs = 1200",
                "observe_child_stale_after_sec = 1200",
                "observe_child_stale_after_sec",
            ),
            (
                "mbe = { enabled = true, python = \"python3\", workers = 2 }",
                "mbe = { enabled = true, python = \"python3\", wokers = 2 }",
                "wokers",
            ),
            ("max_attempts = 2", "max_atempts = 2", "max_atempts"),
        ] {
            let text = PROFILE.replacen(valid, typo, 1);
            let runtime_error = parse_profile(Path::new("profile.toml"), &text)
                .expect_err("runtime must reject misspelled current key")
                .to_string();
            let passive_error =
                toml::from_str::<ploke_records::run_profile::RunProfileRecord>(&text)
                    .expect_err("passive record must reject misspelled current key")
                    .to_string();

            assert!(runtime_error.contains(key));
            assert!(passive_error.contains(key));
        }

        for (table, key) in [
            ("[storage.eval]\nbackned = \"fs\"", "backned"),
            ("[control]\nparallell_cap = 1", "parallell_cap"),
        ] {
            let text = format!("{PROFILE}\n{table}\n");
            let runtime_error = parse_profile(Path::new("profile.toml"), &text)
                .expect_err("runtime must reject misspelled nested key")
                .to_string();
            let passive_error =
                toml::from_str::<ploke_records::run_profile::RunProfileRecord>(&text)
                    .expect_err("passive record must reject misspelled nested key")
                    .to_string();

            assert!(runtime_error.contains(key));
            assert!(passive_error.contains(key));
        }
    }

    #[test]
    fn plan_is_non_writing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let campaign_root = tmp.path().join("campaign");
        let manifest_path = campaign_root.join("campaign.json");
        let source_path = tmp.path().join("profiles").join("overnight.toml");
        let operator = OperatorRunProfile {
            source_path: source_path.clone(),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };

        let plan = plan_run_profile(&manifest_path, &operator).expect("plan profile");

        assert_eq!(plan.source_path, source_path);
        assert_eq!(
            plan.profile_path,
            campaign_root.join("prototype1").join(RUN_PROFILE_FILE)
        );
        assert_eq!(plan.profile, operator.profile);
        assert_eq!(plan.sha256, sha256_hex(&plan.normalized_toml));
        assert!(!campaign_root.exists());
        assert_eq!(fs::read_dir(tmp.path()).expect("read tempdir").count(), 0);
    }

    #[test]
    fn plan_admits_exact_bytes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("profiles").join("overnight.toml"),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };
        let plan = plan_run_profile(&manifest_path, &operator).expect("plan profile");
        let profile_path = plan.profile_path.clone();
        let normalized_toml = plan.normalized_toml.clone();
        let planned_sha = plan.sha256.clone();

        let admitted = admit_run_profile_plan(plan).expect("admit planned profile");
        let admitted_bytes = fs::read(&profile_path).expect("read admitted profile");
        let stored_commitment = load_commitment(&manifest_path)
            .expect("load commitment")
            .expect("commitment exists");

        assert_eq!(admitted_bytes.as_slice(), normalized_toml.as_bytes());
        assert_eq!(sha256_hex(&normalized_toml), planned_sha);
        assert_eq!(admitted.commitment.sha256, planned_sha);
        assert_eq!(stored_commitment.sha256, planned_sha);
        assert_eq!(stored_commitment, admitted.commitment);
        let loaded = load_admitted_run_profile(&manifest_path)
            .expect("strict load")
            .expect("admitted profile");
        assert_eq!(loaded.commitment, admitted.commitment);
        assert_eq!(loaded.profile, admitted.profile);
    }

    #[test]
    fn setup_profile_reconciliation_repairs_marker_last_partial_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("operator.toml"),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };
        let plan = plan_run_profile(&manifest_path, &operator).expect("plan profile");
        let profile_path = plan.profile_path.clone();
        fs::create_dir_all(profile_path.parent().expect("profile parent"))
            .expect("create profile parent");
        fs::write(&profile_path, plan.normalized_toml.as_bytes()).expect("seed exact profile");

        let admitted = ensure_run_profile_plan(plan, "2026-07-13T12:00:00+00:00")
            .expect("repair missing marker");

        assert_eq!(admitted.commitment.admitted_at, "2026-07-13T12:00:00+00:00");
        assert_eq!(
            load_admitted_run_profile(&manifest_path)
                .expect("load profile")
                .expect("profile exists")
                .commitment,
            admitted.commitment
        );
    }

    #[test]
    fn setup_profile_reconciliation_rejects_commitment_first_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("operator.toml"),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };
        let plan = plan_run_profile(&manifest_path, &operator).expect("plan profile");
        let commitment = RunProfileCommitment {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: plan.profile_path.clone(),
            sha256: plan.sha256.clone(),
            source_path: Some(plan.source_path.clone()),
            admitted_at: "2026-07-13T12:00:00+00:00".to_string(),
        };
        fs::create_dir_all(
            commitment_path(&manifest_path)
                .parent()
                .expect("commitment parent"),
        )
        .expect("create commitment parent");
        fs::write(
            commitment_path(&manifest_path),
            serde_json::to_vec_pretty(&commitment).expect("serialize commitment"),
        )
        .expect("seed commitment");

        let error = ensure_run_profile_plan(plan, "2026-07-13T12:00:00+00:00")
            .expect_err("commitment-first state is unsafe");

        assert!(error.to_string().contains("without profile"));
        assert!(!run_profile_path(&manifest_path).exists());
    }

    #[test]
    fn admission_rejects_inconsistent_profile_plan_before_writes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign/campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("operator.toml"),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };
        let mut plan = plan_run_profile(&manifest_path, &operator).expect("plan profile");
        plan.normalized_toml.push_str("\n# drift\n");

        let error = admit_run_profile_plan(plan).expect_err("inconsistent plan must fail");

        assert!(error.to_string().contains("plan digest mismatch"));
        assert!(!tmp.path().join("campaign").exists());
    }

    #[test]
    fn admitted_run_profile_carries_digest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("profiles").join("overnight.toml"),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };

        let admitted = admit_run_profile(&manifest_path, &operator).expect("admit profile");
        let loaded = load_admitted_run_profile(&manifest_path)
            .expect("load admitted profile")
            .expect("profile exists");

        assert_eq!(loaded.profile, admitted.profile);
        assert_eq!(loaded.commitment.sha256, admitted.commitment.sha256);
        assert_eq!(
            loaded.commitment.profile_path,
            tmp.path().join("prototype1").join(RUN_PROFILE_FILE)
        );
        let admitted_text =
            fs::read_to_string(tmp.path().join("prototype1").join(RUN_PROFILE_FILE))
                .expect("read admitted profile");
        assert!(admitted_text.contains("[control]"));
        assert!(admitted_text.contains("mode = \"continuous\""));
    }

    #[test]
    fn absent_profile_pair_returns_none() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");

        assert!(
            load_admitted_run_profile(&manifest_path)
                .expect("absent pair is readable")
                .is_none()
        );
    }

    #[test]
    fn partial_profile_pairs_are_rejected() {
        let profile_only = tempfile::tempdir().expect("profile tempdir");
        let manifest_path = profile_only.path().join("campaign.json");
        let profile_path = run_profile_path(&manifest_path);
        fs::create_dir_all(profile_path.parent().expect("profile parent"))
            .expect("create profile parent");
        fs::write(&profile_path, PROFILE).expect("write profile only");
        let error = load_admitted_run_profile(&manifest_path)
            .expect_err("profile without commitment must fail");
        assert!(error.to_string().contains("has no admission commitment"));

        let commitment_only = tempfile::tempdir().expect("commitment tempdir");
        let manifest_path = commitment_only.path().join("campaign.json");
        let profile_path = run_profile_path(&manifest_path);
        fs::create_dir_all(profile_path.parent().expect("commitment parent"))
            .expect("create commitment parent");
        let commitment = RunProfileCommitment {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: profile_path.clone(),
            sha256: sha256_hex(PROFILE),
            source_path: None,
            admitted_at: Utc::now().to_rfc3339(),
        };
        let bytes = serde_json::to_vec_pretty(&commitment).expect("serialize commitment");
        fs::write(commitment_path(&manifest_path), bytes).expect("write commitment only");
        let error = load_admitted_run_profile(&manifest_path)
            .expect_err("commitment without profile must fail");
        assert!(error.to_string().contains("commitment exists"));
        assert!(error.to_string().contains("is missing"));
    }

    #[test]
    fn commitment_schema_path_and_digest_are_authoritative() {
        for corruption in ["schema", "path", "digest"] {
            let tmp = tempfile::tempdir().expect("tempdir");
            let manifest_path = tmp.path().join("campaign.json");
            let operator = OperatorRunProfile {
                source_path: tmp.path().join("operator.toml"),
                profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
            };
            admit_run_profile(&manifest_path, &operator).expect("admit profile");
            let path = commitment_path(&manifest_path);
            let mut commitment = load_commitment(&manifest_path)
                .expect("load commitment")
                .expect("commitment exists");
            match corruption {
                "schema" => commitment.schema_version = "unsupported.v0".to_string(),
                "path" => commitment.profile_path = tmp.path().join("other-profile.toml"),
                "digest" => commitment.sha256 = "0".repeat(64),
                _ => unreachable!(),
            }
            fs::write(
                &path,
                serde_json::to_vec_pretty(&commitment).expect("serialize corruption"),
            )
            .expect("write corruption");

            let error = load_admitted_run_profile(&manifest_path)
                .expect_err("corrupt commitment must fail");
            assert!(
                error.to_string().contains(corruption)
                    || (corruption == "schema" && error.to_string().contains("unsupported schema"))
            );
        }
    }

    #[test]
    fn second_profile_admission_preserves_existing_pair() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("operator.toml"),
            profile: parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses"),
        };
        admit_run_profile(&manifest_path, &operator).expect("first admission");
        let profile_path = run_profile_path(&manifest_path);
        let stored_path = commitment_path(&manifest_path);
        let profile_before = fs::read(&profile_path).expect("read profile");
        let commitment_before = fs::read(&stored_path).expect("read commitment");

        let error = admit_run_profile(&manifest_path, &operator)
            .expect_err("second admission must not overwrite authority");
        assert!(
            error
                .to_string()
                .contains("requires new profile and commitment paths")
        );
        assert_eq!(
            fs::read(profile_path).expect("reread profile"),
            profile_before
        );
        assert_eq!(
            fs::read(stored_path).expect("reread commitment"),
            commitment_before
        );
    }

    #[test]
    fn run_profile_defaults_oracle_policy_to_record_only() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace(
                "\n[selection.oracle]\nmode = \"record-only\"\nrequire_evidence = true\n",
                "\n",
            ),
        )
        .expect("profile parses");

        assert_eq!(profile.selection.oracle_mode(), OracleMode::RecordOnly);
        assert!(profile.selection.oracle_require_evidence());
    }

    #[test]
    fn run_profile_protocol_defaults_max_tokens_to_campaign_default() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("max_tokens = 4096\n", ""),
        )
        .expect("profile parses");

        assert_eq!(
            profile.protocol_policy().max_tokens,
            default_protocol_max_tokens()
        );
        assert_eq!(profile.protocol_policy().tool_review_parallelism, 2);
        assert_eq!(
            profile.protocol_policy().reasoning,
            ProtocolReasoningPolicy::omit()
        );
    }

    #[test]
    fn run_profile_protocol_model_can_override_to_direct_google() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace(
                "tool_review_parallelism = 2\n\n[protocol.reasoning]",
                "tool_review_parallelism = 2\n\n[protocol.model]\nid = \"google/gemini-2.5-flash\"\nroute_source = \"direct-google\"\nprovider = \"google\"\n\n[protocol.reasoning]",
            ),
        )
        .expect("profile parses");
        let policy = profile.protocol_policy();

        assert_eq!(policy.model_id.as_deref(), Some("google/gemini-2.5-flash"));
        assert_eq!(policy.route_source, Some(ModelRouteSource::DirectGoogle));
        assert_eq!(policy.provider_slug.as_deref(), Some("google"));
        assert_eq!(policy.max_tokens, 4096);
    }

    #[test]
    fn run_profile_protocol_model_still_accepts_openrouter_route() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace(
                "tool_review_parallelism = 2\n\n[protocol.reasoning]",
                "tool_review_parallelism = 2\n\n[protocol.model]\nid = \"google/gemini-2.5-flash\"\nroute_source = \"openrouter\"\nprovider = \"google\"\n\n[protocol.reasoning]",
            ),
        )
        .expect("profile parses");
        let policy = profile.protocol_policy();

        assert_eq!(policy.model_id.as_deref(), Some("google/gemini-2.5-flash"));
        assert_eq!(policy.route_source, Some(ModelRouteSource::OpenRouter));
        assert_eq!(policy.provider_slug.as_deref(), Some("google"));
    }

    #[test]
    fn run_profile_protocol_rejects_zero_max_tokens() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("max_tokens = 4096", "max_tokens = 0"),
        )
        .expect_err("zero protocol token budget should reject");

        assert!(err.to_string().contains("protocol.max_tokens"));
    }

    #[test]
    fn run_profile_protocol_rejects_zero_tool_review_parallelism() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("tool_review_parallelism = 2", "tool_review_parallelism = 0"),
        )
        .expect_err("zero protocol tool review parallelism should reject");

        assert!(err.to_string().contains("protocol.tool_review_parallelism"));
    }

    #[test]
    fn run_profile_protocol_accepts_reasoning_effort_policy() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE
                .replace("mode = \"omit\"", "mode = \"effort\"")
                .replace(
                    "[protocol.reasoning]\nmode = \"effort\"",
                    "[protocol.reasoning]\nmode = \"effort\"\neffort = \"low\"",
                ),
        )
        .expect("profile parses");

        assert_eq!(
            profile.protocol_policy().reasoning,
            ProtocolReasoningPolicy::effort(ploke_llm::ReasoningEffort::Low)
        );
    }

    #[test]
    fn run_profile_protocol_rejects_reasoning_effort_without_effort() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("mode = \"omit\"", "mode = \"effort\""),
        )
        .expect_err("effort mode requires an effort value");

        assert!(err.to_string().contains("protocol.reasoning.effort"));
    }

    #[test]
    fn run_profile_metrics_config_parses_imp_at_k_policy() {
        let text = PROFILE
            .replace(
                "score_points_per_imp_point = 0",
                "score_points_per_imp_point = 7",
            )
            .replace("require_for_score = false", "require_for_score = true");
        let profile = parse_profile(Path::new("profile.toml"), &text).expect("profile parses");
        let policy = profile.selection.metrics_policy();

        assert!(policy.persist);
        assert_eq!(
            policy.score_profile,
            selection_metrics::ScoreProfile::OperationalQualityV1
        );
        assert!(policy.imp_at_k.enabled);
        assert_eq!(policy.imp_at_k.budget_k, 50);
        assert_eq!(policy.imp_at_k.score_points_per_imp_point, 7);
        assert!(policy.imp_at_k.require_for_score);
    }

    #[test]
    fn run_profile_metrics_config_rejects_score_without_persistence() {
        let text = PROFILE
            .replace("persist = true", "persist = false")
            .replace(
                "score_points_per_imp_point = 0",
                "score_points_per_imp_point = 1",
            );
        let err = parse_profile(Path::new("profile.toml"), &text)
            .expect_err("non-persisted metrics cannot drive scoring");

        assert!(err.to_string().contains("persist = false"));
    }

    #[test]
    fn relative_oracle_selection_requires_enabled_mbe_and_targets() {
        let disabled_mbe = PROFILE
            .replace("mode = \"record-only\"", "mode = \"relative-score\"")
            .replace("enabled = true", "enabled = false");
        let err = parse_profile(Path::new("profile.toml"), &disabled_mbe)
            .expect_err("relative oracle scoring requires MBE");
        assert!(err.to_string().contains("execution.mbe.enabled"));

        let empty_targets = PROFILE
            .replace("mode = \"record-only\"", "mode = \"relative-score\"")
            .replace("instance = \"BurntSushi__ripgrep-2209\"\n", "")
            .replace("instances = [\"BurntSushi__ripgrep-2209\"]\n", "");
        let err = parse_profile(Path::new("profile.toml"), &empty_targets)
            .expect_err("relative oracle scoring requires target set");
        assert!(err.to_string().contains("target.instance"));
    }

    #[test]
    fn relative_oracle_selection_can_skip_missing_evidence_requirement() {
        let relaxed = PROFILE
            .replace("mode = \"record-only\"", "mode = \"relative-score\"")
            .replace("require_evidence = true", "require_evidence = false")
            .replace("enabled = true", "enabled = false")
            .replace("instance = \"BurntSushi__ripgrep-2209\"\n", "")
            .replace("instances = [\"BurntSushi__ripgrep-2209\"]\n", "");
        let profile = parse_profile(Path::new("profile.toml"), &relaxed)
            .expect("relaxed relative oracle profile parses");

        assert_eq!(profile.selection.oracle_mode(), OracleMode::RelativeScore);
        assert!(!profile.selection.oracle_require_evidence());
    }

    #[test]
    fn all_resolved_oracle_gate_requires_complete_mbe_evidence() {
        let gated = PROFILE.replace(
            "require_evidence = true",
            "require_evidence = true\ngate = \"all-resolved\"",
        );
        let profile =
            parse_profile(Path::new("profile.toml"), &gated).expect("strict oracle gate parses");
        assert_eq!(profile.selection.oracle_gate(), OracleGate::AllResolved);

        let disabled_mbe = gated.replace("enabled = true", "enabled = false");
        let err = parse_profile(Path::new("profile.toml"), &disabled_mbe)
            .expect_err("strict oracle gate requires MBE");
        assert!(err.to_string().contains("execution.mbe.enabled"));

        let missing_evidence = gated.replace("require_evidence = true", "require_evidence = false");
        let err = parse_profile(Path::new("profile.toml"), &missing_evidence)
            .expect_err("strict oracle gate requires complete evidence");
        assert!(err.to_string().contains("require_evidence = true"));

        let empty_targets = gated
            .replace("instance = \"BurntSushi__ripgrep-2209\"\n", "")
            .replace("instances = [\"BurntSushi__ripgrep-2209\"]\n", "");
        let err = parse_profile(Path::new("profile.toml"), &empty_targets)
            .expect_err("strict oracle gate requires target set");
        assert!(err.to_string().contains("target.instance"));
    }

    #[test]
    fn enabled_mbe_requires_target_set_for_oracle_recording() {
        let empty_targets = PROFILE
            .replace("instance = \"BurntSushi__ripgrep-2209\"\n", "")
            .replace("instances = [\"BurntSushi__ripgrep-2209\"]\n", "");
        let err = parse_profile(Path::new("profile.toml"), &empty_targets)
            .expect_err("enabled MBE requires target set");
        assert!(err.to_string().contains("execution.mbe.enabled"));
    }

    #[test]
    fn target_instances_drive_eval_cohort() {
        let profile = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace(
                "instances = [\"BurntSushi__ripgrep-2209\"]",
                "instances = [\"BurntSushi__ripgrep-2209\", \"BurntSushi__ripgrep-454\"]",
            ),
        )
        .expect("profile parses");

        assert_eq!(
            profile.target.eval_instances(),
            vec![
                "BurntSushi__ripgrep-2209".to_string(),
                "BurntSushi__ripgrep-454".to_string()
            ]
        );
        assert_eq!(
            profile.target.primary_instance(),
            Some("BurntSushi__ripgrep-2209")
        );
    }

    #[test]
    fn target_primary_instance_must_belong_to_instances() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace(
                "instances = [\"BurntSushi__ripgrep-2209\"]",
                "instances = [\"BurntSushi__ripgrep-454\"]",
            ),
        )
        .expect_err("mismatched primary instance should reject");

        assert!(
            err.to_string()
                .contains("target.instance 'BurntSushi__ripgrep-2209' must be included")
        );
    }

    #[test]
    fn legacy_generation_rejects_multi_instance_target_cohort() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE
                .replace("source = \"broad-harness-request\"", "source = \"legacy\"")
                .replace(
                    "instances = [\"BurntSushi__ripgrep-2209\"]",
                    "instances = [\"BurntSushi__ripgrep-2209\", \"BurntSushi__ripgrep-454\"]",
                ),
        )
        .expect_err("legacy generation should reject multi-instance cohort");

        assert!(
            err.to_string()
                .contains("legacy generation currently requires exactly one target instance")
        );
    }

    #[test]
    fn enabled_mbe_requires_python_and_workers() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("python3", "   "),
        )
        .expect_err("blank python must reject");
        assert!(err.to_string().contains("execution.mbe.python"));

        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("workers = 2", "workers = 0"),
        )
        .expect_err("zero workers must reject");
        assert!(err.to_string().contains("execution.mbe.workers"));
    }
}
