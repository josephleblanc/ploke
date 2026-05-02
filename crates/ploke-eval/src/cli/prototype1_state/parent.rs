//! Parent role state for Prototype 1.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    cli::prototype1_state::{
        backend::{GitWorktreeBackend, WorkspaceBackend},
        history::{
            ArtifactLocator, BlockHead, BlockStore, BlockStoreError, FsBlockStore, HistoryError,
            LineageId, LineageState, StoreHead, TreeKeyCommitment,
        },
        identity::{ParentIdentity, parent_identity_path},
        inner::{At, File, LineageKey, Message, MessageBox, Transition},
        observe,
    },
    intervention::{
        Prototype1NodeRecord, load_node_record, prototype1_branch_registry_path,
        prototype1_node_record_path, prototype1_runner_request_path, prototype1_scheduler_path,
    },
    spec::{
        PrepareError, Prototype1ParentError, Prototype1ParentIdentityContext,
        Prototype1ParentNodeContext,
    },
};

/// Parent role before its artifact, identity, and scheduler facts agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unchecked {}

/// Parent role after its artifact, identity, and scheduler facts agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checked {}

/// Parent role after predecessor handoff, if any, has been acknowledged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ready {}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Planned {}

/// Parent role after it has received and validated its child-plan message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selectable {}

/// Parent role after it has locked lineage authority for successor handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retired {}

/// Runtime role carrier for a Parent in a known verification state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parent<S> {
    identity: ParentIdentity,
    node: Prototype1NodeRecord,
    _state: PhantomData<S>,
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
    scheduler: At<SchedulerFile>,
    branches: At<BranchesFile>,
    parent_node_id: String,
    child_generation: u32,
    children: Vec<ChildFiles>,
}

impl ChildPlanFiles {
    pub(crate) fn for_parent(
        manifest_path: &Path,
        parent: &ParentIdentity,
        nodes: &[Prototype1NodeRecord],
    ) -> Self {
        Self {
            message: At::resolve((manifest_path.to_path_buf(), parent.node_id.clone())),
            scheduler: At::resolve(manifest_path.to_path_buf()),
            branches: At::resolve(manifest_path.to_path_buf()),
            parent_node_id: parent.node_id.clone(),
            // Prototype 1 direct-child policy: candidates produced by Parent k
            // are generation k + 1.
            child_generation: parent.generation + 1,
            children: nodes
                .iter()
                .map(|node| ChildFiles {
                    node_id: node.node_id.clone(),
                    node: At::resolve((manifest_path.to_path_buf(), node.node_id.clone())),
                    runner_request: At::resolve((
                        manifest_path.to_path_buf(),
                        node.node_id.clone(),
                    )),
                })
                .collect(),
        }
    }

    pub(crate) fn message(&self) -> &Path {
        self.message.path()
    }

    pub(crate) fn message_at(&self) -> At<ChildPlanFile> {
        self.message.clone()
    }

    pub(crate) fn scheduler(&self) -> &Path {
        self.scheduler.path()
    }

