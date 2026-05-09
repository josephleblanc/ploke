//! Passive runtime channel record DTOs.
//!
//! These records mirror the serialized parent/child channel envelopes. They do
//! not define endpoints, send or receive bytes, validate cursors, or verify
//! body hashes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::evaluation::{EvalSet, Evaluator, InstanceComparison};
use crate::ids::{RecordedAt, RuntimeId};
use crate::invocation::{SuccessorCompletionRecord, SuccessorReadyRecord};
use crate::scheduler::RunnerResultRecord;

/// Durable schema version used by current runtime channel envelopes.
pub const CHANNEL_SCHEMA_VERSION: &str = "prototype1-runtime-channel.v1";

/// Direction of one serialized channel envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Parent wrote this message for the child.
    ParentToChild,
    /// Child wrote this message for the parent.
    ChildToParent,
}

/// Passive byte offset into a transport stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cursor {
    pub offset: u64,
}

/// Serialized channel record with runtime identity and payload hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<M> {
    pub schema_version: String,
    pub direction: Direction,
    pub campaign_id: String,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub message_id: String,
    pub recorded_at: RecordedAt,
    pub body_hash: String,
    pub body: M,
}

/// Parent-to-child protocol messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToChild {
    /// Parent asks this child runtime to stop.
    Cancel { reason: String },
}

/// Child-to-parent protocol messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToParent {
    /// Child runtime has started and can be observed.
    Ready,
    /// Child runtime entered evaluation.
    Evaluating,
    /// Child completed execution and returned its terminal payload.
    Result {
        /// Attempt-scoped runner result produced by the child runtime.
        runner_result: RunnerResultRecord,
        /// Branch evaluation report when the child completed evaluation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evaluation: Option<EvaluationReport>,
    },
    /// Child persisted its attempt-scoped runner result.
    ResultWritten { runner_result_path: PathBuf },
    /// Successor runtime acknowledged bootstrap.
    SuccessorReady { record: SuccessorReadyRecord },
    /// Successor runtime completed its bounded controller turn.
    SuccessorCompletion { record: SuccessorCompletionRecord },
    /// Child failed before writing a normal terminal result.
    Failed { detail: String },
    /// Child process exited.
    Exited { status: Option<i32> },
}

/// Branch evaluation report payload exchanged over child-to-parent channels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationReport {
    pub baseline_campaign_id: String,
    pub branch_id: String,
    pub treatment_campaign_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_procedure_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator_identity: Option<Evaluator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_set_identity: Option<EvalSet>,
    pub branch_registry_path: PathBuf,
    pub evaluation_artifact_path: PathBuf,
    pub treatment_campaign_manifest: PathBuf,
    pub treatment_closure_state_path: PathBuf,
    pub overall_disposition: crate::branch::Disposition,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub compared_instances: Vec<InstanceComparison>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn roundtrips_child_to_parent_envelope() {
        let json = serde_json::json!({
            "schema_version": CHANNEL_SCHEMA_VERSION,
            "direction": "child_to_parent",
            "campaign_id": "campaign-1",
            "node_id": "node-1",
            "runtime_id": "11111111-1111-1111-1111-111111111111",
            "message_id": "22222222-2222-2222-2222-222222222222",
            "recorded_at": 1770000000000i64,
            "body_hash": "abc123",
            "body": {
                "result_written": {
                    "runner_result_path": "nodes/node-1/result.json"
                }
            }
        });

        let record: Envelope<ToParent> = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    fn roundtrips_parent_to_child_envelope() {
        let json = serde_json::json!({
            "schema_version": CHANNEL_SCHEMA_VERSION,
            "direction": "parent_to_child",
            "campaign_id": "campaign-1",
            "node_id": "node-1",
            "runtime_id": "11111111-1111-1111-1111-111111111111",
            "message_id": "22222222-2222-2222-2222-222222222222",
            "recorded_at": 1770000000000i64,
            "body_hash": "abc123",
            "body": {
                "cancel": {
                    "reason": "operator request"
                }
            }
        });

        let record: Envelope<ToChild> = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    #[ignore]
    fn real_campaign_channel_jsonl_parses_representative_child_result() {
        let path = real_run_root()
            .join("nodes")
            .join("node-b3ed41bd152ed715")
            .join("channels")
            .join("d77d2e42-578d-4714-bde8-21fd056e7243")
            .join("child-to-parent.jsonl");
        let file = fs::File::open(&path).unwrap_or_else(|err| {
            panic!(
                "open real-run channel {} (set PLOKE_RECORDS_REAL_RUN_ROOT to override): {err}",
                path.display()
            )
        });

        let mut count = 0usize;
        let mut saw_ready = false;
        let mut saw_result = false;
        for line in BufReader::new(file).lines() {
            let line = line.expect("read channel line");
            if line.trim().is_empty() {
                continue;
            }

            let envelope: Envelope<ToParent> =
                serde_json::from_str(&line).expect("parse child-to-parent envelope");
            assert_eq!(
                envelope.campaign_id,
                "p1-edit-surface-history-long-20260508-1"
            );
            assert_eq!(envelope.node_id, "node-b3ed41bd152ed715");
            count += 1;
            match envelope.body {
                ToParent::Ready => saw_ready = true,
                ToParent::Result {
                    runner_result,
                    evaluation,
                } => {
                    saw_result = true;
                    assert_eq!(runner_result.branch_id.as_str(), "branch-d5aff05ddadfc372");
                    assert!(evaluation.is_some());
                }
                _ => {}
            }
        }

        assert_eq!(count, 3);
        assert!(saw_ready);
        assert!(saw_result);
    }

    fn real_run_root() -> PathBuf {
        std::env::var_os("PLOKE_RECORDS_REAL_RUN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    "/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1",
                )
            })
    }
}
