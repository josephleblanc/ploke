//! Read-only projection of the configuration admitted for one active checkout.
//!
//! The projection starts from the checkout-local parent identity, loads the
//! campaign through the canonical manifest loader, and obtains the run profile
//! only through admission validation. Its passive records are display/query
//! carriers; they do not grant setup, controller, or mutation authority.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use ploke_records::{
    identity::{PARENT_IDENTITY_RELPATH, PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentityRecord},
    ids::GitCommit,
    run_profile::{
        RUN_PROFILE_COMMITMENT_SCHEMA_VERSION, RunMode, RunProfileCommitmentRecord,
        RunProfileRecord,
    },
};
use serde::{Deserialize, Serialize};

pub use crate::cli::prototype1_state::setup_admission::SetupArtifactHashes;

use crate::{
    campaign::{self, CAMPAIGN_MANIFEST_SCHEMA_VERSION, CampaignManifest, ResolvedCampaignConfig},
    cli::prototype1_state::{
        event::ContentHash,
        identity,
        profile::{
            self, AdmittedRunProfile, EffectiveRunControl, Prototype1RunProfile,
            RunProfileCommitment,
        },
        setup_admission,
    },
    spec::PrepareError,
};

const ERROR_PHASE: &str = "prototype1_walk_config_projection";

/// Typed socket snapshot of configuration evidence for the current checkout.
///
/// Deserialization validates agreement among the passive records but does not
/// admit profile bytes or grant controller authority; authoritative snapshots
/// are originated by [`load`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "WalkConfigWire")]
pub struct WalkConfigSnapshot {
    pub identity: ConfigIdentity,
    pub campaign: CampaignConfig,
    pub profile: ProfileConfig,
    pub control: EffectiveControl,
}

/// Checkout-local identity coordinate used to select the campaign.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConfigIdentity {
    pub path: PathBuf,
    pub record: ParentIdentityRecord,
}

/// Exact manifest snapshot and its ordinary resolved campaign configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignConfig {
    pub path: PathBuf,
    pub content_hash: ContentHash,
    pub admission: ConfigAdmission,
    pub manifest: CampaignManifest,
    pub resolved: ResolvedCampaignConfig,
    /// Provider semantics frozen by the hash-bound admitted manifest.
    pub provider: ProviderSelection,
}

/// Stable provider decision for the admitted model route.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderSelection {
    /// Direct Google transport selects Google without an OpenRouter endpoint.
    DirectGoogle,
    /// The admitted manifest pins one OpenRouter endpoint provider.
    Pinned { slug: String },
    /// The admitted manifest delegates to the route's default endpoint choice.
    RouteDefault,
}

impl fmt::Display for ProviderSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectGoogle => formatter.write_str("direct_google"),
            Self::Pinned { slug } => write!(formatter, "pinned:{slug}"),
            Self::RouteDefault => formatter.write_str("route_default"),
        }
    }
}

/// Completed setup receipt that binds the displayed files to reviewed intent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConfigAdmission {
    pub path: PathBuf,
    pub plan_hash: ContentHash,
    pub manifest_path: PathBuf,
    pub setup_root: PathBuf,
    pub root_identity: ParentIdentityRecord,
    pub hashes: SetupArtifactHashes,
    pub started_at: String,
    pub completed_head: GitCommit,
}

/// Passive profile data derived only after admission validation succeeds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileConfig {
    pub record: RunProfileRecord,
    pub commitment: RunProfileCommitmentRecord,
    /// Operator-supplied source path reported by the commitment but not bound
    /// by the current setup receipt schema.
    pub reported_source: Option<PathBuf>,
}

/// Effective controller values plus their typed profile provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectiveControl {
    pub path: PathBuf,
    pub mode: RunMode,
    pub parallel_cap: SourcedValue<u32>,
    pub patch_cap: SourcedValue<u32>,
}

