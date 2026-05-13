//! Parent role state for Prototype 1.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::{
    cli::prototype1_state::{
        backend::{GitWorktreeBackend, WorkspaceBackend},
        edit_surface::harness_request,
        history::{
            ArtifactLocator, BlockHead, BlockStore, BlockStoreError, FsBlockStore, HistoryError,
            LineageId, LineageState, StoreHead, SurfaceEvidence, TreeKeyCommitment,
            surface_attempt,
        },
        identity::{ParentIdentity, parent_identity_path},
        inner::{At, File, LineageKey, Message, MessageBox, Transition},
        observe,
    },
    intervention::{
        PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeRecord, Prototype1NodeStatus,
        Prototype1RunnerRequest, ResolvedTreatmentBranch, prototype1_branch_registry_path,
        prototype1_node_record_path, prototype1_runner_request_path, prototype1_scheduler_path,
        runner_request_from_node,
    },
    loop_graph::RuntimeId,
    spec::{PrepareError, Prototype1ParentIdentityContext, Prototype1ParentNodeContext},
};

/// Parent role before its artifact, identity, and scheduler facts agree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Unchecked;

/// Parent role after its artifact, identity, and scheduler facts agree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Checked;

/// Parent role after predecessor handoff, if any, has been acknowledged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Ready;

/// Parent role after it has published a harness request and is awaiting a
/// request-bound child-plan response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AwaitingHarnessPlan {
    harness_request: harness_request::request::Reference<
        harness_request::request::Broad,
        harness_request::request::Published,
    >,
}

/// Startup evidence before a lineage predecessor exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Genesis {}

/// Startup evidence derived from a sealed predecessor block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Predecessor {}

/// Startup evidence checked against the current parent identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Validated {}

/// Parent role after it has packed a child-plan message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Planned;

/// Parent role after it has received and validated its child-plan message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Selectable;

/// Parent role after it has locked lineage authority for successor handoff.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Retired;

/// Runtime role carrier for a Parent in a known verification state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parent<S> {
    runtime_id: RuntimeId,
    identity: ParentIdentity,
    node: Prototype1NodeRecord,
    state: S,
}

/// Evidence that this runtime may enter the ready Parent path.
///
/// `Startup<Validated>` is the local single-ruler startup gate. Gen0 reaches it
/// only from a checked absent History head. Later runtimes reach it only after
/// the predecessor sealed head and current checkout have already been verified.
/// The fields stay private so callers cannot convert transport evidence, such
/// as successor invocation JSON, into Parent readiness by convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Startup<S> {
    lineage_id: LineageId,
    parent_node_id: String,
    generation: u32,
    state: LineageState,
    kind: StartupKind,
    _state: PhantomData<S>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartupKind {
    Genesis,
    Predecessor { head: BlockHead },
}

/// Cross-runtime message: this parent has planned one or more child artifacts.
#[derive(Debug)]
pub(crate) struct ChildPlan;

/// Body locked into the child-plan message box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChildPlanFiles {
    message: At<ChildPlanFile>,
    parent_node_id: String,
    child_generation: u32,
    children: Vec<ChildFiles>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rejected_surface_attempts: Vec<surface_attempt::Evidence>,
}

impl ChildPlanFiles {
    pub(crate) fn for_parent(
        manifest_path: &Path,
        parent: &ParentIdentity,
        children: Vec<ChildFiles>,
    ) -> Self {
        Self {
            message: At::resolve((manifest_path.to_path_buf(), parent.node_id().to_string())),
            parent_node_id: parent.node_id().to_string(),
            // Prototype 1 direct-child policy: candidates produced by Parent k
            // are generation k + 1.
            child_generation: parent.generation() + 1,
            children,
            rejected_surface_attempts: Vec::new(),
        }
    }

    pub(crate) fn with_rejected_surface_attempts(
        mut self,
        rejected_surface_attempts: Vec<surface_attempt::Evidence>,
    ) -> Self {
        self.rejected_surface_attempts = rejected_surface_attempts;
        self
    }

    pub(crate) fn message(&self) -> &Path {
        self.message.path()
    }

    pub(crate) fn message_at(&self) -> At<ChildPlanFile> {
        self.message.clone()
    }

    pub(crate) fn parent_node_id(&self) -> &str {
        &self.parent_node_id
    }

    pub(crate) fn child_generation(&self) -> u32 {
        self.child_generation
    }

    pub(crate) fn children(&self) -> &[ChildFiles] {
        &self.children
    }

    pub(crate) fn rejected_surface_attempts(&self) -> &[surface_attempt::Evidence] {
        &self.rejected_surface_attempts
    }

    pub(crate) fn contains_child(&self, node_id: &str) -> bool {
        self.children.iter().any(|child| child.node_id() == node_id)
    }

