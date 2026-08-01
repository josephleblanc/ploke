//! Canonical in-process service for planning and admitting a fresh loop run.
//!
//! This facade is shared by operator clients. It accepts typed setup inputs,
//! delegates command construction and policy defaults to the canonical CLI
//! owner, and projects the real hash-bound setup plan. It does not create
//! worktrees, contact providers, or grant controller authority.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use ploke_llm::request::models::ModelRouteSource;
pub use ploke_records::ids::{CampaignId, GitCommit};
pub use ploke_records::{
    run_profile::{
        AntiAttractorPolicy, ArchiveScope, BroadTui, Control, EvalStorage, EvalStorageBackend,
        Execution, ExecutionStopAfter, Generation, GenerationSource, GenerationSurface, ImpAtK,
        Mbe, Metrics, ModelDefaults, ModelRouteSource as ProfileRouteSource, Oracle, OracleGate,
        OracleMode, Patch, PatchGate, Protocol, ProtocolReasoning, ProtocolReasoningEffort,
        ProtocolReasoningMode, RunMode, RunProfileRecord, ScoreProfile, Search, Selection,
        SelectionEvidence, SelectionStrategy, Storage, Target, TraceJsonl,
    },
    scheduler::{ChildBudgetRecord, ChildScheduleModeRecord},
};

pub use crate::campaign::EmbeddingRoute;
use crate::{
    CampaignManifest, PreparedMsbBatch, ResolvedCampaignConfig,
    cli::{self, prototype1_state},
    spec::PrepareError,
    walk_client::{ContentHash, EffectiveControl, WalkConfigSnapshot},
};

/// Existing prepared batch selected for one fresh setup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RunSetupBatch {
    Manifest(PathBuf),
    Id(String),
}

/// Operator run profile selected by registered name or exact TOML path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RunSetupProfile {
    Name(String),
    Path(PathBuf),
}

/// Optional eval-model overrides accepted by canonical setup.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupModel {
    pub id: Option<String>,
    pub provider: Option<String>,
    pub route: Option<ModelRouteSource>,
    pub max_tokens: Option<u32>,
    pub use_default: bool,
}

/// Optional protocol-model overrides accepted by canonical setup.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupProtocol {
    pub id: Option<String>,
    pub provider: Option<String>,
    pub route: Option<ModelRouteSource>,
}

/// Optional embedding overrides accepted by canonical setup.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupEmbedding {
    pub id: Option<String>,
    pub provider: Option<String>,
    pub route: Option<EmbeddingRoute>,
}

/// Exact operator inputs for a fresh Parent(0) setup.
///
/// Search, generation, selection, execution, storage, and control policy stay
/// in the selected run profile. This request intentionally has no local copy
/// of those defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupRequest {
    pub repo_root: PathBuf,
    pub batch: RunSetupBatch,
    pub campaign: CampaignId,
    pub profile: RunSetupProfile,
    pub primary_instance: Option<String>,
    #[serde(default)]
    pub model: RunSetupModel,
    #[serde(default)]
    pub protocol: RunSetupProtocol,
    #[serde(default)]
    pub embedding: RunSetupEmbedding,
}

/// Git base bound into the reviewed setup digest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupCheckout {
    pub branch: String,
    pub head: GitCommit,
}

/// Existing prepared batch and exact cohort selected by setup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupBatchPreview {
    pub manifest_path: PathBuf,
    pub batch: PreparedMsbBatch,
    pub primary_instance: String,
}

/// Canonical campaign manifest and resolved configuration before admission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSetupCampaign {
    pub manifest_path: PathBuf,
    pub manifest_hash: ContentHash,
    pub slice_path: PathBuf,
    pub slice_hash: ContentHash,
    pub manifest: CampaignManifest,
    pub resolved: ResolvedCampaignConfig,
}

/// Runtime-validated profile projected into its passive shared carrier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunSetupProfilePreview {
    pub source_path: PathBuf,
    pub profile_path: PathBuf,
    pub profile_hash: ContentHash,
    pub record: RunProfileRecord,
}

/// Read-only projection of the canonical setup plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSetupPreview {
    pub schema_version: String,
    pub plan_hash: ContentHash,
    pub checkout: RunSetupCheckout,
    pub artifact_branch: String,
    pub batch: RunSetupBatchPreview,
    pub campaign: RunSetupCampaign,
    pub profile: RunSetupProfilePreview,
    pub control: EffectiveControl,
}