/// One effective value and the admitted input or derivation that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourcedValue<T> {
    pub value: T,
    pub source: ValueSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    content = "basis",
    rename_all = "snake_case"
)]
pub enum ValueSource {
    Explicit(ProfileField),
    Derived(DerivationRule),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProfileField {
    #[serde(rename = "control.parallel_cap")]
    ControlParallelCap,
    #[serde(rename = "search.children.parallel_targets")]
    SearchParallelTargets,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DerivationRule {
    SearchFanout,
    DefaultPatchTargets,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WalkConfigWire {
    identity: ConfigIdentity,
    campaign: CampaignConfig,
    profile: ProfileConfig,
    control: EffectiveControl,
}

impl TryFrom<WalkConfigWire> for WalkConfigSnapshot {
    type Error = String;

    fn try_from(wire: WalkConfigWire) -> Result<Self, Self::Error> {
        let snapshot = Self {
            identity: wire.identity,
            campaign: wire.campaign,
            profile: wire.profile,
            control: wire.control,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

impl WalkConfigSnapshot {
    fn validate(&self) -> Result<(), String> {
        let identity = &self.identity.record;
        let campaign = &self.campaign;
        let admission = &campaign.admission;
        let profile = &self.profile;

        if identity.schema_version != PARENT_IDENTITY_SCHEMA_VERSION {
            return Err(format!(
                "unsupported parent identity schema '{}'",
                identity.schema_version
            ));
        }
        if campaign.manifest.schema_version != CAMPAIGN_MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "unsupported campaign manifest schema '{}'",
                campaign.manifest.schema_version
            ));
        }
        if identity.campaign_id != campaign.manifest.campaign_id
            || identity.campaign_id != campaign.resolved.campaign_id
        {
            return Err("configuration campaign identifiers do not agree".to_string());
        }
        let expected_suffix = PathBuf::from("campaigns")
            .join(identity.campaign_id.as_str())
            .join("campaign.json");
        if !campaign.path.is_absolute() || !campaign.path.ends_with(&expected_suffix) {
            return Err("campaign path does not match its configured campaign id".to_string());
        }
        if campaign.path != admission.manifest_path {
            return Err("setup admission manifest path does not match campaign path".to_string());
        }
        if self.identity.path != admission.setup_root.join(PARENT_IDENTITY_RELPATH) {
            return Err("setup admission repository root does not match identity path".to_string());
        }
        if admission.root_identity.schema_version != PARENT_IDENTITY_SCHEMA_VERSION
            || admission.root_identity.campaign_id != identity.campaign_id
            || admission.root_identity.generation != 0
            || admission.root_identity.parent_id != admission.root_identity.node_id
            || admission.root_identity.previous_parent_id.is_some()
            || admission.root_identity.parent_node_id.is_some()
        {
            return Err(
                "setup admission root identity is not a matching generation-0 root".to_string(),
            );
        }
        if setup_admission::setup_admission_path(&campaign.path) != admission.path {
            return Err("setup admission receipt path does not match campaign path".to_string());
        }
        if !admission.setup_root.is_absolute() {
            return Err("setup admission repository root is not absolute".to_string());
        }
        for (label, hash) in [
            ("plan", &admission.plan_hash),
            ("manifest", &admission.hashes.manifest),
            ("slice", &admission.hashes.slice),
            ("profile", &admission.hashes.profile),
        ] {
            validate_hash(label, hash)?;
        }
        if admission.completed_head.as_str().trim().is_empty() {
            return Err("setup admission completion witness is empty".to_string());
        }
        chrono::DateTime::parse_from_rfc3339(&admission.started_at)
            .map_err(|error| format!("invalid setup admission time: {error}"))?;

        let normalized =
            serde_json::to_string_pretty(&campaign.manifest).map_err(|error| error.to_string())?;
        let manifest_hash = ContentHash::of(&normalized);
        if manifest_hash != campaign.content_hash || manifest_hash != admission.hashes.manifest {
            return Err(
                "canonical campaign manifest does not match its setup admission hash".to_string(),
            );
        }

        let expected = resolve_stable_manifest(campaign.manifest.clone())
            .map_err(|error| error.to_string())?;
        let expected = serde_json::to_value(expected).map_err(|error| error.to_string())?;
        let observed =
            serde_json::to_value(&campaign.resolved).map_err(|error| error.to_string())?;
        if observed != expected {
            return Err("resolved campaign configuration does not match its manifest".to_string());
        }
        if campaign.provider != project_provider(&campaign.resolved) {
            return Err("provider selection does not match the admitted campaign".to_string());
        }

        if profile.commitment.schema_version != RUN_PROFILE_COMMITMENT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported run profile commitment schema '{}'",
                profile.commitment.schema_version
            ));
        }
        if profile.commitment.admitted_at != admission.started_at {
            return Err("run profile admission time does not match the setup receipt".to_string());
        }
        if profile.commitment.source_path.is_some() {
            return Err(
                "receipt-unbound profile source must use reported_source instead".to_string(),
            );
        }
        let profile_path = campaign
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("prototype1/run-profile.toml");
        if profile.commitment.profile_path != profile_path || self.control.path != profile_path {
            return Err("run profile paths do not match the campaign-owned profile".to_string());
        }
        let encoded = toml::to_string(&profile.record).map_err(|error| error.to_string())?;
        let runtime: Prototype1RunProfile =
            toml::from_str(&encoded).map_err(|error| error.to_string())?;
        runtime.validate().map_err(|error| error.to_string())?;
        let normalized = toml::to_string_pretty(&runtime).map_err(|error| error.to_string())?;
        let profile_hash = ContentHash::of(&normalized);
        if profile_hash.0 != profile.commitment.sha256 || profile_hash != admission.hashes.profile {
            return Err(
                "canonical run profile does not match its setup admission hash".to_string(),
            );
        }
        let effective =
            profile::resolve_effective_control(profile.commitment.profile_path.clone(), &runtime)
                .map_err(|error| error.to_string())?;
        let expected = project_control(&profile.record, effective);
        if self.control != expected {
            return Err("effective control does not match the admitted run profile".to_string());
        }
        Ok(())
    }
}

