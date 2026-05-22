use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    campaign::{ProtocolCampaignPolicy, default_protocol_max_tokens},
    cli::{
        Prototype1CandidateGenerator, Prototype1StateStopAfter, Prototype1SuccessorSelection,
        Prototype1TraversalMetrics,
    },
    intervention::{Prototype1ChildBudget, Prototype1ChildScheduleMode, Prototype1SearchPolicy},
    layout::ploke_eval_home,
    spec::PrepareError,
    successor_selection::{OracleMode, metrics as selection_metrics},
};

pub(crate) const RUN_PROFILE_SCHEMA_VERSION: &str = "prototype1-run-profile.v1";
pub(crate) const RUN_PROFILE_COMMITMENT_SCHEMA_VERSION: &str =
    "prototype1-run-profile-commitment.v1";

const RUN_PROFILE_FILE: &str = "run-profile.toml";
const RUN_PROFILE_COMMITMENT_FILE: &str = "run-profile.commitment.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Prototype1RunProfile {
    pub(crate) schema_version: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) storage: Storage,
    #[serde(default)]
    pub(crate) target: Target,
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
    pub(crate) fn validate(&self) -> Result<(), PrepareError> {
        if self.schema_version != RUN_PROFILE_SCHEMA_VERSION {
            return Err(profile_error(format!(
                "unsupported Prototype 1 run profile schema '{}', expected '{}'",
                self.schema_version, RUN_PROFILE_SCHEMA_VERSION
            )));
        }
        self.target.validate()?;
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

    pub(crate) fn default_parallel_cap(&self) -> u32 {
        self.search.default_parallel_cap()
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
            max_tokens: self.protocol.max_tokens,
            ..ProtocolCampaignPolicy::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Storage {
    pub(crate) worktree_root: PathBuf,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            worktree_root: PathBuf::from("~/.ploke-eval/worktrees"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
        Ok(())
    }

    fn default_parallel_cap(&self) -> u32 {
        match self.schedule {
            Prototype1ChildScheduleMode::FullBatch => self.children.max,
            Prototype1ChildScheduleMode::AdaptiveBatch => self.children.min,
        }
        .max(1)
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
pub(crate) struct Generation {
    pub(crate) source: GenerationSource,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Protocol {
    #[serde(default = "default_protocol_max_tokens")]
    pub(crate) max_tokens: u32,
}

impl Protocol {
    fn validate(self) -> Result<(), PrepareError> {
        if self.max_tokens == 0 {
            return Err(profile_error(
                "protocol.max_tokens must be greater than zero",
            ));
        }
        Ok(())
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Self {
            max_tokens: default_protocol_max_tokens(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Oracle {
    #[serde(default)]
    pub(crate) mode: OracleMode,
    #[serde(default = "default_oracle_require_evidence")]
    pub(crate) require_evidence: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Execution {
    pub(crate) stop_after: ExecutionStopAfter,
    #[serde(default)]
    pub(crate) trace_jsonl: TraceJsonl,
    #[serde(default)]
    pub(crate) debug_tools: bool,
    #[serde(default)]
    pub(crate) mbe: Mbe,
}

impl Execution {
    fn validate(&self) -> Result<(), PrepareError> {
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
pub(crate) struct Control {
    #[serde(default)]
    pub(crate) mode: RunMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parallel_cap: Option<u32>,
}

impl Control {
    fn validate(self, search: &Search) -> Result<(), PrepareError> {
        let Some(parallel_cap) = self.parallel_cap else {
            return Ok(());
        };
        let derived_parallel_cap = search.default_parallel_cap();
        if parallel_cap == 0 || parallel_cap > derived_parallel_cap {
            return Err(profile_error(format!(
                "profile control.parallel_cap {} widens admitted fanout {}",
                parallel_cap, derived_parallel_cap
            )));
        }
        Ok(())
    }
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
    let profile = parse_profile(&source_path, &text)?;
    Ok(OperatorRunProfile {
        source_path,
        profile,
    })
}

pub(crate) fn admit_run_profile(
    campaign_manifest_path: &Path,
    operator: &OperatorRunProfile,
) -> Result<AdmittedRunProfile, PrepareError> {
    let profile_path = run_profile_path(campaign_manifest_path);
    let text =
        toml::to_string_pretty(&operator.profile).map_err(|err| profile_error(err.to_string()))?;
    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&profile_path, text.as_bytes()).map_err(|source| PrepareError::WriteManifest {
        path: profile_path.clone(),
        source,
    })?;
    let commitment = RunProfileCommitment {
        schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
        profile_path: profile_path.clone(),
        sha256: sha256_hex(&text),
        source_path: Some(operator.source_path.clone()),
        admitted_at: Utc::now().to_rfc3339(),
    };
    write_commitment(campaign_manifest_path, &commitment)?;
    Ok(AdmittedRunProfile {
        commitment,
        profile: operator.profile.clone(),
    })
}

pub(crate) fn load_admitted_run_profile(
    campaign_manifest_path: &Path,
) -> Result<Option<AdmittedRunProfile>, PrepareError> {
    let profile_path = run_profile_path(campaign_manifest_path);
    let text = match fs::read_to_string(&profile_path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: profile_path,
                source,
            });
        }
    };
    let profile = parse_profile(&profile_path, &text)?;
    let commitment = match load_commitment(campaign_manifest_path)? {
        Some(commitment) => commitment,
        None => RunProfileCommitment {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: profile_path.clone(),
            sha256: sha256_hex(&text),
            source_path: None,
            admitted_at: String::new(),
        },
    };
    if commitment.sha256 != sha256_hex(&text) {
        return Err(profile_error(format!(
            "campaign run profile digest mismatch for '{}'",
            profile_path.display()
        )));
    }
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

fn write_commitment(
    campaign_manifest_path: &Path,
    commitment: &RunProfileCommitment,
) -> Result<(), PrepareError> {
    let path = commitment_path(campaign_manifest_path);
    let bytes = serde_json::to_vec_pretty(commitment).map_err(PrepareError::Serialize)?;
    fs::write(&path, bytes).map_err(|source| PrepareError::WriteManifest { path, source })
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
max_tokens = 4000

[execution]
stop_after = "complete"
trace_jsonl = "auto"
debug_tools = true
mbe = { enabled = true, python = "python3", workers = 2 }
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
        assert_eq!(profile.protocol_policy().max_tokens, 4000);
        assert_eq!(
            profile.search_policy().child_budget,
            Prototype1ChildBudget { min: 6, max: 6 }
        );
        assert_eq!(
            profile.execution.state_stop_after(),
            Prototype1StateStopAfter::Complete
        );
        assert!(profile.execution.mbe.enabled);
        assert_eq!(profile.execution.mbe.python, "python3");
        assert_eq!(profile.execution.mbe.workers, 2);
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
            &PROFILE.replace("\n[protocol]\nmax_tokens = 4000\n", "\n"),
        )
        .expect("profile parses");

        assert_eq!(
            profile.protocol_policy().max_tokens,
            default_protocol_max_tokens()
        );
    }

    #[test]
    fn run_profile_protocol_rejects_zero_max_tokens() {
        let err = parse_profile(
            Path::new("profile.toml"),
            &PROFILE.replace("max_tokens = 4000", "max_tokens = 0"),
        )
        .expect_err("zero protocol token budget should reject");

        assert!(err.to_string().contains("protocol.max_tokens"));
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