/// Completed, read-back-validated setup admission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSetupReceipt {
    pub plan_hash: ContentHash,
    pub repo_root: PathBuf,
    pub config: WalkConfigSnapshot,
}

/// One operator profile loaded through the production runtime parser.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileSource {
    pub path: PathBuf,
    pub record: RunProfileRecord,
}

/// Normalized, runtime-validated profile bytes and effective control values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfilePreview {
    pub path: PathBuf,
    pub content_hash: ContentHash,
    pub record: RunProfileRecord,
    pub control: EffectiveControl,
}

/// Read-back-validated receipt for one create-only profile save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileReceipt {
    pub saved: ProfilePreview,
}

/// Build a non-writing setup preview through the canonical setup planner.
pub fn preview_run_setup(request: &RunSetupRequest) -> Result<RunSetupPreview, PrepareError> {
    let request = canonical_request(request)?;
    let command = cli::setup_command(&request)?;
    prototype1_state::cli_facing::project_setup(&command, request.repo_root)
}

/// Admit exactly the reviewed setup plan, failing closed on any input drift.
pub fn admit_run_setup(
    request: &RunSetupRequest,
    expected: &ContentHash,
) -> Result<RunSetupReceipt, PrepareError> {
    let request = canonical_request(request)?;
    let command = cli::setup_command(&request)?;
    prototype1_state::cli_facing::admit_setup_at(
        &command,
        expected.as_str(),
        request.repo_root.clone(),
    )?;
    let config = prototype1_state::walk::config::load(&request.repo_root)?;
    if config.campaign.admission.plan_hash != *expected {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "completed setup plan hash '{}' does not match reviewed hash '{}'",
                config.campaign.admission.plan_hash, expected
            ),
        });
    }
    Ok(RunSetupReceipt {
        plan_hash: expected.clone(),
        repo_root: request.repo_root,
        config,
    })
}

/// Build the current runtime defaults for a newly named operator profile.
pub fn default_run_profile(name: &str) -> Result<RunProfileRecord, PrepareError> {
    let mut seed = toml::Table::new();
    seed.insert(
        "schema_version".to_string(),
        toml::Value::String(ploke_records::run_profile::RUN_PROFILE_SCHEMA_VERSION.to_string()),
    );
    seed.insert("name".to_string(), toml::Value::String(name.to_string()));
    let text = toml::to_string(&seed).map_err(|source| {
        profile_service_error(format!("failed to encode run profile defaults: {source}"))
    })?;
    let path = PathBuf::from(format!("{name}.toml"));
    let runtime = prototype1_state::profile::parse_operator_profile(&path, &text)?;
    prototype1_state::walk::config::passive_profile_record(&runtime)
}

/// Load one selected profile through the production operator-profile loader.
pub fn load_run_profile(profile: &RunSetupProfile) -> Result<ProfileSource, PrepareError> {
    let selected = profile_selector(profile)?;
    let loaded = prototype1_state::profile::load_operator_profile(&selected)?;
    let record = prototype1_state::walk::config::passive_profile_record(&loaded.profile)?;
    Ok(ProfileSource {
        path: loaded.source_path,
        record,
    })
}

/// Validate and normalize a profile without writing the target path.
pub fn preview_run_profile(
    target: &RunSetupProfile,
    record: &RunProfileRecord,
) -> Result<ProfilePreview, PrepareError> {
    profile_plan(target, record).map(|(preview, _)| preview)
}

/// Create exactly the reviewed profile without replacing an existing target.
pub fn save_run_profile(
    target: &RunSetupProfile,
    record: &RunProfileRecord,
    expected: &ContentHash,
) -> Result<ProfileReceipt, PrepareError> {
    let (reviewed, normalized) = profile_plan(target, record)?;
    if reviewed.content_hash != *expected {
        return Err(profile_service_error(format!(
            "run profile changed after preview: reviewed hash '{}', current hash '{}'",
            expected, reviewed.content_hash
        )));
    }
    let created = crate::durable_io::create_atomic(&reviewed.path, normalized.as_bytes()).map_err(
        |source| PrepareError::WriteManifest {
            path: reviewed.path.clone(),
            source,
        },
    )?;
    if !created {
        return Err(profile_service_error(format!(
            "run profile '{}' already exists; profile saves are create-only",
            reviewed.path.display()
        )));
    }

    let loaded = load_run_profile(target)?;
    let saved = preview_run_profile(target, &loaded.record)?;
    if loaded.path != reviewed.path || saved != reviewed {
        return Err(profile_service_error(format!(
            "saved run profile '{}' did not match its reviewed preview",
            reviewed.path.display()
        )));
    }
    Ok(ProfileReceipt { saved })
}

