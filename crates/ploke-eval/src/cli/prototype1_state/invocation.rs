//! Persisted bootstrap contract for prototype1 runtime attempts.
//!
//! This record is the attempt-scoped bootstrap a runtime needs in order to:
//! - identify which durable node it belongs to
//! - know which authority contract it carries
//! - participate in the current handoff attempt
//!
//! Runtime bootstrap facts that used to be read from separate node and runner
//! request files are carried here for child executions. Those files may still
//! be written as projections, but this invocation is the executable boundary
//! for the process that is about to start.
//!
//! The important seam is that Prototype 1 currently has two narrow runtime
//! authority roles. These roles describe authority and bounded behavior, not a
//! permanent storage location for the process:
//!
//! - `Child`: leaf evaluator, executes one node, records, exits
//! - `Successor`: handoff token that lets the next parent acknowledge bootstrap
//!   before entering the same typed parent command as the initial parent
//!
//! Keeping that split explicit here prevents the live runner seam from
//! quietly drifting into a generic "fresh binary can do anything" surface. A
//! successor should be launched from the stable active checkout after it has
//! been advanced to the selected Artifact; temporary child worktrees remain
//! cleanup targets.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::intervention::{
    PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeRecord, Prototype1RunnerRequest,
    ResolvedTreatmentBranch,
};
use crate::spec::PrepareError;

use super::{
    channel::Endpoints,
    event::RuntimeId,
    parent::{Parent, Retired},
};

fn leaf_runner_argv(invocation_path: &Path) -> Vec<String> {
    vec![
        "loop".to_string(),
        "prototype1-runner".to_string(),
        "--invocation".to_string(),
        invocation_path.display().to_string(),
        "--execute".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ]
}

fn successor_parent_argv(
    invocation: &Invocation,
    invocation_path: &Path,
) -> Result<Vec<String>, PrepareError> {
    let active_parent_root = invocation.active_parent_root.as_ref().ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor invocation '{}' is missing active_parent_root",
                invocation_path.display()
            ),
        }
    })?;
    Ok(vec![
        "loop".to_string(),
        "prototype1-state".to_string(),
        "--campaign".to_string(),
        invocation.campaign_id.clone(),
        "--repo-root".to_string(),
        active_parent_root.display().to_string(),
        "--handoff-invocation".to_string(),
        invocation_path.display().to_string(),
        "--stop-after".to_string(),
        "complete".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ])
}

/// Durable schema version for runtime invocations.
pub(crate) const SCHEMA_VERSION: &str = "prototype1-invocation.v1";

/// Runtime role for one invocation attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    /// Leaf evaluator child: acknowledge, self-evaluate, record, exit.
    Child,
    /// Selected continuation runtime for bounded successor bootstrap.
    Successor,
}

/// Persisted bootstrap record for one prototype1 runtime attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub schema_version: String,
    pub role: Role,
    pub campaign_id: String,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub journal_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<Prototype1NodeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Prototype1RunnerRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedTreatmentBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_parent_root: Option<PathBuf>,
    pub created_at: String,
}

/// Executable leaf-child invocation.
///
/// This authority is limited to leaf evaluation and must not recurse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildInvocation {
    inner: Invocation,
}

/// Executable selected-successor bootstrap contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SuccessorInvocation {
    inner: Invocation,
}

/// Classified invocation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InvocationAuthority {
    Child(ChildInvocation),
    Successor(SuccessorInvocation),
}