    fn validate_receiver(&self, identity: &ParentIdentity) -> Result<(), ChildPlanReceiverError> {
        if identity.node_id() != self.parent_node_id {
            return Err(ChildPlanReceiverError::ParentNode {
                expected_parent_node_id: self.parent_node_id.clone(),
                actual_parent_node_id: identity.node_id().to_string(),
            });
        }

        // Match the same direct-child lineage rule encoded when the candidate
        // set was written.
        let actual_generation = identity.generation() + 1;
        if actual_generation != self.child_generation {
            return Err(ChildPlanReceiverError::Generation {
                expected_generation: self.child_generation,
                actual_generation,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChildFiles {
    node: Prototype1NodeRecord,
    request: Prototype1RunnerRequest,
    resolved: ResolvedTreatmentBranch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    surface: Option<SurfaceEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    harness: Option<harness_request::child::Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChildPlanFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LockChildPlan;

impl Transition for LockChildPlan {
    type From = Parent<Ready>;
    type To = Parent<Planned>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnlockChildPlan;

impl Transition for UnlockChildPlan {
    type From = Parent<Planned>;
    type To = Parent<Selectable>;
}

impl File for ChildPlanFile {
    type Params = (PathBuf, String);

    const NAME: &'static str = "prototype1/messages/child-plan/<parent-node-id>.json";

    fn resolve((manifest_path, parent_node_id): Self::Params) -> PathBuf {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("prototype1/messages/child-plan")
            .join(format!("{parent_node_id}.json"))
    }
}

impl MessageBox for ChildPlanFile {
    type Lock = LockChildPlan;
    type Unlock = UnlockChildPlan;
}

impl ChildFiles {
    pub(crate) fn from_resolved(
        campaign_id: &str,
        node: Prototype1NodeRecord,
        resolved: ResolvedTreatmentBranch,
        stop_on_error: bool,
    ) -> Self {
        Self {
            request: runner_request_from_node(campaign_id, &node, stop_on_error),
            node,
            resolved,
            surface: None,
            harness: None,
        }
    }

    pub(crate) fn with_surface(mut self, evidence: SurfaceEvidence) -> Self {
        self.surface = Some(evidence);
        self
    }

    pub(crate) fn with_harness_evidence(
        mut self,
        evidence: harness_request::child::Evidence,
    ) -> Self {
        self.harness = Some(evidence);
        self
    }

    pub(crate) fn node_id(&self) -> &str {
        &self.node.node_id
    }

    pub(crate) fn node_record(&self) -> &Prototype1NodeRecord {
        &self.node
    }

    pub(crate) fn runner_request(&self) -> &Prototype1RunnerRequest {
        &self.request
    }

    pub(crate) fn resolved(&self) -> &ResolvedTreatmentBranch {
        &self.resolved
    }

    pub(crate) fn surface(&self) -> Option<&SurfaceEvidence> {
        self.surface.as_ref()
    }

    pub(crate) fn harness_evidence(&self) -> Option<&harness_request::child::Evidence> {
        self.harness.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SchedulerFile;

impl File for SchedulerFile {
    type Params = PathBuf;

    const NAME: &'static str = "prototype1/scheduler.json";

    fn resolve(manifest_path: Self::Params) -> PathBuf {
        prototype1_scheduler_path(&manifest_path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BranchesFile;

impl File for BranchesFile {
    type Params = PathBuf;

    const NAME: &'static str = "prototype1/branches.json";

    fn resolve(manifest_path: Self::Params) -> PathBuf {
        prototype1_branch_registry_path(&manifest_path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeFile;

impl File for NodeFile {
    type Params = (PathBuf, String);

    const NAME: &'static str = "prototype1/nodes/<node-id>/node.json";

    fn resolve((manifest_path, node_id): Self::Params) -> PathBuf {
        prototype1_node_record_path(&manifest_path, &node_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RunnerRequestFile;

impl File for RunnerRequestFile {
    type Params = (PathBuf, String);

    const NAME: &'static str = "prototype1/nodes/<node-id>/runner-request.json";

    fn resolve((manifest_path, node_id): Self::Params) -> PathBuf {
        prototype1_runner_request_path(&manifest_path, &node_id)
    }
}

impl Message for ChildPlan {
    type Box = ChildPlanFile;
    type Body = ChildPlanFiles;
    type SenderFailed = Parent<Ready>;
    type ReceiveError = ChildPlanReceiverError;

    const KIND: &'static str = "child_plan";

    fn close_sender(
        sender: <<Self::Box as MessageBox>::Lock as Transition>::From,
        _at: &At<Self::Box>,
        _body: &Self::Body,
    ) -> <<Self::Box as MessageBox>::Lock as Transition>::To {
        sender.cast()
    }

    fn fail_sender(
        sender: <<Self::Box as MessageBox>::Lock as Transition>::From,
    ) -> Self::SenderFailed {
        sender
    }

    fn ready_receiver(
        receiver: <<Self::Box as MessageBox>::Unlock as Transition>::From,
        at: &At<Self::Box>,
        body: &Self::Body,
    ) -> Result<<<Self::Box as MessageBox>::Unlock as Transition>::To, Self::ReceiveError> {
        if at.path() != body.message() {
            return Err(ChildPlanReceiverError::MessageBox {
                expected: body.message().to_path_buf(),
                actual: at.path().to_path_buf(),
            });
        }
        body.validate_receiver(receiver.identity())?;
        Ok(receiver.cast())
    }
}

/// Wrong receiver for a packed child-plan message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ChildPlanReceiverError {
    /// The message was read from a different box than the body names.
    #[error("child plan box '{actual}' did not match body box '{expected}'", actual = actual.display(), expected = expected.display())]
    MessageBox { expected: PathBuf, actual: PathBuf },
    /// The packed message names a different parent than the receiver.
    #[error(
        "child plan is addressed to parent node '{expected_parent_node_id}', but receiver is '{actual_parent_node_id}'"
    )]
    ParentNode {
        expected_parent_node_id: String,
        actual_parent_node_id: String,
    },
    /// The packed message names a different child generation than the receiver can accept.
    #[error(
        "child plan is addressed to generation {expected_generation}, but receiver can accept generation {actual_generation}"
    )]
    Generation {
        expected_generation: u32,
        actual_generation: u32,
    },
}

/// Inputs needed to check whether an unchecked Parent role is valid here.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Check<'a> {
    pub campaign_id: &'a str,
    pub active_root: &'a Path,
}

fn parent_node_projection(manifest_path: &Path, identity: &ParentIdentity) -> Prototype1NodeRecord {
    let prototype_root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1");
    let node_dir = prototype_root.join("nodes").join(&identity.node_id());
    let binary_path = node_dir
        .join("bin")
        .join(format!("ploke-eval{}", std::env::consts::EXE_SUFFIX));
    Prototype1NodeRecord {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        node_id: identity.node_id().to_string(),
        parent_node_id: identity.parent_node_id().map(str::to_string),
        generation: identity.generation(),
        instance_id: identity
            .instance_id()
            .map(str::to_string)
            .unwrap_or_else(|| identity.node_id().to_string()),
        source_state_id: identity.branch_id().to_string(),
        operation_target: None,
        base_artifact_id: None,
        patch_id: None,
        derived_artifact_id: None,
        parent_branch_id: None,
        branch_id: identity.branch_id().to_string(),
        candidate_id: identity.node_id().to_string(),
        target_relpath: super::identity::parent_identity_relpath(),
        node_dir: node_dir.clone(),
        workspace_root: PathBuf::new(),
        binary_path,
        runner_request_path: prototype1_runner_request_path(manifest_path, &identity.node_id()),
        runner_result_path: node_dir.join("runner-result.json"),
        status: Prototype1NodeStatus::Running,
        created_at: identity.created_at().to_string(),
        updated_at: identity.created_at().to_string(),
    }
}

impl Parent<Unchecked> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    pub(crate) fn load_with_runtime_id(
        manifest_path: &Path,
        identity: ParentIdentity,
        runtime_id: RuntimeId,
    ) -> Result<Self, PrepareError> {
        let node = parent_node_projection(manifest_path, &identity);
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "artifact_identity",
            transition = "ParentIdentity->Parent<Unchecked>",
            runtime_id = %runtime_id,
            campaign_id = %identity.campaign_id(),
            parent_id = %identity.parent_id(),
            node_id = %identity.node_id(),
            generation = identity.generation(),
            branch_id = %identity.branch_id(),
            "loaded parent runtime identity from active Artifact"
        );
        Ok(Self::from_parts(runtime_id, identity, node))
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(manifest_path, identity),
        fields(
            role = "parent",
            authority = "artifact_identity",
            transition = "ParentIdentity->Parent<Unchecked>",
            campaign_id = %identity.campaign_id(),
            parent_id = %identity.parent_id(),
            node_id = %identity.node_id(),
            generation = identity.generation(),
            branch_id = %identity.branch_id(),
            artifact_branch = ?identity.artifact_branch(),
        )
    )]
    pub(crate) fn load(
        manifest_path: &Path,
        identity: ParentIdentity,
    ) -> Result<Self, PrepareError> {
        Self::load_with_runtime_id(manifest_path, identity, RuntimeId::new())
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self, backend, manifest_path, check),
        fields(
            role = "parent",
            authority = "artifact_backend",
            transition = "Parent<Unchecked>->Parent<Checked>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            active_root = %check.active_root.display(),
        )
    )]
    pub(crate) fn check<B: WorkspaceBackend>(
        self,
        backend: &B,
        manifest_path: &Path,
        check: Check<'_>,
    ) -> Result<Parent<Checked>, PrepareError> {
        let _ = (manifest_path, check.campaign_id);
        backend
            .validate_parent_checkout(check.active_root, &self.identity)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_parent_checkout",
                detail: source.to_string(),
            })?;

        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "artifact_backend",
            transition = "Parent<Unchecked>->Parent<Checked>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            "validated parent checkout against artifact-carried identity"
        );
        Ok(self.cast())
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self, startup),
        fields(
            role = "parent",
            authority = "history_successor_startup",
            transition = "Parent<Unchecked>->Parent<Ready>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
        )
    )]
    pub(crate) fn ready_from_predecessor_startup(
        self,
        startup: Startup<Validated>,
    ) -> Result<Parent<Ready>, PrepareError> {
        startup.validate_parent(&self.identity)?;
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "history_successor_startup",
            transition = "Parent<Unchecked>->Parent<Ready>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            "admitted successor parent after sealed History startup validation"
        );
        Ok(self.cast())
    }
}

impl Parent<Checked> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self, startup),
        fields(
            role = "parent",
            authority = "history_startup",
            transition = "Parent<Checked>->Parent<Ready>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
        )
    )]
    pub(crate) fn ready(self, startup: Startup<Validated>) -> Result<Parent<Ready>, PrepareError> {
        startup.validate_parent(&self.identity)?;
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "history_startup",
            transition = "Parent<Checked>->Parent<Ready>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            "admitted checked parent after startup authority validation"
        );
        Ok(self.cast())
    }
}

