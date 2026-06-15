//! Passive support records for transition journals.
//!
//! This module contains field groups and small enums used by persisted journal
//! entries. It intentionally provides no append, replay, recovery, filesystem,
//! or scheduler APIs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::identity::ParentIdentityRecord;
use crate::ids::{ContentHash, RecordedAt, RuntimeId, TransitionId};
use crate::scheduler::{ContinuationDecisionRecord, RunnerDispositionRecord};
use crate::selection::Decision;

/// Append-only journal phase for one committed transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitPhase {
    Before,
    After,
}

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
    pub campaign_id: CampaignId,
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
    pub campaign_id: CampaignId,
    pub parent_identity: ParentIdentityRecord,
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_runtime_id: Option<RuntimeId>,
    pub pid: u32,
}

/// Committed result for parent-side observation of one terminal child state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservedChildResultRecord {
    TreatmentComplete {
        treatment_campaign_id: String,
    },
    Succeeded {
        evaluation_artifact_path: PathBuf,
        overall_disposition: crate::branch::Disposition,
    },
    Failed {
        disposition: RunnerDispositionRecord,
        detail: Option<String>,
        exit_code: Option<i32>,
    },
}

/// Machine-readable journal entry for parent-side observation of a child result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionRecord {
    pub transition_id: TransitionId,
    pub runtime_id: RuntimeId,
    pub phase: CommitPhase,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub world: World,
    pub child_lifecycle: ChildRuntimeLifecycle,
    pub runner_result_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ObservedChildResultRecord>,
}

/// Operational resource observations emitted by a runtime.
pub mod resource {
    use super::*;

    /// Resource being measured.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Subject {
        CargoTarget,
    }

    /// Parent-turn boundary at which the resource was measured.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Phase {
        ParentStart,
        ParentComplete,
    }

    /// Measurement outcome.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Status {
        Measured,
        Missing,
        Failed,
    }

    /// Machine-readable resource sample for operational introspection.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Sample {
        pub recorded_at: RecordedAt,
        pub campaign_id: CampaignId,
        pub parent_id: String,
        pub node_id: String,
        pub generation: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub runtime_id: Option<RuntimeId>,
        pub subject: Subject,
        pub phase: Phase,
        pub path: PathBuf,
        pub status: Status,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bytes: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
    }
}

/// Legacy storage label for a durable child artifact commit record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildArtifactCommittedRecord {
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub parent_identity: Option<ParentIdentityRecord>,
    pub child_identity: ParentIdentityRecord,
    pub node_id: String,
    pub generation: u32,
    pub target_relpath: PathBuf,
    pub child_branch: String,
    pub target_commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_commit: Option<String>,
}

/// Legacy storage label for active checkout advancement in the parent handoff path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveCheckoutAdvancedRecord {
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub previous_parent_identity: Option<ParentIdentityRecord>,
    pub selected_parent_identity: ParentIdentityRecord,
    pub active_parent_root: PathBuf,
    pub selected_branch: String,
    pub installed_commit: String,
}

/// Legacy storage label for successor handoff acknowledgement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuccessorHandoffRecord {
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub active_parent_root: PathBuf,
    pub binary_path: PathBuf,
    pub invocation_path: PathBuf,
    pub ready_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streams: Option<Streams>,
    pub pid: u32,
}

/// State projected by a recorded child transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChildStateRecord {
    Ready,
    Evaluating,
    ResultWritten { runner_result_path: PathBuf },
}

/// Durable record written by a typed child transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildRecord {
    pub runtime_id: RuntimeId,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub pid: u32,
    pub state: ChildStateRecord,
}

/// State projected by a recorded successor transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SuccessorStateRecord {
    Selected {
        decision: ContinuationDecisionRecord,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selection_decision: Option<Decision>,
    },
    Spawned {
        pid: u32,
        active_parent_root: PathBuf,
        binary_path: PathBuf,
        invocation_path: PathBuf,
        ready_path: PathBuf,
        streams: Streams,
    },
    Checkout {
        phase: CommitPhase,
        active_parent_root: PathBuf,
        selected_branch: String,
        installed_commit: Option<String>,
    },
    Ready {
        pid: u32,
        ready_path: PathBuf,
    },
    TimedOut {
        waited_ms: u64,
        ready_path: PathBuf,
    },
    ExitedBeforeReady {
        exit_code: Option<i32>,
    },
    Completed {
        status: crate::invocation::SuccessorCompletionStatus,
        completion_path: PathBuf,
        trace_path: Option<PathBuf>,
        detail: Option<String>,
    },
}

/// Durable record written by a typed successor transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuccessorRecord {
    pub runtime_id: Option<RuntimeId>,
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub state: SuccessorStateRecord,
}