impl Invocation {
    /// Child-evaluator bootstrap contract.
    fn child(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        channel_root: PathBuf,
        node: Option<Prototype1NodeRecord>,
        request: Option<Prototype1RunnerRequest>,
        resolved: Option<ResolvedTreatmentBranch>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Child,
            campaign_id,
            node_id,
            runtime_id,
            journal_path,
            channel_root: Some(channel_root),
            node,
            request,
            resolved,
            active_parent_root: None,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Selected-successor bootstrap contract.
    fn successor(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        channel_root: PathBuf,
        active_parent_root: PathBuf,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Successor,
            campaign_id,
            node_id,
            runtime_id,
            journal_path,
            channel_root: Some(channel_root),
            node: None,
            request: None,
            resolved: None,
            active_parent_root: Some(active_parent_root),
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Classify the persisted invocation by its runtime authority.
    fn classify(self) -> InvocationAuthority {
        match self.role {
            Role::Child => InvocationAuthority::Child(ChildInvocation { inner: self }),
            Role::Successor => InvocationAuthority::Successor(SuccessorInvocation { inner: self }),
        }
    }
}

impl ChildInvocation {
    /// Create the executable leaf-child invocation used by the live runner.
    pub(crate) fn new(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        channel_root: PathBuf,
    ) -> Self {
        Self {
            inner: Invocation::child(
                campaign_id,
                node_id,
                runtime_id,
                journal_path,
                channel_root,
                None,
                None,
                None,
            ),
        }
    }

    /// Create the executable leaf-child invocation with the typed runtime
    /// payload needed to evaluate without reading node/request projection files.
    pub(crate) fn with_bootstrap(
        campaign_id: String,
        node: Prototype1NodeRecord,
        request: Prototype1RunnerRequest,
        resolved: ResolvedTreatmentBranch,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        channel_root: PathBuf,
    ) -> Result<Self, PrepareError> {
        if request.node_id != node.node_id
            || request.campaign_id != campaign_id
            || request.schema_version != PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION
            || node.schema_version != PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION
            || request.branch_id != node.branch_id
            || request.generation != node.generation
            || request.instance_id != node.instance_id
            || request.source_state_id != node.source_state_id
            || request.target_relpath != node.target_relpath
            || request.binary_path != node.binary_path
            || resolved.instance_id != node.instance_id
            || resolved.source_state_id != node.source_state_id
            || resolved.parent_branch_id != node.parent_branch_id
            || resolved.target_relpath != node.target_relpath
            || resolved.branch.branch_id != node.branch_id
            || resolved.branch.candidate_id != node.candidate_id
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "child invocation bootstrap request does not match node '{}'",
                    node.node_id
                ),
            });
        }
        let node_id = node.node_id.clone();
        Ok(Self {
            inner: Invocation::child(
                campaign_id,
                node_id,
                runtime_id,
                journal_path,
                channel_root,
                Some(node),
                Some(request),
                Some(resolved),
            ),
        })
    }

    /// Access the persisted wire record.
    pub(crate) fn as_invocation(&self) -> &Invocation {
        &self.inner
    }

    /// Campaign this child leaf run belongs to.
    pub(crate) fn campaign_id(&self) -> &str {
        &self.inner.campaign_id
    }

    /// Durable node this child leaf run evaluates.
    pub(crate) fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Runtime identity for this concrete child attempt.
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.inner.runtime_id
    }

    /// Node payload carried by this executable child bootstrap.
    pub(crate) fn node_record(&self) -> Result<&Prototype1NodeRecord, PrepareError> {
        self.inner
            .node
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "child invocation for node '{}' is missing node bootstrap payload",
                    self.inner.node_id
                ),
            })
    }

    /// Runner request payload carried by this executable child bootstrap.
    pub(crate) fn runner_request(&self) -> Result<&Prototype1RunnerRequest, PrepareError> {
        self.inner
            .request
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "child invocation for node '{}' is missing runner request bootstrap payload",
                    self.inner.node_id
                ),
            })
    }

    /// Resolved branch payload carried by this executable child bootstrap.
    pub(crate) fn resolved(&self) -> Result<&ResolvedTreatmentBranch, PrepareError> {
        self.inner
            .resolved
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "child invocation for node '{}' is missing resolved branch bootstrap payload",
                    self.inner.node_id
                ),
            })
    }

    /// Shared journal path used for child acknowledgement.
    pub(crate) fn journal_path(&self) -> &Path {
        &self.inner.journal_path
    }

    /// Per-runtime parent/child channel endpoints for this leaf invocation.
    pub(crate) fn channel_endpoints(&self) -> Option<Endpoints> {
        self.inner.channel_root.as_ref().map(|root| {
            Endpoints::new(
                root.clone(),
                self.inner.campaign_id.clone(),
                self.inner.node_id.clone(),
                self.inner.runtime_id,
            )
        })
    }

    /// CLI argv for launching exactly one leaf child evaluation.
    pub(crate) fn launch_args(&self, invocation_path: &Path) -> Vec<String> {
        leaf_runner_argv(invocation_path)
    }
}

impl SuccessorInvocation {
    /// Create the executable successor invocation used by the detached
    /// handoff path.
    fn new(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        channel_root: PathBuf,
        active_parent_root: PathBuf,
    ) -> Self {
        Self {
            inner: Invocation::successor(
                campaign_id,
                node_id,
                runtime_id,
                journal_path,
                channel_root,
                active_parent_root,
            ),
        }
    }

