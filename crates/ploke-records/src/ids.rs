//! Passive identifiers and coordinates used by record DTOs.
//!
//! These wrappers intentionally validate nothing. They preserve serialized
//! shape and field meaning while authority-bearing interpretation remains in
//! `ploke-eval`.

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
    };
}

string_id! {
    /// Durable identity for one concrete runtime instance.
    RuntimeId
}

impl std::fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

string_id! {
    /// Backend-neutral identity for a recoverable artifact state.
    ArtifactId
}

string_id! {
    /// Identity for one generated or composed patch record.
    PatchId
}

string_id! {
    /// Durable identity for one committed transition attempt.
    TransitionId
}

string_id! {
    /// Stable content hash witness as serialized by upstream records.
    ContentHash
}

string_id! {
    /// History block identity as it appears in persisted records.
    BlockId
}

string_id! {
    /// History entry identity as it appears in persisted records.
    EntryId
}

string_id! {
    /// Lineage identity for a local History chain.
    LineageId
}

string_id! {
    /// Domain hash used for History payloads and roots.
    HistoryHash
}

string_id! {
    /// Serialized sealed block hash.
    BlockHash
}

string_id! {
    /// Serialized local History state root.
    HistoryStateRoot
}

/// Durable machine-readable timestamp for one recorded boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordedAt(pub i64);

/// Target operated over by a runtime or deterministic composition step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum OperationTarget {
    /// One runtime operates over one durable artifact surface.
    Artifact { artifact_id: ArtifactId },
    /// One runtime or composer operates over patches with a shared base.
    PatchSet {
        base_artifact_id: ArtifactId,
        patch_ids: Vec<PatchId>,
    },
    /// One runtime or merge strategy operates over derived artifacts.
    ArtifactSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_artifact_id: Option<ArtifactId>,
        artifact_ids: Vec<ArtifactId>,
    },
}

/// Runtime plus target coordinate for one generative or compositional action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coordinate {
    pub runtime_id: RuntimeId,
    pub target: OperationTarget,
}