/// Load one immutable projection rooted at the checkout's parent identity.
pub(crate) fn load(repo_root: &Path) -> Result<WalkConfigSnapshot, PrepareError> {
    let parent = identity::load_parent_identity(repo_root)?;
    parent.validate_for_command(parent.campaign_id(), None)?;

    let identity = ConfigIdentity {
        path: identity::parent_identity_path(repo_root),
        record: parent.record().clone(),
    };
    let campaign = load_campaign(repo_root, &identity, parent.campaign_id())?;
    let admitted = profile::load_admitted_run_profile(&campaign.path)?.ok_or_else(|| {
        projection_error(format!(
            "campaign '{}' has no admitted run profile at '{}'",
            parent.campaign_id(),
            campaign.path.display()
        ))
    })?;
    let effective = profile::resolve_effective_control(
        admitted.commitment.profile_path.clone(),
        &admitted.profile,
    )?;
    let record = passive_profile(&admitted)?;
    let reported_source = admitted.commitment.source_path.clone();
    let commitment = passive_commitment(&admitted.commitment);
    if commitment.sha256 != campaign.admission.hashes.profile.0 {
        return Err(projection_error(format!(
            "setup admission profile hash '{}' does not match commitment '{}'",
            campaign.admission.hashes.profile, commitment.sha256
        )));
    }
    let control = project_control(&record, effective);

    let snapshot = WalkConfigSnapshot {
        identity,
        campaign,
        profile: ProfileConfig {
            record,
            commitment,
            reported_source,
        },
        control,
    };
    snapshot.validate().map_err(projection_error)?;
    Ok(snapshot)
}

fn load_campaign(
    repo_root: &Path,
    identity: &ConfigIdentity,
    campaign_id: &ploke_records::ids::CampaignId,
) -> Result<CampaignConfig, PrepareError> {
    let path = campaign::campaign_manifest_path(campaign_id)?;
    let before = read_campaign(&path)?;
    let manifest = campaign::load_campaign_manifest(campaign_id)?;
    let after = read_campaign(&path)?;
    if before != after {
        return Err(projection_error(format!(
            "campaign manifest '{}' changed while its configuration snapshot was loaded",
            path.display()
        )));
    }
    let resolved = resolve_stable_manifest(manifest.clone())?;
    let provider = project_provider(&resolved);
    let content_hash = ContentHash::of(&before);
    let admission = load_admission(repo_root, identity, &path, &content_hash)?;

    Ok(CampaignConfig {
        path,
        content_hash,
        admission,
        manifest,
        resolved,
        provider,
    })
}

fn load_admission(
    repo_root: &Path,
    identity: &ConfigIdentity,
    manifest_path: &Path,
    content_hash: &ContentHash,
) -> Result<ConfigAdmission, PrepareError> {
    let path = setup_admission::setup_admission_path(manifest_path);
    let receipt = setup_admission::load_setup_admission(&path)?.ok_or_else(|| {
        projection_error(format!(
            "campaign '{}' has no setup admission receipt at '{}'",
            identity.record.campaign_id,
            path.display()
        ))
    })?;
    let head = receipt.completed_head().ok_or_else(|| {
        projection_error(format!(
            "setup admission receipt '{}' is not complete",
            path.display()
        ))
    })?;
    let intent = &receipt.intent;
    if intent.campaign_id != identity.record.campaign_id
        || intent.manifest_path != manifest_path
        || intent.repo_root != repo_root
    {
        return Err(projection_error(format!(
            "setup admission receipt '{}' does not match the active campaign coordinates",
            path.display()
        )));
    }
    if &intent.hashes.manifest != content_hash {
        return Err(projection_error(format!(
            "setup admission manifest hash '{}' does not match current campaign bytes '{}'",
            intent.hashes.manifest, content_hash
        )));
    }
    let started_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(intent.started_at.0)
        .map(|value| value.to_rfc3339())
        .ok_or_else(|| projection_error("setup admission timestamp is out of range"))?;
    Ok(ConfigAdmission {
        path,
        plan_hash: intent.plan_hash.clone(),
        manifest_path: intent.manifest_path.clone(),
        setup_root: intent.repo_root.clone(),
        root_identity: intent.identity.record().clone(),
        hashes: intent.hashes.clone(),
        started_at,
        completed_head: head.clone(),
    })
}

