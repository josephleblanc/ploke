#![allow(dead_code)]
// REMOVE BY 2026-04-26: prototype transition journal scaffold is not wired into the live controller yet

//! Append-only journal and replay helpers for typed prototype1 transitions.
//!
//! Temporary note:
//! This journal is the new committed-event seam for the typed prototype-state
//! scaffold, but it is still fed by transitions that may delegate part of
//! their work to the legacy artifact-apply path.
//!
//! When the old implementation is replaced, this journal should remain the
//! durable event stream while the transition producers stop depending on the
//! legacy `Intervention*` artifact-mutation layer.
//!
//! Journal discipline:
//! Every transition family recorded in [`JournalEntry`] should also have a
//! replay classifier here so restart/recovery semantics stay symmetric with the
//! forward transition contract.
//!
//! Naming and migration discipline:
//! Several older records in this file have flattened names such as
//! `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, and
//! `SuccessorHandoffEntry`. Those names are legacy storage labels, not the
//! intended semantic shape for new protocol code. They compress role, state,
//! object, and persistence mechanism into one identifier.
//!
//! New History-facing code should normalize these records into structural
//! carriers before admission, for example:
//!
//! ```text
//! Artifact<Committed> or artifact::Record { state: artifact::State::Committed }
//! Checkout<Advanced> or checkout::Record { state: checkout::State::Advanced }
//! Successor<Ready> or successor::Record { state: successor::State::Ready }
//! Parent<Started> or parent::Record { state: parent::State::Started }
//! ```
//!
//! Do not mirror the flattened `*Entry` names into History entry kinds. Import
//! them as legacy evidence, then project them into typed domain facts whose
//! structure lives in the module path, carrier type, and state parameter. Once
//! the structural import path is tested, these flattened records can be marked
//! deprecated to make the migration explicit without breaking replay at once.

use crate::prelude::*;

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

#[cfg(unix)]
use std::os::fd::AsRawFd;

use super::event::{
    ChildRuntimeLifecycle, ContentHash, Hashes, ObservedChildTerminal, Paths, RecordedAt, Refs,
    RuntimeId, TransitionId, World,
};
use super::identity::ParentIdentity;
use super::invocation::{ProcessIncarnation, record_runtime_id};
use super::profile::DEFAULT_OBSERVE_CHILD_STALE_AFTER_SECS;
use super::successor::{
    HandoffAcceptance, ReadyReceipt, Record as SuccessorRecord, State as SuccessorState,
};
use crate::branch_evaluation::BranchDisposition;
use crate::intervention::{
    CommitPhase, Prototype1RunnerDisposition, RecordStore, load_runner_result_at,
};
use crate::projection::OperatorProjectionRead;
use sha2::{Digest, Sha256};

pub(crate) const DEFAULT_OBSERVE_CHILD_STALE_AFTER: Duration =
    Duration::from_secs(DEFAULT_OBSERVE_CHILD_STALE_AFTER_SECS);

/// Append-only machine-readable journal entry for `C1 -> C2`
/// materialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Entry {
    pub transition_id: TransitionId,
    pub phase: CommitPhase,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub world: World,
    pub hashes: Hashes,
}

/// Machine-readable detail for a rejected build transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct FailureInfo {
    pub exit_code: Option<i32>,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
}

/// Committed result for the `C2 -> C3` build transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BuildResult {
    Built,
    CheckFailed(FailureInfo),
    BuildFailed(FailureInfo),
}

/// Append-only machine-readable journal entry for `C2 -> C3` build.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BuildEntry {
    pub transition_id: TransitionId,
    pub phase: CommitPhase,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub world: World,
    pub hashes: Hashes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<BuildResult>,
}

/// Parent-side runtime handoff phase for `C3 -> C4`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpawnPhase {
    Starting,
    Spawned,
    Observed,
}

/// Committed result for the `C3 -> C4` spawn-and-handshake transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpawnObservation {
    Acknowledged,
    TerminatedBeforeAcknowledged { exit_code: Option<i32> },
    ReadyTimedOut { waited_ms: u64 },
}

/// Append-only machine-readable journal entry for `C3 -> C4` child spawn and
/// observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SpawnEntry {
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
    /// Exact process identity for current-schema Spawned/Observed records.
    ///
    /// Legacy journal entries deserialize without this field so they remain
    /// inspectable, but they cannot authorize process cleanup or child launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<ProcessIncarnation>,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streams: Option<Streams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SpawnObservation>,
}

impl SpawnEntry {
    /// Return exact authority to signal the spawned process.
    ///
    /// `None` deliberately makes PID-only legacy replay read-only: a reused
    /// numeric PID must never become a cleanup target.
    pub(crate) fn cleanup_authority(&self) -> Option<(u32, &ProcessIncarnation)> {
        if self.phase != SpawnPhase::Spawned {
            return None;
        }
        Some((self.child_pid?, self.incarnation.as_ref()?))
    }

    /// Whether this is the exact durable Spawned receipt awaited by a child.
    pub(crate) fn matches_child(
        &self,
        runtime_id: RuntimeId,
        pid: u32,
        incarnation: &ProcessIncarnation,
    ) -> bool {
        self.runtime_id == runtime_id && self.cleanup_authority() == Some((pid, incarnation))
    }
}

/// Files receiving stdout and stderr for a spawned child process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Streams {
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

/// Child-side handshake witness for one spawned runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReadyEntry {
    pub runtime_id: RuntimeId,
    pub recorded_at: RecordedAt,
    pub generation: u32,
    pub refs: Refs,
    pub paths: Paths,
    pub pid: u32,
}

/// Committed result for parent-side observation of one terminal child state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservedChildResult {
    TreatmentComplete {
        treatment_campaign_id: CampaignId,
    },
    Succeeded {
        evaluation_artifact_path: PathBuf,
        overall_disposition: BranchDisposition,
    },
    Failed {
        disposition: Prototype1RunnerDisposition,
        detail: Option<String>,
        exit_code: Option<i32>,
    },
}

/// Append-only machine-readable journal entry for parent-side observation of
/// one child runtime's persisted evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CompletionEntry {
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
    pub result: Option<ObservedChildResult>,
}

/// Parent runtime start record for one typed-loop turn.
///
/// This is intentionally parent/artifact shaped rather than legacy
/// branch-registry shaped. It records the active identity a runtime used when
/// entering the typed parent path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ParentStartedEntry {
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub parent_identity: ParentIdentity,
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_runtime_id: Option<RuntimeId>,
    pub pid: u32,
}

/// Operational resource observations emitted by the runtime.
///
/// These records are telemetry, not History authority. They live in the shared
/// transition journal because they are cross-runtime observations that should
/// be easy to deserialize for long-horizon introspection.
pub(crate) mod resource {
    use super::*;

    /// Resource being measured.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum Subject {
        /// Cargo's build artifact directory for the active parent checkout.
        CargoTarget,
    }

    /// Parent-turn boundary at which the resource was measured.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum Phase {
        /// The parent has entered the typed parent path.
        ParentStart,
        /// The parent turn is returning a successful command result.
        ParentComplete,
    }

    /// Measurement outcome.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum Status {
        /// The resource existed and was measured.
        Measured,
        /// The resource path did not exist.
        Missing,
        /// The resource path existed but could not be fully measured.
        Failed,
    }

    /// Machine-readable resource sample for operational introspection.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Sample {
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

/// Legacy storage label for a durable child artifact commit record produced
/// before a child can be selected as successor.
///
/// History import should treat this as evidence for an artifact transition,
/// not as the semantic model itself. The intended normalized shape is an
/// `Artifact<Committed>`-style carrier or equivalent `artifact::Record` with a
/// committed state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ChildArtifactCommittedEntry {
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub parent_identity: Option<ParentIdentity>,
    pub child_identity: ParentIdentity,
    pub node_id: String,
    pub generation: u32,
    pub target_relpath: PathBuf,
    pub child_branch: String,
    pub target_commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_commit: Option<String>,
}

/// Legacy storage label for active checkout advancement in the parent handoff
/// path.
///
/// History import should normalize this into a checkout transition fact. The
/// semantic structure is `Checkout<Advanced>` or equivalent, not a
/// `ActiveCheckoutAdvancedEntry` concept carried forward as a History kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ActiveCheckoutAdvancedEntry {
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub previous_parent_identity: Option<ParentIdentity>,
    pub selected_parent_identity: ParentIdentity,
    pub active_parent_root: PathBuf,
    pub selected_branch: String,
    pub installed_commit: String,
}

