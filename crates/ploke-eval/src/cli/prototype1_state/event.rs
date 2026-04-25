//! Shared committed-event vocabulary for typed prototype1 transitions.
//!
//! Temporary note:
//! These carriers belong to the new journal/event layer even while some
//! transitions still route through the legacy artifact-apply implementation.
//! The goal is to preserve the new event vocabulary as the stable record shape
//! while the old transition machinery is replaced underneath it.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::intervention::Prototype1NodeStatus;

/// Durable identity for one committed transition attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct TransitionId(pub Uuid);

impl TransitionId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for TransitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Durable identity for one concrete child runtime instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RuntimeId(pub Uuid);

impl RuntimeId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RuntimeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Durable machine-readable timestamp for one journal entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RecordedAt(pub i64);

impl RecordedAt {
    pub(crate) fn now() -> Self {
        Self(Utc::now().timestamp_millis())
    }
}

/// Shared lineage marker for artifact/binary relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LineageMark {
    Parent,
    Child,
}

/// Stable content hash witness for artifact state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ContentHash(pub String);

impl ContentHash {
    // TODO(2026-04-26): Replace the string-backed hex carrier with a fixed-size
    // digest representation once the typed prototype-state scaffold is wired
    // into the live controller. This currently favors journal readability and
    // serde convenience, but it is not ownership-honest: hashes want value
    // semantics and should ideally be `Copy`, which would also remove a lot of
    // avoidable clone churn in `c1.rs` / `c2.rs`.
    pub(crate) fn of(payload: &str) -> Self {
        Self(format!("{:x}", Sha256::digest(payload.as_bytes())))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Identity-bearing references attached to one recorded transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Refs {
    pub campaign_id: String,
    pub node_id: String,
    pub instance_id: String,
    pub source_state_id: String,
    pub branch_id: String,
    pub candidate_id: String,
    pub branch_label: String,
    pub spec_id: String,
}

/// Path-bearing context attached to one recorded transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Paths {
    pub repo_root: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    pub target_relpath: PathBuf,
    pub absolute_path: PathBuf,
}

/// World/configuration relation captured at one transition boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct World {
    pub node_status: Prototype1NodeStatus,
    pub running_binary: bool,
    pub running_lineage: LineageMark,
    pub artifact_lineage: LineageMark,
    pub child_binary_present: bool,
    pub child_running: bool,
}

/// Hash witnesses captured around one transition boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Hashes {
    pub source: ContentHash,
    pub current: ContentHash,
    pub proposed: ContentHash,
}