    pub(crate) fn branches(&self) -> &Path {
        self.branches.path()
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

    pub(crate) fn contains_child(&self, node_id: &str) -> bool {
        self.children.iter().any(|child| child.node_id == node_id)
    }

    fn validate_receiver(&self, identity: &ParentIdentity) -> Result<(), ChildPlanReceiverError> {
        if identity.node_id != self.parent_node_id {
            return Err(ChildPlanReceiverError::ParentNode {
                expected_parent_node_id: self.parent_node_id.clone(),
                actual_parent_node_id: identity.node_id.clone(),
            });
        }

        // Match the same direct-child lineage rule encoded when the candidate
        // set was written.
        let actual_generation = identity.generation + 1;
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
    node_id: String,
    node: At<NodeFile>,
    runner_request: At<RunnerRequestFile>,
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
    pub(crate) fn node_id(&self) -> &str {
        &self.node_id
    }

    pub(crate) fn node(&self) -> &Path {
        self.node.path()
    }

    pub(crate) fn runner_request(&self) -> &Path {
        self.runner_request.path()
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
    pub selected_instance: Option<&'a str>,
}

impl Parent<Unchecked> {
    pub(crate) fn load(
        manifest_path: &Path,
        identity: ParentIdentity,
    ) -> Result<Self, PrepareError> {
        let node = load_node_record(manifest_path, &identity.node_id)?;
        Ok(Self {
            identity,
            node,
            _state: PhantomData,
        })
    }

    pub(crate) fn check<B: WorkspaceBackend>(
        self,
        backend: &B,
        manifest_path: &Path,
        check: Check<'_>,
    ) -> Result<Parent<Checked>, PrepareError> {
        backend
            .validate_parent_checkout(check.active_root, &self.identity)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_parent_checkout",
                detail: source.to_string(),
            })?;

        let identity = identity_context(check.active_root, &self.identity);
        let node = node_context(manifest_path, &self.node);

        if identity.generation != node.generation {
            return Err(Prototype1ParentError::GenerationMismatch { identity, node }.into());
        }
        if identity.branch_id != node.branch_id {
            return Err(Prototype1ParentError::BranchMismatch { identity, node }.into());
        }
        if let Some(selected_instance) = check.selected_instance {
            if selected_instance != node.instance_id {
                return Err(Prototype1ParentError::SelectionMismatch {
                    campaign_id: check.campaign_id.to_string(),
                    selected_instance: selected_instance.to_string(),
                    parent: node,
                }
                .into());
            }
        }

        Ok(Parent {
            identity: self.identity,
            node: self.node,
            _state: PhantomData,
        })
    }
}

impl Parent<Checked> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    pub(crate) fn ready(self, startup: Startup<Validated>) -> Result<Parent<Ready>, PrepareError> {
        startup.validate_parent(&self.identity)?;
        Ok(Parent {
            identity: self.identity,
            node: self.node,
            _state: PhantomData,
        })
    }
}

impl Startup<Genesis> {
    pub(crate) fn from_history(
        identity: &ParentIdentity,
        manifest_path: &Path,
    ) -> Result<Startup<Validated>, PrepareError> {
        let startup = observe::Step::start(observe::span!(
            "prototype1.parent.startup.genesis",
            campaign_id = %identity.campaign_id,
            parent_id = %identity.parent_id,
            node_id = %identity.node_id,
            generation = identity.generation,
            manifest_path = %manifest_path.display(),
        ));
        let store = FsBlockStore::for_campaign_manifest(manifest_path);
        let lineage_id = LineageId::new(identity.campaign_id.clone());
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
        if identity.generation != 0 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "genesis startup for parent '{}' requires generation 0, found generation {}",
                    identity.node_id, identity.generation
                ),
            });
        }
        match state.head() {
            StoreHead::Absent { .. } => Ok(Self::validated(identity, state, StartupKind::Genesis)),
            StoreHead::Present(head) => Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "genesis startup for parent '{}' found existing History head at height {}",
                    identity.node_id,
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
            campaign_id = %identity.campaign_id,
            parent_id = %identity.parent_id,
            node_id = %identity.node_id,
            generation = identity.generation,
            manifest_path = %manifest_path.display(),
            active_parent_root = %active_parent_root.display(),
        ));
        let store = FsBlockStore::for_campaign_manifest(manifest_path);
        let lineage_id = LineageId::new(identity.campaign_id.clone());
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
                        identity.campaign_id
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
        if identity.generation == 0 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor startup for parent '{}' cannot enter generation 0",
                    identity.node_id
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
                    identity.node_id
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
        if self.lineage_id.as_str() != identity.campaign_id {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "validated startup lineage '{}' does not match parent campaign '{}'",
                    self.lineage_id.as_str(),
                    identity.campaign_id
                ),
            });
        }
        if self.parent_node_id != identity.node_id {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "validated startup parent '{}' does not match parent identity '{}'",
                    self.parent_node_id, identity.node_id
                ),
            });
        }
        if self.generation != identity.generation {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "validated startup generation {} does not match parent generation {}",
                    self.generation, identity.generation
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
            lineage_id: LineageId::new(identity.campaign_id.clone()),
            parent_node_id: identity.node_id.clone(),
            generation: identity.generation,
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
    let expected = LineageId::new(identity.campaign_id.clone());
    if state.head().lineage_id() != &expected {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "startup lineage '{}' does not match parent campaign '{}'",
                state.head().lineage_id().as_str(),
                identity.campaign_id
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

    pub(crate) fn planned_from_locked_child_plan(self) -> Parent<Planned> {
        self.cast()
    }
}

impl Parent<Planned> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }
}