impl Startup<Genesis> {
    pub(crate) fn from_history(
        identity: &ParentIdentity,
        manifest_path: &Path,
    ) -> Result<Startup<Validated>, PrepareError> {
        let startup = observe::Step::start(observe::span!(
            "prototype1.parent.startup.genesis",
            campaign_id = %identity.campaign_id(),
            parent_id = %identity.parent_id(),
            node_id = %identity.node_id(),
            generation = identity.generation(),
            manifest_path = %manifest_path.display(),
        ));
        let store = FsBlockStore::for_campaign_manifest(manifest_path);
        let lineage_id = LineageId::new(identity.campaign_id().to_string());
        let state = match store.lineage_state(&lineage_id) {
            Ok(state) => state,
            Err(source) => {
                let error = block_store_prepare_error(source);
                startup.fail("lineage_state", &error);
                return Err(error);
            }
        };
        let result = Self::validated_from_state(identity, state);
        match &result {
            Ok(_) => startup.success(),
            Err(error) => startup.fail("genesis_startup", error),
        }
        result
    }

    fn validated_from_state(
        identity: &ParentIdentity,
        state: LineageState,
    ) -> Result<Startup<Validated>, PrepareError> {
        validate_startup_lineage(identity, &state)?;
        if identity.generation() != 0 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "genesis startup for parent '{}' requires generation 0, found generation {}",
                    identity.node_id(),
                    identity.generation()
                ),
            });
        }
        match state.head() {
            StoreHead::Absent { .. } => Ok(Self::validated(identity, state, StartupKind::Genesis)),
            StoreHead::Present(head) => Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "genesis startup for parent '{}' found existing History head at height {}",
                    identity.node_id(),
                    head.block_height()
                ),
            }),
        }
    }
}

