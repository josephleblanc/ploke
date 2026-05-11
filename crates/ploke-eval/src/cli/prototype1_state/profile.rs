use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    cli::{
        Prototype1CandidateGenerator, Prototype1EditSurface, Prototype1StateStopAfter,
        Prototype1SuccessorSelection, Prototype1TraversalMetrics,
    },
    intervention::{Prototype1ChildBudget, Prototype1ChildScheduleMode, Prototype1SearchPolicy},
    layout::ploke_eval_home,
    spec::PrepareError,
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
    pub(crate) execution: Execution,
}

impl Prototype1RunProfile {
    pub(crate) fn validate(&self) -> Result<(), PrepareError> {
        if self.schema_version != RUN_PROFILE_SCHEMA_VERSION {
            return Err(profile_error(format!(
                "unsupported Prototype 1 run profile schema '{}', expected '{}'",
                self.schema_version, RUN_PROFILE_SCHEMA_VERSION
            )));
        }
        self.search.validate()?;
        self.generation.validate()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) surface: Option<GenerationSurface>,
}

impl Generation {
    fn validate(self) -> Result<(), PrepareError> {
        match (self.source, self.surface) {
            (GenerationSource::Legacy, None) => Ok(()),
            (GenerationSource::Legacy, Some(_)) => Err(profile_error(
                "profile generation.surface is only valid when generation.source = 'edit-surface'",
            )),
            (GenerationSource::EditSurface, Some(_)) => Ok(()),
            (GenerationSource::EditSurface, None) => Err(profile_error(
                "profile generation.source = 'edit-surface' requires generation.surface",
            )),
        }
    }

    pub(crate) fn candidate_generator(self) -> Prototype1CandidateGenerator {
        match self.source {
            GenerationSource::Legacy => Prototype1CandidateGenerator::Legacy,
            GenerationSource::EditSurface => Prototype1CandidateGenerator::TuiEditSurface,
        }
    }

    pub(crate) fn edit_surface(self) -> Prototype1EditSurface {
        match self
            .surface
            .unwrap_or(GenerationSurface::WorkspaceExceptPlokeEval)
        {
            GenerationSurface::PlokeTuiTools => Prototype1EditSurface::PlokeTuiTools,
            GenerationSurface::WorkspaceExceptPlokeEval => {
                Prototype1EditSurface::WorkspaceExceptPlokeEval
            }
        }
    }
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
pub(crate) enum GenerationSource {
    Legacy,
    EditSurface,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GenerationSurface {
    PlokeTuiTools,
    WorkspaceExceptPlokeEval,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) strategy: SelectionStrategy,
    pub(crate) evidence: SelectionEvidence,
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
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            strategy: SelectionStrategy::HistoryScoreChildProp,
            evidence: SelectionEvidence::Operational,
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
pub(crate) struct Execution {
    pub(crate) stop_after: ExecutionStopAfter,
    #[serde(default)]
    pub(crate) trace_jsonl: TraceJsonl,
    #[serde(default)]
    pub(crate) debug_tools: bool,
}

impl Execution {
    pub(crate) fn state_stop_after(self) -> Prototype1StateStopAfter {
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
        }
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
    pub(crate) text: String,
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
        text,
        profile,
    })
}

pub(crate) fn admit_run_profile(
    campaign_manifest_path: &Path,
    operator: &OperatorRunProfile,
) -> Result<AdmittedRunProfile, PrepareError> {
    let profile_path = run_profile_path(campaign_manifest_path);
    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&profile_path, operator.text.as_bytes()).map_err(|source| {
        PrepareError::WriteManifest {
            path: profile_path.clone(),
            source,
        }
    })?;
    let commitment = RunProfileCommitment {
        schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
        profile_path: profile_path.clone(),
        sha256: sha256_hex(&operator.text),
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

[execution]
stop_after = "complete"
trace_jsonl = "auto"
debug_tools = true
"#;

    #[test]
    fn run_profile_toml_preserves_runtime_shape() {
        let profile = parse_profile(Path::new("profile.toml"), PROFILE).expect("profile parses");

        assert_eq!(
            profile.generation.candidate_generator(),
            Prototype1CandidateGenerator::TuiEditSurface
        );
        assert_eq!(
            profile.selection.traversal_metrics(),
            Prototype1TraversalMetrics::OperationalAndProtocol
        );
        assert_eq!(
            profile.search_policy().child_budget,
            Prototype1ChildBudget { min: 6, max: 6 }
        );
        assert_eq!(
            profile.execution.state_stop_after(),
            Prototype1StateStopAfter::Complete
        );
    }

    #[test]
    fn admitted_run_profile_carries_digest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let operator = OperatorRunProfile {
            source_path: tmp.path().join("profiles").join("overnight.toml"),
            text: PROFILE.to_string(),
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
    }
}
