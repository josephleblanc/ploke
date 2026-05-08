//! Passive support records for transition journals.
//!
//! This module contains field groups and small enums used by persisted journal
//! entries. It intentionally provides no append, replay, recovery, filesystem,
//! or scheduler APIs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::identity::ParentIdentityRecord;
use crate::ids::{ContentHash, RecordedAt, RuntimeId, TransitionId};

/// Shared lineage marker for artifact/binary relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageMark {
    Parent,
    Child,
}

/// Closed lifecycle carrier for one child runtime after the child binary exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildRuntimeLifecycle {
    Built,
    Spawned,
    Acknowledged,
    Terminated,
}

/// Closed terminal observation carrier for one observed child evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedChildTerminal {
    Succeeded,
    Failed,
}

/// Identity-bearing references attached to one recorded transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refs {
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
pub struct Paths {
    pub repo_root: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    pub target_relpath: PathBuf,
    pub absolute_path: PathBuf,
}

/// World/configuration relation captured at one transition boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub node_status: String,
    pub running_binary: bool,
    pub running_lineage: LineageMark,
    pub artifact_lineage: LineageMark,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_lifecycle: Option<ChildRuntimeLifecycle>,
}

/// Hash witnesses captured around one transition boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hashes {
    pub source: ContentHash,
    pub current: ContentHash,
    pub proposed: ContentHash,
}

/// Machine-readable detail for a rejected build transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureDetail {
    pub exit_code: Option<i32>,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
}

/// Committed result for a build transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildResultRecord {
    Built,
    CheckFailed(FailureDetail),
    BuildFailed(FailureDetail),
}

/// Parent-side runtime handoff phase.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpawnPhase {
    Starting,
    Spawned,
    Observed,
}

/// Parent-side observation of a spawn-and-handshake transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpawnObservationRecord {
    Acknowledged,
    TerminatedBeforeAcknowledged { exit_code: Option<i32> },
    ReadyTimedOut { waited_ms: u64 },
}

/// Files receiving stdout and stderr for a spawned process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Streams {
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

/// Machine-readable journal entry for materialization-style transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitionRecord {
    pub transition_id: TransitionId,
    pub phase: String,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub world: World,
    pub hashes: Hashes,
}

/// Machine-readable journal entry for build transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildRecord {
    pub transition_id: TransitionId,
    pub phase: String,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub world: World,
    pub hashes: Hashes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<BuildResultRecord>,
}

/// Machine-readable journal entry for child spawn observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnRecord {
    pub runtime_id: RuntimeId,
    pub phase: SpawnPhase,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub world: World,
    pub child_lifecycle: ChildRuntimeLifecycle,
    pub parent_pid: u32,
    pub child_pid: Option<u32>,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streams: Option<Streams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SpawnObservationRecord>,
}

/// Child-side handshake witness for one spawned runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadyRecord {
    pub runtime_id: RuntimeId,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub pid: u32,
}

/// Parent runtime start record for one typed-loop turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParentStartedRecord {
    pub recorded_at: RecordedAt,
    pub campaign_id: String,
    pub parent_identity: ParentIdentityRecord,
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_runtime_id: Option<RuntimeId>,
    pub pid: u32,
}