impl Startup<Predecessor> {
    pub(crate) fn from_history(
        identity: &ParentIdentity,
        manifest_path: &Path,
        active_parent_root: &Path,
    ) -> Result<Startup<Validated>, PrepareError> {
        let startup = observe::Step::start(observe::span!(
            "prototype1.parent.startup.predecessor",
            campaign_id = %identity.campaign_id(),
            parent_id = %identity.parent_id(),
            node_id = %identity.node_id(),
            generation = identity.generation(),
            manifest_path = %manifest_path.display(),
            active_parent_root = %active_parent_root.display(),
        ));
        let store = FsBlockStore::for_campaign_manifest(manifest_path);
        let lineage_id = LineageId::new(identity.campaign_id().to_string());
        let state = match store.lineage_state(&lineage_id) {
            Ok(state) => state,
            Err(source) => {
                let error = block_store_prepare_error(source);
                startup.fail("lineage_state", &error);
                return Err(error);
            }
        };
        let head = match state.head() {
            StoreHead::Present(head) => head.clone(),
            StoreHead::Absent { .. } => {
                let error = PrepareError::DatabaseSetup {
                    phase: "prototype1_history_successor_startup",
                    detail: format!(
                        "successor startup for campaign '{}' has no sealed History head to verify",
                        identity.campaign_id()
                    ),
                };
                startup.fail("missing_predecessor_head", &error);
                return Err(error);
            }
        };
        let sealed = match store.sealed_head_block(&head) {
            Ok(sealed) => sealed,
            Err(source) => {
                let error = block_store_prepare_error(source);
                startup.fail("sealed_head_block", &error);
                return Err(error);
            }
        };
        if sealed.selected_parent_identity() != identity {
            let error = PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor startup identity for node '{}' does not match sealed History successor identity '{}'",
                    identity.node_id(),
                    sealed.selected_parent_identity().node_id()
                ),
            };
            startup.fail("selected_parent_identity", &error);
            return Err(error);
        }
        let current_artifact = match GitWorktreeBackend.clean_tree_key(active_parent_root) {
            Ok(key) => match key.tree_key_hash() {
                Ok(hash) => hash,
                Err(source) => {
                    let error = history_prepare_error(source);
                    startup.fail("tree_key_hash", &error);
                    return Err(error);
                }
            },
            Err(source) => {
                let error = backend_prepare_error(source);
                startup.fail("clean_tree_key", &error);
                return Err(error);
            }
        };
        if let Err(source) =
            sealed.verify_current_artifact_tree(&current_artifact, &ArtifactLocator)
        {
            let error = history_prepare_error(source);
            startup.fail("verify_current_artifact_tree", &error);
            return Err(error);
        }
        let current_surface =
            match GitWorktreeBackend.surface_commitment(active_parent_root, active_parent_root) {
                Ok(surface) => surface,
                Err(source) => {
                    let error = backend_prepare_error(source);
                    startup.fail("surface_commitment", &error);
                    return Err(error);
                }
            };
        if let Err(source) = sealed.verify_current_surface(&current_surface) {
            let error = history_prepare_error(source);
            startup.fail("verify_current_surface", &error);
            return Err(error);
        }
        let result = Self::validated_from_state(identity, state);
        match &result {
            Ok(_) => startup.success(),
            Err(error) => startup.fail("predecessor_startup", error),
        }
        result
    }

    fn validated_from_state(
        identity: &ParentIdentity,
        state: LineageState,
    ) -> Result<Startup<Validated>, PrepareError> {
        validate_startup_lineage(identity, &state)?;
        if identity.generation() == 0 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor startup for parent '{}' cannot enter generation 0",
                    identity.node_id()
                ),
            });
        }
        match state.head() {
            StoreHead::Present(head) => Ok(Self::validated(
                identity,
                state.clone(),
                StartupKind::Predecessor { head: head.clone() },
            )),
            StoreHead::Absent { .. } => Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor startup for parent '{}' has no sealed History head",
                    identity.node_id()
                ),
            }),
        }
    }
}