/// Legacy storage label for successor handoff acknowledgement observed by the
/// previous parent.
///
/// History import should normalize this into successor and parent authority
/// facts. It is evidence about a handoff boundary, not the future
/// `Crown<Locked> -> Successor<Admitted> -> Parent<Ruling>` carrier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorHandoffEntry {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<HandoffAcceptance>,
}

/// Single append-only journal entry for typed prototype1 transitions.
///
/// This enum is a replay envelope. It intentionally preserves older storage
/// labels so existing JSONL can still be read, but new History code should not
/// treat each variant name as an ontology. Convert the variant into a
/// structural transition fact first, then propose that fact for History.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum JournalEntry {
    ParentStarted(ParentStartedEntry),
    Resource(resource::Sample),
    ChildArtifactCommitted(ChildArtifactCommittedEntry),
    ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry),
    SuccessorHandoff(SuccessorHandoffEntry),
    Successor(super::successor::Record),
    MaterializeBranch(Entry),
    BuildChild(BuildEntry),
    SpawnChild(SpawnEntry),
    Child(super::child::Record),
    ChildReady(ReadyEntry),
    ObserveChild(CompletionEntry),
}

/// Durable append-only JSONL journal for prototype1 transition records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrototypeJournal {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JournalAppendReceipt {
    pub(crate) path: PathBuf,
    pub(crate) source_event_index: usize,
    pub(crate) source_line: usize,
    pub(crate) byte_start: u64,
    /// Compact JSON payload byte length, excluding the trailing newline.
    pub(crate) byte_len: usize,
    pub(crate) content_sha256: String,
    pub(crate) payload_json: String,
}

impl PrototypeJournal {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn append_with_receipt(
        &mut self,
        entry: JournalEntry,
    ) -> Result<JournalAppendReceipt, PrototypeJournalError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrototypeJournalError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let source_event_index = count_lines(&self.path)?;
        let byte_start = match fs::metadata(&self.path) {
            Ok(metadata) => metadata.len(),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => 0,
            Err(source) => {
                return Err(PrototypeJournalError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };
        let payload_json =
            serde_json::to_string(&entry).map_err(PrototypeJournalError::Serialize)?;
        let byte_len = payload_json.len();
        let content_sha256 = sha256_hex(payload_json.as_bytes());

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| PrototypeJournalError::Open {
                path: self.path.clone(),
                source,
            })?;
        file.write_all(payload_json.as_bytes())
            .map_err(|source| PrototypeJournalError::Write {
                path: self.path.clone(),
                source,
            })?;
        file.write_all(b"\n")
            .map_err(|source| PrototypeJournalError::Write {
                path: self.path.clone(),
                source,
            })?;
        file.sync_data()
            .map_err(|source| PrototypeJournalError::Sync {
                path: self.path.clone(),
                source,
            })?;

        Ok(JournalAppendReceipt {
            path: self.path.clone(),
            source_event_index,
            source_line: source_event_index + 1,
            byte_start,
            byte_len,
            content_sha256,
            payload_json,
        })
    }