    /// Create the executable successor invocation with an explicit channel root.
    pub(crate) fn from_retired_parent_with_channel_root(
        _parent: &Parent<Retired>,
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        channel_root: PathBuf,
        active_parent_root: PathBuf,
    ) -> Self {
        Self::new(
            campaign_id,
            node_id,
            runtime_id,
            journal_path,
            channel_root,
            active_parent_root,
        )
    }

    /// Create the successor launch descriptor after the predecessor has crossed
    /// into `Parent<Retired>`.
    ///
    /// This is still a launch descriptor, not sealed History authority. The
    /// retired parent argument exists to keep ordinary crate code from creating
    /// executable successor invocations without first crossing the
    /// Crown-locking handoff boundary.
    pub(crate) fn from_retired_parent(
        _parent: &Parent<Retired>,
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
        active_parent_root: PathBuf,
    ) -> Self {
        let channel_root = successor_channel_root_from_journal(&journal_path, &node_id, runtime_id);
        Self::from_retired_parent_with_channel_root(
            _parent,
            campaign_id,
            node_id,
            runtime_id,
            journal_path,
            channel_root,
            active_parent_root,
        )
    }

    /// Access the persisted wire record.
    pub(crate) fn as_invocation(&self) -> &Invocation {
        &self.inner
    }

    /// Campaign this successor bootstrap belongs to.
    pub(crate) fn campaign_id(&self) -> &str {
        &self.inner.campaign_id
    }

    /// Durable node this successor bootstrap belongs to.
    pub(crate) fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Runtime identity for this successor bootstrap attempt.
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.inner.runtime_id
    }

    /// Stable active parent checkout root for this successor runtime.
    pub(crate) fn active_parent_root(&self) -> Option<&Path> {
        self.inner.active_parent_root.as_deref()
    }

    /// Shared journal path used for successor acknowledgement and completion.
    pub(crate) fn journal_path(&self) -> &Path {
        &self.inner.journal_path
    }

    /// Per-runtime parent/successor channel endpoints for this bootstrap.
    pub(crate) fn channel_endpoints(&self) -> Option<Endpoints> {
        self.inner.channel_root.as_ref().map(|root| {
            Endpoints::new(
                root.clone(),
                self.inner.campaign_id.clone(),
                self.inner.node_id.clone(),
                self.inner.runtime_id,
            )
        })
    }

    /// CLI argv for launching the successor as the next typed parent.
    fn launch_args(&self, invocation_path: &Path) -> Result<Vec<String>, PrepareError> {
        successor_parent_argv(&self.inner, invocation_path)
    }

    /// CLI argv for a successor launch after the predecessor retired.
    ///
    /// This keeps launch argv construction on the same side of the handoff
    /// boundary as invocation construction. The invocation JSON remains a
    /// persisted descriptor; it is not itself authority to spawn another
    /// runtime.
    pub(crate) fn launch_args_for_retired_parent(
        &self,
        _parent: &Parent<Retired>,
        invocation_path: &Path,
    ) -> Result<Vec<String>, PrepareError> {
        self.launch_args(invocation_path)
    }
}

/// Directory containing persisted invocation records for one node.
pub(crate) fn invocations_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("invocations")
}

/// Attempt-scoped invocation path for one concrete runtime.
pub(crate) fn invocation_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    invocations_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Per-runtime parent/child channel root.
pub(crate) fn channel_root(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    node_dir.join("channels").join(runtime_id.to_string())
}

/// Infer the conventional successor channel root from the shared transition journal.
fn successor_channel_root_from_journal(
    journal_path: &Path,
    node_id: &str,
    runtime_id: RuntimeId,
) -> PathBuf {
    let prototype1_root = journal_path.parent().unwrap_or_else(|| Path::new("."));
    channel_root(&prototype1_root.join("nodes").join(node_id), runtime_id)
}

/// Directory containing attempt-scoped result artifacts for one node.
pub(crate) fn results_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("results")
}

/// Attempt-scoped result path for one concrete runtime.
pub(crate) fn result_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    results_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Persist one invocation record.
fn write_invocation(path: &Path, invocation: &Invocation) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(invocation).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Load one persisted invocation record.
pub(crate) fn load(path: &Path) -> Result<Invocation, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Load and classify one persisted invocation by authority.
pub(crate) fn load_authority(path: &Path) -> Result<InvocationAuthority, PrepareError> {
    Ok(load(path)?.classify())
}

/// Load an executable invocation for the live Prototype 1 runner.
pub(crate) fn load_executable(path: &Path) -> Result<InvocationAuthority, PrepareError> {
    load_authority(path)
}

/// Persist one executable leaf-child invocation.
pub(crate) fn write_child_invocation(
    path: &Path,
    invocation: &ChildInvocation,
) -> Result<(), PrepareError> {
    write_invocation(path, invocation.as_invocation())
}