fn resolve_stable_manifest(
    manifest: CampaignManifest,
) -> Result<ResolvedCampaignConfig, PrepareError> {
    campaign::resolve_explicit_manifest(manifest)
}

fn validate_hash(label: &str, hash: &ContentHash) -> Result<(), String> {
    let text = hash.0.as_bytes();
    if text.len() == 64
        && text
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        Ok(())
    } else {
        Err(format!(
            "setup admission {label} hash is not canonical lowercase SHA-256"
        ))
    }
}

fn project_provider(config: &ResolvedCampaignConfig) -> ProviderSelection {
    if config.route_source.is_direct_google() {
        ProviderSelection::DirectGoogle
    } else if let Some(slug) = config.provider_slug.clone() {
        ProviderSelection::Pinned { slug }
    } else {
        ProviderSelection::RouteDefault
    }
}

fn read_campaign(path: &Path) -> Result<String, PrepareError> {
    fs::read_to_string(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PrepareError::MissingCampaignManifest(path.to_path_buf())
        } else {
            PrepareError::ReadCampaignManifest {
                path: path.to_path_buf(),
                source,
            }
        }
    })
}

/// Re-encode the already validated runtime profile into its shared passive
/// wire carrier. The source bytes are never parsed here; admission validation
/// has already checked their commitment, digest, schema, and runtime policy.
fn passive_profile(admitted: &AdmittedRunProfile) -> Result<RunProfileRecord, PrepareError> {
    let validated = toml::to_string(&admitted.profile).map_err(|source| {
        projection_error(format!(
            "validated run profile could not be serialized into its passive carrier: {source}"
        ))
    })?;
    toml::from_str(&validated).map_err(|source| {
        projection_error(format!(
            "validated run profile did not match the shared passive carrier: {source}"
        ))
    })
}

fn passive_commitment(commitment: &RunProfileCommitment) -> RunProfileCommitmentRecord {
    RunProfileCommitmentRecord {
        schema_version: commitment.schema_version.clone(),
        profile_path: commitment.profile_path.clone(),
        sha256: commitment.sha256.clone(),
        source_path: None,
        admitted_at: commitment.admitted_at.clone(),
    }
}

fn project_control(record: &RunProfileRecord, effective: EffectiveRunControl) -> EffectiveControl {
    let parallel_cap = SourcedValue {
        value: effective.parallel_cap,
        source: if effective.defaulted_from_profile {
            ValueSource::Derived(DerivationRule::SearchFanout)
        } else {
            ValueSource::Explicit(ProfileField::ControlParallelCap)
        },
    };
    let patch_cap = SourcedValue {
        value: effective.patch_generation_parallel_cap,
        source: if effective.patch_generation_defaulted_from_profile {
            ValueSource::Derived(DerivationRule::DefaultPatchTargets)
        } else {
            ValueSource::Explicit(ProfileField::SearchParallelTargets)
        },
    };

    EffectiveControl {
        path: effective.path,
        mode: record.control.mode,
        parallel_cap,
        patch_cap,
    }
}

fn projection_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: ERROR_PHASE,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs};

    use ploke_llm::request::models::ModelRouteSource;
    use ploke_records::{ids::CampaignId, run_profile::RunMode};

    use crate::{
        campaign::{CampaignManifest, save_campaign_manifest},
        cli::prototype1_state::{
            backend::GitCommit,
            event::RecordedAt,
            identity::{ParentIdentity, write_parent_identity},
            profile::{admit_run_profile, load_operator_profile},
            setup_admission::{
                SetupAdmissionIntent, SetupCheckoutBase, create_setup_admission,
                replace_setup_admission, setup_admission_path,
            },
        },
        intervention::{
            PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeRecord, Prototype1NodeStatus,
            Prototype1RunnerRequest,
        },
        target_registry::RegistryDatasetSource,
        test_support::{EnvGuard, env_guard_os},
    };

    use super::*;

    const DEFAULT_PROFILE: &str = r#"
schema_version = "prototype1-run-profile.v1"
name = "defaults"
"#;

    const EXPLICIT_PROFILE: &str = r#"
schema_version = "prototype1-run-profile.v1"
name = "explicit-control"