impl Parent<Selectable> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    pub(super) fn into_retired_and_lineage(self) -> (Parent<Retired>, LineageKey) {
        let lineage = LineageKey::from_debug_value(self.identity.campaign_id.clone());
        (self.cast(), lineage)
    }
}

impl<S> Parent<S> {
    fn cast<T>(self) -> Parent<T> {
        Parent {
            identity: self.identity,
            node: self.node,
            _state: PhantomData,
        }
    }
}

fn identity_context(
    active_root: &Path,
    identity: &ParentIdentity,
) -> Prototype1ParentIdentityContext {
    Prototype1ParentIdentityContext {
        path: parent_identity_path(active_root),
        node_id: identity.node_id.clone(),
        generation: identity.generation,
        branch_id: identity.branch_id.clone(),
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
    use crate::{
        cli::prototype1_state::{
            history::{ActorRef, ArtifactRef, EvidenceRef, SealBlock, SuccessorRef},
            inner::{LockCrown, Open},
        },
        intervention::{PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeStatus},
    };

    fn identity(node_id: &str, generation: u32) -> ParentIdentity {
        ParentIdentity {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: "campaign".to_string(),
            parent_id: node_id.to_string(),
            node_id: node_id.to_string(),
            generation,
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: format!("branch-{node_id}"),
            artifact_branch: Some(format!("artifact-{node_id}")),
            created_at: "2026-04-27T00:00:00Z".to_string(),
        }
    }

    fn parent(node_id: &str, generation: u32) -> Parent<Ready> {
        let identity = identity(node_id, generation);
        Parent {
            node: node_record(node_id, generation, None),
            identity,
            _state: PhantomData,
        }
    }

    fn checked_parent(node_id: &str, generation: u32) -> Parent<Checked> {
        let identity = identity(node_id, generation);
        Parent {
            node: node_record(node_id, generation, None),
            identity,
            _state: PhantomData,
        }
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

    #[test]
    fn child_plan_receive_returns_received_capability_for_ready_parent() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let sender = parent("parent-a", 0);
        let child = node_record("child-1", 1, Some("parent-a"));
        let files = ChildPlanFiles::for_parent(manifest_path, sender.identity(), &[child]);

        let at = files.message_at();
        let (planned, locked) = Open::<ChildPlan>::from_sender(sender, files)
            .lock(at, |_, _| Ok::<_, std::convert::Infallible>(()))
            .unwrap();
        let (selectable, received) = locked.unlock(planned).unwrap();

        assert_eq!(selectable.identity().node_id, "parent-a");
        assert!(received.body().contains_child("child-1"));
    }

    #[test]
    fn selectable_parent_locks_crown_and_retires() {
        let manifest_path = Path::new("/tmp/campaign.json");
        let sender = parent("parent-a", 0);
        let child = node_record("child-1", 1, Some("parent-a"));
        let files = ChildPlanFiles::for_parent(manifest_path, sender.identity(), &[child]);

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
            ArtifactRef::new("artifact:successor"),
            crate::cli::prototype1_state::event::RecordedAt(30),
        ));

        assert_eq!(retired.identity.node_id, "parent-a");
        assert!(locked.lineage_key().matches_debug_str("campaign"));
    }

    #[test]
    fn genesis_startup_allows_generation_zero_ready_parent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("campaign.json");
        let parent = checked_parent("parent-a", 0);

        let startup =
            Startup::<Genesis>::from_history(parent.identity(), &manifest_path).expect("startup");
        let ready = parent.ready(startup).expect("ready parent");

        assert_eq!(ready.identity().node_id, "parent-a");
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
        let files = ChildPlanFiles::for_parent(manifest_path, sender.identity(), &[child]);

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