/// Persist one executable branch-successor invocation.
fn write_successor_invocation(
    path: &Path,
    invocation: &SuccessorInvocation,
) -> Result<(), PrepareError> {
    write_invocation(path, invocation.as_invocation())
}

/// Persist a successor launch descriptor after the predecessor retired.
///
/// This is intentionally separate from raw invocation writing: creating a file
/// that can launch the next parent must remain downstream of the Crown-locking
/// handoff transition.
pub(crate) fn write_successor_invocation_for_retired_parent(
    _parent: &Parent<Retired>,
    path: &Path,
    invocation: &SuccessorInvocation,
) -> Result<(), PrepareError> {
    write_successor_invocation(path, invocation)
}

/// Successor-ready acknowledgement written by a detached successor runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorReadyRecord {
    pub schema_version: String,
    pub campaign_id: String,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub pid: u32,
    pub recorded_at: String,
}

/// Durable schema version for successor-ready acknowledgements.
pub(crate) const SUCCESSOR_READY_SCHEMA_VERSION: &str = "prototype1-successor-ready.v1";

/// Terminal status for one successor rehydration attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuccessorCompletionStatus {
    /// The successor completed one bounded rehydrated controller generation.
    Succeeded,
    /// The successor acknowledged handoff but failed during rehydration.
    Failed,
}

/// Completion record written after a successor attempts controller rehydration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorCompletionRecord {
    pub schema_version: String,
    pub campaign_id: String,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub status: SuccessorCompletionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub recorded_at: String,
}

/// Durable schema version for successor completion records.
pub(crate) const SUCCESSOR_COMPLETION_SCHEMA_VERSION: &str = "prototype1-successor-completion.v1";

/// Directory containing successor-ready acknowledgements for one node.
pub(crate) fn successor_ready_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("successor-ready")
}

/// Successor-ready acknowledgement path for one concrete runtime.
pub(crate) fn successor_ready_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    successor_ready_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Directory containing successor completion records for one node.
pub(crate) fn successor_completion_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("successor-completion")
}

/// Successor completion path for one concrete runtime.
pub(crate) fn successor_completion_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    successor_completion_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Persist one successor-ready acknowledgement.
pub(crate) fn write_successor_ready_record(
    path: &Path,
    record: &SuccessorReadyRecord,
) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(record).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Load one persisted successor-ready acknowledgement.
pub(crate) fn load_successor_ready_record(
    path: &Path,
) -> Result<SuccessorReadyRecord, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Persist one successor completion record.
pub(crate) fn write_successor_completion_record(
    path: &Path,
    record: &SuccessorCompletionRecord,
) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(record).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successor_launch_args_reenter_typed_parent_command() {
        let runtime_id = RuntimeId::new();
        let invocation = SuccessorInvocation::new(
            "campaign-1".to_string(),
            "node-2".to_string(),
            runtime_id,
            PathBuf::from("/tmp/prototype1/journal.jsonl"),
            PathBuf::from("/tmp/prototype1/nodes/node-2/channels/runtime-2"),
            PathBuf::from("/repo/stable-parent"),
        );
        let invocation_path =
            PathBuf::from(format!("/tmp/prototype1/invocations/{runtime_id}.json"));

        let argv = invocation
            .launch_args(&invocation_path)
            .expect("successor parent argv");

        assert_eq!(
            argv,
            vec![
                "loop",
                "prototype1-state",
                "--campaign",
                "campaign-1",
                "--repo-root",
                "/repo/stable-parent",
                "--handoff-invocation",
                invocation_path.to_str().expect("utf8 path"),
                "--stop-after",
                "complete",
                "--format",
                "json",
            ]
        );
    }

    #[test]
    fn successor_invocation_projects_channel_endpoints() {
        let runtime_id = RuntimeId::new();
        let channel_root = PathBuf::from("/tmp/prototype1/nodes/node-2/channels/runtime-2");
        let invocation = SuccessorInvocation::new(
            "campaign-1".to_string(),
            "node-2".to_string(),
            runtime_id,
            PathBuf::from("/tmp/prototype1/journal.jsonl"),
            channel_root.clone(),
            PathBuf::from("/repo/stable-parent"),
        );

        let endpoints = invocation
            .channel_endpoints()
            .expect("successor channel endpoints");

        assert_eq!(endpoints.root(), channel_root.as_path());
        assert_eq!(endpoints.child_to_parent().runtime_id(), runtime_id);
    }
}
