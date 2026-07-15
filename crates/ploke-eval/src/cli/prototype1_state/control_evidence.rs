//! Canonical evidence carried by durable controller-session cursors.
//!
//! The initial cursor is bound to completed setup authority, parent identity,
//! admitted profile, and the stable origin schema. Every later cursor is the
//! digest of a full graph-bound transition certificate stored in the same
//! journal record as the transition result.

use std::path::{Path, PathBuf};

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    edge::ControlEdge,
    event::ContentHash,
    identity::ParentIdentity,
    invocation::ProcessIncarnation,
    invocation::{Invocation, Role},
    journal::{ActiveCheckoutAdvancedEntry, SuccessorHandoffEntry},
    profile::RunProfileCommitment,
    session::{AttemptIntent, Cursor, EpochReceipt, SessionId},
    setup_admission::Prototype1SetupAdmission,
    successor,
    walk::phase::WalkPhase,
};

const INITIAL_SCHEMA: &str = "prototype1-control-origin.v1";
const SUCCESSOR_SCHEMA: &str = "prototype1-control-successor-origin.v1";
const EVIDENCE_SCHEMA: &str = "prototype1-control-evidence.v1";

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct InitialEvidence<'a> {
    schema_version: &'static str,
    setup: &'a Prototype1SetupAdmission,
    parent: &'a ParentIdentity,
    profile: &'a RunProfileCommitment,
    phase: WalkPhase,
}

/// Exact persisted authority that admits a selected successor's own session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SuccessorOrigin {
    invocation_path: PathBuf,
    invocation: Invocation,
    checkout: ActiveCheckoutAdvancedEntry,
    spawned: successor::Record,
}

impl SuccessorOrigin {
    pub(crate) fn new(
        invocation_path: PathBuf,
        invocation: Invocation,
        checkout: ActiveCheckoutAdvancedEntry,
        spawned: successor::Record,
        parent: &ParentIdentity,
        profile: &RunProfileCommitment,
        repo_root: &Path,
    ) -> Result<Self, EvidenceError> {
        let origin = Self {
            invocation_path,
            invocation,
            checkout,
            spawned,
        };
        origin.validate(parent, profile, repo_root)?;
        Ok(origin)
    }

    pub(crate) fn invocation_path(&self) -> &Path {
        &self.invocation_path
    }

    pub(crate) fn runtime_id(&self) -> super::event::RuntimeId {
        self.invocation.runtime_id
    }

    pub(crate) fn installed_commit(&self) -> &str {
        &self.checkout.installed_commit
    }

    pub(crate) fn selected_branch(&self) -> &str {
        &self.checkout.selected_branch
    }

    pub(crate) fn active_root(&self) -> Option<&Path> {
        self.invocation.active_parent_root.as_deref()
    }

    pub(crate) fn predecessor(&self) -> Option<&ParentIdentity> {
        self.checkout.previous_parent_identity.as_ref()
    }

    pub(crate) fn spawned_pid(&self) -> u32 {
        match &self.spawned.state {
            successor::State::Spawned { pid, .. } => *pid,
            _ => unreachable!("successor origin construction requires Spawned evidence"),
        }
    }

    pub(crate) fn spawned_incarnation(&self) -> Option<&ProcessIncarnation> {
        match &self.spawned.state {
            successor::State::Spawned { incarnation, .. } => incarnation.as_ref(),
            _ => unreachable!("successor origin construction requires Spawned evidence"),
        }
    }

    pub(crate) fn binary_path(&self) -> &Path {
        match &self.spawned.state {
            successor::State::Spawned { binary_path, .. } => binary_path,
            _ => unreachable!("successor origin construction requires Spawned evidence"),
        }
    }

    /// Prove that the predecessor's acknowledgement names the exact runtime
    /// and process admitted by this successor origin.
    pub(crate) fn matches_handoff(&self, handoff: &SuccessorHandoffEntry) -> bool {
        let successor::State::Spawned {
            pid,
            incarnation: _,
            active_parent_root,
            binary_path,
            invocation_path,
            ready_path,
            streams,
        } = &self.spawned.state
        else {
            return false;
        };
        handoff.campaign_id == self.invocation.campaign_id
            && handoff.node_id == self.invocation.node_id
            && handoff.runtime_id == self.invocation.runtime_id
            && handoff.pid == *pid
            && same_path(&handoff.active_parent_root, active_parent_root)
            && same_path(&handoff.binary_path, binary_path)
            && same_path(&handoff.invocation_path, invocation_path)
            && same_path(&handoff.ready_path, ready_path)
            && handoff
                .streams
                .as_ref()
                .is_none_or(|handoff_streams| handoff_streams == streams)
    }