    pub(crate) fn load_entries(&self) -> Result<Vec<JournalEntry>, PrototypeJournalError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.path).map_err(|source| PrototypeJournalError::Read {
            path: self.path.clone(),
            source,
        })?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_number, line) in reader.lines().enumerate() {
            let line = line.map_err(|source| PrototypeJournalError::Read {
                path: self.path.clone(),
                source,
            })?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let entry = serde_json::from_str(trimmed).map_err(|source| {
                PrototypeJournalError::ParseLine {
                    path: self.path.clone(),
                    line_number: line_number + 1,
                    source,
                }
            })?;
            entries.push(entry);
        }

        Ok(entries)
    }

    pub(crate) fn replay_materialize_branch(
        &self,
    ) -> Result<Vec<MaterializeBranchReplay>, PrototypeJournalError> {
        let mut grouped = BTreeMap::<TransitionId, MaterializeBranchPhases>::new();

        for entry in self.load_entries()? {
            let JournalEntry::MaterializeBranch(entry) = entry else {
                continue;
            };
            grouped
                .entry(entry.transition_id)
                .or_default()
                .record(entry)?;
        }

        let mut replay = Vec::new();
        for (transition_id, phases) in grouped {
            match (phases.before, phases.after) {
                (Some(before), Some(after)) => {
                    replay.push(MaterializeBranchReplay {
                        before,
                        outcome: MaterializeBranchOutcome::Committed {
                            after: Box::new(after),
                        },
                    });
                }
                (Some(before), None) => {
                    let (observed_hash, disposition) =
                        classify_pending_materialization(&before.paths.absolute_path, &before)?;
                    replay.push(MaterializeBranchReplay {
                        before,
                        outcome: MaterializeBranchOutcome::Pending {
                            observed_hash,
                            disposition,
                        },
                    });
                }
                (None, Some(_)) => {
                    return Err(PrototypeJournalError::AfterWithoutBefore { transition_id });
                }
                (None, None) => {}
            }
        }

        Ok(replay)
    }

    pub(crate) fn replay_build_child(&self) -> Result<Vec<BuildReplay>, PrototypeJournalError> {
        let mut grouped = BTreeMap::<TransitionId, BuildPhases>::new();

        for entry in self.load_entries()? {
            let JournalEntry::BuildChild(entry) = entry else {
                continue;
            };
            grouped
                .entry(entry.transition_id)
                .or_default()
                .record(entry)?;
        }

        let mut replay = Vec::new();
        for (transition_id, phases) in grouped {
            match (phases.before, phases.after) {
                (Some(before), Some(after)) => {
                    replay.push(BuildReplay {
                        before,
                        outcome: BuildOutcome::Committed {
                            after: Box::new(after),
                        },
                    });
                }
                (Some(before), None) => {
                    let (binary_present, disposition) =
                        classify_pending_build(&before.paths.binary_path);
                    replay.push(BuildReplay {
                        before,
                        outcome: BuildOutcome::Pending {
                            binary_present,
                            disposition,
                        },
                    });
                }
                (None, Some(_)) => {
                    return Err(PrototypeJournalError::AfterWithoutBefore { transition_id });
                }
                (None, None) => {}
            }
        }

        Ok(replay)
    }

    pub(crate) fn replay_spawn_child(&self) -> Result<Vec<SpawnReplay>, PrototypeJournalError> {
        let mut grouped = BTreeMap::<RuntimeId, SpawnPhases>::new();

        for entry in self.load_entries()? {
            match entry {
                JournalEntry::SpawnChild(entry) => {
                    grouped
                        .entry(entry.runtime_id)
                        .or_default()
                        .record_spawn(entry)?;
                }
                JournalEntry::ChildReady(entry) => {
                    grouped
                        .entry(entry.runtime_id)
                        .or_default()
                        .record_ready(entry)?;
                }
                JournalEntry::Child(entry) => {
                    if let Some(ready) = entry.ready_entry() {
                        grouped
                            .entry(ready.runtime_id)
                            .or_default()
                            .record_ready(ready)?;
                    }
                }
                JournalEntry::MaterializeBranch(_)
                | JournalEntry::BuildChild(_)
                | JournalEntry::ParentStarted(_)
                | JournalEntry::Resource(_)
                | JournalEntry::ChildArtifactCommitted(_)
                | JournalEntry::ActiveCheckoutAdvanced(_)
                | JournalEntry::SuccessorHandoff(_)
                | JournalEntry::Successor(_)
                | JournalEntry::ObserveChild(_) => {}
            }
        }

        let mut replay = Vec::new();
        for (runtime_id, phases) in grouped {
            match (
                phases.starting,
                phases.spawned,
                phases.observed,
                phases.ready,
            ) {
                (starting, Some(spawned), Some(observed), ready) => {
                    replay.push(SpawnReplay {
                        starting,
                        spawned,
                        outcome: SpawnOutcome::Committed {
                            observed: Box::new(observed),
                            ready,
                        },
                    });
                }
                (starting, Some(spawned), None, ready) => {
                    let disposition = if ready.is_some() {
                        PendingSpawn::AcknowledgedUnobserved
                    } else {
                        PendingSpawn::SpawnedUnacknowledged
                    };
                    replay.push(SpawnReplay {
                        starting,
                        spawned,
                        outcome: SpawnOutcome::Pending { ready, disposition },
                    });
                }
                (Some(starting), None, None, None) => {
                    replay.push(SpawnReplay {
                        starting: None,
                        spawned: starting,
                        outcome: SpawnOutcome::Pending {
                            ready: None,
                            disposition: PendingSpawn::StartRecorded,
                        },
                    });
                }
                (_, None, Some(_), _) => {
                    return Err(PrototypeJournalError::ObservedWithoutSpawned { runtime_id });
                }
                (_, None, None, Some(_)) => {
                    return Err(PrototypeJournalError::ReadyWithoutSpawned { runtime_id });
                }
                (None, None, None, None) => {
                    unreachable!("spawn replay groups are only created from recorded entries")
                }
            }
        }

        Ok(replay)
    }

    pub(crate) fn replay_all(&self) -> Result<ReplayLog, PrototypeJournalError> {
        Ok(ReplayLog {
            materialize: self.replay_materialize_branch()?,
            build: self.replay_build_child()?,
            spawn: self.replay_spawn_child()?,
            completion: self.replay_observe_child()?,
        })
    }

    pub(crate) fn replay_observe_child(
        &self,
    ) -> Result<Vec<CompletionReplay>, PrototypeJournalError> {
        self.replay_observe_child_at(RecordedAt::now(), DEFAULT_OBSERVE_CHILD_STALE_AFTER)
    }

    pub(crate) fn replay_observe_child_at(
        &self,
        now: RecordedAt,
        stale_after: Duration,
    ) -> Result<Vec<CompletionReplay>, PrototypeJournalError> {
        let mut grouped = BTreeMap::<TransitionId, CompletionPhases>::new();

        for entry in self.load_entries()? {
            let JournalEntry::ObserveChild(entry) = entry else {
                continue;
            };
            grouped
                .entry(entry.transition_id)
                .or_default()
                .record(entry)?;
        }

        let mut replay = Vec::new();
        for (transition_id, phases) in grouped {
            match (phases.before, phases.after) {
                (Some(before), Some(after)) => {
                    replay.push(CompletionReplay {
                        before,
                        outcome: CompletionOutcome::Committed {
                            after: Box::new(after),
                        },
                    });
                }
                (Some(before), None) => {
                    let disposition = if before.runner_result_path.exists() {
                        PendingCompletion::TerminalResultWrittenUnobserved(
                            classify_pending_completion(&before.runner_result_path)?,
                        )
                    } else if pending_observe_age(before.recorded_at, now)
                        >= stale_after.as_millis() as u64
                    {
                        PendingCompletion::StaleOrHung {
                            age_ms: pending_observe_age(before.recorded_at, now),
                            stale_after_ms: stale_after.as_millis() as u64,
                        }
                    } else {
                        PendingCompletion::ResultPending
                    };
                    replay.push(CompletionReplay {
                        before,
                        outcome: CompletionOutcome::Pending { disposition },
                    });
                }
                (None, Some(_)) => {
                    return Err(PrototypeJournalError::AfterWithoutBefore { transition_id });
                }
                (None, None) => {}
            }
        }

        Ok(replay)
    }

    /// Project one exact successor Ready record under a cross-process
    /// check-and-append lock. The session journal remains the Ready authority;
    /// this transition-journal record is its idempotent lifecycle projection.
    pub(crate) fn project_ready(
        &mut self,
        proposed: SuccessorRecord,
    ) -> Result<ReadyReceipt, PrototypeJournalError> {
        let runtime_id =
            proposed
                .runtime_id
                .ok_or_else(|| PrototypeJournalError::ReadyConflict {
                    runtime_id: None,
                    detail: "Ready projection has no successor runtime".to_string(),
                })?;
        let (pid, ready_path, receipt) = match &proposed.state {
            SuccessorState::Ready {
                pid,
                ready_path,
                controller: Some(receipt),
            } => (*pid, ready_path, receipt.clone()),
            _ => {
                return Err(PrototypeJournalError::ReadyConflict {
                    runtime_id: Some(runtime_id),
                    detail: "projection is not a controller-backed Ready record".to_string(),
                });
            }
        };
        receipt
            .validate_persisted()
            .map_err(|detail| PrototypeJournalError::ReadyConflict {
                runtime_id: Some(runtime_id),
                detail: format!("Ready receipt is invalid: {detail}"),
            })?;

        let _lock = lock_ready(&self.path)?;
        let entries = self.load_entries()?;
        let mut spawned = false;
        let mut persisted = None;
        for record in entries.iter().filter_map(|entry| match entry {
            JournalEntry::Successor(record)
                if record.campaign_id == proposed.campaign_id
                    && record.node_id == proposed.node_id
                    && record.runtime_id == Some(runtime_id) =>
            {
                Some(record)
            }
            _ => None,
        }) {
            match &record.state {
                SuccessorState::Spawned {
                    pid: spawned_pid,
                    ready_path: spawned_path,
                    ..
                } => {
                    if spawned || *spawned_pid != pid || spawned_path != ready_path {
                        return Err(PrototypeJournalError::ReadyConflict {
                            runtime_id: Some(runtime_id),
                            detail: "Ready does not match one exact Spawned record".to_string(),
                        });
                    }
                    spawned = true;
                }
                SuccessorState::Ready {
                    pid: existing_pid,
                    ready_path: existing_path,
                    controller: Some(existing),
                } => {
                    if persisted.is_some()
                        || *existing_pid != pid
                        || existing_path != ready_path
                        || existing != &receipt
                    {
                        return Err(PrototypeJournalError::ReadyConflict {
                            runtime_id: Some(runtime_id),
                            detail: "successor runtime has conflicting or duplicate Ready evidence"
                                .to_string(),
                        });
                    }
                    persisted = Some(existing.clone());
                }
                SuccessorState::Ready { .. } => {
                    return Err(PrototypeJournalError::ReadyConflict {
                        runtime_id: Some(runtime_id),
                        detail: "successor runtime has legacy Ready without controller authority"
                            .to_string(),
                    });
                }
                SuccessorState::TimedOut { .. }
                | SuccessorState::ExitedBeforeReady { .. }
                | SuccessorState::Completed { .. } => {
                    return Err(PrototypeJournalError::ReadyConflict {
                        runtime_id: Some(runtime_id),
                        detail: "terminal successor runtime cannot publish Ready".to_string(),
                    });
                }
                _ => {}
            }
        }
        if !spawned {
            return Err(PrototypeJournalError::ReadyConflict {
                runtime_id: Some(runtime_id),
                detail: "Ready has no matching Spawned record".to_string(),
            });
        }
        if let Some(existing) = persisted {
            return Ok(existing);
        }

        RecordStore::append(self, JournalEntry::Successor(proposed))?;
        Ok(receipt)
    }

    /// Project one exact predecessor acceptance under the same successor
    /// lifecycle transaction used by Ready publication.
    ///
    /// The accepted Ready receipt remains session authority. This projection
    /// is idempotent so a recovery process can finish the stranded predecessor
    /// fence after crashing between transition-journal and session-journal
    /// writes.
    pub(crate) fn project_handoff(
        &mut self,
        runtime_id: RuntimeId,
        acceptance: HandoffAcceptance,
    ) -> Result<SuccessorHandoffEntry, PrototypeJournalError> {
        acceptance.ready().validate_persisted().map_err(|detail| {
            PrototypeJournalError::HandoffConflict {
                runtime_id,
                detail: format!("accepted Ready receipt is invalid: {detail}"),
            }
        })?;
        let ready = acceptance.ready().record();
        if ready.runtime_id != record_runtime_id(runtime_id) {
            return Err(PrototypeJournalError::HandoffConflict {
                runtime_id,
                detail: "accepted Ready receipt names a different runtime".to_string(),
            });
        }

        let _lock = lock_ready(&self.path)?;
        let entries = self.load_entries()?;
        let mut spawned = None;
        let mut projected = None;
        let mut existing = None;
        for entry in entries {
            match entry {
                JournalEntry::Successor(record)
                    if record.campaign_id == ready.campaign_id
                        && record.node_id == ready.node_id
                        && record.runtime_id == Some(runtime_id) =>
                {
                    match record.state {
                        SuccessorState::Spawned {
                            pid,
                            incarnation: _,
                            active_parent_root,
                            binary_path,
                            invocation_path,
                            ready_path,
                            streams,
                        } => {
                            if spawned.is_some() || pid != ready.pid {
                                return Err(PrototypeJournalError::HandoffConflict {
                                    runtime_id,
                                    detail: "handoff does not match one exact Spawned record"
                                        .to_string(),
                                });
                            }
                            spawned = Some((
                                active_parent_root,
                                binary_path,
                                invocation_path,
                                ready_path,
                                streams,
                                pid,
                            ));
                        }
                        SuccessorState::Ready {
                            pid,
                            ready_path,
                            controller: Some(receipt),
                        } => {
                            if projected.is_some()
                                || pid != ready.pid
                                || receipt != *acceptance.ready()
                            {
                                return Err(PrototypeJournalError::HandoffConflict {
                                    runtime_id,
                                    detail: "handoff acceptance conflicts with Ready projection"
                                        .to_string(),
                                });
                            }
                            projected = Some(ready_path);
                        }
                        SuccessorState::Ready { .. }
                        | SuccessorState::TimedOut { .. }
                        | SuccessorState::ExitedBeforeReady { .. }
                        | SuccessorState::Completed { .. } => {
                            return Err(PrototypeJournalError::HandoffConflict {
                                runtime_id,
                                detail: "handoff cannot follow conflicting or terminal successor evidence"
                                    .to_string(),
                            });
                        }
                        SuccessorState::Selected { .. }
                        | SuccessorState::Stopped { .. }
                        | SuccessorState::Checkout { .. } => {}
                    }
                }
                JournalEntry::SuccessorHandoff(entry)
                    if entry.campaign_id == ready.campaign_id
                        && entry.node_id == ready.node_id
                        && entry.runtime_id == runtime_id =>
                {
                    if existing.is_some() {
                        return Err(PrototypeJournalError::HandoffConflict {
                            runtime_id,
                            detail: "successor runtime has duplicate handoff projections"
                                .to_string(),
                        });
                    }
                    existing = Some(entry);
                }
                _ => {}
            }
        }

        let (active_parent_root, binary_path, invocation_path, ready_path, streams, pid) = spawned
            .ok_or_else(|| PrototypeJournalError::HandoffConflict {
                runtime_id,
                detail: "handoff has no matching Spawned record".to_string(),
            })?;
        let projected = projected.ok_or_else(|| PrototypeJournalError::HandoffConflict {
            runtime_id,
            detail: "handoff has no matching Ready projection".to_string(),
        })?;
        if projected != ready_path {
            return Err(PrototypeJournalError::HandoffConflict {
                runtime_id,
                detail: "Spawned and Ready records disagree on the channel path".to_string(),
            });
        }
        let proposed = SuccessorHandoffEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: ready.campaign_id.clone(),
            node_id: ready.node_id.clone(),
            runtime_id,
            active_parent_root,
            binary_path,
            invocation_path,
            ready_path,
            streams: Some(streams),
            pid,
            acceptance: Some(acceptance),
        };
        if let Some(existing) = existing {
            if same_handoff(&existing, &proposed) {
                return Ok(existing);
            }
            return Err(PrototypeJournalError::HandoffConflict {
                runtime_id,
                detail: "successor runtime has a conflicting handoff projection".to_string(),
            });
        }
        RecordStore::append(self, JournalEntry::SuccessorHandoff(proposed.clone()))?;
        Ok(proposed)
    }

    /// Probe the dedicated Ready projection transaction without waiting.
    pub(crate) fn ready_idle(&self) -> Result<bool, PrototypeJournalError> {
        let (file, path) = open_ready(&self.path)?;
        #[cfg(unix)]
        {
            // SAFETY: `file` owns a valid descriptor and remains alive for
            // this nonblocking transaction-barrier probe.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result == 0 {
                return Ok(true);
            }
            let source = io::Error::last_os_error();
            if source
                .raw_os_error()
                .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
            {
                return Ok(false);
            }
            return Err(PrototypeJournalError::Lock { path, source });
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            Err(PrototypeJournalError::Lock {
                path,
                source: io::Error::new(
                    io::ErrorKind::Unsupported,
                    "successor Ready projection requires an OS file lock",
                ),
            })
        }
    }
}