fn profile_plan(
    target: &RunSetupProfile,
    record: &RunProfileRecord,
) -> Result<(ProfilePreview, String), PrepareError> {
    let selected = profile_selector(target)?;
    let path = prototype1_state::profile::resolve_operator_profile_path(&selected)?;
    let draft = toml::to_string(record).map_err(|source| {
        profile_service_error(format!("failed to encode run profile draft: {source}"))
    })?;
    let runtime = prototype1_state::profile::parse_operator_profile(&path, &draft)?;
    let normalized = toml::to_string_pretty(&runtime).map_err(|source| {
        profile_service_error(format!("failed to normalize run profile: {source}"))
    })?;
    let record = prototype1_state::walk::config::passive_profile_record(&runtime)?;
    let effective = prototype1_state::profile::resolve_effective_control(path.clone(), &runtime)?;
    let control = prototype1_state::walk::config::project_control(&record, effective);
    Ok((
        ProfilePreview {
            path,
            content_hash: ContentHash::of(&normalized),
            record,
            control,
        },
        normalized,
    ))
}

pub(crate) fn profile_selector(profile: &RunSetupProfile) -> Result<String, PrepareError> {
    match profile {
        RunSetupProfile::Name(name) => {
            let path = Path::new(name);
            let mut components = path.components();
            let one_name = matches!(components.next(), Some(Component::Normal(_)))
                && components.next().is_none();
            if !one_name || name.ends_with(".toml") {
                return Err(profile_service_error(format!(
                    "registered run profile name '{name}' must be one non-.toml path component"
                )));
            }
            Ok(name.clone())
        }
        RunSetupProfile::Path(path) => {
            if path.as_os_str().is_empty() {
                return Err(profile_service_error(
                    "explicit run profile path must not be empty",
                ));
            }
            let path = if path.is_absolute() {
                path.clone()
            } else {
                std::env::current_dir()
                    .map_err(|source| PrepareError::DatabaseSetup {
                        phase: "run_profile_current_dir",
                        detail: source.to_string(),
                    })?
                    .join(path)
            };
            path.to_str().map(str::to_string).ok_or_else(|| {
                profile_service_error(format!(
                    "run profile path '{}' is not valid UTF-8",
                    path.display()
                ))
            })
        }
    }
}

fn profile_service_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: detail.into(),
    }
}