impl Startup<Validated> {
    fn validate_parent(&self, identity: &ParentIdentity) -> Result<(), PrepareError> {
        match (&self.kind, self.state.head()) {
            (StartupKind::Genesis, StoreHead::Absent { .. }) => {}
            (StartupKind::Predecessor { head }, StoreHead::Present(actual)) if head == actual => {}
            (StartupKind::Genesis, StoreHead::Present(head)) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "validated genesis startup unexpectedly carries History head at height {}",
                        head.block_height()
                    ),
                });
            }
            (StartupKind::Predecessor { .. }, StoreHead::Absent { .. }) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail:
                        "validated predecessor startup unexpectedly carries absent History head"
                            .to_string(),
                });
            }
            (StartupKind::Predecessor { .. }, StoreHead::Present(_)) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "validated predecessor startup head changed after validation"
                        .to_string(),
                });
            }
        }
        if self.lineage_id.as_str() != identity.campaign_id() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "validated startup lineage '{}' does not match parent campaign '{}'",
                    self.lineage_id.as_str(),
                    identity.campaign_id()
                ),
            });
        }
        if self.parent_node_id != identity.node_id() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "validated startup parent '{}' does not match parent identity '{}'",
                    self.parent_node_id,
                    identity.node_id()
                ),
            });
        }
        if self.generation != identity.generation() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "validated startup generation {} does not match parent generation {}",
                    self.generation,
                    identity.generation()
                ),
            });
        }
        Ok(())
    }
}

impl<S> Startup<S> {
    fn validated(
        identity: &ParentIdentity,
        state: LineageState,
        kind: StartupKind,
    ) -> Startup<Validated> {
        Startup {
            lineage_id: LineageId::new(identity.campaign_id().to_string()),
            parent_node_id: identity.node_id().to_string(),
            generation: identity.generation(),
            state,
            kind,
            _state: PhantomData,
        }
    }
}

fn validate_startup_lineage(
    identity: &ParentIdentity,
    state: &LineageState,
) -> Result<(), PrepareError> {
    let expected = LineageId::new(identity.campaign_id().to_string());
    if state.head().lineage_id() != &expected {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "startup lineage '{}' does not match parent campaign '{}'",
                state.head().lineage_id().as_str(),
                identity.campaign_id()
            ),
        });
    }
    Ok(())
}

fn history_prepare_error(error: HistoryError) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_history",
        detail: error.to_string(),
    }
}

fn block_store_prepare_error(error: BlockStoreError) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_history_store",
        detail: error.to_string(),
    }
}

fn backend_prepare_error(
    error: crate::cli::prototype1_state::backend::BackendError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_history_tree_key",
        detail: error.to_string(),
    }
}

impl Parent<Ready> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self),
        fields(
            role = "parent",
            authority = "parent_broadcast_channel",
            transition = "Parent<Ready>->Parent<Planned>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
        )
    )]
    pub(crate) fn planned_from_locked_child_plan(self) -> Parent<Planned> {
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "parent_broadcast_channel",
            transition = "Parent<Ready>->Parent<Planned>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            "locked parent child-plan broadcast"
        );
        self.cast()
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self, harness_request),
        fields(
            role = "parent",
            authority = "parent_broadcast_channel",
            transition = "Parent<Ready>->Parent<AwaitingHarnessPlan>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            request_id = %harness_request.request_id(),
        )
    )]
    pub(crate) fn awaiting_harness_plan_for_request(
        self,
        harness_request: harness_request::request::Reference<
            harness_request::request::Broad,
            harness_request::request::Published,
        >,
    ) -> Parent<AwaitingHarnessPlan> {
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "parent_broadcast_channel",
            transition = "Parent<Ready>->Parent<AwaitingHarnessPlan>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            request_id = %harness_request.request_id(),
            request_hash = %harness_request.request_hash(),
            "published broad harness request and is awaiting a request-bound child-plan response"
        );
        self.into_state(AwaitingHarnessPlan { harness_request })
    }
}