[search]
max_generations = 2
max_total_nodes = 12
children = { min = 2, max = 4, parallel_targets = 2 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = true
explore_from_rejected = true

[control]
mode = "step"
parallel_cap = 1
"#;

    /// Filesystem-only fixture that exercises the production identity,
    /// campaign, and run-profile admission formats without replacing them.
    struct Fixture {
        _env: EnvGuard,
        _temp: tempfile::TempDir,
        repo_root: PathBuf,
        manifest_path: PathBuf,
        campaign_id: CampaignId,
    }

    impl Fixture {
        fn new(profile: &str) -> Self {
            let temp = tempfile::tempdir().expect("tempdir");
            let home = temp.path().join("eval-home");
            let env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(home.as_os_str()))]);
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).expect("create repo root");

            let campaign_id = CampaignId::from("config-projection");
            let mut manifest = CampaignManifest::new(campaign_id.clone());
            manifest.dataset_sources = vec![RegistryDatasetSource {
                key: Some("fixture".to_string()),
                path: temp.path().join("dataset.jsonl"),
                label: "fixture".to_string(),
                url: None,
            }];
            manifest.model_id = Some("google/gemini-3.5-flash".to_string());
            manifest.route_source = Some(ModelRouteSource::DirectGoogle);
            manifest.instances_root = Some(temp.path().join("instances"));
            manifest.batches_root = Some(temp.path().join("batches"));
            let manifest_path = save_campaign_manifest(&manifest).expect("save campaign manifest");

            let branch = "prototype1-parent-0".to_string();
            let parent = ParentIdentity::root_bootstrap(
                campaign_id.clone(),
                "node-0",
                "fixture-instance",
                branch.clone(),
                Some(branch.clone()),
            );
            write_parent_identity(&repo_root, &parent).expect("write parent identity");

            let source_path = temp.path().join("profile.toml");
            fs::write(&source_path, profile).expect("write operator profile");
            let operator = load_operator_profile(
                source_path
                    .to_str()
                    .expect("profile path must be valid UTF-8"),
            )
            .expect("load operator profile");
            let mut admitted = admit_run_profile(&manifest_path, &operator).expect("admit profile");
            let admitted_ms =
                chrono::DateTime::parse_from_rfc3339(&admitted.commitment.admitted_at)
                    .expect("parse admission time")
                    .timestamp_millis();
            admitted.commitment.admitted_at =
                chrono::DateTime::<chrono::Utc>::from_timestamp_millis(admitted_ms)
                    .expect("normalized admission time")
                    .to_rfc3339();
            let commitment_path = manifest_path
                .parent()
                .expect("campaign root")
                .join("prototype1/run-profile.commitment.json");
            fs::write(
                commitment_path,
                serde_json::to_vec_pretty(&admitted.commitment)
                    .expect("serialize normalized commitment"),
            )
            .expect("write normalized commitment");

            let node_dir = manifest_path
                .parent()
                .expect("campaign root")
                .join("prototype1/nodes/node-0");
            let request_path = node_dir.join("runner-request.json");
            let result_path = node_dir.join("runner-result.json");
            let binary_path = node_dir.join("bin/ploke-eval");
            let created_at = parent.created_at().to_string();
            let node = Prototype1NodeRecord {
                schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
                node_id: "node-0".to_string(),
                parent_node_id: None,
                generation: 0,
                instance_id: "fixture-instance".to_string(),
                source_state_id: "prototype1-root:config-projection".to_string(),
                operation_target: None,
                base_artifact_id: None,
                patch_id: None,
                derived_artifact_id: None,
                parent_branch_id: None,
                branch_id: branch.clone(),
                candidate_id: "root-parent".to_string(),
                target_relpath: PathBuf::from(PARENT_IDENTITY_RELPATH),
                node_dir: node_dir.clone(),
                workspace_root: repo_root.clone(),
                binary_path: binary_path.clone(),
                runner_request_path: request_path.clone(),
                runner_result_path: result_path,
                status: Prototype1NodeStatus::Planned,
                created_at: created_at.clone(),
                updated_at: created_at,
            };
            let request = Prototype1RunnerRequest {
                schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
                campaign_id: campaign_id.clone(),
                node_id: "node-0".to_string(),
                generation: 0,
                instance_id: "fixture-instance".to_string(),
                source_state_id: "prototype1-root:config-projection".to_string(),
                operation_target: None,
                base_artifact_id: None,
                patch_id: None,
                derived_artifact_id: None,
                branch_id: branch.clone(),
                target_relpath: PathBuf::from(PARENT_IDENTITY_RELPATH),
                workspace_root: repo_root.clone(),
                binary_path,
                stop_on_error: false,
                runner_args: vec!["loop".to_string(), "prototype1-state".to_string()],
            };
            let manifest_text = fs::read_to_string(&manifest_path).expect("read manifest");
            let intent = SetupAdmissionIntent {
                plan_hash: ContentHash::of("fixture setup plan"),
                campaign_id: campaign_id.clone(),
                manifest_path: manifest_path.clone(),
                repo_root: repo_root.clone(),
                artifact_branch: branch.clone(),
                batch_manifest: temp.path().join("batch.json"),
                hashes: SetupArtifactHashes {
                    manifest: ContentHash::of(&manifest_text),
                    slice: ContentHash::of("fixture batch slice"),
                    profile: ContentHash(admitted.commitment.sha256.clone()),
                },
                checkout: SetupCheckoutBase {
                    branch: "main".to_string(),
                    head: GitCommit("before-setup".to_string()),
                },
                node,
                request,
                identity: parent,
                started_at: RecordedAt(admitted_ms),
            };
            let admission_path = setup_admission_path(&manifest_path);
            let admitting =
                create_setup_admission(&admission_path, intent).expect("create setup admission");
            let complete = admitting
                .clone()
                .complete(
                    GitCommit("completed-head".to_string()),
                    RecordedAt(admitted_ms + 1),
                )
                .expect("complete setup admission");
            replace_setup_admission(&admission_path, &admitting, &complete)
                .expect("persist completed setup admission");

            Self {
                _env: env,
                _temp: temp,
                repo_root,
                manifest_path,
                campaign_id,
            }
        }

        fn commitment_path(&self) -> PathBuf {
            self.manifest_path
                .parent()
                .expect("campaign root")
                .join("prototype1/run-profile.commitment.json")
        }

        fn profile_path(&self) -> PathBuf {
            self.manifest_path
                .parent()
                .expect("campaign root")
                .join("prototype1/run-profile.toml")
        }

        fn admission_path(&self) -> PathBuf {
            setup_admission_path(&self.manifest_path)
        }
    }

    #[test]
    fn projection_preserves_explicit_profile_and_campaign_snapshot() {
        let fixture = Fixture::new(EXPLICIT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");
        let manifest_text = fs::read_to_string(&fixture.manifest_path).expect("read manifest");

        assert_eq!(
            projection.identity.path,
            identity::parent_identity_path(&fixture.repo_root)
        );
        assert_eq!(projection.identity.record.campaign_id, fixture.campaign_id);
        assert_eq!(projection.campaign.path, fixture.manifest_path);
        assert_eq!(projection.campaign.admission.path, fixture.admission_path());
        assert_eq!(
            projection.campaign.admission.manifest_path,
            fixture.manifest_path
        );
        assert_eq!(
            projection.campaign.admission.hashes.manifest,
            projection.campaign.content_hash
        );
        assert_eq!(
            projection.campaign.content_hash,
            ContentHash::of(&manifest_text)
        );
        assert_eq!(
            projection.campaign.manifest.campaign_id,
            fixture.campaign_id
        );
        assert_eq!(
            projection.campaign.resolved.campaign_id,
            fixture.campaign_id
        );
        assert_eq!(
            projection.campaign.resolved.model_id,
            "google/gemini-3.5-flash"
        );
        assert_eq!(
            projection.campaign.provider,
            ProviderSelection::DirectGoogle
        );
        assert_eq!(projection.profile.record.name, "explicit-control");
        assert_eq!(projection.profile.record.control.parallel_cap, Some(1));
        assert_eq!(
            projection.profile.record.search.children.parallel_targets,
            Some(2)
        );
        assert_eq!(
            projection.profile.commitment.profile_path,
            fixture.profile_path()
        );
        assert_eq!(projection.profile.commitment.source_path, None);
        assert!(projection.profile.reported_source.is_some());
        assert_eq!(projection.control.path, fixture.profile_path());
        assert_eq!(projection.control.mode, RunMode::Step);
        assert_eq!(projection.control.parallel_cap.value, 1);
        assert_eq!(
            projection.control.parallel_cap.source,
            ValueSource::Explicit(ProfileField::ControlParallelCap)
        );
        assert_eq!(projection.control.patch_cap.value, 2);
        assert_eq!(
            projection.control.patch_cap.source,
            ValueSource::Explicit(ProfileField::SearchParallelTargets)
        );

        let encoded = serde_json::to_string(&projection).expect("serialize typed snapshot");
        let decoded: WalkConfigSnapshot =
            serde_json::from_str(&encoded).expect("deserialize typed snapshot");

        assert_eq!(decoded.identity, projection.identity);
        assert_eq!(decoded.profile, projection.profile);
        assert_eq!(decoded.control, projection.control);
        assert_eq!(decoded.campaign.path, projection.campaign.path);
        assert_eq!(
            decoded.campaign.resolved.model_id,
            projection.campaign.resolved.model_id
        );
    }

    #[test]
    fn projection_preserves_runtime_defaults_and_derivations() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");

        assert_eq!(projection.profile.record.control.parallel_cap, None);
        assert_eq!(
            projection.profile.record.search.children.parallel_targets,
            None
        );
        assert_eq!(projection.control.mode, RunMode::Continuous);
        assert_eq!(projection.control.parallel_cap.value, 3);
        assert_eq!(
            projection.control.parallel_cap.source,
            ValueSource::Derived(DerivationRule::SearchFanout)
        );
        assert_eq!(projection.control.patch_cap.value, 3);
        assert_eq!(
            projection.control.patch_cap.source,
            ValueSource::Derived(DerivationRule::DefaultPatchTargets)
        );
    }

    #[test]
    fn snapshot_decode_does_not_consult_client_eval_home() {
        let encoded = {
            let fixture = Fixture::new(DEFAULT_PROFILE);
            let projection = load(&fixture.repo_root).expect("load projection");
            serde_json::to_string(&projection).expect("serialize typed snapshot")
        };
        let alternate = tempfile::tempdir().expect("alternate eval home");
        let _env = env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(alternate.path().as_os_str()),
        )]);

        let decoded: WalkConfigSnapshot =
            serde_json::from_str(&encoded).expect("decode independent server snapshot");

        assert_eq!(
            decoded.identity.record.campaign_id.as_str(),
            "config-projection"
        );
    }

    #[test]
    fn projection_rejects_profile_without_commitment() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        fs::remove_file(fixture.commitment_path()).expect("remove commitment");

        let error = load(&fixture.repo_root).expect_err("missing commitment must fail");

        assert!(error.to_string().contains("has no admission commitment"));
    }

    #[test]
    fn projection_rejects_profile_digest_mismatch() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let mut profile = fs::read_to_string(fixture.profile_path()).expect("read profile");
        profile.push_str("\n# uncommitted drift\n");
        fs::write(fixture.profile_path(), profile).expect("write profile drift");

        let error = load(&fixture.repo_root).expect_err("digest mismatch must fail");

        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn projection_rejects_commitment_time_drift_after_setup() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let path = fixture.commitment_path();
        let mut commitment: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read profile commitment"))
                .expect("parse profile commitment");
        commitment["admitted_at"] =
            serde_json::Value::String("2026-07-13T00:00:00+00:00".to_string());
        fs::write(
            &path,
            serde_json::to_string_pretty(&commitment).expect("serialize changed commitment"),
        )
        .expect("write commitment drift");

        let error = load(&fixture.repo_root).expect_err("commitment time drift must fail");

        assert!(error.to_string().contains("admission time"));
    }

    #[test]
    fn projection_rejects_missing_setup_admission() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        fs::remove_file(fixture.admission_path()).expect("remove setup admission");

        let error = load(&fixture.repo_root).expect_err("missing setup admission must fail");

        assert!(error.to_string().contains("no setup admission receipt"));
    }

    #[test]
    fn projection_rejects_campaign_drift_after_setup() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let mut manifest: CampaignManifest = serde_json::from_str(
            &fs::read_to_string(&fixture.manifest_path).expect("read campaign"),
        )
        .expect("parse campaign");
        manifest.model_id = Some("google/gemini-2.5-flash".to_string());
        fs::write(
            &fixture.manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize changed campaign"),
        )
        .expect("write campaign drift");

        let error = load(&fixture.repo_root).expect_err("campaign drift must fail");

        assert!(error.to_string().contains("setup admission manifest hash"));
    }

    #[test]
    fn snapshot_decode_rejects_mismatched_campaign_ids() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");
        let mut value = serde_json::to_value(projection).expect("serialize projection");
        value["campaign"]["resolved"]["campaign_id"] =
            serde_json::Value::String("forged-campaign".to_string());

        let error = serde_json::from_value::<WalkConfigSnapshot>(value)
            .expect_err("mismatched campaign ids must fail");

        assert!(error.to_string().contains("campaign identifiers"));
    }

    #[test]
    fn snapshot_decode_allows_successor_identity_in_active_root() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");
        let mut value = serde_json::to_value(projection).expect("serialize projection");
        value["identity"]["record"]["parent_id"] = serde_json::Value::String("node-1".to_string());
        value["identity"]["record"]["node_id"] = serde_json::Value::String("node-1".to_string());
        value["identity"]["record"]["generation"] = serde_json::json!(1);
        value["identity"]["record"]["previous_parent_id"] =
            serde_json::Value::String("node-0".to_string());
        value["identity"]["record"]["parent_node_id"] =
            serde_json::Value::String("node-0".to_string());

        serde_json::from_value::<WalkConfigSnapshot>(value)
            .expect("successor identity remains in the admitted active root");
    }

    #[test]
    fn snapshot_decode_rejects_profile_path_and_control_tampering() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");
        let value = serde_json::to_value(&projection).expect("serialize projection");

        let mut changed_path = value.clone();
        changed_path["profile"]["commitment"]["profile_path"] =
            serde_json::Value::String("/tmp/forged-profile.toml".to_string());
        let error = serde_json::from_value::<WalkConfigSnapshot>(changed_path)
            .expect_err("mismatched profile path must fail");
        assert!(error.to_string().contains("run profile paths"));

        let mut changed_paths = value.clone();
        changed_paths["profile"]["commitment"]["profile_path"] =
            serde_json::Value::String("/tmp/forged-profile.toml".to_string());
        changed_paths["control"]["path"] =
            serde_json::Value::String("/tmp/forged-profile.toml".to_string());
        let error = serde_json::from_value::<WalkConfigSnapshot>(changed_paths)
            .expect_err("coordinated profile path tampering must fail");
        assert!(error.to_string().contains("run profile paths"));

        let mut changed_control = value;
        changed_control["control"]["parallel_cap"]["value"] = serde_json::json!(99);
        let error = serde_json::from_value::<WalkConfigSnapshot>(changed_control)
            .expect_err("forged effective control must fail");
        assert!(error.to_string().contains("effective control"));
    }

    #[test]
    fn snapshot_decode_rejects_coherent_payload_drift_from_admitted_hashes() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");
        let value = serde_json::to_value(&projection).expect("serialize projection");

        let mut changed_campaign = value.clone();
        changed_campaign["campaign"]["manifest"]["model_id"] =
            serde_json::Value::String("google/gemini-2.5-flash".to_string());
        changed_campaign["campaign"]["resolved"]["model_id"] =
            serde_json::Value::String("google/gemini-2.5-flash".to_string());
        let error = serde_json::from_value::<WalkConfigSnapshot>(changed_campaign)
            .expect_err("coherent campaign drift must fail its admitted hash");
        assert!(error.to_string().contains("canonical campaign manifest"));

        let mut changed_profile = value;
        changed_profile["profile"]["record"]["name"] =
            serde_json::Value::String("forged-profile".to_string());
        let error = serde_json::from_value::<WalkConfigSnapshot>(changed_profile)
            .expect_err("coherent profile drift must fail its admitted hash");
        assert!(error.to_string().contains("canonical run profile"));
    }

    #[test]
    fn snapshot_decode_rejects_coordinated_campaign_path_tampering() {
        let fixture = Fixture::new(DEFAULT_PROFILE);
        let projection = load(&fixture.repo_root).expect("load projection");
        let mut value = serde_json::to_value(projection).expect("serialize projection");
        let forged = fixture.repo_root.join("forged/campaign.json");
        let receipt = forged
            .parent()
            .expect("forged campaign root")
            .join("prototype1/setup-admission.json");
        value["campaign"]["path"] =
            serde_json::Value::String(forged.to_string_lossy().into_owned());
        value["campaign"]["admission"]["manifest_path"] =
            serde_json::Value::String(forged.to_string_lossy().into_owned());
        value["campaign"]["admission"]["path"] =
            serde_json::Value::String(receipt.to_string_lossy().into_owned());

        let error = serde_json::from_value::<WalkConfigSnapshot>(value)
            .expect_err("coordinated campaign path tampering must fail");

        assert!(error.to_string().contains("configured campaign id"));
    }

    #[test]
    fn stable_resolution_preserves_absent_openrouter_preference() {
        let campaign_id = CampaignId::from("openrouter-no-preference");
        let mut manifest = CampaignManifest::new(campaign_id);
        manifest.dataset_sources = vec![RegistryDatasetSource {
            key: Some("fixture".to_string()),
            path: PathBuf::from("/tmp/dataset.jsonl"),
            label: "fixture".to_string(),
            url: None,
        }];
        manifest.model_id = Some("google/gemini-3.5-flash".to_string());
        manifest.route_source = Some(ModelRouteSource::OpenRouter);
        manifest.provider_slug = None;
        manifest.instances_root = Some(PathBuf::from("/tmp/instances"));
        manifest.batches_root = Some(PathBuf::from("/tmp/batches"));

        let resolved = resolve_stable_manifest(manifest).expect("stable OpenRouter resolution");

        assert_eq!(resolved.provider_slug, None);
        assert_eq!(project_provider(&resolved), ProviderSelection::RouteDefault);
    }
}