fn same_handoff(left: &SuccessorHandoffEntry, right: &SuccessorHandoffEntry) -> bool {
    left.campaign_id == right.campaign_id
        && left.node_id == right.node_id
        && left.runtime_id == right.runtime_id
        && left.active_parent_root == right.active_parent_root
        && left.binary_path == right.binary_path
        && left.invocation_path == right.invocation_path
        && left.ready_path == right.ready_path
        && left.streams == right.streams
        && left.pid == right.pid
        && left.acceptance == right.acceptance
}

fn lock_ready(path: &Path) -> Result<File, PrototypeJournalError> {
    let (file, lock_path) = open_ready(path)?;
    #[cfg(unix)]
    {
        // SAFETY: `file` owns a valid descriptor and remains alive for the
        // complete Ready check-and-append transaction.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result != 0 {
            return Err(PrototypeJournalError::Lock {
                path: lock_path,
                source: io::Error::last_os_error(),
            });
        }
    }
    #[cfg(not(unix))]
    {
        return Err(PrototypeJournalError::Lock {
            path: lock_path,
            source: io::Error::new(
                io::ErrorKind::Unsupported,
                "successor Ready projection requires an OS file lock",
            ),
        });
    }
    Ok(file)
}

fn open_ready(path: &Path) -> Result<(File, PathBuf), PrototypeJournalError> {
    let lock_path = path.with_extension("ready.lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrototypeJournalError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| PrototypeJournalError::Open {
            path: lock_path.clone(),
            source,
        })?;
    Ok((file, lock_path))
}