impl Parent<Planned> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }
}

impl Parent<AwaitingHarnessPlan> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    pub(crate) fn harness_request(
        &self,
    ) -> &harness_request::request::Reference<
        harness_request::request::Broad,
        harness_request::request::Published,
    > {
        &self.state.harness_request
    }

    pub(crate) fn accept_harness_plan(self) -> Parent<Ready> {
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "parent_broadcast_channel",
            transition = "Parent<AwaitingHarnessPlan>->Parent<Ready>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            request_id = %self.state.harness_request.request_id(),
            request_hash = %self.state.harness_request.request_hash(),
            "accepted request-bound harness response and resumed child-plan locking"
        );
        self.into_state(Ready)
    }
}

impl Parent<Selectable> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self),
        fields(
            role = "parent",
            authority = "crown_lineage_lock",
            transition = "Parent<Selectable>->Parent<Retired>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
        )
    )]
    pub(super) fn into_retired_and_lineage(self) -> (Parent<Retired>, LineageKey) {
        info!(
            target: "ploke_exec",
            role = "parent",
            authority = "crown_lineage_lock",
            transition = "Parent<Selectable>->Parent<Retired>",
            runtime_id = %self.runtime_id,
            campaign_id = %self.identity.campaign_id(),
            parent_id = %self.identity.parent_id(),
            node_id = %self.identity.node_id(),
            generation = self.identity.generation(),
            branch_id = %self.identity.branch_id(),
            "parent locked lineage authority for successor handoff"
        );
        let lineage = LineageKey::from_debug_value(self.identity.campaign_id().to_string());
        (self.cast(), lineage)
    }
}

impl<S> Parent<S>
where
    S: Default,
{
    fn from_parts(
        runtime_id: RuntimeId,
        identity: ParentIdentity,
        node: Prototype1NodeRecord,
    ) -> Self {
        Self {
            runtime_id,
            identity,
            node,
            state: S::default(),
        }
    }
}

impl<S> Parent<S> {
    pub(crate) fn node(&self) -> &Prototype1NodeRecord {
        &self.node
    }

    pub(crate) fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }

    fn cast<T>(self) -> Parent<T>
    where
        T: Default,
    {
        Parent {
            runtime_id: self.runtime_id,
            identity: self.identity,
            node: self.node,
            state: T::default(),
        }
    }

    fn into_state<T>(self, state: T) -> Parent<T> {
        Parent {
            runtime_id: self.runtime_id,
            identity: self.identity,
            node: self.node,
            state,
        }
    }
}

fn identity_context(
    active_root: &Path,
    identity: &ParentIdentity,
) -> Prototype1ParentIdentityContext {
    Prototype1ParentIdentityContext {
        path: parent_identity_path(active_root),
        node_id: identity.node_id().to_string(),
        generation: identity.generation(),
        branch_id: identity.branch_id().to_string(),
    }
}

