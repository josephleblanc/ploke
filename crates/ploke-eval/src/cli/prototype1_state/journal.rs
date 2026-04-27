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

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::event::{
    ChildRuntimeLifecycle, ContentHash, Hashes, ObservedChildTerminal, Paths, RecordedAt, Refs,
    RuntimeId, TransitionId, World,
};
use super::identity::ParentIdentity;
use crate::branch_evaluation::BranchDisposition;
use crate::intervention::{
    CommitPhase, Prototype1RunnerDisposition, RecordStore, load_runner_result_at,
};
use crate::spec::PrepareError;

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
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streams: Option<Streams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SpawnObservation>,
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
    pub campaign_id: String,
    pub parent_identity: ParentIdentity,
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_runtime_id: Option<RuntimeId>,
    pub pid: u32,
}

/// Durable child artifact commit record produced before a child can be selected
/// as successor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ChildArtifactCommittedEntry {
    pub recorded_at: RecordedAt,
    pub campaign_id: String,
    pub parent_identity: Option<ParentIdentity>,
    pub child_identity: ParentIdentity,
    pub node_id: String,
    pub generation: u32,
    pub target_relpath: PathBuf,
    pub child_branch: String,
    pub target_commit: String,
    pub identity_commit: String,
}

/// Active checkout advancement record for the parent handoff path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ActiveCheckoutAdvancedEntry {
    pub recorded_at: RecordedAt,
    pub campaign_id: String,
    pub previous_parent_identity: Option<ParentIdentity>,
    pub selected_parent_identity: ParentIdentity,
    pub active_parent_root: PathBuf,
    pub selected_branch: String,
    pub installed_commit: String,
}

/// Successor handoff acknowledgement observed by the previous parent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorHandoffEntry {
    pub recorded_at: RecordedAt,
    pub campaign_id: String,
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

/// Single append-only journal entry for typed prototype1 transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum JournalEntry {
    ParentStarted(ParentStartedEntry),
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

impl PrototypeJournal {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
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
    let runner_result = load_runner_result_at(runner_result_path).map_err(|source| {
        PrototypeJournalError::LoadRunnerResult {
            path: runner_result_path.to_path_buf(),
            source,
        }
    })?;
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
    TerminalResultWrittenUnobserved(ObservedChildTerminal),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    use crate::cli::prototype1_state::event::{
        Hashes, LineageMark, Paths, RecordedAt, Refs, RuntimeId, World,
    };

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
                campaign_id: "campaign".to_string(),
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
                campaign_id: "campaign".to_string(),
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
                campaign_id: "campaign".to_string(),
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
                campaign_id: "campaign".to_string(),
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
                campaign_id: "campaign".to_string(),
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
            campaign_id: "campaign".to_string(),
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