fn count_lines(path: &Path) -> Result<usize, PrototypeJournalError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(PrototypeJournalError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let reader = BufReader::new(file);
    let mut count = 0;
    for line in reader.lines() {
        line.map_err(|source| PrototypeJournalError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        count += 1;
    }
    Ok(count)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{digest:x}")
}

pub(crate) fn prototype1_transition_journal_path(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join("transition-journal.jsonl")
}

impl RecordStore for PrototypeJournal {
    type Entry = JournalEntry;
    type Error = PrototypeJournalError;

    fn append(&mut self, entry: Self::Entry) -> Result<(), Self::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrototypeJournalError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| PrototypeJournalError::Open {
                path: self.path.clone(),
                source,
            })?;
        let mut line = serde_json::to_string(&entry).map_err(PrototypeJournalError::Serialize)?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .map_err(|source| PrototypeJournalError::Write {
                path: self.path.clone(),
                source,
            })?;
        file.sync_data()
            .map_err(|source| PrototypeJournalError::Sync {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MaterializeBranchPhases {
    before: Option<Entry>,
    after: Option<Entry>,
}

impl MaterializeBranchPhases {
    fn record(&mut self, entry: Entry) -> Result<(), PrototypeJournalError> {
        record_commit_phase(
            &mut self.before,
            &mut self.after,
            entry.transition_id,
            entry.phase,
            entry,
        )
    }
}

#[derive(Debug, Default)]
struct BuildPhases {
    before: Option<BuildEntry>,
    after: Option<BuildEntry>,
}

impl BuildPhases {
    fn record(&mut self, entry: BuildEntry) -> Result<(), PrototypeJournalError> {
        record_commit_phase(
            &mut self.before,
            &mut self.after,
            entry.transition_id,
            entry.phase,
            entry,
        )
    }
}

#[derive(Debug, Default)]
struct SpawnPhases {
    starting: Option<SpawnEntry>,
    spawned: Option<SpawnEntry>,
    observed: Option<SpawnEntry>,
    ready: Option<ReadyEntry>,
}

impl SpawnPhases {
    fn record_spawn(&mut self, entry: SpawnEntry) -> Result<(), PrototypeJournalError> {
        match entry.phase {
            SpawnPhase::Starting => record_unique(
                &mut self.starting,
                entry.runtime_id,
                DuplicateKey::SpawnPhase(SpawnPhase::Starting),
                entry,
            ),
            SpawnPhase::Spawned => record_unique(
                &mut self.spawned,
                entry.runtime_id,
                DuplicateKey::SpawnPhase(SpawnPhase::Spawned),
                entry,
            ),
            SpawnPhase::Observed => record_unique(
                &mut self.observed,
                entry.runtime_id,
                DuplicateKey::SpawnPhase(SpawnPhase::Observed),
                entry,
            ),
        }
    }

    fn record_ready(&mut self, entry: ReadyEntry) -> Result<(), PrototypeJournalError> {
        record_unique(
            &mut self.ready,
            entry.runtime_id,
            DuplicateKey::Ready,
            entry,
        )
    }
}

#[derive(Debug, Default)]
struct CompletionPhases {
    before: Option<CompletionEntry>,
    after: Option<CompletionEntry>,
}

impl CompletionPhases {
    fn record(&mut self, entry: CompletionEntry) -> Result<(), PrototypeJournalError> {
        record_commit_phase(
            &mut self.before,
            &mut self.after,
            entry.transition_id,
            entry.phase,
            entry,
        )
    }
}

fn record_commit_phase<T>(
    before: &mut Option<T>,
    after: &mut Option<T>,
    transition_id: TransitionId,
    phase: CommitPhase,
    entry: T,
) -> Result<(), PrototypeJournalError> {
    match phase {
        CommitPhase::Before => record_unique(
            before,
            transition_id,
            DuplicateKey::CommitPhase(CommitPhase::Before),
            entry,
        ),
        CommitPhase::After => record_unique(
            after,
            transition_id,
            DuplicateKey::CommitPhase(CommitPhase::After),
            entry,
        ),
    }
}

enum DuplicateKey {
    CommitPhase(CommitPhase),
    SpawnPhase(SpawnPhase),
    Ready,
}

fn record_unique<T, I: Copy>(
    slot: &mut Option<T>,
    id: I,
    duplicate: DuplicateKey,
    entry: T,
) -> Result<(), PrototypeJournalError>
where
    PrototypeJournalError: FromDuplicate<I>,
{
    if slot.is_some() {
        return Err(PrototypeJournalError::from_duplicate(id, duplicate));
    }
    *slot = Some(entry);
    Ok(())
}

trait FromDuplicate<I> {
    fn from_duplicate(id: I, duplicate: DuplicateKey) -> Self;
}

impl FromDuplicate<TransitionId> for PrototypeJournalError {
    fn from_duplicate(id: TransitionId, duplicate: DuplicateKey) -> Self {
        match duplicate {
            DuplicateKey::CommitPhase(phase) => Self::DuplicatePhase {
                transition_id: id,
                phase,
            },
            DuplicateKey::SpawnPhase(_) | DuplicateKey::Ready => {
                unreachable!("spawn duplicate keys do not use TransitionId")
            }
        }
    }
}

impl FromDuplicate<RuntimeId> for PrototypeJournalError {
    fn from_duplicate(id: RuntimeId, duplicate: DuplicateKey) -> Self {
        match duplicate {
            DuplicateKey::SpawnPhase(phase) => Self::DuplicateSpawnPhase {
                runtime_id: id,
                phase,
            },
            DuplicateKey::Ready => Self::DuplicateReady { runtime_id: id },
            DuplicateKey::CommitPhase(_) => {
                unreachable!("commit-phase duplicate keys do not use RuntimeId")
            }
        }
    }
}

fn classify_pending_materialization(
    absolute_path: &Path,
    before: &Entry,
) -> Result<(Option<ContentHash>, PendingMaterialization), PrototypeJournalError> {
    let text = match fs::read_to_string(absolute_path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok((None, PendingMaterialization::MissingTarget));
        }
        Err(source) => {
            return Err(PrototypeJournalError::Read {
                path: absolute_path.to_path_buf(),
                source,
            });
        }
    };

    let observed_hash = ContentHash::of(&text);
    let disposition = if observed_hash == before.hashes.current {
        PendingMaterialization::NotApplied
    } else if observed_hash == before.hashes.proposed {
        PendingMaterialization::AppliedUncommitted
    } else {
        PendingMaterialization::Inconsistent
    };

    Ok((Some(observed_hash), disposition))
}

fn classify_pending_build(binary_path: &Path) -> (bool, PendingBuild) {
    let binary_present = binary_path.exists();
    let disposition = if binary_present {
        PendingBuild::BuiltUncommitted
    } else {
        PendingBuild::NotBuilt
    };
    (binary_present, disposition)
}

fn classify_pending_completion(
    runner_result_path: &Path,
) -> Result<ObservedChildTerminal, PrototypeJournalError> {
    let runner_result =
        load_runner_result_at(runner_result_path, OperatorProjectionRead::cli_operator()).map_err(
            |source| PrototypeJournalError::LoadRunnerResult {
                path: runner_result_path.to_path_buf(),
                source,
            },
        )?;
    let terminal = match runner_result.disposition {
        Prototype1RunnerDisposition::Succeeded => ObservedChildTerminal::Succeeded,
        Prototype1RunnerDisposition::CompileFailed
        | Prototype1RunnerDisposition::TreatmentFailed => ObservedChildTerminal::Failed,
    };
    Ok(terminal)
}

/// Replay view over all currently known typed transition families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayLog {
    pub materialize: Vec<MaterializeBranchReplay>,
    pub build: Vec<BuildReplay>,
    pub spawn: Vec<SpawnReplay>,
    pub completion: Vec<CompletionReplay>,
}

/// Replay classification for one materialize-branch transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterializeBranchReplay {
    pub before: Entry,
    pub outcome: MaterializeBranchOutcome,
}

/// Replay outcome for one materialize-branch transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MaterializeBranchOutcome {
    Committed {
        after: Box<Entry>,
    },
    Pending {
        observed_hash: Option<ContentHash>,
        disposition: PendingMaterialization,
    },
}

/// Recovery-relevant disposition for a `before` record without a matching
/// `after`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingMaterialization {
    MissingTarget,
    NotApplied,
    AppliedUncommitted,
    Inconsistent,
}

/// Replay classification for one build-child transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildReplay {
    pub before: BuildEntry,
    pub outcome: BuildOutcome,
}

/// Replay outcome for one build-child transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildOutcome {
    Committed {
        after: Box<BuildEntry>,
    },
    Pending {
        binary_present: bool,
        disposition: PendingBuild,
    },
}

/// Recovery-relevant disposition for a build `before` record without a
/// matching `after`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingBuild {
    NotBuilt,
    BuiltUncommitted,
}

/// Replay classification for one spawn-child runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnReplay {
    pub starting: Option<SpawnEntry>,
    pub spawned: SpawnEntry,
    pub outcome: SpawnOutcome,
}

/// Replay outcome for one spawn-child runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SpawnOutcome {
    Committed {
        observed: Box<SpawnEntry>,
        ready: Option<ReadyEntry>,
    },
    Pending {
        ready: Option<ReadyEntry>,
        disposition: PendingSpawn,
    },
}

/// Recovery-relevant disposition for a spawn that has not yet been fully
/// observed by the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingSpawn {
    StartRecorded,
    SpawnedUnacknowledged,
    AcknowledgedUnobserved,
}

/// Replay classification for one child-completion observation transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionReplay {
    pub before: CompletionEntry,
    pub outcome: CompletionOutcome,
}

/// Replay outcome for one child-completion observation transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompletionOutcome {
    Committed { after: Box<CompletionEntry> },
    Pending { disposition: PendingCompletion },
}

/// Recovery-relevant disposition for a completion observation that has not yet
/// been fully recorded by the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingCompletion {
    ResultPending,
    StaleOrHung { age_ms: u64, stale_after_ms: u64 },
    TerminalResultWrittenUnobserved(ObservedChildTerminal),
}

fn pending_observe_age(recorded_at: RecordedAt, now: RecordedAt) -> u64 {
    now.0.saturating_sub(recorded_at.0) as u64
}