fn node_context(manifest_path: &Path, node: &Prototype1NodeRecord) -> Prototype1ParentNodeContext {
    Prototype1ParentNodeContext {
        path: prototype1_node_record_path(manifest_path, &node.node_id),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        instance_id: node.instance_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Id, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{Layer, Registry};

    use crate::{
        cli::prototype1_state::{
            edit_surface::harness_request::{HarnessChildBudget, PublishedBroadHarnessRequest},
            history::{ActorRef, ArtifactRef, EvidenceRef, SealBlock, SuccessorRef},
            identity::ParentIdentityRecord,
            inner::{LockCrown, Open},
        },
        intervention::{PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeStatus},
    };

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
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &Id,
            _ctx: Context<'_, S>,
        ) {
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

    fn trace_contains(lines: &[String], needles: &[&str]) -> bool {
        lines
            .iter()
            .any(|line| needles.iter().all(|needle| line.contains(needle)))
    }

    fn identity(node_id: &str, generation: u32) -> ParentIdentity {
        ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: "campaign".to_string(),
            parent_id: node_id.to_string(),
            node_id: node_id.to_string(),
            generation,
            instance_id: Some("instance".to_string()),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: format!("branch-{node_id}"),
            artifact_branch: Some(format!("artifact-{node_id}")),
            created_at: "2026-04-27T00:00:00Z".to_string(),
        })
    }

    fn runtime_id(node_id: &str, generation: u32) -> RuntimeId {
        let mut bytes = [0_u8; 16];
        for (dst, src) in bytes.iter_mut().zip(node_id.as_bytes().iter().copied()) {
            *dst = src;
        }
        bytes[12..].copy_from_slice(&generation.to_be_bytes());
        RuntimeId(uuid::Uuid::from_bytes(bytes))
    }

    fn parent(node_id: &str, generation: u32) -> Parent<Ready> {
        let identity = identity(node_id, generation);
        Parent::from_parts(
            runtime_id(node_id, generation),
            identity,
            node_record(node_id, generation, None),
        )
    }

    fn checked_parent(node_id: &str, generation: u32) -> Parent<Checked> {
        let identity = identity(node_id, generation);
        Parent::from_parts(
            runtime_id(node_id, generation),
            identity,
            node_record(node_id, generation, None),
        )
    }

    fn node_record(
        node_id: &str,
        generation: u32,
        parent_node_id: Option<&str>,
    ) -> Prototype1NodeRecord {
        let node_dir = PathBuf::from(format!("/tmp/prototype1/nodes/{node_id}"));
        Prototype1NodeRecord {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            node_id: node_id.to_string(),
            parent_node_id: parent_node_id.map(ToOwned::to_owned),
            generation,
            instance_id: "instance".to_string(),
            source_state_id: "source".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: format!("branch-{node_id}"),
            candidate_id: format!("candidate-{node_id}"),
            target_relpath: PathBuf::from("crates/ploke-core/tool_text/read_file.md"),
            node_dir: node_dir.clone(),
            workspace_root: node_dir.join("worktree"),
            binary_path: node_dir.join("bin/ploke-eval"),
            runner_request_path: node_dir.join("runner-request.json"),
            runner_result_path: node_dir.join("runner-result.json"),
            status: Prototype1NodeStatus::Planned,
            created_at: "2026-04-27T00:00:00Z".to_string(),
            updated_at: "2026-04-27T00:00:00Z".to_string(),
        }
    }

    fn resolved_for(node: &Prototype1NodeRecord) -> ResolvedTreatmentBranch {
        ResolvedTreatmentBranch {
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            parent_branch_id: node.parent_branch_id.clone(),
            target_relpath: node.target_relpath.clone(),
            source_content: "old".to_string(),
            source_content_hash: "old-hash".to_string(),
            selected_branch_id: None,
            branch: crate::intervention::TreatmentBranchNode {
                branch_id: node.branch_id.clone(),
                candidate_id: node.candidate_id.clone(),
                patch_id: None,
                branch_label: "test".to_string(),
                synthesized_spec_id: "spec".to_string(),
                proposed_content: "new".to_string(),
                proposed_content_hash: "new-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: crate::intervention::TreatmentBranchStatus::Synthesized,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
            },
        }
    }

    fn child_files(parent: &ParentIdentity, child: Prototype1NodeRecord) -> ChildFiles {
        let resolved = resolved_for(&child);
        ChildFiles::from_resolved(parent.campaign_id(), child, resolved, false)
    }

    #[test]
    fn parent_planning_transition_emits_authority_trace() {
        let parent = parent("parent-a", 0);
        let runtime_id = *parent.runtime_id();

        let (_planned, trace) = collect_traces(|| parent.planned_from_locked_child_plan());

        assert!(trace_contains(
            &trace,
            &[
                "transition=Parent<Ready>->Parent<Planned>",
                "authority=parent_broadcast_channel",
                "role=parent",
                "node_id=parent-a",
                &format!("runtime_id={runtime_id}"),
            ],
        ));
        assert!(trace_contains(
            &trace,
            &[
                "locked parent child-plan broadcast",
                "campaign_id=campaign",
                "branch_id=branch-parent-a",
            ],
        ));
    }

    #[test]
    fn awaiting_harness_plan_accepts_published_request_identity() {
        let admission_binding = {
            let admission = crate::cli::prototype1_state::backend::EditSurfaceAdmission::new(
                crate::loop_graph::Coordinate {
                    runtime_id: crate::loop_graph::RuntimeId::new(),
                    target: crate::loop_graph::OperationTarget::Artifact {
                        artifact_id: crate::loop_graph::ArtifactId::new("/repo"),
                    },
                },
                crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(
                    "workspace except ploke-eval",
                ),
            );
            crate::cli::prototype1_state::edit_surface::harness_request::RequestAdmissionBinding::from_admission(&admission)
                .expect("request admission binding should project from admission")
        };
        let published = PublishedBroadHarnessRequest::prototype1_workspace(
            "parent-a".to_string(),
            PathBuf::from("/repo"),
            HarnessChildBudget {
                min_children: 1,
                max_children: 3,
            },
            Path::new("/tmp/prototype1"),
            PathBuf::from("/tmp/prompts/broad-harness.json"),
            PathBuf::from("/tmp/prompts/broad-harness.md"),
            PathBuf::from("/tmp/plans/child-plan.json"),
            admission_binding,
        );

        let awaiting = parent("parent-a", 0).awaiting_harness_plan_for_request((&published).into());

        assert_eq!(
            awaiting.harness_request().request_id(),
            published.request_id()
        );
        assert_eq!(
            awaiting.harness_request().request_hash(),
            published.request_hash()
        );
    }

    #[test]
    fn child_plan_receive_returns_received_capability_for_ready_parent() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let sender = parent("parent-a", 0);
        let sender_runtime_id = *sender.runtime_id();
        let child = node_record("child-1", 1, Some("parent-a"));
        let files = ChildPlanFiles::for_parent(
            manifest_path,
            sender.identity(),
            vec![child_files(sender.identity(), child)],
        );

        let at = files.message_at();
        let (planned, locked) = Open::<ChildPlan>::from_sender(sender, files)
            .lock(at, |_, _| Ok::<_, std::convert::Infallible>(()))
            .unwrap();
        let (selectable, received) = locked.unlock(planned).unwrap();

        assert_eq!(selectable.identity().node_id(), "parent-a");
        assert_eq!(*selectable.runtime_id(), sender_runtime_id);
        assert!(received.body().contains_child("child-1"));
    }

    #[test]
    fn child_plan_files_deserializes_nested_child_manifest() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let sender = parent("parent-a", 0);
        let child = node_record("child-1", 1, Some("parent-a"));
        let files = ChildPlanFiles::for_parent(
            manifest_path,
            sender.identity(),
            vec![child_files(sender.identity(), child)],
        );

        let json = serde_json::to_string(&files).expect("serialize child-plan files");
        let decoded: ChildPlanFiles =
            serde_json::from_str(&json).expect("deserialize child-plan files");

        assert_eq!(decoded, files);
        assert_eq!(decoded.parent_node_id(), "parent-a");
        assert_eq!(decoded.child_generation(), 1);
        assert_eq!(decoded.children().len(), 1);
        assert_eq!(decoded.children()[0].node_id(), "child-1");
        assert_eq!(
            decoded.children()[0].runner_request().node_id.as_str(),
            "child-1"
        );
        assert_eq!(
            decoded.children()[0].resolved().branch.branch_id,
            "branch-child-1"
        );
    }

    #[test]
    fn selectable_parent_locks_crown_and_retires() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let sender = parent("parent-a", 0);
        let runtime_id = *sender.runtime_id();
        let child = node_record("child-1", 1, Some("parent-a"));
        let files = ChildPlanFiles::for_parent(
            manifest_path,
            sender.identity(),
            vec![child_files(sender.identity(), child)],
        );

        let at = files.message_at();
        let (planned, locked_plan) = Open::<ChildPlan>::from_sender(sender, files)
            .lock(at, |_, _| Ok::<_, std::convert::Infallible>(()))
            .unwrap();
        let (selectable, _received) = locked_plan.unlock(planned).unwrap();
        let (retired, locked) = selectable.lock_crown(SealBlock::from_handoff(
            EvidenceRef::new("transition:crown-lock"),
            SuccessorRef::new(
                ActorRef::Process("successor".to_string()),
                ArtifactRef::new("artifact:successor"),
            ),
            ParentIdentity::root_bootstrap(
                "campaign-1",
                "child-1",
                "instance-child-1",
                "branch-child-1",
                Some("artifact-branch-child-1".to_string()),
            ),
            ArtifactRef::new("artifact:successor"),
            crate::cli::prototype1_state::event::RecordedAt(30),
        ));

        assert_eq!(retired.identity.node_id(), "parent-a");
        assert_eq!(*retired.runtime_id(), runtime_id);
        assert!(locked.lineage_key().matches_debug_str("campaign"));
    }

    #[test]
    fn genesis_startup_allows_generation_zero_ready_parent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let parent = checked_parent("parent-a", 0);
        let runtime_id = *parent.runtime_id();

        let startup =
            Startup::<Genesis>::from_history(parent.identity(), &manifest_path).expect("startup");
        let ready = parent.ready(startup).expect("ready parent");

        assert_eq!(ready.identity().node_id(), "parent-a");
        assert_eq!(*ready.runtime_id(), runtime_id);
    }

    #[test]
    fn unchecked_parent_load_keeps_explicit_runtime_id() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let identity = identity("parent-a", 0);
        let runtime_id = runtime_id("parent-a", 42);

        let unchecked =
            Parent::<Unchecked>::load_with_runtime_id(manifest_path, identity.clone(), runtime_id)
                .expect("load unchecked parent");

        assert_eq!(*unchecked.runtime_id(), runtime_id);
        assert_eq!(unchecked.identity(), &identity);
    }

    #[test]
    fn genesis_startup_rejects_later_generation_parent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let parent = checked_parent("parent-b", 1);

        let error = Startup::<Genesis>::from_history(parent.identity(), &manifest_path)
            .expect_err("generation one cannot use genesis startup");

        assert!(matches!(error, PrepareError::InvalidBatchSelection { .. }));
    }

    #[test]
    fn child_plan_receive_rejects_wrong_ready_parent() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let sender = parent("parent-a", 0);
        let receiver = parent("parent-b", 0);
        let child = node_record("child-1", 1, Some("parent-a"));
        let files = ChildPlanFiles::for_parent(
            manifest_path,
            sender.identity(),
            vec![child_files(sender.identity(), child)],
        );

        let at = files.message_at();
        let (_planned, locked) = Open::<ChildPlan>::from_sender(sender, files)
            .lock(at, |_, _| Ok::<_, std::convert::Infallible>(()))
            .unwrap();
        let err = locked
            .unlock(receiver.planned_from_locked_child_plan())
            .unwrap_err();
        let (_failed, source) = err.into_parts();

        assert!(matches!(source, ChildPlanReceiverError::ParentNode { .. }));
    }
}
