//! Durable intent and read-back facts for resumable Prototype 1 setup.
//!
//! Setup admission is a small saga, not one atomic filesystem operation.  The
//! durable record therefore stores immutable intent plus only the coarse
//! `Admitting`/`Complete` lifecycle.  Callers must derive per-phase progress
//! from the authoritative artifacts on every retry; a stored "last completed
//! phase" cannot distinguish an effect that completed just before a crash.

use std::{
    fs::{self, File, OpenOptions},
    io,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};

use crate::{
    cli::prototype1_state::{
        backend::GitCommit,
        event::{ContentHash, RecordedAt},
        identity::ParentIdentity,
    },
    durable_io,
    intervention::{Prototype1NodeRecord, Prototype1NodeStatus, Prototype1RunnerRequest},
    spec::PrepareError,
};

pub(crate) const SETUP_ADMISSION_SCHEMA: &str = "prototype1-setup-admission.v1";
pub(crate) const SETUP_ADMISSION_FILE: &str = "setup-admission.json";
#[derive(Debug)]
pub(crate) struct SetupLock {
    file: File,
}

impl Drop for SetupLock {
    fn drop(&mut self) {
        // Closing the descriptor also releases the lock; explicit unlock keeps
        // the lifecycle clear and permits deterministic same-process tests.
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

pub(crate) fn acquire_setup_lock(path: &Path) -> Result<SetupLock, PrepareError> {
    let path = path.to_path_buf();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|source| PrepareError::WriteManifest {
            path: path.clone(),
            source,
        })?;
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(SetupLock { file });
    }
    let source = io::Error::last_os_error();
    let blocked = source.kind() == io::ErrorKind::WouldBlock
        || source
            .raw_os_error()
            .is_some_and(|code| code == libc::EAGAIN || code == libc::EWOULDBLOCK);
    if blocked {
        return Err(admission_error(format!(
            "prototype1 setup is already being admitted under lock '{}'",
            path.display()
        )));
    }
    Err(PrepareError::WriteManifest { path, source })
}

/// Immutable artifact witnesses committed by the reviewed setup plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetupArtifactHashes {
    pub(crate) manifest: ContentHash,
    pub(crate) slice: ContentHash,
    pub(crate) profile: ContentHash,
}

/// Checkout state captured before setup creates the generation-0 branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetupCheckoutBase {
    pub(crate) branch: String,
    #[serde(with = "git_commit_serde")]
    pub(crate) head: GitCommit,
}

/// Exact intent persisted before any setup side effect.
///
/// Existing production carriers remain authoritative for the root node,
/// runner request, and parent identity.  The admission record composes those
/// carriers instead of defining setup-local mirrors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetupAdmissionIntent {
    pub(crate) plan_hash: ContentHash,
    pub(crate) campaign_id: CampaignId,
    pub(crate) manifest_path: PathBuf,
    pub(crate) repo_root: PathBuf,
    pub(crate) artifact_branch: String,
    pub(crate) batch_manifest: PathBuf,
    pub(crate) hashes: SetupArtifactHashes,
    pub(crate) checkout: SetupCheckoutBase,
    pub(crate) node: Prototype1NodeRecord,
    pub(crate) request: Prototype1RunnerRequest,
    pub(crate) identity: ParentIdentity,
    pub(crate) started_at: RecordedAt,
}

/// Coarse durable lifecycle.  Fine-grained progress is always observed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
pub(crate) enum SetupAdmissionState {
    Admitting,
    Complete {
        #[serde(with = "git_commit_serde")]
        head: GitCommit,
        completed_at: RecordedAt,
    },
}

/// Receipt-first setup authority for one reviewed plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Prototype1SetupAdmission {
    pub(crate) schema_version: String,
    pub(crate) intent: SetupAdmissionIntent,
    pub(crate) state: SetupAdmissionState,
}