#[derive(Debug, Error)]
pub(crate) enum PrototypeJournalError {
    #[error("failed to create journal directory '{path}': {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to open journal '{path}': {source}")]
    Open {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to lock journal transaction '{path}': {source}")]
    Lock {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize journal entry: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write journal '{path}': {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to sync journal '{path}': {source}")]
    Sync {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read journal '{path}': {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse journal '{path}' at line {line_number}: {source}")]
    ParseLine {
        path: PathBuf,
        line_number: usize,
        source: serde_json::Error,
    },
    #[error("failed to load runner result '{path}': {source}")]
    LoadRunnerResult { path: PathBuf, source: PrepareError },
    #[error("duplicate '{phase:?}' entry for transition '{transition_id}'")]
    DuplicatePhase {
        transition_id: TransitionId,
        phase: CommitPhase,
    },
    #[error(
        "found an after entry without a matching before entry for transition '{transition_id}'"
    )]
    AfterWithoutBefore { transition_id: TransitionId },
    #[error("duplicate '{phase:?}' spawn entry for runtime '{runtime_id}'")]
    DuplicateSpawnPhase {
        runtime_id: RuntimeId,
        phase: SpawnPhase,
    },
    #[error("duplicate child-ready entry for runtime '{runtime_id}'")]
    DuplicateReady { runtime_id: RuntimeId },
    #[error(
        "found an observed spawn entry without a matching spawned entry for runtime '{runtime_id}'"
    )]
    ObservedWithoutSpawned { runtime_id: RuntimeId },
    #[error(
        "found a child-ready entry without a matching spawned entry for runtime '{runtime_id}'"
    )]
    ReadyWithoutSpawned { runtime_id: RuntimeId },
    #[error("successor Ready projection conflict for runtime {runtime_id:?}: {detail}")]
    ReadyConflict {
        runtime_id: Option<RuntimeId>,
        detail: String,
    },
    #[error("successor handoff projection conflict for runtime {runtime_id}: {detail}")]
    HandoffConflict {
        runtime_id: RuntimeId,
        detail: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    use crate::cli::prototype1_state::{
        event::{Hashes, LineageMark, Paths, RecordedAt, Refs, RuntimeId, World},
        invocation::{SUCCESSOR_READY_SCHEMA_VERSION, SuccessorReadyRecord},
        profile::RunMode,
        session::{Cursor, Fence, SessionId},
        successor::{HandoffAcceptance, PredecessorAttempt, ReadyCommit, ReadyReceipt},
        walk::phase::WalkPhase,
    };

    #[cfg(unix)]
    #[test]
    fn ready_idle_detects_projection_lock() {
        let temp = tempdir().expect("tempdir");
        let journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        assert!(journal.ready_idle().expect("idle Ready probe"));
        let (guard, _) = open_ready(journal.path()).expect("open Ready lock");
        // SAFETY: `guard` owns the descriptor for the duration of this test.
        assert_eq!(unsafe { libc::flock(guard.as_raw_fd(), libc::LOCK_EX) }, 0);
        assert!(!journal.ready_idle().expect("busy Ready probe"));
        drop(guard);
        assert!(journal.ready_idle().expect("released Ready probe"));
    }

    #[test]
    fn handoff_projection_is_exact_and_idempotent() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("transition-journal.jsonl");
        let mut journal = PrototypeJournal::new(path);
        let runtime_id = RuntimeId::new();
        let campaign_id = CampaignId::from("campaign");
        let node_id = "node-1".to_string();
        let pid = 8_080;
        let ready_path = temp.path().join("successor-ready.json");
        let spawned = SuccessorRecord {
            runtime_id: Some(runtime_id),
            recorded_at: RecordedAt::now(),
            campaign_id: campaign_id.clone(),
            node_id: node_id.clone(),
            state: SuccessorState::Spawned {
                pid,
                incarnation: None,
                active_parent_root: temp.path().to_path_buf(),
                binary_path: temp.path().join("ploke-eval"),
                invocation_path: temp.path().join("successor-invocation.json"),
                ready_path: ready_path.clone(),
                streams: Streams {
                    stdout: temp.path().join("successor.stdout"),
                    stderr: temp.path().join("successor.stderr"),
                },
            },
        };
        RecordStore::append(&mut journal, JournalEntry::Successor(spawned))
            .expect("record successor spawn");

        let commit = ReadyCommit::new(
            SessionId::for_test(9),
            TransitionId::new(),
            Fence::for_test(2),
            Cursor::new(WalkPhase::R4c, ContentHash::of("successor R4c"))
                .expect("valid R4c cursor"),
            RunMode::Continuous,
        )
        .expect("construct Ready commit");
        let ready = ReadyReceipt::new(
            SuccessorReadyRecord {
                schema_version: SUCCESSOR_READY_SCHEMA_VERSION.to_string(),
                campaign_id,
                node_id,
                runtime_id: record_runtime_id(runtime_id),
                pid,
                incarnation: Some(
                    crate::cli::prototype1_state::invocation::ProcessIncarnation {
                        boot_id: uuid::Uuid::from_u128(1),
                        start_ticks: 1,
                    },
                ),
                recorded_at: "2026-07-13T00:00:00Z".to_string(),
            },
            commit,
            None,
            None,
        )
        .expect("construct Ready receipt");
        let projected = journal
            .project_ready(SuccessorRecord {
                runtime_id: Some(runtime_id),
                recorded_at: RecordedAt::now(),
                campaign_id: ready.record().campaign_id.clone(),
                node_id: ready.record().node_id.clone(),
                state: SuccessorState::Ready {
                    pid,
                    ready_path,
                    controller: Some(ready.clone()),
                },
            })
            .expect("project exact Ready");
        assert_eq!(projected, ready);

        let attempt = PredecessorAttempt::new(
            SessionId::for_test(10),
            TransitionId::new(),
            Fence::for_test(4),
            true,
            true,
        );
        let acceptance = HandoffAcceptance::from_persisted(ready, attempt)
            .expect("construct handoff acceptance");
        let first = journal
            .project_handoff(runtime_id, acceptance.clone())
            .expect("project handoff");
        let second = journal
            .project_handoff(runtime_id, acceptance.clone())
            .expect("retry exact handoff");
        assert_eq!(second, first);
        let entries = journal.load_entries().expect("load handoff journal");
        assert_eq!(
            entries
                .iter()
                .filter(|entry| matches!(entry, JournalEntry::SuccessorHandoff(_)))
                .count(),
            1
        );

        let conflict = HandoffAcceptance::from_persisted(
            acceptance.ready().clone(),
            PredecessorAttempt::new(
                acceptance.attempt().session(),
                TransitionId::new(),
                acceptance.attempt().fence(),
                acceptance.attempt().allow_live_api(),
                acceptance.attempt().allow_git_changes(),
            ),
        )
        .expect("construct conflicting acceptance");
        assert!(matches!(
            journal.project_handoff(runtime_id, conflict),
            Err(PrototypeJournalError::HandoffConflict { .. })
        ));
    }

    fn sample_entry(
        transition_id: TransitionId,
        phase: CommitPhase,
        absolute_path: &Path,
        current_hash: &str,
        proposed_hash: &str,
    ) -> Entry {
        Entry {
            transition_id,
            phase,
            recorded_at: RecordedAt(1_777_091_200_000),
            generation: 1,
            refs: Refs {
                campaign_id: CampaignId::from("campaign"),
                node_id: "node-1".to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state-1".to_string(),
                branch_id: "branch-1".to_string(),
                candidate_id: "candidate-1".to_string(),
                branch_label: "minimal".to_string(),
                spec_id: "spec-1".to_string(),
            },
            paths: Paths {
                repo_root: absolute_path.parent().unwrap().to_path_buf(),
                workspace_root: absolute_path.parent().unwrap().to_path_buf(),
                binary_path: absolute_path.parent().unwrap().join("ploke-eval"),
                target_relpath: PathBuf::from("target.md"),
                absolute_path: absolute_path.to_path_buf(),
            },
            world: World {
                node_status: crate::intervention::Prototype1NodeStatus::Planned,
                running_binary: true,
                running_lineage: LineageMark::Parent,
                artifact_lineage: LineageMark::Parent,
                child_lifecycle: None,
            },
            hashes: Hashes {
                source: ContentHash(current_hash.to_string()),
                current: ContentHash(current_hash.to_string()),
                proposed: ContentHash(proposed_hash.to_string()),
            },
        }
    }

    fn sample_build_entry(
        transition_id: TransitionId,
        phase: CommitPhase,
        binary_path: &Path,
        result: Option<BuildResult>,
    ) -> BuildEntry {
        BuildEntry {
            transition_id,
            phase,
            recorded_at: RecordedAt(1_777_091_200_000),
            generation: 1,
            refs: Refs {
                campaign_id: CampaignId::from("campaign"),
                node_id: "node-2".to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state-2".to_string(),
                branch_id: "branch-2".to_string(),
                candidate_id: "candidate-2".to_string(),
                branch_label: "build".to_string(),
                spec_id: "spec-2".to_string(),
            },
            paths: Paths {
                repo_root: binary_path.parent().unwrap().to_path_buf(),
                workspace_root: binary_path.parent().unwrap().to_path_buf(),
                binary_path: binary_path.to_path_buf(),
                target_relpath: PathBuf::from("target.md"),
                absolute_path: binary_path.parent().unwrap().join("target.md"),
            },
            world: World {
                node_status: crate::intervention::Prototype1NodeStatus::WorkspaceStaged,
                running_binary: true,
                running_lineage: LineageMark::Parent,
                artifact_lineage: LineageMark::Child,
                child_lifecycle: None,
            },
            hashes: Hashes {
                source: ContentHash("source".to_string()),
                current: ContentHash("proposed".to_string()),
                proposed: ContentHash("proposed".to_string()),
            },
            result,
        }
    }

    fn sample_spawn_entry(
        runtime_id: RuntimeId,
        phase: SpawnPhase,
        binary_path: &Path,
        result: Option<SpawnObservation>,
    ) -> SpawnEntry {
        let child_lifecycle = match result {
            Some(SpawnObservation::Acknowledged) => ChildRuntimeLifecycle::Acknowledged,
            Some(SpawnObservation::TerminatedBeforeAcknowledged { .. }) => {
                ChildRuntimeLifecycle::Terminated
            }
            Some(SpawnObservation::ReadyTimedOut { .. }) => ChildRuntimeLifecycle::Terminated,
            None => ChildRuntimeLifecycle::Spawned,
        };
        SpawnEntry {
            runtime_id,
            phase,
            recorded_at: RecordedAt(1_777_091_200_000),
            generation: 1,
            refs: Refs {
                campaign_id: CampaignId::from("campaign"),
                node_id: "node-3".to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state-3".to_string(),
                branch_id: "branch-3".to_string(),
                candidate_id: "candidate-3".to_string(),
                branch_label: "spawn".to_string(),
                spec_id: "spec-3".to_string(),
            },
            paths: Paths {
                repo_root: binary_path.parent().unwrap().to_path_buf(),
                workspace_root: binary_path.parent().unwrap().to_path_buf(),
                binary_path: binary_path.to_path_buf(),
                target_relpath: PathBuf::from("target.md"),
                absolute_path: binary_path.parent().unwrap().join("target.md"),
            },
            world: World {
                node_status: crate::intervention::Prototype1NodeStatus::BinaryBuilt,
                running_binary: true,
                running_lineage: LineageMark::Parent,
                artifact_lineage: LineageMark::Child,
                child_lifecycle: Some(child_lifecycle),
            },
            child_lifecycle,
            parent_pid: 111,
            child_pid: Some(222),
            incarnation: Some(ProcessIncarnation {
                boot_id: uuid::Uuid::from_u128(1),
                start_ticks: 2,
            }),
            argv: vec!["prototype1-runner".to_string()],
            streams: None,
            result,
        }
    }

    fn sample_ready_entry(runtime_id: RuntimeId, binary_path: &Path) -> ReadyEntry {
        ReadyEntry {
            runtime_id,
            recorded_at: RecordedAt(1_777_091_200_100),
            generation: 1,
            refs: Refs {
                campaign_id: CampaignId::from("campaign"),
                node_id: "node-3".to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state-3".to_string(),
                branch_id: "branch-3".to_string(),
                candidate_id: "candidate-3".to_string(),
                branch_label: "spawn".to_string(),
                spec_id: "spec-3".to_string(),
            },
            paths: Paths {
                repo_root: binary_path.parent().unwrap().to_path_buf(),
                workspace_root: binary_path.parent().unwrap().to_path_buf(),
                binary_path: binary_path.to_path_buf(),
                target_relpath: PathBuf::from("target.md"),
                absolute_path: binary_path.parent().unwrap().join("target.md"),
            },
            pid: 222,
        }
    }

    #[test]
    fn legacy_spawn_replays_without_cleanup_authority() {
        let tmp = tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let entry = sample_spawn_entry(
            runtime_id,
            SpawnPhase::Spawned,
            &tmp.path().join("ploke-eval"),
            None,
        );
        let mut value = serde_json::to_value(JournalEntry::SpawnChild(entry))
            .expect("serialize current Spawned entry");
        value
            .as_object_mut()
            .expect("journal entry object")
            .remove("incarnation");

        let JournalEntry::SpawnChild(legacy) =
            serde_json::from_value(value).expect("deserialize legacy Spawned entry")
        else {
            panic!("expected SpawnChild entry");
        };

        assert_eq!(legacy.child_pid, Some(222));
        assert_eq!(legacy.incarnation, None);
        assert_eq!(legacy.cleanup_authority(), None);
    }

    fn sample_completion_entry(
        transition_id: TransitionId,
        phase: CommitPhase,
        runtime_id: RuntimeId,
        runner_result_path: &Path,
        result: Option<ObservedChildResult>,
    ) -> CompletionEntry {
        CompletionEntry {
            transition_id,
            runtime_id,
            phase,
            recorded_at: RecordedAt(1_777_091_200_000),
            generation: 1,
            refs: Refs {
                campaign_id: CampaignId::from("campaign"),
                node_id: "node-4".to_string(),
                instance_id: "instance".to_string(),
                source_state_id: "state-4".to_string(),
                branch_id: "branch-4".to_string(),
                candidate_id: "candidate-4".to_string(),
                branch_label: "observe".to_string(),
                spec_id: "spec-4".to_string(),
            },
            paths: Paths {
                repo_root: runner_result_path.parent().unwrap().to_path_buf(),
                workspace_root: runner_result_path.parent().unwrap().to_path_buf(),
                binary_path: runner_result_path.parent().unwrap().join("ploke-eval"),
                target_relpath: PathBuf::from("target.md"),
                absolute_path: runner_result_path.parent().unwrap().join("target.md"),
            },
            world: World {
                node_status: crate::intervention::Prototype1NodeStatus::Running,
                running_binary: true,
                running_lineage: LineageMark::Parent,
                artifact_lineage: LineageMark::Child,
                child_lifecycle: Some(if result.is_some() {
                    ChildRuntimeLifecycle::Terminated
                } else {
                    ChildRuntimeLifecycle::Acknowledged
                }),
            },
            child_lifecycle: if result.is_some() {
                ChildRuntimeLifecycle::Terminated
            } else {
                ChildRuntimeLifecycle::Acknowledged
            },
            runner_result_path: runner_result_path.to_path_buf(),
            result,
        }
    }

    #[test]
    fn replay_marks_before_only_source_hash_as_not_applied() {
        let tmp = tempdir().expect("tempdir");
        let artifact_path = tmp.path().join("target.md");
        fs::write(&artifact_path, "source").expect("write source");
        let current_hash = ContentHash::of("source");
        let proposed_hash = ContentHash::of("proposed");
        let transition_id = TransitionId::new();

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::MaterializeBranch(sample_entry(
                transition_id,
                CommitPhase::Before,
                &artifact_path,
                &current_hash.0,
                &proposed_hash.0,
            )))
            .expect("append before");

        let replay = journal
            .replay_materialize_branch()
            .expect("replay materialize branch");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            MaterializeBranchReplay {
                outcome: MaterializeBranchOutcome::Pending {
                    disposition: PendingMaterialization::NotApplied,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn replay_marks_before_only_proposed_hash_as_applied_uncommitted() {
        let tmp = tempdir().expect("tempdir");
        let artifact_path = tmp.path().join("target.md");
        fs::write(&artifact_path, "proposed").expect("write proposed");
        let current_hash = ContentHash::of("source");
        let proposed_hash = ContentHash::of("proposed");
        let transition_id = TransitionId::new();

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::MaterializeBranch(sample_entry(
                transition_id,
                CommitPhase::Before,
                &artifact_path,
                &current_hash.0,
                &proposed_hash.0,
            )))
            .expect("append before");

        let replay = journal
            .replay_materialize_branch()
            .expect("replay materialize branch");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            MaterializeBranchReplay {
                outcome: MaterializeBranchOutcome::Pending {
                    disposition: PendingMaterialization::AppliedUncommitted,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn replay_marks_matching_before_after_as_committed() {
        let tmp = tempdir().expect("tempdir");
        let artifact_path = tmp.path().join("target.md");
        fs::write(&artifact_path, "proposed").expect("write proposed");
        let current_hash = ContentHash::of("source");
        let proposed_hash = ContentHash::of("proposed");
        let transition_id = TransitionId::new();

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::MaterializeBranch(sample_entry(
                transition_id,
                CommitPhase::Before,
                &artifact_path,
                &current_hash.0,
                &proposed_hash.0,
            )))
            .expect("append before");
        journal
            .append(JournalEntry::MaterializeBranch(sample_entry(
                transition_id,
                CommitPhase::After,
                &artifact_path,
                &current_hash.0,
                &proposed_hash.0,
            )))
            .expect("append after");

        let replay = journal
            .replay_materialize_branch()
            .expect("replay materialize branch");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            MaterializeBranchReplay {
                outcome: MaterializeBranchOutcome::Committed { .. },
                ..
            }
        ));
    }

    #[test]
    fn replay_marks_before_only_missing_binary_as_not_built() {
        let tmp = tempdir().expect("tempdir");
        let binary_path = tmp.path().join("ploke-eval");
        let transition_id = TransitionId::new();

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::BuildChild(sample_build_entry(
                transition_id,
                CommitPhase::Before,
                &binary_path,
                None,
            )))
            .expect("append build before");

        let replay = journal.replay_build_child().expect("replay build child");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            BuildReplay {
                outcome: BuildOutcome::Pending {
                    binary_present: false,
                    disposition: PendingBuild::NotBuilt,
                },
                ..
            }
        ));
    }

    #[test]
    fn replay_marks_ready_without_observed_as_ready_unobserved() {
        let tmp = tempdir().expect("tempdir");
        let binary_path = tmp.path().join("ploke-eval");
        let runtime_id = RuntimeId::new();

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::SpawnChild(sample_spawn_entry(
                runtime_id,
                SpawnPhase::Spawned,
                &binary_path,
                None,
            )))
            .expect("append spawned");
        journal
            .append(JournalEntry::ChildReady(sample_ready_entry(
                runtime_id,
                &binary_path,
            )))
            .expect("append ready");

        let replay = journal.replay_spawn_child().expect("replay spawn child");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            SpawnReplay {
                outcome: SpawnOutcome::Pending {
                    ready: Some(_),
                    disposition: PendingSpawn::AcknowledgedUnobserved,
                },
                ..
            }
        ));
    }

    #[test]
    fn replay_marks_written_runner_result_as_terminal_unobserved() {
        let tmp = tempdir().expect("tempdir");
        let runner_result_path = tmp.path().join("runner-result.json");
        let transition_id = TransitionId::new();
        let runtime_id = RuntimeId::new();

        let runner_result = crate::intervention::Prototype1RunnerResult {
            schema_version: "prototype1_runner_result.v1".to_string(),
            campaign_id: CampaignId::from("campaign"),
            node_id: "node-4".to_string(),
            generation: 1,
            branch_id: "branch-4".to_string(),
            status: crate::intervention::Prototype1NodeStatus::Failed,
            disposition: Prototype1RunnerDisposition::TreatmentFailed,
            treatment_campaign_id: None,
            evaluation_artifact_path: None,
            detail: Some("child failed".to_string()),
            exit_code: Some(1),
            stdout_excerpt: None,
            stderr_excerpt: None,
            recorded_at: "2026-04-25T00:00:00Z".to_string(),
        };
        fs::write(
            &runner_result_path,
            serde_json::to_string(&runner_result).expect("serialize runner result"),
        )
        .expect("write runner result");

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::ObserveChild(sample_completion_entry(
                transition_id,
                CommitPhase::Before,
                runtime_id,
                &runner_result_path,
                None,
            )))
            .expect("append completion before");

        let replay = journal
            .replay_observe_child()
            .expect("replay observe child");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            CompletionReplay {
                outcome: CompletionOutcome::Pending {
                    disposition: PendingCompletion::TerminalResultWrittenUnobserved(
                        ObservedChildTerminal::Failed
                    ),
                },
                ..
            }
        ));
    }

    #[test]
    fn replay_marks_missing_runner_result_as_stale_or_hung() {
        let tmp = tempdir().expect("tempdir");
        let transition_id = TransitionId::new();
        let runtime_id = RuntimeId::new();
        let runner_result_path = tmp.path().join("missing-runner-result.json");

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::ObserveChild(sample_completion_entry(
                transition_id,
                CommitPhase::Before,
                runtime_id,
                &runner_result_path,
                None,
            )))
            .expect("append completion before");

        let replay = journal
            .replay_observe_child_at(RecordedAt(1_777_091_211_000), Duration::from_secs(10))
            .expect("replay observe child");

        assert_eq!(replay.len(), 1);
        assert!(matches!(
            &replay[0],
            CompletionReplay {
                outcome: CompletionOutcome::Pending {
                    disposition: PendingCompletion::StaleOrHung {
                        age_ms: 11_000,
                        stale_after_ms: 10_000
                    },
                },
                ..
            }
        ));
    }

    #[test]
    fn replay_observe_child_uses_default_stale_threshold() {
        let tmp = tempdir().expect("tempdir");
        let transition_id = TransitionId::new();
        let runtime_id = RuntimeId::new();
        let runner_result_path = tmp.path().join("missing-runner-result.json");

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::ObserveChild(sample_completion_entry(
                transition_id,
                CommitPhase::Before,
                runtime_id,
                &runner_result_path,
                None,
            )))
            .expect("append completion before");

        let replay = journal
            .replay_observe_child()
            .expect("replay observe child");

        assert_eq!(replay.len(), 1);
        let expected_stale_after_ms = DEFAULT_OBSERVE_CHILD_STALE_AFTER.as_millis() as u64;
        match &replay[0].outcome {
            CompletionOutcome::Pending {
                disposition: PendingCompletion::StaleOrHung { stale_after_ms, .. },
            } => assert_eq!(*stale_after_ms, expected_stale_after_ms),
            other => panic!("expected stale pending completion, got {other:?}"),
        }
    }

    #[test]
    fn replay_all_collects_each_transition_family() {
        let tmp = tempdir().expect("tempdir");
        let artifact_path = tmp.path().join("target.md");
        let binary_path = tmp.path().join("ploke-eval");
        fs::write(&artifact_path, "proposed").expect("write proposed");
        fs::write(&binary_path, "binary").expect("write binary");

        let current_hash = ContentHash::of("source");
        let proposed_hash = ContentHash::of("proposed");
        let transition_id = TransitionId::new();
        let build_id = TransitionId::new();
        let runtime_id = RuntimeId::new();

        let mut journal = PrototypeJournal::new(tmp.path().join("transition-journal.jsonl"));
        journal
            .append(JournalEntry::MaterializeBranch(sample_entry(
                transition_id,
                CommitPhase::Before,
                &artifact_path,
                &current_hash.0,
                &proposed_hash.0,
            )))
            .expect("append materialize before");
        journal
            .append(JournalEntry::MaterializeBranch(sample_entry(
                transition_id,
                CommitPhase::After,
                &artifact_path,
                &current_hash.0,
                &proposed_hash.0,
            )))
            .expect("append materialize after");
        journal
            .append(JournalEntry::BuildChild(sample_build_entry(
                build_id,
                CommitPhase::Before,
                &binary_path,
                None,
            )))
            .expect("append build before");
        journal
            .append(JournalEntry::BuildChild(sample_build_entry(
                build_id,
                CommitPhase::After,
                &binary_path,
                Some(BuildResult::Built),
            )))
            .expect("append build after");
        journal
            .append(JournalEntry::SpawnChild(sample_spawn_entry(
                runtime_id,
                SpawnPhase::Spawned,
                &binary_path,
                None,
            )))
            .expect("append spawned");
        journal
            .append(JournalEntry::ChildReady(sample_ready_entry(
                runtime_id,
                &binary_path,
            )))
            .expect("append ready");
        journal
            .append(JournalEntry::SpawnChild(sample_spawn_entry(
                runtime_id,
                SpawnPhase::Observed,
                &binary_path,
                Some(SpawnObservation::Acknowledged),
            )))
            .expect("append observed");

        let replay = journal.replay_all().expect("replay all");

        assert_eq!(replay.materialize.len(), 1);
        assert_eq!(replay.build.len(), 1);
        assert_eq!(replay.spawn.len(), 1);
    }
}