fn canonical_request(request: &RunSetupRequest) -> Result<RunSetupRequest, PrepareError> {
    if !request.repo_root.exists() {
        return Err(PrepareError::MissingRepoRoot(request.repo_root.clone()));
    }
    if !request.repo_root.is_dir() {
        return Err(PrepareError::RepoRootNotDirectory(
            request.repo_root.clone(),
        ));
    }
    let repo_root =
        request
            .repo_root
            .canonicalize()
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_setup_canonicalize_root",
                detail: format!(
                    "failed to canonicalize setup repository '{}': {source}",
                    request.repo_root.display()
                ),
            })?;
    let mut request = request.clone();
    request.repo_root = repo_root;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{
        Cli, Command, LoopCommand, LoopSubcommand, Prototype1ChildScheduleMode,
        Prototype1LoopStopAfter,
    };

    #[test]
    fn service_defaults_match_clap_setup_defaults() {
        let request = RunSetupRequest {
            repo_root: PathBuf::from("/tmp/existing-worktree"),
            batch: RunSetupBatch::Id("prepared-batch".to_string()),
            campaign: CampaignId::from("setup-defaults"),
            profile: RunSetupProfile::Name("step-profile".to_string()),
            primary_instance: None,
            model: RunSetupModel::default(),
            protocol: RunSetupProtocol::default(),
            embedding: RunSetupEmbedding::default(),
        };
        let service = cli::setup_command(&request).expect("service command");
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-setup",
            "--batch-id",
            "prepared-batch",
            "--campaign",
            "setup-defaults",
            "--profile",
            "step-profile",
        ])
        .expect("canonical CLI defaults");
        let Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Setup(parsed),
        }) = parsed.command
        else {
            panic!("expected prototype1 setup command");
        };
        let parsed = parsed.input;

        assert_eq!(service.max_turns, parsed.max_turns);
        assert_eq!(service.max_tool_calls, parsed.max_tool_calls);
        assert_eq!(service.wall_clock_secs, parsed.wall_clock_secs);
        assert_eq!(service.eval_max_tokens, parsed.eval_max_tokens);
        assert_eq!(service.index_debug_snapshots, parsed.index_debug_snapshots);
        assert_eq!(service.max_generations, parsed.max_generations);
        assert_eq!(service.max_total_nodes, parsed.max_total_nodes);
        assert_eq!(service.min_children, parsed.min_children);
        assert_eq!(service.max_children, parsed.max_children);
        assert_eq!(
            service.child_schedule_mode,
            Prototype1ChildScheduleMode::FullBatch
        );
        assert_eq!(service.child_schedule_mode, parsed.child_schedule_mode);
        assert_eq!(
            service.require_keep_for_continuation,
            parsed.require_keep_for_continuation
        );
        assert_eq!(service.explore_from_rejected, parsed.explore_from_rejected);
        assert_eq!(
            service.stop_after,
            Prototype1LoopStopAfter::InterventionApply
        );
        assert_eq!(service.stop_after, parsed.stop_after);
        assert_eq!(service.format, parsed.format);
    }

    #[test]
    fn setup_request_canonicalizes_relative_repo_root() {
        let request = RunSetupRequest {
            repo_root: PathBuf::from("."),
            batch: RunSetupBatch::Id("prepared-batch".to_string()),
            campaign: CampaignId::from("setup-relative-root"),
            profile: RunSetupProfile::Name("step-profile".to_string()),
            primary_instance: None,
            model: RunSetupModel::default(),
            protocol: RunSetupProtocol::default(),
            embedding: RunSetupEmbedding::default(),
        };

        let normalized = canonical_request(&request).expect("canonical setup request");
        assert!(normalized.repo_root.is_absolute());
        assert_eq!(
            normalized.repo_root,
            std::env::current_dir()
                .expect("current directory")
                .canonicalize()
                .expect("canonical current directory")
        );
    }

    #[test]
    fn typed_profile_selector_preserves_name_and_path_semantics() {
        assert_eq!(
            profile_selector(&RunSetupProfile::Name("operator-profile".to_string()))
                .expect("registered name"),
            "operator-profile"
        );
        for name in ["operator.toml", "group/operator", "/tmp/operator"] {
            let error = profile_selector(&RunSetupProfile::Name(name.to_string()))
                .expect_err("path-shaped registered name must fail")
                .to_string();
            assert!(
                error.contains("one non-.toml path component"),
                "unexpected error for {name}: {error}"
            );
        }

        let relative = PathBuf::from("operator-profile");
        let selected = profile_selector(&RunSetupProfile::Path(relative.clone()))
            .expect("explicit relative path");
        assert_eq!(
            PathBuf::from(&selected),
            std::env::current_dir()
                .expect("current directory")
                .join(&relative)
        );

        let request = RunSetupRequest {
            repo_root: PathBuf::from("/tmp/existing-worktree"),
            batch: RunSetupBatch::Id("prepared-batch".to_string()),
            campaign: CampaignId::from("setup-profile-path"),
            profile: RunSetupProfile::Path(relative),
            primary_instance: None,
            model: RunSetupModel::default(),
            protocol: RunSetupProtocol::default(),
            embedding: RunSetupEmbedding::default(),
        };
        assert_eq!(
            cli::setup_command(&request)
                .expect("service command")
                .profile,
            Some(selected)
        );
    }

    #[test]
    fn profile_defaults_use_runtime_policy_and_derived_control() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = RunSetupProfile::Path(temp.path().join("defaults.toml"));
        let record = default_run_profile("runtime-defaults").expect("runtime defaults");

        assert_eq!(
            record.schema_version,
            ploke_records::run_profile::RUN_PROFILE_SCHEMA_VERSION
        );
        assert_eq!(record.name, "runtime-defaults");
        assert_eq!(record.search.max_generations, 1);
        assert_eq!(record.search.max_total_nodes, 32);
        assert_eq!(record.search.children.min, 2);
        assert_eq!(record.search.children.max, 6);
        assert_eq!(record.search.children.parallel_targets, None);
        assert_eq!(record.control.parallel_cap, None);

        let preview = preview_run_profile(&target, &record).expect("profile preview");
        assert_eq!(preview.control.parallel_cap.value, 3);
        assert_eq!(preview.control.patch_cap.value, 3);
        assert_eq!(
            preview.control.parallel_cap.source,
            crate::walk_client::ValueSource::Derived(
                crate::walk_client::DerivationRule::SearchFanout
            )
        );
        assert_eq!(
            preview.control.patch_cap.source,
            crate::walk_client::ValueSource::Derived(
                crate::walk_client::DerivationRule::DefaultPatchTargets
            )
        );
        assert!(!preview.path.exists(), "preview must not write the profile");
    }

    #[test]
    fn profile_edits_roundtrip_without_dropping_policy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = RunSetupProfile::Path(temp.path().join("edited.toml"));
        let mut record = default_run_profile("edited").expect("runtime defaults");
        record.storage.worktree_root = PathBuf::from("/operator/worktrees");
        record.search.max_generations = 7;
        record.selection.seed = 41;
        record.protocol.tool_review_parallelism = 5;
        record.execution.broad_tui.graph_nearest = Some(9);

        let preview = preview_run_profile(&target, &record).expect("edited profile preview");

        assert_eq!(preview.record, record);
        assert_eq!(preview.record.selection.seed, 41);
        assert_eq!(preview.record.protocol.tool_review_parallelism, 5);
        assert_eq!(preview.record.execution.broad_tui.graph_nearest, Some(9));
    }

    #[test]
    fn profile_preview_preserves_runtime_validation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = RunSetupProfile::Path(temp.path().join("invalid.toml"));
        let valid = default_run_profile("invalid").expect("runtime defaults");

        let mut reversed = valid.clone();
        reversed.search.children.min = reversed.search.children.max + 1;
        let error = preview_run_profile(&target, &reversed)
            .expect_err("minimum above maximum must fail")
            .to_string();
        assert!(
            error.contains("cannot exceed max"),
            "unexpected error: {error}"
        );

        let mut zero = valid.clone();
        zero.search.children.min = 0;
        let error = preview_run_profile(&target, &zero)
            .expect_err("zero child budget must fail")
            .to_string();
        assert!(
            error.contains("must be nonzero"),
            "unexpected error: {error}"
        );

        let mut widening = valid;
        widening.control.parallel_cap = Some(4);
        let error = preview_run_profile(&target, &widening)
            .expect_err("control fanout widening must fail")
            .to_string();
        assert!(
            error.contains("widens admitted fanout 3"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn profile_save_is_hash_bound_and_loadable_by_name() {
        let home = tempfile::tempdir().expect("eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            home.path().as_os_str().into(),
        )]);
        let selected = RunSetupProfile::Name("operator-profile".to_string());
        let mut record = default_run_profile("operator-profile").expect("runtime defaults");
        record.search.max_generations = 3;
        record.selection.seed = 29;
        record.execution.broad_tui.timeout_secs = Some(2_400);
        let preview = preview_run_profile(&selected, &record).expect("profile preview");
        let expected_path = home
            .path()
            .join("profiles/prototype1/operator-profile.toml");
        assert_eq!(preview.path, expected_path);
        assert!(!expected_path.exists());

        let receipt =
            save_run_profile(&selected, &record, &preview.content_hash).expect("hash-bound create");
        let loaded = load_run_profile(&selected).expect("production profile load");
        let bytes = std::fs::read_to_string(&expected_path).expect("saved profile bytes");

        assert_eq!(receipt.saved, preview);
        assert_eq!(loaded.path, expected_path);
        assert_eq!(loaded.record, record);
        assert_eq!(ContentHash::of(&bytes), preview.content_hash);
    }

    #[test]
    fn profile_save_rejects_stale_hash_without_writing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("stale.toml");
        let target = RunSetupProfile::Path(path.clone());
        let record = default_run_profile("stale").expect("runtime defaults");
        let preview = preview_run_profile(&target, &record).expect("profile preview");
        let mut changed = record;
        changed.selection.seed = 1;

        let error = save_run_profile(&target, &changed, &preview.content_hash)
            .expect_err("stale preview hash must fail")
            .to_string();

        assert!(error.contains("changed after preview"));
        assert!(!path.exists(), "stale profile must not be written");
    }

    #[test]
    fn profile_save_never_overwrites_existing_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("existing.toml");
        let target = RunSetupProfile::Path(path.clone());
        let record = default_run_profile("existing").expect("runtime defaults");
        let preview = preview_run_profile(&target, &record).expect("profile preview");
        std::fs::write(&path, "operator-owned bytes\n").expect("seed existing profile");

        let error = save_run_profile(&target, &record, &preview.content_hash)
            .expect_err("existing profile must not be overwritten")
            .to_string();

        assert!(error.contains("already exists"));
        assert_eq!(
            std::fs::read_to_string(path).expect("existing bytes"),
            "operator-owned bytes\n"
        );
    }
}