impl Prototype1SetupAdmission {
    pub(crate) fn new(intent: SetupAdmissionIntent) -> Result<Self, PrepareError> {
        let receipt = Self {
            schema_version: SETUP_ADMISSION_SCHEMA.to_string(),
            intent,
            state: SetupAdmissionState::Admitting,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub(crate) fn complete(
        mut self,
        head: GitCommit,
        completed_at: RecordedAt,
    ) -> Result<Self, PrepareError> {
        if head.0.trim().is_empty() {
            return Err(admission_error("setup completion requires a Git HEAD"));
        }
        match &self.state {
            SetupAdmissionState::Admitting => {
                self.state = SetupAdmissionState::Complete { head, completed_at };
            }
            SetupAdmissionState::Complete {
                head: stored,
                completed_at: stored_at,
            } if stored == &head && stored_at == &completed_at => {}
            SetupAdmissionState::Complete { .. } => {
                return Err(admission_error(
                    "completed setup admission cannot be replaced by a different completion",
                ));
            }
        }
        self.validate()?;
        Ok(self)
    }

    pub(crate) fn completed_head(&self) -> Option<&GitCommit> {
        match &self.state {
            SetupAdmissionState::Admitting => None,
            SetupAdmissionState::Complete { head, .. } => Some(head),
        }
    }

    fn validate(&self) -> Result<(), PrepareError> {
        if self.schema_version != SETUP_ADMISSION_SCHEMA {
            return Err(admission_error(format!(
                "unsupported setup admission schema '{}'",
                self.schema_version
            )));
        }
        let intent = &self.intent;
        for (field, value) in [
            ("plan_hash", intent.plan_hash.0.as_str()),
            ("manifest_hash", intent.hashes.manifest.0.as_str()),
            ("slice_hash", intent.hashes.slice.0.as_str()),
            ("profile_hash", intent.hashes.profile.0.as_str()),
            ("artifact_branch", intent.artifact_branch.as_str()),
            ("checkout.branch", intent.checkout.branch.as_str()),
            ("checkout.head", intent.checkout.head.0.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(admission_error(format!(
                    "setup admission field '{field}' must not be empty"
                )));
            }
        }

        let node = &intent.node;
        let request = &intent.request;
        let identity = &intent.identity;
        if node.status != Prototype1NodeStatus::Planned
            || node.generation != 0
            || request.generation != 0
            || identity.generation() != 0
            || node.parent_node_id.is_some()
            || identity.parent_node_id().is_some()
            || identity.previous_parent_id().is_some()
        {
            return Err(admission_error(
                "setup admission requires an unstarted generation-0 root",
            ));
        }
        if request.campaign_id != intent.campaign_id
            || identity.campaign_id() != &intent.campaign_id
        {
            return Err(admission_error(
                "setup campaign does not match runner request and parent identity",
            ));
        }
        if node.node_id != request.node_id
            || node.node_id != identity.node_id()
            || identity.parent_id() != node.node_id
        {
            return Err(admission_error(
                "setup root node does not match runner request and parent identity",
            ));
        }
        if node.instance_id != request.instance_id
            || identity.instance_id() != Some(node.instance_id.as_str())
        {
            return Err(admission_error(
                "setup root instance does not match runner request and parent identity",
            ));
        }
        if node.branch_id != request.branch_id
            || node.branch_id != identity.branch_id()
            || node.branch_id != intent.artifact_branch
            || identity.artifact_branch() != Some(intent.artifact_branch.as_str())
        {
            return Err(admission_error(
                "setup artifact branch does not match root carriers",
            ));
        }
        if node.workspace_root != intent.repo_root || request.workspace_root != intent.repo_root {
            return Err(admission_error(
                "setup repository root does not match root carriers",
            ));
        }
        if let SetupAdmissionState::Complete { head, .. } = &self.state
            && head.0.trim().is_empty()
        {
            return Err(admission_error(
                "completed setup admission has an empty witness",
            ));
        }
        Ok(())
    }

    fn validate_intent(&self, expected: &SetupAdmissionIntent) -> Result<(), PrepareError> {
        self.validate()?;
        if &self.intent != expected {
            return Err(admission_error(format!(
                "setup admission intent mismatch for campaign '{}': expected plan '{}', stored '{}'",
                expected.campaign_id, expected.plan_hash, self.intent.plan_hash
            )));
        }
        Ok(())
    }
}

/// Stable receipt location below the campaign-owned Prototype 1 root.
pub(crate) fn setup_admission_path(campaign_manifest: &Path) -> PathBuf {
    campaign_manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join(SETUP_ADMISSION_FILE)
}

/// Load a receipt and validate its schema and internal carrier agreement.
pub(crate) fn load_setup_admission(
    path: &Path,
) -> Result<Option<Prototype1SetupAdmission>, PrepareError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let receipt = serde_json::from_slice::<Prototype1SetupAdmission>(&bytes).map_err(|source| {
        PrepareError::ParseManifest {
            path: path.to_path_buf(),
            source,
        }
    })?;
    receipt.validate()?;
    Ok(Some(receipt))
}

/// Load a receipt using only stable plan/checkout coordinates.
///
/// Recovery calls this before regenerating timestamped node or identity
/// carriers; the exact persisted carriers then remain authoritative.
pub(crate) fn load_matching_admission(
    path: &Path,
    plan_hash: &ContentHash,
    campaign_id: &CampaignId,
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<Option<Prototype1SetupAdmission>, PrepareError> {
    let Some(receipt) = load_setup_admission(path)? else {
        return Ok(None);
    };
    let intent = &receipt.intent;
    if &intent.plan_hash != plan_hash
        || &intent.campaign_id != campaign_id
        || intent.manifest_path != manifest_path
        || intent.repo_root != repo_root
    {
        return Err(admission_error(format!(
            "setup admission coordinates do not match campaign '{}' and plan '{}'",
            campaign_id, plan_hash
        )));
    }
    Ok(Some(receipt))
}

/// Remove an interrupted receipt writer's exact staging remnants.
///
/// The caller must hold the Git-worktree setup lock.
pub(crate) fn cleanup_setup_staging(path: &Path) -> Result<(), PrepareError> {
    durable_io::cleanup_staging(path).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Create the intent before effects, or return the exactly matching receipt.
///
/// The final link is a no-clobber operation.  Concurrent creators cannot
/// replace each other's intent; the loser loads and validates the winner.
pub(crate) fn create_setup_admission(
    path: &Path,
    intent: SetupAdmissionIntent,
) -> Result<Prototype1SetupAdmission, PrepareError> {
    if let Some(receipt) = load_setup_admission(path)? {
        receipt.validate_intent(&intent)?;
        return Ok(receipt);
    }

    let expected = Prototype1SetupAdmission::new(intent.clone())?;
    if create_json_new(path, &expected)? {
        let stored = load_setup_admission(path)?
            .ok_or_else(|| admission_error("setup admission disappeared after atomic creation"))?;
        stored.validate_intent(&intent)?;
        return Ok(stored);
    }

    let stored = load_setup_admission(path)?
        .ok_or_else(|| admission_error("concurrent setup admission was not readable"))?;
    stored.validate_intent(&intent)?;
    Ok(stored)
}

/// Replace a receipt after exact read-back validation and fsync the update.
///
/// This is intentionally not a phase-progress API.  Its expected use is the
/// final `Admitting -> Complete` transition after every phase fact is verified.
pub(crate) fn replace_setup_admission(
    path: &Path,
    expected: &Prototype1SetupAdmission,
    next: &Prototype1SetupAdmission,
) -> Result<(), PrepareError> {
    expected.validate()?;
    next.validate()?;
    if expected.intent != next.intent {
        return Err(admission_error(
            "setup admission replacement cannot change immutable intent",
        ));
    }
    match (&expected.state, &next.state) {
        (SetupAdmissionState::Admitting, SetupAdmissionState::Complete { .. }) => {}
        (left, right) if left == right => {}
        _ => {
            return Err(admission_error(
                "setup admission replacement is not a monotonic transition",
            ));
        }
    }
    let stored = load_setup_admission(path)?
        .ok_or_else(|| admission_error("setup admission is missing before replacement"))?;
    if &stored != expected {
        return Err(admission_error(
            "setup admission changed before exact replacement",
        ));
    }

    write_json_atomic(path, next)?;
    let stored = load_setup_admission(path)?
        .ok_or_else(|| admission_error("setup admission is missing after replacement"))?;
    if &stored != next {
        return Err(admission_error(
            "setup admission read-back does not match atomic replacement",
        ));
    }
    Ok(())
}

/// Atomically replace JSON and fsync both file contents and directory entry.
pub(crate) fn write_json_atomic<T>(path: &Path, value: &T) -> Result<(), PrepareError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(PrepareError::Serialize)?;
    durable_io::write_atomic(path, &bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn create_json_new<T>(path: &Path, value: &T) -> Result<bool, PrepareError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(PrepareError::Serialize)?;
    durable_io::create_atomic(path, &bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn admission_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: detail.into(),
    }
}

mod git_commit_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::GitCommit;

    pub(super) fn serialize<S>(head: &GitCommit, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&head.0)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<GitCommit, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(GitCommit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::prototype1_state::identity::parent_identity_path,
        intervention::PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION,
    };

    fn sample_intent(root: &Path) -> SetupAdmissionIntent {
        let campaign_id = CampaignId::from("campaign-1");
        let node_id = "node-root".to_string();
        let branch = "prototype1-parent-campaign-1-gen0".to_string();
        let node_dir = root.join("campaign/prototype1/nodes/node-root");
        let request_path = node_dir.join("runner-request.json");
        let result_path = node_dir.join("runner-result.json");
        let binary_path = node_dir.join("bin/ploke-eval");
        let node = Prototype1NodeRecord {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            node_id: node_id.clone(),
            parent_node_id: None,
            generation: 0,
            instance_id: "instance-1".to_string(),
            source_state_id: "prototype1-root:campaign-1".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: branch.clone(),
            candidate_id: "root-parent".to_string(),
            target_relpath: PathBuf::from(".ploke/prototype1/parent_identity.json"),
            node_dir: node_dir.clone(),
            workspace_root: root.to_path_buf(),
            binary_path: binary_path.clone(),
            runner_request_path: request_path.clone(),
            runner_result_path: result_path,
            status: Prototype1NodeStatus::Planned,
            created_at: "2026-07-13T00:00:00Z".to_string(),
            updated_at: "2026-07-13T00:00:00Z".to_string(),
        };
        let request = Prototype1RunnerRequest {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            campaign_id: campaign_id.clone(),
            node_id: node_id.clone(),
            generation: 0,
            instance_id: "instance-1".to_string(),
            source_state_id: "prototype1-root:campaign-1".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            branch_id: branch.clone(),
            target_relpath: PathBuf::from(".ploke/prototype1/parent_identity.json"),
            workspace_root: root.to_path_buf(),
            binary_path,
            stop_on_error: false,
            runner_args: vec!["loop".to_string(), "prototype1-state".to_string()],
        };
        let identity = ParentIdentity::root_bootstrap(
            campaign_id.clone(),
            node_id,
            "instance-1",
            branch.clone(),
            Some(branch.clone()),
        );
        SetupAdmissionIntent {
            plan_hash: ContentHash("plan-hash".to_string()),
            campaign_id,
            manifest_path: root.join("campaign/campaign.json"),
            repo_root: root.to_path_buf(),
            artifact_branch: branch,
            batch_manifest: root.join("batch.json"),
            hashes: SetupArtifactHashes {
                manifest: ContentHash("manifest-hash".to_string()),
                slice: ContentHash("slice-hash".to_string()),
                profile: ContentHash("profile-hash".to_string()),
            },
            checkout: SetupCheckoutBase {
                branch: "main".to_string(),
                head: GitCommit("abc123".to_string()),
            },
            node,
            request,
            identity,
            started_at: RecordedAt(1_784_000_000_000),
        }
    }

    #[test]
    fn admission_path_is_stable() {
        let manifest = Path::new("/tmp/campaign/campaign.json");
        assert_eq!(
            setup_admission_path(manifest),
            PathBuf::from("/tmp/campaign/prototype1/setup-admission.json")
        );
    }

    #[test]
    fn setup_lock_rejects_concurrent_admission_and_releases_on_drop() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lock_path = tmp.path().join("prototype1-setup.lock");
        let first = acquire_setup_lock(&lock_path).expect("first setup lock");

        let error = acquire_setup_lock(&lock_path).expect_err("concurrent setup lock");
        assert!(error.to_string().contains("already being admitted"));

        drop(first);
        acquire_setup_lock(&lock_path).expect("reacquire released setup lock");
    }

    #[test]
    fn create_reuses_exact_intent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let path = setup_admission_path(&intent.manifest_path);

        let created = create_setup_admission(&path, intent.clone()).expect("create receipt");
        let loaded = create_setup_admission(&path, intent).expect("reuse receipt");

        assert_eq!(loaded, created);
        assert!(matches!(loaded.state, SetupAdmissionState::Admitting));
        assert_eq!(
            load_setup_admission(&path).expect("load receipt"),
            Some(created)
        );
    }

