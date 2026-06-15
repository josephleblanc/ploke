//! Passive invocation and successor-result records.
//!
//! These records describe persisted process bootstrap and result evidence. They
//! do not classify executable authority or construct child/successor runtime
//! handles.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::branch::ResolvedTreatmentBranch;
use crate::ids::{CampaignId, RuntimeId};
use crate::scheduler::{NodeRecord, RunnerRequestRecord};

/// Durable schema version for runtime invocation records.
pub const INVOCATION_SCHEMA_VERSION: &str = "prototype1-invocation.v1";

/// Durable schema version for successor-ready acknowledgements.
pub const SUCCESSOR_READY_SCHEMA_VERSION: &str = "prototype1-successor-ready.v1";

/// Durable schema version for successor completion records.
pub const SUCCESSOR_COMPLETION_SCHEMA_VERSION: &str = "prototype1-successor-completion.v1";

/// Runtime role recorded for one invocation attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Leaf evaluator child.
    Child,
    /// Selected continuation runtime for bounded successor bootstrap.
    Successor,
}

/// Raw persisted bootstrap record for one prototype1 runtime attempt.
///
/// These payloads are passive typed records. Deserializing them does not create
/// a runnable child/successor or grant authority to execute the loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvocationRecord {
    pub schema_version: String,
    pub role: Role,
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub journal_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<RunnerRequestRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedTreatmentBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_parent_root: Option<PathBuf>,
    pub created_at: String,
}

/// Successor-ready acknowledgement written by a detached successor runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuccessorReadyRecord {
    pub schema_version: String,
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub pid: u32,
    pub recorded_at: String,
}

/// Terminal status for one successor rehydration attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuccessorCompletionStatus {
    /// The successor completed one bounded rehydrated controller generation.
    Succeeded,
    /// The successor acknowledged handoff but failed during rehydration.
    Failed,
}

/// Completion record written after a successor attempts controller rehydration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuccessorCompletionRecord {
    pub schema_version: String,
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub status: SuccessorCompletionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub recorded_at: String,
}