/// Single append-only passive journal entry for prototype1 transition records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JournalEntry {
    ParentStarted(ParentStartedRecord),
    Resource(resource::Sample),
    ChildArtifactCommitted(ChildArtifactCommittedRecord),
    ActiveCheckoutAdvanced(ActiveCheckoutAdvancedRecord),
    SuccessorHandoff(SuccessorHandoffRecord),
    Successor(SuccessorRecord),
    MaterializeBranch(TransitionRecord),
    BuildChild(BuildRecord),
    SpawnChild(SpawnRecord),
    Child(ChildRecord),
    ChildReady(ReadyRecord),
    ObserveChild(CompletionRecord),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;

    use super::*;

    fn refs_json() -> serde_json::Value {
        serde_json::json!({
            "campaign_id": "campaign-1",
            "node_id": "node-1",
            "instance_id": "instance-1",
            "source_state_id": "source-1",
            "branch_id": "branch-1",
            "candidate_id": "candidate-1",
            "branch_label": "candidate 1",
            "spec_id": "spec-1"
        })
    }

    fn paths_json() -> serde_json::Value {
        serde_json::json!({
            "repo_root": "/repo",
            "workspace_root": "/repo/work",
            "binary_path": "/repo/target/debug/ploke",
            "target_relpath": "AGENTS.md",
            "absolute_path": "/repo/AGENTS.md"
        })
    }

    fn world_json() -> serde_json::Value {
        serde_json::json!({
            "node_status": "running",
            "running_binary": true,
            "running_lineage": "parent",
            "artifact_lineage": "child",
            "child_lifecycle": "spawned"
        })
    }

    fn hashes_json() -> serde_json::Value {
        serde_json::json!({
            "source": "hash-source",
            "current": "hash-current",
            "proposed": "hash-proposed"
        })
    }

    #[test]
    fn roundtrips_materialize_branch_entry() {
        let json = serde_json::json!({
            "kind": "materialize_branch",
            "transition_id": "33333333-3333-3333-3333-333333333333",
            "phase": "before",
            "recorded_at": 1770000000000i64,
            "generation": 1,
            "refs": refs_json(),
            "paths": paths_json(),
            "world": world_json(),
            "hashes": hashes_json()
        });

        let record: JournalEntry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    fn roundtrips_observe_child_entry() {
        let json = serde_json::json!({
            "kind": "observe_child",
            "transition_id": "33333333-3333-3333-3333-333333333333",
            "runtime_id": "11111111-1111-1111-1111-111111111111",
            "phase": "after",
            "recorded_at": 1770000000000i64,
            "generation": 1,
            "refs": refs_json(),
            "paths": paths_json(),
            "world": world_json(),
            "child_lifecycle": "terminated",
            "runner_result_path": "nodes/node-1/result.json",
            "result": {
                "succeeded": {
                    "evaluation_artifact_path": "eval/report.json",
                    "overall_disposition": "keep"
                }
            }
        });

        let record: JournalEntry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    fn roundtrips_observe_child_treatment_complete_entry() {
        let json = serde_json::json!({
            "kind": "observe_child",
            "transition_id": "33333333-3333-3333-3333-333333333333",
            "runtime_id": "11111111-1111-1111-1111-111111111111",
            "phase": "after",
            "recorded_at": 1770000000000i64,
            "generation": 1,
            "refs": refs_json(),
            "paths": paths_json(),
            "world": world_json(),
            "child_lifecycle": "terminated",
            "runner_result_path": "nodes/node-1/result.json",
            "result": {
                "treatment_complete": {
                    "treatment_campaign_id": "campaign-treatment-1"
                }
            }
        });

        let record: JournalEntry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    fn roundtrips_child_record_entry() {
        let json = serde_json::json!({
            "kind": "child",
            "runtime_id": "11111111-1111-1111-1111-111111111111",
            "recorded_at": 1770000000000i64,
            "generation": 1,
            "refs": refs_json(),
            "paths": paths_json(),
            "pid": 1234,
            "state": {
                "result_written": {
                    "runner_result_path": "nodes/node-1/result.json"
                }
            }
        });

        let record: JournalEntry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    #[ignore]
    fn real_campaign_transition_journal_parses_representative_entries() {
        let path = real_run_root().join("transition-journal.jsonl");
        let file = fs::File::open(&path).unwrap_or_else(|err| {
            panic!(
                "open real-run journal {} (set PLOKE_RECORDS_REAL_RUN_ROOT to override): {err}",
                path.display()
            )
        });

        let mut count = 0usize;
        let mut parent_started = None;
        let mut observed_child_count = 0usize;
        for line in BufReader::new(file).lines() {
            let line = line.expect("read journal line");
            if line.trim().is_empty() {
                continue;
            }

            let entry: JournalEntry = serde_json::from_str(&line).expect("parse journal line");
            count += 1;
            match entry {
                JournalEntry::ParentStarted(record) if parent_started.is_none() => {
                    parent_started = Some(record);
                }
                JournalEntry::ObserveChild(_) => observed_child_count += 1,
                _ => {}
            }
        }

        let parent_started = parent_started.expect("parent_started entry");
        assert!(!parent_started.campaign_id.is_empty());
        assert!(!parent_started.parent_identity.node_id.is_empty());
        assert!(count > 0);
        assert!(observed_child_count > 0);
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