    #[test]
    fn create_rejects_different_intent_without_rewrite() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let path = setup_admission_path(&intent.manifest_path);
        let created = create_setup_admission(&path, intent.clone()).expect("create receipt");
        let before = fs::read(&path).expect("read receipt");
        let mut changed = intent;
        changed.plan_hash = ContentHash("different-plan".to_string());

        let error = create_setup_admission(&path, changed).expect_err("reject mismatch");

        assert!(error.to_string().contains("intent mismatch"));
        assert_eq!(fs::read(&path).expect("reread receipt"), before);
        assert_eq!(
            load_setup_admission(&path).expect("load receipt"),
            Some(created)
        );
    }

    #[test]
    fn replacement_is_monotonic_and_exact() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let path = setup_admission_path(&intent.manifest_path);
        let current = create_setup_admission(&path, intent).expect("create receipt");
        let next = current
            .clone()
            .complete(
                GitCommit("def456".to_string()),
                RecordedAt(1_784_000_060_000),
            )
            .expect("complete receipt");

        replace_setup_admission(&path, &current, &next).expect("replace receipt");
        assert_eq!(
            load_setup_admission(&path).expect("load receipt"),
            Some(next.clone())
        );

        let error = replace_setup_admission(&path, &current, &next)
            .expect_err("stale expected receipt must fail");
        assert!(
            error
                .to_string()
                .contains("changed before exact replacement")
        );
        assert!(
            fs::read_dir(path.parent().expect("receipt parent"))
                .expect("read receipt parent")
                .all(|entry| !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-"))
        );
    }

    #[test]
    fn matching_load_uses_stable_coordinates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let path = setup_admission_path(&intent.manifest_path);
        let created = create_setup_admission(&path, intent.clone()).expect("create receipt");

        let loaded = load_matching_admission(
            &path,
            &intent.plan_hash,
            &intent.campaign_id,
            &intent.manifest_path,
            &intent.repo_root,
        )
        .expect("matching receipt");
        assert_eq!(loaded, Some(created));

        let error = load_matching_admission(
            &path,
            &ContentHash("other-plan".to_string()),
            &intent.campaign_id,
            &intent.manifest_path,
            &intent.repo_root,
        )
        .expect_err("reject changed plan");
        assert!(error.to_string().contains("coordinates do not match"));
    }

    #[test]
    fn intent_reuses_parent_identity_carrier() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        assert_eq!(
            parent_identity_path(&intent.repo_root),
            intent
                .repo_root
                .join(".ploke/prototype1/parent_identity.json")
        );
        assert_eq!(intent.identity.node_id(), intent.node.node_id);
    }
}