    pub(crate) fn validate(
        &self,
        parent: &ParentIdentity,
        profile: &RunProfileCommitment,
        repo_root: &Path,
    ) -> Result<(), EvidenceError> {
        let root = self.invocation.active_parent_root.as_deref();
        let branch = parent.artifact_branch();
        let predecessor = self
            .checkout
            .previous_parent_identity
            .as_ref()
            .ok_or_else(|| EvidenceError::SuccessorOrigin {
                detail: "successor checkout is missing its predecessor identity".to_string(),
            })?;
        if parent.generation() == 0
            || predecessor.campaign_id() != parent.campaign_id()
            || predecessor.generation().checked_add(1) != Some(parent.generation())
            || parent.previous_parent_id() != Some(predecessor.parent_id())
            || parent.parent_node_id() != Some(predecessor.node_id())
            || self.invocation.role != Role::Successor
            || self.invocation.campaign_id != *parent.campaign_id()
            || self.invocation.node_id != parent.node_id()
            || self.invocation.run_profile.as_ref() != Some(profile)
            || root.is_none_or(|root| !same_path(root, repo_root))
            || self.checkout.campaign_id != *parent.campaign_id()
            || self.checkout.selected_parent_identity != *parent
            || !same_path(&self.checkout.active_parent_root, repo_root)
            || branch.is_some_and(|branch| branch != self.checkout.selected_branch)
            || self.spawned.campaign_id != *parent.campaign_id()
            || self.spawned.node_id != parent.node_id()
            || self.spawned.runtime_id != Some(self.invocation.runtime_id)
        {
            return Err(EvidenceError::SuccessorOrigin {
                detail: "successor invocation, checkout, runtime, parent generation, or profile coordinates do not match"
                    .to_string(),
            });
        }
        let successor::State::Spawned {
            active_parent_root,
            invocation_path,
            ..
        } = &self.spawned.state
        else {
            return Err(EvidenceError::SuccessorOrigin {
                detail: "successor origin requires the exact Spawned runtime record".to_string(),
            });
        };
        if !same_path(active_parent_root, repo_root)
            || !same_path(invocation_path, &self.invocation_path)
        {
            return Err(EvidenceError::SuccessorOrigin {
                detail: "successor Spawned paths do not match the invocation origin".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SuccessorEvidence<'a> {
    schema_version: &'static str,
    origin: &'a SuccessorOrigin,
    parent: &'a ParentIdentity,
    profile: &'a RunProfileCommitment,
    phase: WalkPhase,
}

/// Full preimage of one committed controller cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorEvidence {
    schema_version: String,
    session_id: SessionId,
    parent: ParentIdentity,
    profile: RunProfileCommitment,
    graph_version: String,
    prior: Cursor,
    intent: AttemptIntent,
    edge: ControlEdge,
    witness: ContentHash,
    epoch: EpochReceipt,
}

impl CursorEvidence {
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn graph_version(&self) -> &str {
        &self.graph_version
    }

    pub fn intent(&self) -> &AttemptIntent {
        &self.intent
    }

    pub fn epoch(&self) -> &EpochReceipt {
        &self.epoch
    }

    pub(crate) fn new(
        session_id: SessionId,
        parent: &ParentIdentity,
        profile: &RunProfileCommitment,
        intent: &AttemptIntent,
        target: WalkPhase,
        witness: &ContentHash,
        epoch: &EpochReceipt,
    ) -> Result<Self, EvidenceError> {
        let edge = ControlEdge::for_graph(
            &intent.epoch.transition_graph_version,
            intent.expected,
            target,
        )
        .ok_or(EvidenceError::UnknownEdge {
            graph: intent.epoch.transition_graph_version.clone(),
            from: intent.expected,
            to: target,
        })?;
        let evidence = Self {
            schema_version: EVIDENCE_SCHEMA.to_string(),
            session_id,
            parent: parent.clone(),
            profile: profile.clone(),
            graph_version: intent.epoch.transition_graph_version.clone(),
            prior: Cursor {
                phase: intent.expected,
                evidence: intent.evidence.clone(),
            },
            intent: intent.clone(),
            edge,
            witness: witness.clone(),
            epoch: epoch.clone(),
        };
        evidence.validate(session_id, parent, profile, intent, target, epoch)?;
        Ok(evidence)
    }

    pub fn edge(&self) -> ControlEdge {
        self.edge
    }

    pub fn prior(&self) -> &Cursor {
        &self.prior
    }

    pub fn witness(&self) -> &ContentHash {
        &self.witness
    }

    pub(crate) fn cursor(&self) -> Result<Cursor, EvidenceError> {
        Ok(Cursor {
            phase: self.edge.to(),
            evidence: hash_json(self)?,
        })
    }

    pub(crate) fn validate(
        &self,
        session_id: SessionId,
        parent: &ParentIdentity,
        profile: &RunProfileCommitment,
        intent: &AttemptIntent,
        target: WalkPhase,
        epoch: &EpochReceipt,
    ) -> Result<(), EvidenceError> {
        if self.schema_version != EVIDENCE_SCHEMA {
            return Err(EvidenceError::Schema {
                actual: self.schema_version.clone(),
            });
        }
        if self.session_id != session_id {
            return Err(EvidenceError::Session);
        }
        if &self.parent != parent {
            return Err(EvidenceError::Parent);
        }
        if &self.profile != profile {
            return Err(EvidenceError::Profile);
        }
        if self.graph_version != intent.epoch.transition_graph_version {
            return Err(EvidenceError::Graph);
        }
        if &self.intent != intent {
            return Err(EvidenceError::Intent);
        }
        if &self.epoch != epoch || self.epoch.before != intent.epoch {
            return Err(EvidenceError::Epoch);
        }
        let expected_prior = Cursor {
            phase: intent.expected,
            evidence: intent.evidence.clone(),
        };
        if self.prior != expected_prior {
            return Err(EvidenceError::Prior);
        }
        let expected_edge = ControlEdge::for_graph(&self.graph_version, intent.expected, target)
            .ok_or(EvidenceError::UnknownEdge {
                graph: self.graph_version.clone(),
                from: intent.expected,
                to: target,
            })?;
        if self.edge != expected_edge {
            return Err(EvidenceError::Edge);
        }
        if !intent.targets.contains(&target) {
            return Err(EvidenceError::Target { target });
        }
        if Cursor::new(target, self.witness.clone()).is_err() {
            return Err(EvidenceError::Witness);
        }
        if self.edge.requires_live() && !intent.allow_live_api {
            return Err(EvidenceError::Live { edge: self.edge });
        }
        if self.edge.requires_checkout() && !intent.allow_git_changes {
            return Err(EvidenceError::Checkout { edge: self.edge });
        }
        Ok(())
    }
}

pub(crate) fn initial_cursor(
    setup: &Prototype1SetupAdmission,
    parent: &ParentIdentity,
    profile: &RunProfileCommitment,
) -> Result<Cursor, EvidenceError> {
    setup.validate().map_err(|error| EvidenceError::Setup {
        detail: error.to_string(),
    })?;
    if setup.completed_head().is_none() {
        return Err(EvidenceError::SetupIncomplete);
    }
    if setup.intent.campaign_id != *parent.campaign_id() {
        return Err(EvidenceError::Campaign {
            setup: setup.intent.campaign_id.clone(),
            parent: parent.campaign_id().clone(),
        });
    }
    if &setup.intent.identity != parent {
        return Err(EvidenceError::SetupParent {
            setup: setup.intent.identity.node_id().to_string(),
            parent: parent.node_id().to_string(),
        });
    }
    if setup.intent.hashes.profile.0 != profile.sha256 {
        return Err(EvidenceError::Profile);
    }
    let preimage = InitialEvidence {
        schema_version: INITIAL_SCHEMA,
        setup,
        parent,
        profile,
        phase: WalkPhase::R3,
    };
    Ok(Cursor {
        phase: WalkPhase::R3,
        evidence: hash_json(&preimage)?,
    })
}

pub(crate) fn successor_cursor(
    origin: &SuccessorOrigin,
    parent: &ParentIdentity,
    profile: &RunProfileCommitment,
    repo_root: &Path,
) -> Result<Cursor, EvidenceError> {
    origin.validate(parent, profile, repo_root)?;
    let preimage = SuccessorEvidence {
        schema_version: SUCCESSOR_SCHEMA,
        origin,
        parent,
        profile,
        phase: WalkPhase::R3,
    };
    Ok(Cursor {
        phase: WalkPhase::R3,
        evidence: hash_json(&preimage)?,
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn hash_json<T: Serialize>(value: &T) -> Result<ContentHash, EvidenceError> {
    let bytes = serde_json::to_vec(value).map_err(EvidenceError::Serialize)?;
    Ok(ContentHash(format!("{:x}", Sha256::digest(bytes))))
}

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error("controller evidence schema '{actual}' is unsupported")]
    Schema { actual: String },
    #[error("controller evidence belongs to a different session")]
    Session,
    #[error("controller evidence belongs to a different parent")]
    Parent,
    #[error("controller evidence belongs to a different admitted profile")]
    Profile,
    #[error("controller evidence belongs to a different transition graph")]
    Graph,
    #[error("controller evidence does not contain the exact admitted transition intent")]
    Intent,
    #[error("controller evidence does not contain the exact observed transition epoch")]
    Epoch,
    #[error("controller evidence does not continue the admitted prior cursor")]
    Prior,
    #[error("controller evidence names a different semantic edge")]
    Edge,
    #[error("transition graph '{graph}' has no retained edge from {from} to {to}")]
    UnknownEdge {
        graph: String,
        from: WalkPhase,
        to: WalkPhase,
    },
    #[error("transition intent did not admit target {target}")]
    Target { target: WalkPhase },
    #[error("transition execution witness is not a canonical SHA-256 digest")]
    Witness,
    #[error("edge {edge} requires live-provider authority")]
    Live { edge: ControlEdge },
    #[error("edge {edge} requires checkout-mutation authority")]
    Checkout { edge: ControlEdge },
    #[error("completed setup authority is required for the initial cursor")]
    SetupIncomplete,
    #[error("successor controller origin is invalid: {detail}")]
    SuccessorOrigin { detail: String },
    #[error("setup authority is invalid: {detail}")]
    Setup { detail: String },
    #[error("setup campaign '{setup}' does not match parent campaign '{parent}'")]
    Campaign {
        setup: CampaignId,
        parent: CampaignId,
    },
    #[error("setup parent '{setup}' does not match controller parent '{parent}'")]
    SetupParent { setup: String, parent: String },
    #[error("failed to serialize canonical controller evidence: {0}")]
    Serialize(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ploke_records::{identity::ParentIdentityRecord, ids::CampaignId};
    use uuid::Uuid;

    use super::*;
    use crate::cli::prototype1_state::{
        edge::GRAPH_VERSION_V2,
        event::TransitionId,
        walk::epoch::{ServerEpoch, WALK_PROTOCOL_VERSION},
    };

    #[test]
    fn certificate_has_golden() {
        let session_id =
            serde_json::from_str::<SessionId>("\"12345678-1234-5678-9234-567812345678\"")
                .expect("fixed session id");
        let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: CampaignId::from("campaign-golden"),
            parent_id: "node-parent".to_string(),
            node_id: "node-parent".to_string(),
            generation: 3,
            instance_id: Some("instance-golden".to_string()),
            previous_parent_id: Some("node-prior".to_string()),
            parent_node_id: Some("node-prior".to_string()),
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("prototype1-parent-golden-gen3".to_string()),
            created_at: "2026-07-13T12:34:56Z".to_string(),
        });
        let profile = RunProfileCommitment {
            schema_version: "prototype1-run-profile-commitment.v1".to_string(),
            profile_path: PathBuf::from("/campaign/prototype1/run-profile.toml"),
            sha256: "a".repeat(64),
            source_path: Some(PathBuf::from("/operator/golden.toml")),
            admitted_at: "2026-07-13T12:30:00Z".to_string(),
        };
        let epoch = ServerEpoch {
            protocol_version: WALK_PROTOCOL_VERSION,
            transition_graph_version: GRAPH_VERSION_V2.to_string(),
            repo_root: PathBuf::from("/repo/golden"),
            exe_path: PathBuf::from("/repo/golden/target/debug/ploke-eval"),
            exe_modified_unix_ms: Some(1_784_000_000_123),
            git_head: Some("b".repeat(40)),
            active_branch: Some("prototype1-parent-golden-gen3".to_string()),
            source_status_hash: Some("c".repeat(64)),
        };
        let intent = AttemptIntent {
            transition_id: TransitionId(
                Uuid::parse_str("87654321-4321-8765-8321-876543218765")
                    .expect("fixed transition id"),
            ),
            expected: WalkPhase::R7,
            targets: vec![WalkPhase::R8],
            allow_live_api: true,
            allow_git_changes: false,
            epoch: epoch.clone(),
            evidence: ContentHash("d".repeat(64)),
            retry: 0,
        };
        let receipt = EpochReceipt {
            before: epoch.clone(),
            after: Some(epoch),
        };
        let certificate = CursorEvidence::new(
            session_id,
            &parent,
            &profile,
            &intent,
            WalkPhase::R8,
            &ContentHash("e".repeat(64)),
            &receipt,
        )
        .expect("golden certificate");

        assert_eq!(
            certificate.cursor().expect("golden cursor").evidence.0,
            "b330335294f431f98dbcf9016cdec34334457094b72c7c94416098730ab101a4"
        );
    }
}
