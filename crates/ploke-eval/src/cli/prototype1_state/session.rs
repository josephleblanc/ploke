//! Durable, process-exclusive controller sessions for Prototype 1 parents.
//!
//! The kernel lock is the live authority primitive. The append-only journal is
//! durable evidence about session creation, fences, transition attempts, clean
//! release, and explicit recovery. A journal record never substitutes for the
//! held file descriptor.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::fd::AsRawFd;

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::cli::prototype1_state::{
    control_evidence::{CursorEvidence, SuccessorOrigin, initial_cursor, successor_cursor},
    driver::control::{ControlEffect, ControlPermit, HandoffPermit},
    edge::{GRAPH_VERSION_V1, GRAPH_VERSION_V2},
    event::{ContentHash, RecordedAt, RuntimeId, TransitionId},
    identity::ParentIdentity,
    invocation::{ProcessIncarnation, process_incarnation, record_runtime_id},
    journal::JournalAppendReceipt,
    profile::{AdmittedRunProfile, RunMode, RunProfileCommitment},
    setup_admission::Prototype1SetupAdmission,
    successor::{HandoffAcceptance, ReadyCommit, ReadyReceipt},
    walk::{
        epoch::{ServerEpoch, TRANSITION_GRAPH_VERSION},
        phase::WalkPhase,
        protocol::SessionVersion,
    },
};

const SCHEMA_VERSION_V1: &str = "prototype1-control-session.v1";
const SCHEMA_VERSION_V2: &str = "prototype1-control-session.v2";
const SCHEMA_VERSION_V3: &str = "prototype1-control-session.v3";
const SCHEMA_VERSION_V4: &str = "prototype1-control-session.v4";
const SCHEMA_VERSION: &str = "prototype1-control-session.v5";
const TRANSITION_KEY_VERSION_V1: &str = "prototype1-control-transition-key.v1";
const TRANSITION_KEY_VERSION: &str = "prototype1-control-transition-key.v2";
const LOCK_FILE: &str = "controller.lock";
const JOURNAL_FILE: &str = "control-journal.jsonl";

/// Durable identity for one parent-scoped controller session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[cfg(test)]
    pub(crate) fn for_test(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Last transition boundary proven committed for one controller session.
///
/// The session kernel treats `evidence` as an opaque canonical digest. The
/// driver that reconstructs the typed state owns the evidence preimage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub(crate) phase: WalkPhase,
    pub(crate) evidence: ContentHash,
}

impl Cursor {
    pub(crate) fn new(phase: WalkPhase, evidence: ContentHash) -> Result<Self, Error> {
        let cursor = Self { phase, evidence };
        cursor.validate()?;
        Ok(cursor)
    }

    fn validate(&self) -> Result<(), Error> {
        validate_hash(&self.evidence).map_err(|detail| Error::InvalidIntent { detail })
    }

    /// Operator-facing typestate phase proven by this durable cursor.
    pub const fn phase(&self) -> WalkPhase {
        self.phase
    }

    /// Canonical evidence digest bound to this cursor.
    pub fn evidence(&self) -> &str {
        &self.evidence.0
    }
}

/// Monotonic token issued whenever a controller takes mutation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Fence(u64);

impl Fence {
    pub(crate) fn get(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for Fence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Stable filesystem locations for one parent coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Paths {
    root: PathBuf,
    lock: PathBuf,
    journal: PathBuf,
}

impl Paths {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn lock(&self) -> &Path {
        &self.lock
    }

    pub(crate) fn journal(&self) -> &Path {
        &self.journal
    }
}

/// Immutable session origin.
///
/// Genesis sessions use completed setup authority; selected successors use the
/// exact installed/spawned transfer. `Historical` exists only for incident
/// regressions that predate both admission schemes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
enum Origin {
    Admitted { setup: Prototype1SetupAdmission },
    Successor { transfer: SuccessorOrigin },
    Historical { source: ContentHash },
}

impl Origin {
    fn label(&self) -> String {
        match self {
            Self::Admitted { setup } => setup.intent.plan_hash.0.clone(),
            Self::Successor { transfer } => {
                format!("successor:{}", transfer.runtime_id())
            }
            Self::Historical { source } => source.0.clone(),
        }
    }

    fn handoff_path(&self) -> Option<PathBuf> {
        match self {
            Self::Successor { transfer } => Some(transfer.invocation_path().to_path_buf()),
            Self::Admitted { .. } | Self::Historical { .. } => None,
        }
    }
}

/// Request to claim mutation authority for one parent session.
#[derive(Debug, Clone)]
pub(crate) struct Claim {
    origin: Origin,
    origin_cursor: Cursor,
    parent: ParentIdentity,
    profile: RunProfileCommitment,
    mode: RunMode,
    cursor: Cursor,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    pid: u32,
    incarnation: Option<ProcessIncarnation>,
}

/// Exact authority under which a successor process may claim its session.
#[derive(Debug, Clone)]
pub(crate) enum SuccessorAuthority {
    /// Before Ready, only the exact process recorded by Spawned may claim.
    Spawned,
    /// After predecessor acceptance, a replacement process may resume from
    /// the exact persisted handoff while retaining runtime lineage.
    Transferred(HandoffAcceptance),
}

impl Claim {
    /// Build a production claim from the completed Stage-1 setup authority.
    pub(crate) fn from_setup(
        setup: Prototype1SetupAdmission,
        parent: ParentIdentity,
        admitted: &AdmittedRunProfile,
        mode: RunMode,
        cursor: Cursor,
        epoch: ServerEpoch,
    ) -> Result<Self, Error> {
        epoch
            .ensure_not_stale_now()
            .map_err(|source| Error::EpochStale {
                detail: source.to_string(),
            })?;
        if setup.completed_head().is_none() {
            return Err(Error::InvalidClaim {
                detail: "controller claim requires a completed setup admission".to_string(),
            });
        }
        if setup.intent.campaign_id != *parent.campaign_id() {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "setup campaign '{}' does not match parent campaign '{}'",
                    setup.intent.campaign_id,
                    parent.campaign_id()
                ),
            });
        }
        if setup.intent.hashes.profile.0 != admitted.commitment.sha256 {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "setup profile hash '{}' does not match admitted commitment '{}'",
                    setup.intent.hashes.profile, admitted.commitment.sha256
                ),
            });
        }
        if setup.intent.repo_root != epoch.repo_root {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "setup repository '{}' does not match controller epoch '{}'",
                    setup.intent.repo_root.display(),
                    epoch.repo_root.display()
                ),
            });
        }
        if admitted.profile.control.mode != mode {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "requested controller mode {mode:?} does not match admitted profile mode {:?}",
                    admitted.profile.control.mode
                ),
            });
        }
        cursor.validate()?;
        let origin_cursor =
            initial_cursor(&setup, &parent, &admitted.commitment).map_err(|error| {
                Error::InvalidClaim {
                    detail: error.to_string(),
                }
            })?;
        let pid = std::process::id();
        let incarnation = process_incarnation(pid)
            .map_err(|source| Error::InvalidClaim {
                detail: format!("cannot capture controller process incarnation: {source}"),
            })?
            .ok_or_else(|| Error::InvalidClaim {
                detail: format!(
                    "controller process {pid} disappeared before its exact incarnation could be recorded"
                ),
            })?;
        Ok(Self {
            origin: Origin::Admitted { setup },
            origin_cursor,
            parent,
            profile: admitted.commitment.clone(),
            mode,
            cursor,
            epoch,
            runtime_id: None,
            pid,
            incarnation: Some(incarnation),
        })
    }

    /// Build a production claim from an installed and spawned successor
    /// transfer, before that runtime records Ready.
    pub(crate) fn from_successor(
        transfer: SuccessorOrigin,
        authority: SuccessorAuthority,
        parent: ParentIdentity,
        admitted: &AdmittedRunProfile,
        mode: RunMode,
        cursor: Cursor,
        epoch: ServerEpoch,
    ) -> Result<Self, Error> {
        epoch
            .ensure_not_stale_now()
            .map_err(|source| Error::EpochStale {
                detail: source.to_string(),
            })?;
        if admitted.profile.control.mode != mode {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "requested controller mode {mode:?} does not match admitted profile mode {:?}",
                    admitted.profile.control.mode
                ),
            });
        }
        transfer
            .validate(&parent, &admitted.commitment, &epoch.repo_root)
            .map_err(|source| Error::InvalidClaim {
                detail: source.to_string(),
            })?;
        let runtime_id = transfer.runtime_id();
        let origin_cursor =
            successor_cursor(&transfer, &parent, &admitted.commitment, &epoch.repo_root).map_err(
                |source| Error::InvalidClaim {
                    detail: source.to_string(),
                },
            )?;
        cursor.validate()?;
        let pid = std::process::id();
        let incarnation = process_incarnation(pid)
            .map_err(|source| Error::InvalidClaim {
                detail: format!("cannot capture successor process incarnation: {source}"),
            })?
            .ok_or_else(|| Error::InvalidClaim {
                detail: format!("successor process {pid} disappeared before its controller claim"),
            })?;
        if !same_path(&epoch.exe_path, transfer.binary_path()) {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "successor executable '{}' does not match transferred binary '{}'",
                    epoch.exe_path.display(),
                    transfer.binary_path().display()
                ),
            });
        }
        match authority {
            SuccessorAuthority::Spawned => {
                let expected = transfer.spawned_incarnation().ok_or_else(|| {
                    Error::InvalidClaim {
                        detail: "legacy successor Spawned origin has no process incarnation; it cannot create a current controller session"
                            .to_string(),
                    }
                })?;
                if pid != transfer.spawned_pid() || &incarnation != expected {
                    return Err(Error::InvalidClaim {
                        detail: format!(
                            "successor process {pid} does not match its exact Spawned incarnation"
                        ),
                    });
                }
            }
            SuccessorAuthority::Transferred(acceptance) => {
                acceptance
                    .ready()
                    .validate_persisted()
                    .map_err(|detail| Error::InvalidClaim { detail })?;
                let ready = acceptance.ready();
                if matches!(cursor.phase, WalkPhase::R3 | WalkPhase::R4a)
                    || ready.record().campaign_id != *parent.campaign_id()
                    || ready.record().node_id != parent.node_id()
                    || ready.record().runtime_id != record_runtime_id(runtime_id)
                    || ready.commit().mode() != mode
                {
                    return Err(Error::InvalidClaim {
                        detail:
                            "successor restart does not match the exact transferred Ready authority"
                                .to_string(),
                    });
                }
            }
        }
        Ok(Self {
            origin: Origin::Successor { transfer },
            origin_cursor,
            parent,
            profile: admitted.commitment.clone(),
            mode,
            cursor,
            epoch,
            runtime_id: Some(runtime_id),
            pid,
            incarnation: Some(incarnation),
        })
    }

    /// Construct a claim for a byte-verified historical incident fixture.
    #[cfg(test)]
    pub(crate) fn historical(
        source: ContentHash,
        parent: ParentIdentity,
        profile: RunProfileCommitment,
        mode: RunMode,
        cursor: Cursor,
        epoch: ServerEpoch,
    ) -> Self {
        let pid = std::process::id();
        let incarnation = process_incarnation(pid)
            .expect("capture historical fixture process incarnation")
            .expect("test process has a process incarnation");
        Self {
            origin: Origin::Historical { source },
            origin_cursor: cursor.clone(),
            parent,
            profile,
            mode,
            cursor,
            epoch,
            runtime_id: None,
            pid,
            incarnation: Some(incarnation),
        }
    }

    /// Reclaim an existing session only through its replayed creation and
    /// cursor authority.
    ///
    /// This constructor cannot create a session: it copies the immutable
    /// origin binding from `Created` so `Store::claim` can expose a
    /// `RecoveryLease` for the exact historical owner fence.
    pub(crate) fn from_recovery(
        created: &Created,
        cursor: Cursor,
        epoch: ServerEpoch,
    ) -> Result<Self, Error> {
        let origin_cursor = created.cursor.clone().ok_or_else(|| Error::InvalidClaim {
            detail: "controller recovery requires a committed origin cursor".to_string(),
        })?;
        cursor.validate()?;
        let pid = std::process::id();
        let incarnation = process_incarnation(pid).map_err(|source| Error::InvalidClaim {
            detail: format!("cannot capture recovery process incarnation: {source}"),
        })?;
        Ok(Self {
            origin: created.origin.clone(),
            origin_cursor,
            parent: created.parent.clone(),
            profile: created.profile.clone(),
            mode: created.mode,
            cursor,
            epoch,
            runtime_id: created.successor_runtime(),
            pid,
            incarnation,
        })
    }

    pub(crate) fn with_runtime(mut self, runtime_id: RuntimeId) -> Self {
        self.runtime_id = Some(runtime_id);
        self
    }

    #[cfg(test)]
    fn with_pid(mut self, pid: u32) -> Self {
        self.pid = pid;
        self.incarnation = if pid == std::process::id() {
            process_incarnation(pid).ok().flatten()
        } else {
            Some(ProcessIncarnation {
                boot_id: Uuid::from_u128(u128::from(pid)),
                start_ticks: u64::from(pid),
            })
        };
        self
    }
}

/// Admission result for a controller claim.
#[derive(Debug)]
pub(crate) enum Outcome {
    Acquired(Lease<Idle>),
    Conflict(Conflict),
    Recoverable(RecoveryLease),
}

/// Result of a claim that is forbidden from minting normal mutation
/// authority. Recovery retries can therefore converge without appending a new
/// predecessor fence after another process already completed reconciliation.
#[derive(Debug)]
pub(crate) enum RecoveryOutcome {
    Required(RecoveryLease),
    Conflict(Conflict),
    Resolved,
}

/// A competing controller cannot establish the requested durable facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Conflict {
    Locked {
        path: PathBuf,
    },
    Setup {
        session_id: SessionId,
        active: String,
        requested: String,
    },
    Parent {
        session_id: SessionId,
        active: ParentIdentity,
        requested: ParentIdentity,
    },
    Profile {
        session_id: SessionId,
        active_sha256: String,
        requested_sha256: String,
    },
    Mode {
        session_id: SessionId,
        active: RunMode,
        requested: RunMode,
    },
    OriginCursor {
        session_id: SessionId,
        active: Option<Cursor>,
        requested: Cursor,
    },
    Cursor {
        session_id: SessionId,
        active: Option<Cursor>,
        requested: Cursor,
    },
    Schema {
        session_id: SessionId,
        active: String,
        supported: String,
    },
    Abandoned {
        session_id: SessionId,
        detail: String,
    },
}

/// A lock-holding authority for explicit recovery or takeover.
#[derive(Debug)]
pub(crate) struct RecoveryLease {
    lock: Option<File>,
    paths: Paths,
    request: Claim,
    replay: Option<Replay>,
    cause: Option<RecoveryCause>,
    damage: Option<Tail>,
    head: JournalHead,
}

/// Exact handoff recovery resolution certified while the predecessor recovery
/// lock remains held. Visible handoff projections may only be written after
/// this token exists.
pub(crate) struct PreparedHandoff {
    recovery: RecoveryLease,
    resolution: RecoveryResolution,
}

impl PreparedHandoff {
    pub(crate) fn commit(mut self) -> Result<(), RecoveryFailure> {
        match self.recovery.resolve_inner(self.resolution) {
            Ok(()) => Ok(()),
            Err(source) => Err(failure(self.recovery, source)),
        }
    }
}

impl RecoveryLease {
    pub(crate) fn paths(&self) -> &Paths {
        &self.paths
    }

    pub(crate) fn cause(&self) -> Option<&RecoveryCause> {
        self.cause.as_ref()
    }

    /// Reconcile an owner that lost its kernel lock without an in-flight edge.
    pub(crate) fn abandon_owner(self) -> Result<(), RecoveryFailure> {
        self.resolve(RecoveryResolution::AbandonOwner).map(drop)
    }

    /// Permanently close a session whose effectful attempt cannot be safely
    /// reconciled. This preserves the unresolved attempt and never restores
    /// mutation authority.
    pub(crate) fn abandon_session(self, detail: String) -> Result<(), RecoveryFailure> {
        self.resolve(RecoveryResolution::AbandonSession { detail })
            .map(drop)
    }

    /// Admit the exact current epoch requested by this recovery claim.
    pub(crate) fn admit_epoch(self) -> Result<(), RecoveryFailure> {
        let resolution = match self.cause.as_ref() {
            Some(RecoveryCause::EpochChanged {
                prior, requested, ..
            }) => RecoveryResolution::AdmitEpoch {
                prior: prior.clone(),
                next: requested.clone(),
            },
            _ => {
                return Err(failure(
                    self,
                    Error::InvalidResolution {
                        detail: "epoch admission requires an exact EpochChanged recovery cause"
                            .to_string(),
                    },
                ));
            }
        };
        self.resolve(resolution).map(drop)
    }

    /// Preserve and truncate exactly one incomplete final record.
    ///
    /// Malformed complete records and sequence violations remain blockers.
    pub(crate) fn repair_tail(
        mut self,
        expected: &ContentHash,
    ) -> Result<Repaired, RecoveryFailure> {
        match self.repair_tail_inner(expected) {
            Ok(evidence) => Ok(Repaired {
                lease: self,
                evidence,
            }),
            Err(source) => {
                drop(self);
                Err(Failure::Uncertain { source })
            }
        }
    }

    fn repair_tail_inner(&mut self, expected: &ContentHash) -> Result<PathBuf, Error> {
        ensure_fresh(&self.request.epoch)?;
        if let Some(replay) = self.replay.as_ref() {
            replay.ensure_mutable()?;
        }
        if !matches!(
            self.cause,
            Some(RecoveryCause::Journal(Damage::Truncated { .. }))
        ) {
            return Err(Error::RepairUnsupported {
                detail: "only an incomplete final journal record can be repaired".to_string(),
            });
        }
        let tail = self
            .damage
            .as_ref()
            .ok_or_else(|| Error::RepairUnsupported {
                detail: "truncated journal recovery is missing its exact tail evidence".to_string(),
            })?;
        let actual = content_hash(&tail.bytes);
        if &actual != expected {
            return Err(Error::RepairMismatch {
                expected: expected.0.clone(),
                actual: actual.0,
            });
        }

        let session_id = self
            .replay
            .as_ref()
            .and_then(|replay| replay.created.as_ref())
            .map(|created| created.session_id)
            .ok_or_else(|| Error::RepairUnsupported {
                detail: "truncated journal has no durable session to own repair evidence"
                    .to_string(),
            })?;
        let evidence_path = damaged_path(&self.paths.journal, expected);
        preserve_tail(&evidence_path, &tail.bytes)?;
        let entry = Entry::TailRepaired {
            session_id,
            discarded: expected.clone(),
            evidence_path: evidence_path.clone(),
            recorded_at: RecordedAt::now(),
        };
        replace_tail(
            &self.paths.journal,
            &self.head,
            tail,
            &entry,
            &self.request.epoch,
        )?;

        let Load::Ready { lines, head } = load_entries(&self.paths.journal)? else {
            return Err(Error::RepairUnsupported {
                detail: "journal remained damaged after exact tail repair".to_string(),
            });
        };
        let replay = Replay::from_lines(&lines).map_err(Error::Replay)?;
        self.head = head;
        self.cause = replay
            .recovery()
            .or_else(|| replay.epoch_change(&self.request.epoch));
        self.replay = Some(replay);
        self.damage = None;
        Ok(evidence_path)
    }

    /// Resolve one stranded R12 handoff with the exact persisted Ready offer
    /// accepted by the predecessor attempt.
    ///
    /// This is deliberately distinct from generic attempt resolution. Replay
    /// records the recovered transfer certificate separately from ordinary
    /// clean releases, so no other recovery can confer successor authority.
    pub(crate) fn prepare_handoff(
        mut self,
        permit: HandoffPermit,
    ) -> Result<PreparedHandoff, RecoveryFailure> {
        let (acceptance, witness, epoch) = permit.into_parts();
        if epoch.after.as_ref() != Some(&self.request.epoch) {
            return Err(failure(
                self,
                Error::InvalidResolution {
                    detail: "handoff recovery epoch does not match the fresh recovery claim"
                        .to_string(),
                },
            ));
        }
        let resolution = RecoveryResolution::AcceptHandoff {
            acceptance,
            result: AttemptResult::Committed {
                phase: WalkPhase::R13b,
                evidence: witness,
            },
            evidence: None,
            epoch,
        };
        let resolution = match self.certify_resolution(resolution) {
            Ok(resolution) => resolution,
            Err(source) => return Err(failure(self, source)),
        };
        Ok(PreparedHandoff {
            recovery: self,
            resolution,
        })
    }

    /// Resolve the old fence while retaining exclusive recovery authority.
    fn resolve(mut self, resolution: RecoveryResolution) -> Result<Self, RecoveryFailure> {
        match self.resolve_inner(resolution) {
            Ok(()) => Ok(self),
            Err(source) => Err(failure(self, source)),
        }
    }

    fn resolve_inner(&mut self, resolution: RecoveryResolution) -> Result<(), Error> {
        let resolution = self.certify_resolution(resolution)?;
        self.append_resolution(resolution)
    }

    fn certify_resolution(
        &mut self,
        resolution: RecoveryResolution,
    ) -> Result<RecoveryResolution, Error> {
        ensure_fresh(&self.request.epoch)?;
        let cause = self.cause.clone().ok_or(Error::RecoveryResolved)?;
        let replay = self
            .replay
            .as_mut()
            .ok_or_else(|| Error::RepairUnsupported {
                detail: "journal sequence damage must be repaired before resolution".to_string(),
            })?;
        replay.ensure_mutable()?;
        let resolution = certify_resolution(replay, resolution)?;
        validate_resolution_current(replay, &cause, &resolution)?;
        Ok(resolution)
    }

    fn append_resolution(&mut self, resolution: RecoveryResolution) -> Result<(), Error> {
        ensure_fresh(&self.request.epoch)?;
        let cause = self.cause.clone().ok_or(Error::RecoveryResolved)?;
        let replay = self
            .replay
            .as_mut()
            .ok_or_else(|| Error::RepairUnsupported {
                detail: "journal sequence damage must be repaired before resolution".to_string(),
            })?;
        replay.ensure_mutable()?;
        validate_resolution_current(replay, &cause, &resolution)?;
        let created = replay
            .created
            .as_ref()
            .ok_or_else(|| Error::RepairUnsupported {
                detail: "recovery journal has no session creation record".to_string(),
            })?;
        let entry = match (&cause, resolution) {
            (
                RecoveryCause::EpochChanged { .. },
                RecoveryResolution::AdmitEpoch { prior, next },
            ) => Entry::EpochAdmitted {
                session_id: created.session_id,
                prior,
                next,
                recorded_at: RecordedAt::now(),
            },
            (_, resolution) => {
                let owner = replay
                    .active
                    .as_ref()
                    .ok_or_else(|| Error::RepairUnsupported {
                        detail: "recovery journal has no active owner fence".to_string(),
                    })?;
                Entry::Recovered {
                    session_id: created.session_id,
                    fence: owner.fence,
                    resolution,
                    recorded_at: RecordedAt::now(),
                }
            }
        };
        let line = Line {
            number: self.head.index + 1,
            entry: entry.clone(),
        };
        let mut next = replay.clone();
        next.apply(&line).map_err(Error::Replay)?;
        let (_, head) = append_entry(&self.paths.journal, &entry, &self.head)?;
        *replay = next;
        self.head = head;
        self.cause = replay
            .recovery()
            .or_else(|| replay.epoch_change(&self.request.epoch));
        Ok(())
    }

    /// Issue the next fence after the previous one has been resolved.
    pub(crate) fn take_over(mut self) -> Result<Lease<Idle>, RecoveryFailure> {
        if self.cause.is_some() {
            return Err(failure(self, Error::RecoveryPending));
        }
        if let Err(source) = ensure_fresh(&self.request.epoch) {
            return Err(failure(self, source));
        }
        let mut replay = self.replay.take().unwrap_or_default();
        if let Err(source) = replay.ensure_mutable() {
            self.replay = Some(replay);
            return Err(failure(self, source));
        }
        let mut head = self.head.clone();
        let session_id = if let Some(created) = replay.created.as_ref() {
            created.session_id
        } else {
            if let Err(source) = validate_new_claim(&self.request) {
                return Err(failure(self, source));
            }
            let session_id = SessionId::new();
            let entry = Entry::Created {
                schema_version: SCHEMA_VERSION.to_string(),
                session_id,
                origin: self.request.origin.clone(),
                parent: self.request.parent.clone(),
                profile: self.request.profile.clone(),
                mode: self.request.mode,
                cursor: Some(self.request.origin_cursor.clone()),
                recorded_at: RecordedAt::now(),
            };
            let line = Line {
                number: head.index + 1,
                entry: entry.clone(),
            };
            let mut next = replay.clone();
            if let Err(source) = next.apply(&line).map_err(Error::Replay) {
                self.replay = Some(replay);
                return Err(failure(self, source));
            }
            head = match append_entry(&self.paths.journal, &entry, &head) {
                Ok((_, head)) => head,
                Err(source) => {
                    self.replay = Some(replay);
                    return Err(failure(self, source));
                }
            };
            replay = next;
            self.head = head.clone();
            session_id
        };
        let fence = match replay.max_fence.checked_add(1) {
            Some(value) => Fence(value),
            None => {
                self.replay = Some(replay);
                return Err(failure(self, Error::FenceExhausted { session_id }));
            }
        };
        let entry = Entry::Acquired {
            session_id,
            fence,
            epoch: self.request.epoch.clone(),
            runtime_id: self.request.runtime_id,
            pid: self.request.pid,
            incarnation: self.request.incarnation.clone(),
            recorded_at: RecordedAt::now(),
        };
        if let Err(source) = ensure_fresh(&self.request.epoch) {
            self.replay = Some(replay);
            return Err(failure(self, source));
        }
        let line = Line {
            number: head.index + 1,
            entry: entry.clone(),
        };
        let mut next = replay.clone();
        if let Err(source) = next.apply(&line).map_err(Error::Replay) {
            self.replay = Some(replay);
            return Err(failure(self, source));
        }
        head = match append_entry(&self.paths.journal, &entry, &head) {
            Ok((_, head)) => head,
            Err(source) => {
                self.replay = Some(replay);
                return Err(failure(self, source));
            }
        };
        replay = next;
        let cursor = replay
            .cursor
            .clone()
            .expect("mutable controller session has a committed cursor");
        let lock = self.lock.take().expect("recovery lease owns its lock");
        let handoff = self.request.origin.handoff_path();
        Ok(Lease {
            lock,
            paths: self.paths,
            session_id,
            parent: self.request.parent,
            profile: self.request.profile,
            mode: self.request.mode,
            cursor,
            epoch: self.request.epoch,
            runtime_id: self.request.runtime_id,
            handoff,
            fence,
            head,
            attempts: replay.attempts,
            state: Idle,
        })
    }
}

/// Durable reason a new owner requires reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryCause {
    Journal(Damage),
    OwnerLost {
        session_id: SessionId,
        fence: Fence,
        pid: u32,
        incarnation: Option<ProcessIncarnation>,
        runtime_id: Option<RuntimeId>,
        epoch: ServerEpoch,
    },
    AttemptPending {
        session_id: SessionId,
        fence: Fence,
        intent: AttemptIntent,
    },
    AttemptIndeterminate {
        session_id: SessionId,
        fence: Fence,
        intent: AttemptIntent,
        detail: String,
    },
    EpochChanged {
        session_id: SessionId,
        prior: ServerEpoch,
        requested: ServerEpoch,
    },
}

/// Explicit old-fence terminalization chosen after reconstruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub(crate) enum RecoveryResolution {
    AbandonOwner,
    AbandonSession {
        detail: String,
    },
    ResolveAttempt {
        transition_id: TransitionId,
        result: AttemptResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<CursorEvidence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epoch: Option<EpochReceipt>,
    },
    /// Terminalize an exact stranded R12 attempt by accepting the successor's
    /// already-atomic Ready offer. This never projects as an ordinary release.
    AcceptHandoff {
        acceptance: HandoffAcceptance,
        result: AttemptResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<CursorEvidence>,
        epoch: EpochReceipt,
    },
    AdmitEpoch {
        prior: ServerEpoch,
        next: ServerEpoch,
    },
}

/// Journal damage is evidence, never permission to skip arbitrary records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Damage {
    Truncated { line: usize, tail: ContentHash },
    Malformed { line: usize, detail: String },
    Sequence { line: usize, detail: String },
}

/// Filesystem-backed session owner.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    root: PathBuf,
}

struct LockedClaim {
    lock: File,
    paths: Paths,
    request: Claim,
    replay: Option<Replay>,
    cause: Option<RecoveryCause>,
    damage: Option<Tail>,
    head: JournalHead,
}

/// Lock-free projection of durable session evidence.
///
/// An active owner here means only that the journal has not recorded release;
/// inspection does not acquire the kernel lock and therefore does not label an
/// owner live or lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionSnapshot {
    pub(crate) paths: Paths,
    pub(crate) journal_revision: usize,
    pub(crate) created: Option<Created>,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) last_epoch: Option<ServerEpoch>,
    pub(crate) active: Option<Owner>,
    /// Fence of the latest explicit `Released` record. Recovery clears active
    /// ownership but deliberately does not project as a clean release.
    pub(crate) released: Option<Fence>,
    releases: BTreeSet<Fence>,
    recoveries: BTreeSet<Fence>,
    handoffs: BTreeMap<Fence, HandoffAcceptance>,
    ready: Option<ReadyReceipt>,
    pub(crate) attempts: Vec<Attempt>,
    pub(crate) damage: Option<Damage>,
    pub(crate) abandoned: Option<String>,
}

impl SessionSnapshot {
    /// Return the exact already-committed predecessor handoff attempt for one
    /// owner fence. This is used only to close the crash window between a
    /// committed R13b receipt and the later clean release record.
    pub(crate) fn committed_handoff(
        &self,
        session_id: SessionId,
        fence: Fence,
    ) -> Option<AttemptReceipt> {
        if self.created.as_ref()?.session_id() != session_id {
            return None;
        }
        self.attempts.iter().find_map(|attempt| {
            let receipt = attempt.terminal()?;
            (receipt.fence == fence
                && receipt.intent.expected == WalkPhase::R12
                && receipt.intent.targets.contains(&WalkPhase::R13b)
                && receipt.intent.allow_git_changes
                && matches!(
                    receipt.result,
                    AttemptResult::Committed {
                        phase: WalkPhase::R13b,
                        ..
                    }
                ))
            .then_some(receipt.clone())
        })
    }

    /// Validate the exact predecessor attempt named by a handoff acceptance.
    /// Once that exact R12->R13b fence was committed and cleanly released, a
    /// later predecessor-finalization claim cannot revoke the transferred
    /// successor authority.
    pub(crate) fn accepted_handoff(
        &self,
        parent: &ParentIdentity,
        acceptance: &HandoffAcceptance,
    ) -> Result<bool, Error> {
        let attempt = acceptance.attempt();
        if let Some(damage) = &self.damage {
            return Err(Error::InvalidClaim {
                detail: format!("cannot validate handoff from damaged session: {damage:?}"),
            });
        }
        let created = self.created.as_ref().ok_or_else(|| Error::InvalidClaim {
            detail: "handoff predecessor has no created controller session".to_string(),
        })?;
        if created.parent() != parent || created.session_id() != attempt.session() {
            return Err(Error::InvalidClaim {
                detail: "handoff acceptance names a different predecessor session".to_string(),
            });
        }
        if self
            .cursor
            .as_ref()
            .is_none_or(|cursor| !matches!(cursor.phase, WalkPhase::R13b | WalkPhase::R14b))
        {
            return Ok(false);
        }
        let committed = self.attempts.iter().find_map(|recorded| {
            let receipt = recorded.terminal()?;
            (receipt.intent.transition_id == attempt.transition()
                && receipt.intent.expected == WalkPhase::R12
                && receipt.fence == attempt.fence()
                && matches!(
                    receipt.result,
                    AttemptResult::Committed {
                        phase: WalkPhase::R13b,
                        ..
                    }
                ))
            .then_some(recorded)
        });
        let Some(committed) = committed else {
            return Ok(false);
        };
        let transferred = match committed {
            Attempt::Finished(_) => self.releases.contains(&attempt.fence()),
            Attempt::Recovered { .. } => {
                self.recoveries.contains(&attempt.fence())
                    && self.handoffs.get(&attempt.fence()) == Some(acceptance)
            }
            Attempt::Pending { .. } => false,
        };
        Ok(transferred)
    }

    /// Read the exact Ready commitment atomically recorded with the R4c lease
    /// release.
    pub(crate) fn ready_commit(&self, runtime_id: RuntimeId) -> Result<ReadyCommit, Error> {
        let receipt = self
            .ready_receipt(runtime_id)?
            .ok_or_else(|| Error::InvalidClaim {
                detail: "successor session has no atomic R4c Ready release".to_string(),
            })?;
        Ok(receipt.commit().clone())
    }

    /// Revalidate the full atomically released Ready authority. Transition
    /// journal and channel records are projections of this exact receipt.
    pub(crate) fn validate_ready_receipt(
        &self,
        runtime_id: RuntimeId,
        receipt: &ReadyReceipt,
    ) -> Result<(), Error> {
        receipt
            .validate_persisted()
            .map_err(|detail| Error::InvalidClaim { detail })?;
        let persisted = self.ready.as_ref().ok_or_else(|| Error::InvalidClaim {
            detail: "successor session has no atomic R4c Ready release".to_string(),
        })?;
        if persisted != receipt {
            return Err(Error::InvalidClaim {
                detail: "Ready projection does not match the atomic session receipt".to_string(),
            });
        }
        let projected = self.projected_ready(runtime_id)?;
        if receipt.commit() != &projected {
            return Err(Error::InvalidClaim {
                detail: "Ready receipt does not match the durable R4c transition".to_string(),
            });
        }
        if !self.releases.contains(&projected.fence()) {
            return Err(Error::InvalidClaim {
                detail: "Ready receipt does not have a clean release on its R4c commit fence"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Return the exact post-edge epoch certified by the terminal R4c attempt
    /// that published this Ready receipt.
    pub(crate) fn ready_epoch(
        &self,
        runtime_id: RuntimeId,
        receipt: &ReadyReceipt,
    ) -> Result<&ServerEpoch, Error> {
        self.validate_ready_receipt(runtime_id, receipt)?;
        let commit = receipt.commit();
        let attempt = self
            .attempts
            .iter()
            .find_map(|attempt| {
                let terminal = attempt.terminal()?;
                (terminal.intent.transition_id == commit.transition_id()
                    && terminal.fence == commit.fence())
                .then_some(terminal)
            })
            .ok_or_else(|| Error::InvalidClaim {
                detail: "Ready receipt has no exact terminal R4c attempt".to_string(),
            })?;
        attempt
            .epoch
            .as_ref()
            .and_then(|epoch| epoch.after.as_ref())
            .ok_or_else(|| Error::InvalidClaim {
                detail: "Ready receipt has no complete terminal R4c epoch".to_string(),
            })
    }

    pub(crate) fn ready_receipt(
        &self,
        runtime_id: RuntimeId,
    ) -> Result<Option<&ReadyReceipt>, Error> {
        let Some(receipt) = self.ready.as_ref() else {
            return Ok(None);
        };
        self.validate_ready_receipt(runtime_id, receipt)?;
        Ok(Some(receipt))
    }

    /// Revalidate a previously published Ready receipt after the successor has
    /// advanced beyond R4c. The original committed edge remains in the session
    /// journal even when the current cursor has moved forward.
    pub(crate) fn validate_ready_commit(
        &self,
        runtime_id: RuntimeId,
        commit: &ReadyCommit,
    ) -> Result<(), Error> {
        let receipt = self
            .ready_receipt(runtime_id)?
            .ok_or_else(|| Error::InvalidClaim {
                detail: "successor session has no atomic R4c Ready release".to_string(),
            })?;
        if receipt.commit() != commit {
            return Err(Error::InvalidClaim {
                detail: "Ready projection does not match the atomic session commit".to_string(),
            });
        }
        Ok(())
    }

    fn projected_ready(&self, runtime_id: RuntimeId) -> Result<ReadyCommit, Error> {
        if let Some(damage) = &self.damage {
            return Err(Error::InvalidClaim {
                detail: format!("cannot validate Ready from damaged session evidence: {damage:?}"),
            });
        }
        let created = self.created.as_ref().ok_or_else(|| Error::InvalidClaim {
            detail: "cannot validate Ready without a created successor session".to_string(),
        })?;
        if created.successor_runtime() != Some(runtime_id) {
            return Err(Error::InvalidClaim {
                detail: "Ready runtime does not match the successor session origin".to_string(),
            });
        }
        let (receipt, evidence) = self
            .attempts
            .iter()
            .rev()
            .find_map(|attempt| {
                let receipt = attempt.terminal()?;
                match &receipt.result {
                    AttemptResult::Committed { phase, evidence } if *phase == WalkPhase::R4c => {
                        Some((receipt, evidence.clone()))
                    }
                    _ => None,
                }
            })
            .ok_or_else(|| Error::InvalidClaim {
                detail: "R4c cursor has no committed transition receipt".to_string(),
            })?;
        let cursor = Cursor {
            phase: WalkPhase::R4c,
            evidence,
        };
        ReadyCommit::new(
            created.session_id,
            receipt.intent.transition_id,
            receipt.fence,
            cursor.clone(),
            created.mode,
        )
        .map_err(|detail| Error::InvalidClaim { detail })
    }
}

impl Store {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn for_manifest(manifest: &Path) -> Self {
        let campaign = manifest.parent().unwrap_or_else(|| Path::new("."));
        Self::new(campaign.join("prototype1/control"))
    }

    pub(crate) fn paths(&self, parent: &ParentIdentity) -> Paths {
        let root = self.root.join("sessions").join(coordinate_key(parent));
        Paths {
            lock: root.join(LOCK_FILE),
            journal: root.join(JOURNAL_FILE),
            root,
        }
    }

    /// Inspect durable evidence without creating paths, opening the lock file,
    /// incrementing a fence, or deciding whether a recorded owner is alive.
    pub(crate) fn inspect(
        &self,
        parent: &ParentIdentity,
    ) -> Result<Option<SessionSnapshot>, Error> {
        let paths = self.paths(parent);
        match fs::metadata(&paths.journal) {
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(Error::Read {
                    path: paths.journal.clone(),
                    source,
                });
            }
        }
        let (lines, damage) = match load_entries(&paths.journal)? {
            Load::Ready { lines, .. } => (lines, None),
            Load::Damaged { lines, cause, .. } => (lines, Some(cause)),
        };
        if lines.is_empty() && damage.is_none() {
            return Ok(None);
        }
        let replay = Replay::from_lines(&lines).map_err(Error::Replay)?;
        let attempts = replay
            .attempt_order
            .iter()
            .filter_map(|id| replay.attempts.get(id).cloned())
            .collect();
        Ok(Some(SessionSnapshot {
            paths,
            journal_revision: lines.len(),
            created: replay.created,
            cursor: replay.cursor,
            last_epoch: replay.last_epoch,
            active: replay.active,
            released: replay.released,
            releases: replay.releases,
            recoveries: replay.recoveries,
            handoffs: replay.handoffs,
            ready: replay.ready,
            attempts,
            damage,
            abandoned: replay.abandoned,
        }))
    }

    /// Probe the journal publication barrier without waiting. Timeout
    /// arbitration uses this after stopping a successor so it never blocks on
    /// an exclusive append lock held by that stopped process.
    pub(crate) fn journal_idle(&self, parent: &ParentIdentity) -> Result<bool, Error> {
        let path = self.paths(parent).journal;
        let file = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(source) => return Err(Error::Open { path, source }),
        };
        try_lock_shared(&file, &path)
    }

    /// Try to claim the stable lock inode without waiting.
    pub(crate) fn claim(&self, request: Claim) -> Result<Outcome, Error> {
        let claim = match self.lock_claim(request)? {
            Ok(claim) => claim,
            Err(conflict) => return Ok(Outcome::Conflict(conflict)),
        };
        if claim.cause.is_some() {
            return Ok(Outcome::Recoverable(claim.into_recovery()));
        }
        acquire_lease(
            claim.lock,
            claim.paths,
            claim.request,
            claim.replay.unwrap_or_default(),
            claim.head,
        )
        .map(Outcome::Acquired)
    }

    /// Claim only if the durable session is still at the exact client-observed
    /// version when the kernel lock is acquired.
    ///
    /// This closes the gap between socket job admission and asynchronous job
    /// execution without changing the unguarded claim path used by direct
    /// controller adapters.
    pub(crate) fn claim_version(
        &self,
        request: Claim,
        expected: &SessionVersion,
    ) -> Result<Outcome, Error> {
        let claim = match self.lock_claim(request)? {
            Ok(claim) => claim,
            Err(conflict) => return Ok(Outcome::Conflict(conflict)),
        };
        let actual = claim.version()?;
        if &actual != expected {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "controller version changed before lease acquisition: expected {expected:?}, found {actual:?}"
                ),
            });
        }
        if claim.cause.is_some() {
            return Ok(Outcome::Recoverable(claim.into_recovery()));
        }
        acquire_lease(
            claim.lock,
            claim.paths,
            claim.request,
            claim.replay.unwrap_or_default(),
            claim.head,
        )
        .map(Outcome::Acquired)
    }

    /// Claim only when durable replay still requires explicit recovery.
    /// Unlike `claim`, this never appends `Acquired` or issues a new fence.
    pub(crate) fn claim_recovery(&self, request: Claim) -> Result<RecoveryOutcome, Error> {
        let claim = match self.lock_claim(request)? {
            Ok(claim) => claim,
            Err(conflict) => return Ok(RecoveryOutcome::Conflict(conflict)),
        };
        if claim.cause.is_some() {
            Ok(RecoveryOutcome::Required(claim.into_recovery()))
        } else {
            Ok(RecoveryOutcome::Resolved)
        }
    }

    /// Claim recovery only if the exact client-observed session version still
    /// holds under the same kernel lock used for resolution.
    pub(crate) fn claim_recovery_version(
        &self,
        request: Claim,
        expected: &SessionVersion,
    ) -> Result<RecoveryOutcome, Error> {
        let claim = match self.lock_claim(request)? {
            Ok(claim) => claim,
            Err(conflict) => return Ok(RecoveryOutcome::Conflict(conflict)),
        };
        let actual = claim.version()?;
        if &actual != expected {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "controller version changed before recovery acquisition: expected {expected:?}, found {actual:?}"
                ),
            });
        }
        if claim.cause.is_some() {
            Ok(RecoveryOutcome::Required(claim.into_recovery()))
        } else {
            Ok(RecoveryOutcome::Resolved)
        }
    }

    fn lock_claim(&self, request: Claim) -> Result<Result<LockedClaim, Conflict>, Error> {
        let paths = self.paths(&request.parent);
        create_dir_synced(&paths.root)?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&paths.lock)
            .map_err(|source| Error::Open {
                path: paths.lock.clone(),
                source,
            })?;
        sync_dir(&paths.root)?;
        if !try_lock(&lock, &paths.lock)? {
            return Ok(Err(Conflict::Locked {
                path: paths.lock.clone(),
            }));
        }

        let (lines, damage, cause, head) = match load_entries(&paths.journal)? {
            Load::Ready { lines, head } => (lines, None, None, head),
            Load::Damaged {
                lines,
                cause,
                tail,
                head,
            } => (lines, Some(tail), Some(RecoveryCause::Journal(cause)), head),
        };
        let (replay, cause) = match Replay::from_lines(&lines) {
            Ok(replay) => (Some(replay), cause),
            Err(cause) => (None, Some(RecoveryCause::Journal(cause))),
        };
        if let Some(replay) = replay.as_ref()
            && let Some(conflict) = binding_conflict(replay, &request)
        {
            return Ok(Err(conflict));
        }
        let cause = cause
            .or_else(|| replay.as_ref().and_then(Replay::recovery))
            .or_else(|| {
                replay
                    .as_ref()
                    .and_then(|replay| replay.epoch_change(&request.epoch))
            });
        Ok(Ok(LockedClaim {
            lock,
            paths,
            request,
            replay,
            cause,
            damage,
            head,
        }))
    }
}

impl LockedClaim {
    fn version(&self) -> Result<SessionVersion, Error> {
        let replay = self.replay.as_ref().ok_or_else(|| Error::InvalidClaim {
            detail: "damaged controller journal has no exact session version".to_string(),
        })?;
        Ok(SessionVersion {
            session_id: replay.created.as_ref().map(|created| created.session_id),
            cursor: replay.cursor.clone(),
            journal_revision: self.head.index,
        })
    }

    fn into_recovery(self) -> RecoveryLease {
        RecoveryLease {
            lock: Some(self.lock),
            paths: self.paths,
            request: self.request,
            replay: self.replay,
            cause: self.cause,
            damage: self.damage,
            head: self.head,
        }
    }
}

/// Lease state with no in-flight transition attempt.
#[derive(Debug)]
pub(crate) struct Idle;

/// Lease state after intent is durable and before an outcome is durable.
#[derive(Debug)]
pub(crate) struct Pending {
    intent: AttemptIntent,
}

/// The edge may have crossed its effect boundary and must be reconstructed
/// before mutation authority can become idle again.
#[derive(Debug)]
pub(crate) struct Uncertain;

/// Live authority token. It is neither cloneable nor serializable.
#[derive(Debug)]
pub(crate) struct Lease<S> {
    lock: File,
    paths: Paths,
    session_id: SessionId,
    parent: ParentIdentity,
    profile: RunProfileCommitment,
    mode: RunMode,
    cursor: Cursor,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    handoff: Option<PathBuf>,
    fence: Fence,
    head: JournalHead,
    attempts: BTreeMap<TransitionId, Attempt>,
    state: S,
}

impl<S> Lease<S> {
    pub(crate) fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub(crate) fn campaign_id(&self) -> &CampaignId {
        self.parent.campaign_id()
    }

    pub(crate) fn parent(&self) -> &ParentIdentity {
        &self.parent
    }

    pub(crate) fn profile(&self) -> &RunProfileCommitment {
        &self.profile
    }

    pub(crate) fn mode(&self) -> RunMode {
        self.mode
    }

    pub(crate) fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub(crate) fn epoch(&self) -> &ServerEpoch {
        &self.epoch
    }

    pub(crate) fn runtime_id(&self) -> Option<RuntimeId> {
        self.runtime_id
    }

    pub(crate) fn handoff_path(&self) -> Option<&Path> {
        self.handoff.as_deref()
    }

    pub(crate) fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) fn paths(&self) -> &Paths {
        &self.paths
    }
}

impl Lease<Idle> {
    /// Prepare the exact R4c commitment this live fence may atomically release
    /// as successor Ready authority.
    pub(crate) fn prepare_ready(&self, runtime_id: RuntimeId) -> Result<ReadyCommit, Error> {
        if self.runtime_id != Some(runtime_id) {
            return Err(Error::InvalidClaim {
                detail: "Ready runtime does not match the successor lease".to_string(),
            });
        }
        if self.cursor.phase != WalkPhase::R4c {
            return Err(Error::InvalidClaim {
                detail: format!(
                    "successor Ready requires the live lease at R4c, found {}",
                    self.cursor.phase
                ),
            });
        }
        let receipt = self
            .attempts
            .values()
            .rev()
            .filter_map(Attempt::terminal)
            .find(|receipt| {
                receipt.fence == self.fence
                    && matches!(
                        &receipt.result,
                        AttemptResult::Committed { phase, evidence }
                            if *phase == WalkPhase::R4c && evidence == &self.cursor.evidence
                    )
            })
            .ok_or_else(|| Error::InvalidClaim {
                detail: "live successor fence has no exact committed R4c transition".to_string(),
            })?;
        ReadyCommit::new(
            self.session_id,
            receipt.intent.transition_id,
            self.fence,
            self.cursor.clone(),
            self.mode,
        )
        .map_err(|detail| Error::InvalidClaim { detail })
    }

    pub(crate) fn intent(&self, allow_git_changes: bool) -> Result<AttemptIntent, Error> {
        self.intent_with_live_api(false, allow_git_changes)
    }

    pub(crate) fn intent_with_live_api(
        &self,
        allow_live_api: bool,
        allow_git_changes: bool,
    ) -> Result<AttemptIntent, Error> {
        let mut retry = 0_u32;
        for attempt in self.attempts.values() {
            let Some(receipt) = attempt.terminal() else {
                continue;
            };
            if receipt.intent.cursor() != self.cursor
                || receipt.intent.allow_live_api != allow_live_api
                || receipt.intent.allow_git_changes != allow_git_changes
                || receipt.intent.epoch.transition_graph_version
                    != self.epoch.transition_graph_version
                || !matches!(
                    receipt.result,
                    AttemptResult::Rejected { .. } | AttemptResult::Cancelled { .. }
                )
            {
                continue;
            }
            retry = retry.max(
                receipt
                    .intent
                    .retry
                    .checked_add(1)
                    .ok_or(Error::RetryExhausted)?,
            );
        }
        AttemptIntent::with_retry(
            self.session_id,
            self.cursor.clone(),
            allow_live_api,
            allow_git_changes,
            self.epoch.clone(),
            retry,
        )
    }

    /// Durably admit one exact typed edge only through the sealed common driver.
    pub(crate) fn admit(
        self,
        permit: &ControlPermit,
        intent: AttemptIntent,
    ) -> Result<Begin, AdmissionFailure> {
        let mismatch = if permit.session_id() != self.session_id {
            Some("control permit belongs to a different session".to_string())
        } else if permit.fence() != self.fence {
            Some("control permit belongs to a different controller fence".to_string())
        } else if permit.repo_root() != self.epoch.repo_root.as_path() {
            Some("control permit belongs to a different repository epoch".to_string())
        } else if permit.transition_id() != intent.transition_id
            || permit.expected() != intent.expected
            || permit.targets() != intent.targets.as_slice()
            || permit.allow_live_api() != intent.allow_live_api
            || permit.allow_git_changes() != intent.allow_git_changes
        {
            Some("control permit does not match the exact transition intent".to_string())
        } else {
            None
        };
        if let Some(detail) = mismatch {
            return Err(failure(self, Error::InvalidIntent { detail }));
        }
        self.begin(intent)
    }

    /// Raw journal admission is private; session tests exercise it directly.
    fn begin(self, intent: AttemptIntent) -> Result<Begin, AdmissionFailure> {
        if let Some(attempt) = self.attempts.get(&intent.transition_id).cloned() {
            return match attempt {
                Attempt::Pending { .. } => Err(failure(
                    self,
                    Error::AttemptPending {
                        transition_id: intent.transition_id,
                    },
                )),
                Attempt::Finished(receipt) | Attempt::Recovered { receipt, .. }
                    if receipt.intent == intent =>
                {
                    Ok(Begin::Existing {
                        lease: self,
                        receipt,
                    })
                }
                Attempt::Finished(_) | Attempt::Recovered { .. } => Err(failure(
                    self,
                    Error::AttemptMismatch {
                        transition_id: intent.transition_id,
                    },
                )),
            };
        }
        if let Err(source) = intent.validate_current() {
            return Err(failure(self, source));
        }
        if let Err(source) = intent.validate_for(self.session_id) {
            return Err(failure(self, source));
        }
        if intent.epoch != self.epoch {
            return Err(failure(self, Error::EpochMismatch));
        }
        if let Err(source) = self.epoch.ensure_not_stale_now() {
            return Err(failure(
                self,
                Error::EpochStale {
                    detail: source.to_string(),
                },
            ));
        }
        let requested = intent.cursor();
        if requested != self.cursor {
            let active = self.cursor.clone();
            return Err(failure(self, Error::CursorMismatch { active, requested }));
        }

        let entry = Entry::Began {
            session_id: self.session_id,
            fence: self.fence,
            intent: intent.clone(),
            recorded_at: RecordedAt::now(),
        };
        let (append, head) = match append_entry(&self.paths.journal, &entry, &self.head) {
            Ok(appended) => appended,
            Err(source) => {
                return Err(failure(self, source));
            }
        };
        let mut attempts = self.attempts;
        attempts.insert(
            intent.transition_id,
            Attempt::Pending {
                fence: self.fence,
                intent: intent.clone(),
            },
        );
        Ok(Begin::Started {
            lease: Lease {
                lock: self.lock,
                paths: self.paths,
                session_id: self.session_id,
                parent: self.parent,
                profile: self.profile,
                mode: self.mode,
                cursor: self.cursor,
                epoch: self.epoch,
                runtime_id: self.runtime_id,
                handoff: self.handoff,
                fence: self.fence,
                head,
                attempts,
                state: Pending { intent },
            },
            append,
        })
    }

    /// Record a clean release. An uncertain append drops the lock so the next
    /// claimant must replay the durable journal before proceeding.
    pub(crate) fn release(self) -> Result<JournalAppendReceipt, ReleaseFailure> {
        let entry = Entry::Released {
            session_id: self.session_id,
            fence: self.fence,
            ready: None,
            recorded_at: RecordedAt::now(),
        };
        match append_entry(&self.paths.journal, &entry, &self.head) {
            Ok((receipt, _)) => Ok(receipt),
            Err(source) => Err(failure(self, source)),
        }
    }

    /// Atomically release one exact successor bootstrap fence together with
    /// its full Ready authority. No later projection is needed to prove Ready.
    pub(crate) fn release_ready(
        self,
        ready: ReadyReceipt,
    ) -> Result<JournalAppendReceipt, ReleaseFailure> {
        let validation = ready
            .validate()
            .map_err(|detail| Error::InvalidClaim { detail })
            .and_then(|()| {
                let runtime_id = self.runtime_id.ok_or_else(|| Error::InvalidClaim {
                    detail: "genesis controller cannot release successor Ready".to_string(),
                })?;
                let expected = self.prepare_ready(runtime_id)?;
                if ready.commit() != &expected {
                    return Err(Error::InvalidClaim {
                        detail: "Ready receipt does not match the live R4c lease".to_string(),
                    });
                }
                let record = ready.record();
                let incarnation =
                    process_incarnation(record.pid).map_err(|source| Error::InvalidClaim {
                        detail: format!(
                            "cannot verify successor Ready process incarnation: {source}"
                        ),
                    })?;
                if record.campaign_id != *self.parent.campaign_id()
                    || record.node_id != self.parent.node_id()
                    || record.runtime_id != record_runtime_id(runtime_id)
                    || record.pid != std::process::id()
                    || record.incarnation != incarnation
                    || ready
                        .endpoint()
                        .is_some_and(|endpoint| endpoint.repo_root() != self.epoch.repo_root)
                {
                    return Err(Error::InvalidClaim {
                        detail: "Ready receipt does not match the live successor owner".to_string(),
                    });
                }
                Ok(())
            });
        if let Err(source) = validation {
            return Err(failure(self, source));
        }
        let entry = Entry::Released {
            session_id: self.session_id,
            fence: self.fence,
            ready: Some(ready),
            recorded_at: RecordedAt::now(),
        };
        match append_entry(&self.paths.journal, &entry, &self.head) {
            Ok((receipt, _)) => Ok(receipt),
            Err(source) => Err(failure(self, source)),
        }
    }
}

impl Lease<Pending> {
    pub(crate) fn intent(&self) -> &AttemptIntent {
        &self.state.intent
    }

    /// Commit only an effect minted by the canonical driver for this exact
    /// durable attempt. Unbound steps from legacy adapters cannot advance a
    /// session cursor.
    pub(crate) fn commit(self, effect: ControlEffect) -> Result<Finished, FinishFailure> {
        let intent = &self.state.intent;
        let transition = effect.transition();
        let mismatch = if effect.session_id() != self.session_id {
            Some("driver effect belongs to a different controller session".to_string())
        } else if effect.fence() != self.fence {
            Some("driver effect belongs to a different controller fence".to_string())
        } else if effect.repo_root() != intent.epoch.repo_root.as_path() {
            Some("driver effect belongs to a different repository epoch".to_string())
        } else if effect.transition_id() != intent.transition_id {
            Some("driver effect belongs to a different transition attempt".to_string())
        } else if transition.from() != intent.expected {
            Some(format!(
                "driver effect starts at {} instead of admitted source {}",
                transition.from(),
                intent.expected
            ))
        } else if !intent.targets.contains(&transition.to()) {
            Some(format!(
                "driver effect target {} is not one of {:?}",
                transition.to(),
                intent.targets
            ))
        } else {
            None
        };
        if let Some(detail) = mismatch {
            return Err(failure(self, Error::ResultMismatch { detail }));
        }
        self.finish(AttemptResult::Committed {
            phase: transition.to(),
            evidence: effect.witness().clone(),
        })
    }

    /// Record a proven pre-effect rejection without letting a caller choose a
    /// phase or cursor digest.
    pub(crate) fn reject(
        self,
        permit: &ControlPermit,
        detail: impl Into<String>,
    ) -> Result<Finished, FinishFailure> {
        if let Err(source) = self.validate_permit(permit) {
            return Err(failure(self, source));
        }
        let result = AttemptResult::Rejected {
            phase: self.state.intent.expected,
            evidence: self.state.intent.evidence.clone(),
            detail: detail.into(),
        };
        self.finish(result)
    }

    /// Record an edge whose effect boundary could not be proven after the
    /// attempt. This deliberately drops idle mutation authority.
    pub(crate) fn mark_indeterminate(
        self,
        permit: &ControlPermit,
        detail: impl Into<String>,
    ) -> Result<Finished, FinishFailure> {
        if let Err(source) = self.validate_permit(permit) {
            return Err(failure(self, source));
        }
        self.finish(AttemptResult::Indeterminate {
            phase: None,
            evidence: None,
            detail: detail.into(),
        })
    }

    fn validate_permit(&self, permit: &ControlPermit) -> Result<(), Error> {
        let intent = &self.state.intent;
        if permit.session_id() != self.session_id
            || permit.fence() != self.fence
            || permit.transition_id() != intent.transition_id
            || permit.repo_root() != intent.epoch.repo_root.as_path()
            || permit.expected() != intent.expected
            || permit.targets() != intent.targets.as_slice()
            || permit.allow_live_api() != intent.allow_live_api
            || permit.allow_git_changes() != intent.allow_git_changes
        {
            return Err(Error::ResultMismatch {
                detail: "control permit no longer matches the pending attempt".to_string(),
            });
        }
        Ok(())
    }

    /// Finish the one attempt admitted under this lease and fence.
    fn finish(self, result: AttemptResult) -> Result<Finished, FinishFailure> {
        if let Err(source) = validate_result_current(&self.state.intent, &result) {
            return Err(failure(self, source));
        }
        let before = self.epoch.clone();
        let (after, capture_error) = match ServerEpoch::capture(&before.repo_root) {
            Ok(after) => (Some(after), None),
            Err(source) => (None, Some(source.to_string())),
        };
        let epoch_receipt = EpochReceipt {
            before: before.clone(),
            after: after.clone(),
        };
        let (result, next_epoch) = if let Some(detail) = capture_error {
            (
                AttemptResult::Indeterminate {
                    phase: None,
                    evidence: None,
                    detail: format!("controller epoch capture failed after the edge: {detail}"),
                },
                before,
            )
        } else {
            settle_epoch(&self.state.intent, result, &epoch_receipt)
        };
        let (result, evidence) = match result {
            AttemptResult::Committed {
                phase,
                evidence: witness,
            } => {
                let evidence = match CursorEvidence::new(
                    self.session_id,
                    &self.parent,
                    &self.profile,
                    &self.state.intent,
                    phase,
                    &witness,
                    &epoch_receipt,
                ) {
                    Ok(evidence) => evidence,
                    Err(error) => {
                        return Err(failure(
                            self,
                            Error::ResultMismatch {
                                detail: error.to_string(),
                            },
                        ));
                    }
                };
                let cursor = match evidence.cursor() {
                    Ok(cursor) => cursor,
                    Err(error) => {
                        return Err(failure(
                            self,
                            Error::ResultMismatch {
                                detail: error.to_string(),
                            },
                        ));
                    }
                };
                (
                    AttemptResult::Committed {
                        phase,
                        evidence: cursor.evidence,
                    },
                    Some(evidence),
                )
            }
            result => (result, None),
        };
        if let Err(source) = validate_result_current(&self.state.intent, &result) {
            return Err(failure(self, source));
        }
        let next_cursor = match &result {
            AttemptResult::Committed { phase, evidence } => Cursor {
                phase: *phase,
                evidence: evidence.clone(),
            },
            AttemptResult::Rejected { .. }
            | AttemptResult::Cancelled { .. }
            | AttemptResult::Indeterminate { .. } => self.cursor.clone(),
        };
        let transition_id = self.state.intent.transition_id;
        let entry = Entry::Finished {
            session_id: self.session_id,
            transition_id,
            fence: self.fence,
            result: result.clone(),
            evidence: evidence.clone(),
            epoch: Some(epoch_receipt.clone()),
            recorded_at: RecordedAt::now(),
        };
        let (append, next_head) = match append_entry(&self.paths.journal, &entry, &self.head) {
            Ok(appended) => appended,
            Err(source) => {
                return Err(failure(self, source));
            }
        };
        let uncertain = matches!(result, AttemptResult::Indeterminate { .. });
        let receipt = AttemptReceipt {
            intent: self.state.intent,
            fence: self.fence,
            result,
            evidence,
            epoch: Some(epoch_receipt),
        };
        let mut attempts = self.attempts;
        attempts.insert(transition_id, Attempt::Finished(receipt.clone()));
        let Lease {
            lock,
            paths,
            session_id,
            parent,
            profile,
            mode,
            runtime_id,
            handoff,
            fence,
            head: _,
            state: _,
            ..
        } = self;
        if uncertain {
            Ok(Finished::Uncertain {
                lease: Lease {
                    lock,
                    paths,
                    session_id,
                    parent,
                    profile: profile.clone(),
                    mode,
                    cursor: next_cursor,
                    epoch: next_epoch,
                    runtime_id,
                    handoff: handoff.clone(),
                    fence,
                    head: next_head.clone(),
                    attempts,
                    state: Uncertain,
                },
                append,
                receipt,
            })
        } else {
            Ok(Finished::Terminal {
                lease: Lease {
                    lock,
                    paths,
                    session_id,
                    parent,
                    profile,
                    mode,
                    cursor: next_cursor,
                    epoch: next_epoch,
                    runtime_id,
                    handoff,
                    fence,
                    head: next_head,
                    attempts,
                    state: Idle,
                },
                append,
                receipt,
            })
        }
    }
}

/// Exact intent for one idempotent typed edge attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptIntent {
    pub(crate) transition_id: TransitionId,
    pub(crate) expected: WalkPhase,
    pub(crate) targets: Vec<WalkPhase>,
    #[serde(default)]
    pub(crate) allow_live_api: bool,
    pub(crate) allow_git_changes: bool,
    pub(crate) epoch: ServerEpoch,
    pub(crate) evidence: ContentHash,
    #[serde(default)]
    pub(crate) retry: u32,
}

impl AttemptIntent {
    pub(crate) fn new(
        session_id: SessionId,
        cursor: Cursor,
        allow_git_changes: bool,
        epoch: ServerEpoch,
    ) -> Result<Self, Error> {
        Self::with_retry(session_id, cursor, false, allow_git_changes, epoch, 0)
    }

    pub(crate) fn with_live_api(
        session_id: SessionId,
        cursor: Cursor,
        allow_git_changes: bool,
        epoch: ServerEpoch,
    ) -> Result<Self, Error> {
        Self::with_retry(session_id, cursor, true, allow_git_changes, epoch, 0)
    }

    fn with_retry(
        session_id: SessionId,
        cursor: Cursor,
        allow_live_api: bool,
        allow_git_changes: bool,
        epoch: ServerEpoch,
        retry: u32,
    ) -> Result<Self, Error> {
        cursor.validate()?;
        let targets = current_targets(cursor.phase, allow_git_changes);
        let transition_id = transition_id(
            session_id,
            &cursor,
            &targets,
            allow_live_api,
            allow_git_changes,
            &epoch.transition_graph_version,
            retry,
        )?;
        let intent = Self {
            transition_id,
            expected: cursor.phase,
            targets,
            allow_live_api,
            allow_git_changes,
            epoch,
            evidence: cursor.evidence,
            retry,
        };
        intent.validate_current()?;
        Ok(intent)
    }

    fn cursor(&self) -> Cursor {
        Cursor {
            phase: self.expected,
            evidence: self.evidence.clone(),
        }
    }

    fn validate_for(&self, session_id: SessionId) -> Result<(), Error> {
        self.validate_record()?;
        let expected = transition_id(
            session_id,
            &self.cursor(),
            &self.targets,
            self.allow_live_api,
            self.allow_git_changes,
            &self.epoch.transition_graph_version,
            self.retry,
        )?;
        if self.transition_id != expected {
            return Err(Error::InvalidIntent {
                detail: format!(
                    "transition id {} does not match deterministic session key {expected}",
                    self.transition_id
                ),
            });
        }
        Ok(())
    }

    fn validate_current(&self) -> Result<(), Error> {
        if self.epoch.transition_graph_version != TRANSITION_GRAPH_VERSION {
            return Err(Error::InvalidIntent {
                detail: format!(
                    "new transition intent uses graph '{}', current graph is '{}'",
                    self.epoch.transition_graph_version, TRANSITION_GRAPH_VERSION
                ),
            });
        }
        self.validate_record()?;
        if !self.allow_live_api && self.targets.iter().copied().any(target_requires_live_api) {
            return Err(Error::InvalidIntent {
                detail: format!(
                    "phase {} requires explicit live-provider authority for targets {:?}",
                    self.expected, self.targets
                ),
            });
        }
        self.cursor().validate()?;
        Ok(())
    }

    /// Validate durable shape without reinterpreting it through a newer graph.
    ///
    /// The creating epoch is part of the record. A replacement binary must be
    /// able to replay the old record before it can surface `EpochChanged` and
    /// ask the operator to admit the new graph.
    fn validate_record(&self) -> Result<(), Error> {
        let expected = targets_for_version(
            &self.epoch.transition_graph_version,
            self.expected,
            self.allow_git_changes,
        )
        .ok_or_else(|| Error::InvalidIntent {
            detail: format!(
                "transition graph '{}' has no retained validator for phase {}",
                self.epoch.transition_graph_version, self.expected
            ),
        })?;
        if self.targets != expected {
            return Err(Error::InvalidIntent {
                detail: format!(
                    "phase {} in graph '{}' requires admitted targets {expected:?}, got {:?}",
                    self.expected, self.epoch.transition_graph_version, self.targets
                ),
            });
        }
        if self.targets.is_empty() {
            return Err(Error::InvalidIntent {
                detail: format!("phase {} has no admitted typed edge", self.expected),
            });
        }
        if self.targets.contains(&self.expected) {
            return Err(Error::InvalidIntent {
                detail: format!("phase {} cannot also be an admitted target", self.expected),
            });
        }
        let mut unique = self.targets.clone();
        unique.sort_by_key(|phase| phase.as_str());
        unique.dedup();
        if unique.len() != self.targets.len() {
            return Err(Error::InvalidIntent {
                detail: format!(
                    "phase {} contains duplicate admitted targets {:?}",
                    self.expected, self.targets
                ),
            });
        }
        if !self.allow_git_changes
            && self
                .targets
                .iter()
                .any(|target| matches!(target, WalkPhase::R13b | WalkPhase::R13c))
        {
            return Err(Error::InvalidIntent {
                detail: "transition intent cannot admit a checkout-mutating handoff target without git-changes authority"
                    .to_string(),
            });
        }
        if self.evidence.0.trim().is_empty() {
            return Err(Error::InvalidIntent {
                detail: "transition evidence hash must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct TransitionKeyV1<'a> {
    schema_version: &'static str,
    cursor: &'a Cursor,
    targets: &'a [WalkPhase],
    allow_git_changes: bool,
    graph_version: &'a str,
    retry: u32,
}

#[derive(Serialize)]
struct TransitionKey<'a> {
    schema_version: &'static str,
    cursor: &'a Cursor,
    targets: &'a [WalkPhase],
    allow_live_api: bool,
    allow_git_changes: bool,
    graph_version: &'a str,
    retry: u32,
}

fn transition_id(
    session_id: SessionId,
    cursor: &Cursor,
    targets: &[WalkPhase],
    allow_live_api: bool,
    allow_git_changes: bool,
    graph_version: &str,
    retry: u32,
) -> Result<TransitionId, Error> {
    let bytes = if allow_live_api {
        serde_json::to_vec(&TransitionKey {
            schema_version: TRANSITION_KEY_VERSION,
            cursor,
            targets,
            allow_live_api,
            allow_git_changes,
            graph_version,
            retry,
        })
    } else {
        serde_json::to_vec(&TransitionKeyV1 {
            schema_version: TRANSITION_KEY_VERSION_V1,
            cursor,
            targets,
            allow_git_changes,
            graph_version,
            retry,
        })
    };
    let bytes = bytes.map_err(Error::Serialize)?;
    Ok(TransitionId(Uuid::new_v5(&session_id.0, &bytes)))
}

fn current_targets(expected: WalkPhase, allow_git_changes: bool) -> Vec<WalkPhase> {
    v2_targets(expected, allow_git_changes).expect("v2 target table covers every walk phase")
}

fn target_requires_live_api(phase: WalkPhase) -> bool {
    matches!(
        phase,
        WalkPhase::R6 | WalkPhase::R8 | WalkPhase::R11a | WalkPhase::R11
    )
}

/// Resolve the exact target table that created a durable intent.
///
/// Every production graph bump must retain an explicit validator for the old
/// table here before changing `TRANSITION_GRAPH_VERSION`. Unknown versions are
/// journal damage, not authority to trust self-declared targets.
fn targets_for_version(
    version: &str,
    expected: WalkPhase,
    allow_git_changes: bool,
) -> Option<Vec<WalkPhase>> {
    match version {
        GRAPH_VERSION_V2 => v2_targets(expected, allow_git_changes),
        GRAPH_VERSION_V1 => v1_targets(expected, allow_git_changes),
        _ => None,
    }
}

fn v2_targets(expected: WalkPhase, allow_git_changes: bool) -> Option<Vec<WalkPhase>> {
    let targets = match expected {
        WalkPhase::Empty => vec![WalkPhase::R0],
        WalkPhase::R0 => vec![WalkPhase::R1],
        WalkPhase::R1 => vec![WalkPhase::R2a, WalkPhase::R3],
        WalkPhase::R2a => vec![WalkPhase::R3],
        WalkPhase::R3 => vec![WalkPhase::R4a],
        WalkPhase::R4a => vec![WalkPhase::R4b, WalkPhase::R4c],
        WalkPhase::R4b => vec![WalkPhase::R4c],
        WalkPhase::R4c => vec![WalkPhase::R5],
        WalkPhase::R5 => vec![WalkPhase::R6],
        WalkPhase::R6 => vec![WalkPhase::R7],
        WalkPhase::R7 => vec![WalkPhase::R8],
        WalkPhase::R8 => vec![WalkPhase::R9],
        WalkPhase::R9 => vec![WalkPhase::R10],
        WalkPhase::R10 => vec![WalkPhase::R11a, WalkPhase::R11],
        WalkPhase::R11a | WalkPhase::R11 => vec![WalkPhase::R12],
        WalkPhase::R12 if allow_git_changes => {
            vec![WalkPhase::R13a, WalkPhase::R13b, WalkPhase::R13c]
        }
        WalkPhase::R12 => vec![WalkPhase::R13a],
        WalkPhase::R13a => vec![WalkPhase::R14a],
        WalkPhase::R13b => vec![WalkPhase::R14b],
        WalkPhase::R13c | WalkPhase::R14a | WalkPhase::R14b => Vec::new(),
    };
    Some(targets)
}

fn v1_targets(expected: WalkPhase, allow_git_changes: bool) -> Option<Vec<WalkPhase>> {
    let targets = match expected {
        WalkPhase::Empty => vec![WalkPhase::R0],
        WalkPhase::R0 => vec![WalkPhase::R1],
        WalkPhase::R1 => vec![WalkPhase::R2a, WalkPhase::R3],
        WalkPhase::R2a => Vec::new(),
        WalkPhase::R3 => vec![WalkPhase::R4a],
        WalkPhase::R4a => vec![WalkPhase::R4b, WalkPhase::R4c],
        WalkPhase::R4b => vec![WalkPhase::R4c],
        WalkPhase::R4c => vec![WalkPhase::R5],
        WalkPhase::R5 => vec![WalkPhase::R6],
        WalkPhase::R6 => vec![WalkPhase::R7],
        WalkPhase::R7 => vec![WalkPhase::R8],
        WalkPhase::R8 => vec![WalkPhase::R9],
        WalkPhase::R9 => vec![WalkPhase::R10],
        WalkPhase::R10 => vec![WalkPhase::R11a, WalkPhase::R11],
        WalkPhase::R11a | WalkPhase::R11 => vec![WalkPhase::R12],
        WalkPhase::R12 if allow_git_changes => vec![WalkPhase::R13a, WalkPhase::R13b],
        WalkPhase::R12 => vec![WalkPhase::R13a],
        WalkPhase::R13a => vec![WalkPhase::R14a],
        WalkPhase::R13b => vec![WalkPhase::R14b],
        WalkPhase::R13c => return None,
        WalkPhase::R14a | WalkPhase::R14b => Vec::new(),
    };
    Some(targets)
}

/// Result of admitting an idempotency key.
#[derive(Debug)]
pub(crate) enum Begin {
    Started {
        lease: Lease<Pending>,
        append: JournalAppendReceipt,
    },
    Existing {
        lease: Lease<Idle>,
        receipt: AttemptReceipt,
    },
}

/// Durable attempt receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptReceipt {
    pub(crate) intent: AttemptIntent,
    pub(crate) fence: Fence,
    pub(crate) result: AttemptResult,
    pub(crate) evidence: Option<CursorEvidence>,
    pub(crate) epoch: Option<EpochReceipt>,
}

/// Controller epoch observed immediately before and after an attempted edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpochReceipt {
    pub(crate) before: ServerEpoch,
    pub(crate) after: Option<ServerEpoch>,
}

/// Durable classification of the effect boundary reached by one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "status")]
pub(crate) enum AttemptResult {
    Committed {
        phase: WalkPhase,
        evidence: ContentHash,
    },
    Rejected {
        phase: WalkPhase,
        evidence: ContentHash,
        detail: String,
    },
    Cancelled {
        phase: WalkPhase,
        evidence: ContentHash,
        detail: String,
    },
    Indeterminate {
        phase: Option<WalkPhase>,
        evidence: Option<ContentHash>,
        detail: String,
    },
}

/// Completion either restores idle authority or blocks it for reconciliation.
#[derive(Debug)]
pub(crate) enum Finished {
    Terminal {
        lease: Lease<Idle>,
        append: JournalAppendReceipt,
        receipt: AttemptReceipt,
    },
    Uncertain {
        lease: Lease<Uncertain>,
        append: JournalAppendReceipt,
        receipt: AttemptReceipt,
    },
}

/// An append uncertainty drops the live lock and requires a fresh journal
/// replay. Failures proven to precede an append retain their typed authority.
#[derive(Debug)]
pub(crate) enum Failure<A> {
    Retained { authority: A, source: Error },
    Uncertain { source: Error },
}

pub(crate) type AdmissionFailure = Failure<Lease<Idle>>;
pub(crate) type FinishFailure = Failure<Lease<Pending>>;
pub(crate) type ReleaseFailure = Failure<Lease<Idle>>;
pub(crate) type RecoveryFailure = Failure<RecoveryLease>;

/// Successful exact-tail repair retains recovery authority.
#[derive(Debug)]
pub(crate) struct Repaired {
    pub(crate) lease: RecoveryLease,
    pub(crate) evidence: PathBuf,
}

fn failure<A>(authority: A, source: Error) -> Failure<A> {
    if matches!(
        source,
        Error::AppendUncertain { .. }
            | Error::JournalRevision { .. }
            | Error::JournalPrefix { .. }
            | Error::Replay(Damage::Truncated { .. })
    ) {
        drop(authority);
        Failure::Uncertain { source }
    } else {
        Failure::Retained { authority, source }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum Entry {
    Created {
        schema_version: String,
        session_id: SessionId,
        origin: Origin,
        parent: ParentIdentity,
        profile: RunProfileCommitment,
        mode: RunMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<Cursor>,
        recorded_at: RecordedAt,
    },
    Acquired {
        session_id: SessionId,
        fence: Fence,
        epoch: ServerEpoch,
        runtime_id: Option<RuntimeId>,
        pid: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        incarnation: Option<ProcessIncarnation>,
        recorded_at: RecordedAt,
    },
    Began {
        session_id: SessionId,
        fence: Fence,
        intent: AttemptIntent,
        recorded_at: RecordedAt,
    },
    Finished {
        session_id: SessionId,
        transition_id: TransitionId,
        fence: Fence,
        result: AttemptResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<CursorEvidence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epoch: Option<EpochReceipt>,
        recorded_at: RecordedAt,
    },
    Recovered {
        session_id: SessionId,
        fence: Fence,
        resolution: RecoveryResolution,
        recorded_at: RecordedAt,
    },
    EpochAdmitted {
        session_id: SessionId,
        prior: ServerEpoch,
        next: ServerEpoch,
        recorded_at: RecordedAt,
    },
    TailRepaired {
        session_id: SessionId,
        discarded: ContentHash,
        evidence_path: PathBuf,
        recorded_at: RecordedAt,
    },
    Released {
        session_id: SessionId,
        fence: Fence,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ready: Option<ReadyReceipt>,
        recorded_at: RecordedAt,
    },
}

#[derive(Debug, Clone)]
struct Line {
    number: usize,
    entry: Entry,
}

enum Load {
    Ready {
        lines: Vec<Line>,
        head: JournalHead,
    },
    Damaged {
        lines: Vec<Line>,
        cause: Damage,
        tail: Tail,
        head: JournalHead,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JournalHead {
    index: usize,
    hash: ContentHash,
}

impl JournalHead {
    fn new(index: usize, bytes: &[u8]) -> Self {
        Self {
            index,
            hash: content_hash(bytes),
        }
    }

    fn empty() -> Self {
        Self::new(0, &[])
    }
}

#[derive(Debug, Clone)]
struct Tail {
    valid_len: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Created {
    schema_version: String,
    session_id: SessionId,
    origin: Origin,
    parent: ParentIdentity,
    profile: RunProfileCommitment,
    mode: RunMode,
    cursor: Option<Cursor>,
}

impl Created {
    pub(crate) fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub(crate) fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub(crate) fn parent(&self) -> &ParentIdentity {
        &self.parent
    }

    pub(crate) fn profile(&self) -> &RunProfileCommitment {
        &self.profile
    }

    pub(crate) fn mode(&self) -> RunMode {
        self.mode
    }

    pub(crate) fn cursor(&self) -> Option<&Cursor> {
        self.cursor.as_ref()
    }

    pub(crate) fn handoff_path(&self) -> Option<&Path> {
        match &self.origin {
            Origin::Successor { transfer } => Some(transfer.invocation_path()),
            Origin::Admitted { .. } | Origin::Historical { .. } => None,
        }
    }

    pub(crate) fn successor_runtime(&self) -> Option<RuntimeId> {
        match &self.origin {
            Origin::Successor { transfer } => Some(transfer.runtime_id()),
            Origin::Admitted { .. } | Origin::Historical { .. } => None,
        }
    }

    pub(crate) fn predecessor(&self) -> Option<&ParentIdentity> {
        match &self.origin {
            Origin::Successor { transfer } => transfer.predecessor(),
            Origin::Admitted { .. } | Origin::Historical { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner {
    session_id: SessionId,
    fence: Fence,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    pid: u32,
    incarnation: Option<ProcessIncarnation>,
}

impl Owner {
    pub(crate) fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) fn epoch(&self) -> &ServerEpoch {
        &self.epoch
    }

    pub(crate) fn runtime_id(&self) -> Option<RuntimeId> {
        self.runtime_id
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn incarnation(&self) -> Option<&ProcessIncarnation> {
        self.incarnation.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Attempt {
    Pending {
        fence: Fence,
        intent: AttemptIntent,
    },
    Finished(AttemptReceipt),
    Recovered {
        prior: Box<Attempt>,
        receipt: AttemptReceipt,
    },
}

impl Attempt {
    fn intent(&self) -> &AttemptIntent {
        match self {
            Self::Pending { intent, .. } => intent,
            Self::Finished(receipt) | Self::Recovered { receipt, .. } => &receipt.intent,
        }
    }

    fn terminal(&self) -> Option<&AttemptReceipt> {
        match self {
            Self::Pending { .. } => None,
            Self::Finished(receipt) | Self::Recovered { receipt, .. } => Some(receipt),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Replay {
    created: Option<Created>,
    cursor: Option<Cursor>,
    active: Option<Owner>,
    released: Option<Fence>,
    releases: BTreeSet<Fence>,
    recoveries: BTreeSet<Fence>,
    handoffs: BTreeMap<Fence, HandoffAcceptance>,
    ready: Option<ReadyReceipt>,
    last_epoch: Option<ServerEpoch>,
    max_fence: u64,
    attempts: BTreeMap<TransitionId, Attempt>,
    attempt_order: Vec<TransitionId>,
    abandoned: Option<String>,
}

impl Replay {
    fn from_lines(lines: &[Line]) -> Result<Self, Damage> {
        let mut replay = Self::default();
        for line in lines {
            replay.apply(line)?;
        }
        Ok(replay)
    }

    fn ensure_mutable(&self) -> Result<(), Error> {
        let Some(created) = self.created.as_ref() else {
            return Ok(());
        };
        if created.schema_version != SCHEMA_VERSION {
            return Err(Error::LegacySchema {
                active: created.schema_version.clone(),
                supported: SCHEMA_VERSION.to_string(),
            });
        }
        if self.cursor.is_none() {
            return Err(Error::CursorUnbound {
                session_id: created.session_id,
            });
        }
        if let Some(detail) = self.abandoned.as_ref() {
            return Err(Error::Abandoned {
                session_id: created.session_id,
                detail: detail.clone(),
            });
        }
        Ok(())
    }

    fn apply(&mut self, line: &Line) -> Result<(), Damage> {
        let sequence = |detail: String| Damage::Sequence {
            line: line.number,
            detail,
        };
        match &line.entry {
            Entry::Created {
                schema_version,
                session_id,
                origin,
                parent,
                profile,
                mode,
                cursor,
                ..
            } => {
                if schema_version != SCHEMA_VERSION
                    && schema_version != SCHEMA_VERSION_V4
                    && schema_version != SCHEMA_VERSION_V3
                    && schema_version != SCHEMA_VERSION_V2
                    && schema_version != SCHEMA_VERSION_V1
                {
                    return Err(sequence(format!(
                        "unsupported session schema '{schema_version}'"
                    )));
                }
                if self.created.is_some() || line.number != 1 {
                    return Err(sequence(
                        "session creation must be the first and only created entry".to_string(),
                    ));
                }
                match (schema_version.as_str(), cursor) {
                    (
                        SCHEMA_VERSION | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V3 | SCHEMA_VERSION_V2,
                        Some(cursor),
                    ) => cursor
                        .validate()
                        .map_err(|error| sequence(error.to_string()))?,
                    (
                        SCHEMA_VERSION | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V3 | SCHEMA_VERSION_V2,
                        None,
                    ) => {
                        return Err(sequence(format!(
                            "{schema_version} session creation is missing its committed cursor"
                        )));
                    }
                    (SCHEMA_VERSION_V1, None) => {}
                    (SCHEMA_VERSION_V1, Some(_)) => {
                        return Err(sequence(
                            "v1 session creation cannot contain a v2 cursor".to_string(),
                        ));
                    }
                    _ => unreachable!("supported schemas were checked"),
                }
                if matches!(schema_version.as_str(), SCHEMA_VERSION | SCHEMA_VERSION_V4) {
                    let expected = match origin {
                        Origin::Admitted { setup } => Some(
                            initial_cursor(setup, parent, profile)
                                .map_err(|error| sequence(error.to_string()))?,
                        ),
                        Origin::Successor { transfer } => {
                            let root = transfer.active_root().ok_or_else(|| {
                                sequence(
                                    "successor session origin is missing its active root"
                                        .to_string(),
                                )
                            })?;
                            Some(
                                successor_cursor(transfer, parent, profile, root)
                                    .map_err(|error| sequence(error.to_string()))?,
                            )
                        }
                        Origin::Historical { .. } => None,
                    };
                    if expected
                        .as_ref()
                        .is_some_and(|expected| cursor.as_ref() != Some(expected))
                    {
                        return Err(sequence(
                            "session origin cursor does not match its admitted authority"
                                .to_string(),
                        ));
                    }
                }
                if schema_version == SCHEMA_VERSION
                    && let Origin::Successor { transfer } = origin
                {
                    let incarnation = transfer.spawned_incarnation().ok_or_else(|| {
                        sequence(
                            "v5 successor session origin has no Spawned process incarnation"
                                .to_string(),
                        )
                    })?;
                    if incarnation.boot_id.is_nil() || incarnation.start_ticks == 0 {
                        return Err(sequence(
                            "v5 successor session origin has an incomplete Spawned process incarnation"
                                .to_string(),
                        ));
                    }
                }
                self.created = Some(Created {
                    schema_version: schema_version.clone(),
                    session_id: *session_id,
                    origin: origin.clone(),
                    parent: parent.clone(),
                    profile: profile.clone(),
                    mode: *mode,
                    cursor: cursor.clone(),
                });
                self.cursor = cursor.clone();
            }
            Entry::Acquired {
                session_id,
                fence,
                epoch,
                runtime_id,
                pid,
                incarnation,
                ..
            } => {
                self.require_session(*session_id, line.number)?;
                let created = self.created.as_ref().expect("session was required");
                if self.active.is_some() {
                    return Err(sequence(
                        "lease acquired while another lease is active".to_string(),
                    ));
                }
                if let Some(admitted) = self.last_epoch.as_ref()
                    && admitted != epoch
                {
                    return Err(sequence(
                        "lease epoch changed without an explicit epoch admission".to_string(),
                    ));
                }
                if self.last_epoch.is_none()
                    && let Origin::Admitted { setup } = &created.origin
                    && matches!(
                        created.schema_version.as_str(),
                        SCHEMA_VERSION | SCHEMA_VERSION_V4
                    )
                    && setup.completed_head().map(|head| head.0.as_str())
                        != epoch.git_head.as_deref()
                {
                    return Err(sequence(
                        "initial controller epoch does not match the completed setup head"
                            .to_string(),
                    ));
                }
                if self.last_epoch.is_none()
                    && let Origin::Admitted { setup } = &created.origin
                    && matches!(
                        created.schema_version.as_str(),
                        SCHEMA_VERSION | SCHEMA_VERSION_V4
                    )
                    && epoch.active_branch.as_deref() != Some(setup.intent.artifact_branch.as_str())
                {
                    return Err(sequence(
                        "initial controller epoch does not match the completed setup branch"
                            .to_string(),
                    ));
                }
                if created.schema_version == SCHEMA_VERSION {
                    let incarnation = incarnation.as_ref().ok_or_else(|| {
                        sequence("v5 controller acquisition has no process incarnation".to_string())
                    })?;
                    if incarnation.boot_id.is_nil() || incarnation.start_ticks == 0 {
                        return Err(sequence(
                            "v5 controller acquisition has an incomplete process incarnation"
                                .to_string(),
                        ));
                    }
                }
                if self.last_epoch.is_none()
                    && let Origin::Successor { transfer } = &created.origin
                    && matches!(
                        created.schema_version.as_str(),
                        SCHEMA_VERSION | SCHEMA_VERSION_V4
                    )
                {
                    if *runtime_id != Some(transfer.runtime_id()) {
                        return Err(sequence(
                            "initial successor acquisition runtime does not match its Spawned origin"
                                .to_string(),
                        ));
                    }
                    if *pid != transfer.spawned_pid() {
                        return Err(sequence(
                            "initial successor acquisition pid does not match its Spawned origin"
                                .to_string(),
                        ));
                    }
                    if created.schema_version == SCHEMA_VERSION
                        && incarnation.as_ref() != transfer.spawned_incarnation()
                    {
                        return Err(sequence(
                            "initial successor acquisition incarnation does not match its Spawned origin"
                                .to_string(),
                        ));
                    }
                    if !same_path(&epoch.exe_path, transfer.binary_path()) {
                        return Err(sequence(
                            "initial successor acquisition binary does not match its Spawned origin"
                                .to_string(),
                        ));
                    }
                }
                if let Origin::Successor { transfer } = &created.origin
                    && matches!(
                        created.schema_version.as_str(),
                        SCHEMA_VERSION | SCHEMA_VERSION_V4
                    )
                    && *runtime_id != Some(transfer.runtime_id())
                {
                    return Err(sequence(
                        "successor acquisition runtime changed from its admitted origin"
                            .to_string(),
                    ));
                }
                let expected = self
                    .max_fence
                    .checked_add(1)
                    .ok_or_else(|| sequence("fence counter overflow".to_string()))?;
                if fence.0 != expected {
                    return Err(sequence(format!(
                        "fence {} is not the next token after {}",
                        fence.0, self.max_fence
                    )));
                }
                self.max_fence = fence.0;
                self.last_epoch.get_or_insert_with(|| epoch.clone());
                self.released = None;
                self.active = Some(Owner {
                    session_id: *session_id,
                    fence: *fence,
                    epoch: epoch.clone(),
                    runtime_id: *runtime_id,
                    pid: *pid,
                    incarnation: incarnation.clone(),
                });
            }
            Entry::Began {
                session_id,
                fence,
                intent,
                ..
            } => {
                self.require_owner(*session_id, *fence, line.number)?;
                intent
                    .validate_record()
                    .map_err(|error| sequence(error.to_string()))?;
                let created = self.created.as_ref().expect("session was required");
                if matches!(
                    created.schema_version.as_str(),
                    SCHEMA_VERSION | SCHEMA_VERSION_V4
                ) {
                    intent
                        .cursor()
                        .validate()
                        .map_err(|error| sequence(error.to_string()))?;
                    intent
                        .validate_for(*session_id)
                        .map_err(|error| sequence(error.to_string()))?;
                } else if created.schema_version == SCHEMA_VERSION_V3 {
                    intent
                        .cursor()
                        .validate()
                        .map_err(|error| sequence(error.to_string()))?;
                    intent
                        .validate_for(*session_id)
                        .map_err(|error| sequence(error.to_string()))?;
                } else if created.schema_version == SCHEMA_VERSION_V2 {
                    if intent.allow_live_api {
                        return Err(sequence(
                            "v2 transition intent cannot contain v3 live-provider authority"
                                .to_string(),
                        ));
                    }
                    intent
                        .cursor()
                        .validate()
                        .map_err(|error| sequence(error.to_string()))?;
                    intent
                        .validate_for(*session_id)
                        .map_err(|error| sequence(error.to_string()))?;
                }
                let requested = intent.cursor();
                match self.cursor.as_ref() {
                    Some(active) if active != &requested => {
                        return Err(sequence(format!(
                            "transition {} expected cursor {requested:?}, active cursor is {active:?}",
                            intent.transition_id
                        )));
                    }
                    None if created.schema_version == SCHEMA_VERSION_V1 => {
                        self.cursor = Some(requested);
                    }
                    None => {
                        return Err(sequence(
                            "transition began before a session cursor was established".to_string(),
                        ));
                    }
                    Some(_) => {}
                }
                if self.attempts.contains_key(&intent.transition_id) {
                    return Err(sequence(format!(
                        "transition {} was started more than once",
                        intent.transition_id
                    )));
                }
                let owner = self.active.as_ref().expect("owner was required");
                if owner.epoch != intent.epoch {
                    return Err(sequence(format!(
                        "transition {} used a different controller epoch",
                        intent.transition_id
                    )));
                }
                self.attempts.insert(
                    intent.transition_id,
                    Attempt::Pending {
                        fence: *fence,
                        intent: intent.clone(),
                    },
                );
                self.attempt_order.push(intent.transition_id);
            }
            Entry::Finished {
                session_id,
                transition_id,
                fence,
                result,
                evidence,
                epoch,
                ..
            } => {
                self.require_owner(*session_id, *fence, line.number)?;
                let Some(Attempt::Pending {
                    fence: began,
                    intent,
                }) = self.attempts.get(transition_id)
                else {
                    return Err(sequence(format!(
                        "transition {transition_id} finished without a pending attempt"
                    )));
                };
                if began != fence {
                    return Err(sequence(format!(
                        "transition {transition_id} changed fence from {began} to {fence}"
                    )));
                }
                let intent = intent.clone();
                let created = self.created.as_ref().expect("session was required");
                if matches!(
                    created.schema_version.as_str(),
                    SCHEMA_VERSION | SCHEMA_VERSION_V4
                ) {
                    validate_result_current(&intent, result)
                        .map_err(|error| sequence(error.to_string()))?;
                    validate_cursor_evidence(
                        created,
                        *session_id,
                        &intent,
                        result,
                        evidence.as_ref(),
                        epoch.as_ref(),
                    )
                    .map_err(|error| sequence(error.to_string()))?;
                } else if created.schema_version == SCHEMA_VERSION_V3 {
                    if evidence.is_some() {
                        return Err(sequence(
                            "v3 transition result cannot contain v4 cursor evidence".to_string(),
                        ));
                    }
                    validate_result_current(&intent, result)
                        .map_err(|error| sequence(error.to_string()))?;
                } else {
                    if evidence.is_some() {
                        return Err(sequence(format!(
                            "{} transition result cannot contain v4 cursor evidence",
                            created.schema_version
                        )));
                    }
                    validate_result(&intent, result)
                        .map_err(|error| sequence(error.to_string()))?;
                }
                match (created.schema_version.as_str(), epoch) {
                    (
                        SCHEMA_VERSION | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V3 | SCHEMA_VERSION_V2,
                        Some(epoch),
                    ) => {
                        validate_epoch(&intent, result, epoch)
                            .map_err(|error| sequence(error.to_string()))?;
                    }
                    (
                        SCHEMA_VERSION | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V3 | SCHEMA_VERSION_V2,
                        None,
                    ) => {
                        return Err(sequence(format!(
                            "{} transition result is missing its epoch receipt",
                            created.schema_version
                        )));
                    }
                    (SCHEMA_VERSION_V1, None) => {}
                    (SCHEMA_VERSION_V1, Some(_)) => {
                        return Err(sequence(
                            "v1 transition result cannot contain a v2 epoch receipt".to_string(),
                        ));
                    }
                    _ => unreachable!("supported schemas were checked"),
                }
                let receipt = AttemptReceipt {
                    intent: intent.clone(),
                    fence: *fence,
                    result: result.clone(),
                    evidence: evidence.clone(),
                    epoch: epoch.clone(),
                };
                self.attempts
                    .insert(*transition_id, Attempt::Finished(receipt));
                if let AttemptResult::Committed { phase, evidence } = result {
                    self.cursor = Some(Cursor {
                        phase: *phase,
                        evidence: evidence.clone(),
                    });
                }
                if let Some(epoch) = epoch
                    && let Some(after) = epoch.after.as_ref()
                    && after != &epoch.before
                    && admits_epoch_change(&intent, result, &epoch.before, after)
                {
                    self.last_epoch = Some(after.clone());
                    self.active.as_mut().expect("owner was required").epoch = after.clone();
                }
            }
            Entry::Recovered {
                session_id,
                fence,
                resolution,
                ..
            } => {
                self.require_owner(*session_id, *fence, line.number)?;
                let cause = self.recovery().ok_or_else(|| {
                    sequence("recovery event appeared without a recovery cause".to_string())
                })?;
                let created = self.created.as_ref().expect("session was required");
                if matches!(
                    created.schema_version.as_str(),
                    SCHEMA_VERSION | SCHEMA_VERSION_V4
                ) {
                    validate_resolution_current(self, &cause, resolution)
                        .map_err(|error| sequence(error.to_string()))?;
                } else if created.schema_version == SCHEMA_VERSION_V3 {
                    validate_resolution_v3(self, &cause, resolution)
                        .map_err(|error| sequence(error.to_string()))?;
                } else if created.schema_version == SCHEMA_VERSION_V2 {
                    validate_resolution_v2(self, &cause, resolution)
                        .map_err(|error| sequence(error.to_string()))?;
                } else {
                    validate_resolution(self, &cause, resolution)
                        .map_err(|error| sequence(error.to_string()))?;
                }
                match resolution {
                    RecoveryResolution::AbandonOwner => {}
                    RecoveryResolution::AbandonSession { detail } => {
                        if self.abandoned.replace(detail.clone()).is_some() {
                            return Err(sequence(
                                "controller session was abandoned more than once".to_string(),
                            ));
                        }
                    }
                    RecoveryResolution::ResolveAttempt {
                        transition_id,
                        result,
                        evidence,
                        epoch,
                    } => {
                        let prior = self.attempts.remove(transition_id).ok_or_else(|| {
                            sequence(format!(
                                "recovery transition {transition_id} was not recorded"
                            ))
                        })?;
                        let intent = prior.intent().clone();
                        self.attempts.insert(
                            *transition_id,
                            Attempt::Recovered {
                                prior: Box::new(prior),
                                receipt: AttemptReceipt {
                                    intent: intent.clone(),
                                    fence: *fence,
                                    result: result.clone(),
                                    evidence: evidence.clone(),
                                    epoch: epoch.clone(),
                                },
                            },
                        );
                        if let AttemptResult::Committed { phase, evidence } = result {
                            self.cursor = Some(Cursor {
                                phase: *phase,
                                evidence: evidence.clone(),
                            });
                        }
                        if let Some(epoch) = epoch
                            && let Some(after) = epoch.after.as_ref()
                            && after != &epoch.before
                            && admits_epoch_change(&intent, result, &epoch.before, after)
                        {
                            self.last_epoch = Some(after.clone());
                        }
                    }
                    RecoveryResolution::AcceptHandoff {
                        acceptance,
                        result,
                        evidence,
                        epoch,
                    } => {
                        let transition_id = acceptance.attempt().transition();
                        let prior = self.attempts.remove(&transition_id).ok_or_else(|| {
                            sequence(format!(
                                "handoff recovery transition {transition_id} was not recorded"
                            ))
                        })?;
                        let preserve_cursor = matches!(
                            &prior,
                            Attempt::Finished(receipt) if &receipt.result == result
                        );
                        let intent = prior.intent().clone();
                        self.attempts.insert(
                            transition_id,
                            Attempt::Recovered {
                                prior: Box::new(prior),
                                receipt: AttemptReceipt {
                                    intent: intent.clone(),
                                    fence: *fence,
                                    result: result.clone(),
                                    evidence: evidence.clone(),
                                    epoch: Some(epoch.clone()),
                                },
                            },
                        );
                        if !preserve_cursor
                            && let AttemptResult::Committed { phase, evidence } = result
                        {
                            self.cursor = Some(Cursor {
                                phase: *phase,
                                evidence: evidence.clone(),
                            });
                        }
                        if let Some(after) = epoch.after.as_ref()
                            && after != &epoch.before
                            && admits_epoch_change(&intent, result, &epoch.before, after)
                        {
                            self.last_epoch = Some(after.clone());
                        }
                        if self.handoffs.insert(*fence, acceptance.clone()).is_some() {
                            return Err(sequence(
                                "controller fence recorded more than one recovered handoff"
                                    .to_string(),
                            ));
                        }
                    }
                    RecoveryResolution::AdmitEpoch { .. } => {
                        return Err(sequence(
                            "epoch admission used an owner-recovery journal entry".to_string(),
                        ));
                    }
                }
                self.recoveries.insert(*fence);
                self.released = None;
                self.active = None;
            }
            Entry::EpochAdmitted {
                session_id,
                prior,
                next,
                ..
            } => {
                self.require_session(*session_id, line.number)?;
                if self.active.is_some() {
                    return Err(sequence(
                        "controller epoch changed while a lease was active".to_string(),
                    ));
                }
                let Some(admitted) = self.last_epoch.as_ref() else {
                    return Err(sequence(
                        "controller epoch changed before an initial epoch was established"
                            .to_string(),
                    ));
                };
                if admitted != prior {
                    return Err(sequence(
                        "controller epoch admission did not match the established epoch"
                            .to_string(),
                    ));
                }
                if prior == next {
                    return Err(sequence(
                        "controller epoch admission did not change the epoch".to_string(),
                    ));
                }
                self.last_epoch = Some(next.clone());
            }
            Entry::TailRepaired { session_id, .. } => {
                self.require_session(*session_id, line.number)?;
            }
            Entry::Released {
                session_id,
                fence,
                ready,
                ..
            } => {
                self.require_owner(*session_id, *fence, line.number)?;
                if self.unresolved_for(*fence).is_some() {
                    return Err(sequence(
                        "lease released with an unresolved transition attempt".to_string(),
                    ));
                }
                if let Some(ready) = ready {
                    ready.validate_persisted().map_err(&sequence)?;
                    if self.ready.is_some() {
                        return Err(sequence(
                            "successor session recorded more than one atomic Ready release"
                                .to_string(),
                        ));
                    }
                    let created = self.created.as_ref().ok_or_else(|| {
                        sequence("Ready release appeared before session creation".to_string())
                    })?;
                    let Origin::Successor { transfer } = &created.origin else {
                        return Err(sequence(
                            "only a successor session may record an atomic Ready release"
                                .to_string(),
                        ));
                    };
                    let owner = self
                        .active
                        .as_ref()
                        .ok_or_else(|| sequence("Ready release has no active owner".to_string()))?;
                    let commit = ready.commit();
                    let record = ready.record();
                    let attempt = self
                        .attempts
                        .get(&commit.transition_id())
                        .and_then(Attempt::terminal)
                        .ok_or_else(|| {
                            sequence(
                                "Ready release does not name a terminal transition attempt"
                                    .to_string(),
                            )
                        })?;
                    let committed = matches!(
                        &attempt.result,
                        AttemptResult::Committed { phase, evidence }
                            if *phase == WalkPhase::R4c
                                && commit.cursor().phase == WalkPhase::R4c
                                && evidence == &commit.cursor().evidence
                    );
                    if !committed
                        || attempt.fence != *fence
                        || commit.session_id() != *session_id
                        || commit.fence() != *fence
                        || commit.cursor()
                            != self.cursor.as_ref().ok_or_else(|| {
                                sequence("Ready release has no committed cursor".to_string())
                            })?
                        || commit.mode() != created.mode
                        || owner.runtime_id != Some(transfer.runtime_id())
                        || owner.pid != transfer.spawned_pid()
                        || (created.schema_version == SCHEMA_VERSION
                            && (owner.incarnation.as_ref() != transfer.spawned_incarnation()
                                || record.incarnation.as_ref() != transfer.spawned_incarnation()))
                        || record.campaign_id != *created.parent.campaign_id()
                        || record.node_id != created.parent.node_id()
                        || record.runtime_id != record_runtime_id(transfer.runtime_id())
                        || record.pid != owner.pid
                        || record.incarnation != owner.incarnation
                        || ready.endpoint().is_some_and(|endpoint| {
                            endpoint.repo_root() != owner.epoch.repo_root.as_path()
                        })
                    {
                        return Err(sequence(
                            "atomic Ready release does not match its R4c owner, origin, or transition"
                                .to_string(),
                        ));
                    }
                    self.ready = Some(ready.clone());
                }
                self.releases.insert(*fence);
                self.released = Some(*fence);
                self.active = None;
            }
        }
        Ok(())
    }

    fn require_session(&self, session_id: SessionId, line: usize) -> Result<(), Damage> {
        let Some(created) = self.created.as_ref() else {
            return Err(Damage::Sequence {
                line,
                detail: "session event appeared before creation".to_string(),
            });
        };
        if created.session_id != session_id {
            return Err(Damage::Sequence {
                line,
                detail: format!(
                    "session id changed from {} to {}",
                    created.session_id, session_id
                ),
            });
        }
        Ok(())
    }

    fn require_owner(
        &self,
        session_id: SessionId,
        fence: Fence,
        line: usize,
    ) -> Result<(), Damage> {
        self.require_session(session_id, line)?;
        let Some(active) = self.active.as_ref() else {
            return Err(Damage::Sequence {
                line,
                detail: "owner event appeared without an active lease".to_string(),
            });
        };
        if active.session_id != session_id || active.fence != fence {
            return Err(Damage::Sequence {
                line,
                detail: format!(
                    "owner event used session {session_id} fence {fence}, active session {} fence {}",
                    active.session_id, active.fence
                ),
            });
        }
        Ok(())
    }

    fn unresolved_for(&self, fence: Fence) -> Option<(TransitionId, &Attempt)> {
        self.attempts.iter().find_map(|(transition_id, attempt)| {
            let unresolved = match attempt {
                Attempt::Pending { fence: active, .. } => *active == fence,
                Attempt::Finished(receipt) => {
                    receipt.fence == fence
                        && matches!(receipt.result, AttemptResult::Indeterminate { .. })
                }
                Attempt::Recovered { .. } => false,
            };
            unresolved.then_some((*transition_id, attempt))
        })
    }

    fn recovery(&self) -> Option<RecoveryCause> {
        let session_id = self.created.as_ref()?.session_id;
        let owner = self.active.as_ref()?;
        if let Some((_, Attempt::Pending { intent, .. })) = self.unresolved_for(owner.fence) {
            return Some(RecoveryCause::AttemptPending {
                session_id,
                fence: owner.fence,
                intent: intent.clone(),
            });
        }
        if let Some((_, Attempt::Finished(receipt))) = self.unresolved_for(owner.fence)
            && let AttemptResult::Indeterminate { detail, .. } = &receipt.result
        {
            return Some(RecoveryCause::AttemptIndeterminate {
                session_id,
                fence: owner.fence,
                intent: receipt.intent.clone(),
                detail: detail.clone(),
            });
        }
        Some(RecoveryCause::OwnerLost {
            session_id,
            fence: owner.fence,
            pid: owner.pid,
            incarnation: owner.incarnation.clone(),
            runtime_id: owner.runtime_id,
            epoch: owner.epoch.clone(),
        })
    }

    fn epoch_change(&self, requested: &ServerEpoch) -> Option<RecoveryCause> {
        let session_id = self.created.as_ref()?.session_id;
        let prior = self.last_epoch.as_ref()?;
        (prior != requested).then(|| RecoveryCause::EpochChanged {
            session_id,
            prior: prior.clone(),
            requested: requested.clone(),
        })
    }
}

fn acquire_lease(
    lock: File,
    paths: Paths,
    request: Claim,
    mut replay: Replay,
    mut head: JournalHead,
) -> Result<Lease<Idle>, Error> {
    ensure_fresh(&request.epoch)?;
    let session_id = if let Some(created) = replay.created.as_ref() {
        created.session_id
    } else {
        validate_new_claim(&request)?;
        let session_id = SessionId::new();
        let entry = Entry::Created {
            schema_version: SCHEMA_VERSION.to_string(),
            session_id,
            origin: request.origin.clone(),
            parent: request.parent.clone(),
            profile: request.profile.clone(),
            mode: request.mode,
            cursor: Some(request.origin_cursor.clone()),
            recorded_at: RecordedAt::now(),
        };
        let line = Line {
            number: head.index + 1,
            entry: entry.clone(),
        };
        let mut next = replay.clone();
        next.apply(&line).map_err(Error::Replay)?;
        (_, head) = append_entry(&paths.journal, &entry, &head)?;
        replay = next;
        session_id
    };
    let fence = Fence(
        replay
            .max_fence
            .checked_add(1)
            .ok_or(Error::FenceExhausted { session_id })?,
    );
    let entry = Entry::Acquired {
        session_id,
        fence,
        epoch: request.epoch.clone(),
        runtime_id: request.runtime_id,
        pid: request.pid,
        incarnation: request.incarnation.clone(),
        recorded_at: RecordedAt::now(),
    };
    ensure_fresh(&request.epoch)?;
    let line = Line {
        number: head.index + 1,
        entry: entry.clone(),
    };
    let mut next = replay.clone();
    next.apply(&line).map_err(Error::Replay)?;
    (_, head) = append_entry(&paths.journal, &entry, &head)?;
    replay = next;
    let handoff = request.origin.handoff_path();
    Ok(Lease {
        lock,
        paths,
        session_id,
        parent: request.parent,
        profile: request.profile,
        mode: request.mode,
        cursor: replay
            .cursor
            .expect("acquired controller session has a committed cursor"),
        epoch: request.epoch,
        runtime_id: request.runtime_id,
        handoff,
        fence,
        head,
        attempts: replay.attempts,
        state: Idle,
    })
}

fn ensure_fresh(epoch: &ServerEpoch) -> Result<(), Error> {
    epoch
        .ensure_not_stale_now()
        .map_err(|source| Error::EpochStale {
            detail: source.to_string(),
        })
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn validate_new_claim(request: &Claim) -> Result<(), Error> {
    if request.cursor != request.origin_cursor {
        return Err(Error::InvalidClaim {
            detail: "new controller session must begin at its admitted R3 origin cursor"
                .to_string(),
        });
    }
    match &request.origin {
        Origin::Admitted { setup } => {
            let expected = setup
                .completed_head()
                .expect("admitted claim construction requires completed setup");
            if request.epoch.git_head.as_deref() != Some(expected.0.as_str()) {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "completed setup head '{}' does not match controller epoch Git HEAD {:?}",
                        expected.0, request.epoch.git_head
                    ),
                });
            }
            if request.epoch.active_branch.as_deref() != Some(setup.intent.artifact_branch.as_str())
            {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "completed setup branch '{}' does not match controller epoch branch {:?}",
                        setup.intent.artifact_branch, request.epoch.active_branch
                    ),
                });
            }
        }
        Origin::Successor { transfer } => {
            if request.pid != transfer.spawned_pid() {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "spawned successor pid '{}' does not match initial controller pid '{}'",
                        transfer.spawned_pid(),
                        request.pid
                    ),
                });
            }
            let expected = transfer.spawned_incarnation().ok_or_else(|| Error::InvalidClaim {
                detail: "legacy successor Spawned origin has no process incarnation; it cannot create a current controller session"
                    .to_string(),
            })?;
            if request.incarnation.as_ref() != Some(expected) {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "spawned successor pid '{}' does not match its exact process incarnation",
                        request.pid
                    ),
                });
            }
            if !same_path(&request.epoch.exe_path, transfer.binary_path()) {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "spawned successor binary '{}' does not match controller executable '{}'",
                        transfer.binary_path().display(),
                        request.epoch.exe_path.display()
                    ),
                });
            }
            if request.epoch.git_head.as_deref() != Some(transfer.installed_commit()) {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "installed successor commit '{}' does not match controller epoch Git HEAD {:?}",
                        transfer.installed_commit(),
                        request.epoch.git_head
                    ),
                });
            }
            if request.epoch.active_branch.as_deref() != Some(transfer.selected_branch()) {
                return Err(Error::InvalidClaim {
                    detail: format!(
                        "installed successor branch '{}' does not match controller epoch branch {:?}",
                        transfer.selected_branch(),
                        request.epoch.active_branch
                    ),
                });
            }
        }
        Origin::Historical { .. } => {}
    }
    Ok(())
}

fn binding_conflict(replay: &Replay, request: &Claim) -> Option<Conflict> {
    let created = replay.created.as_ref()?;
    if let Some(detail) = replay.abandoned.as_ref() {
        return Some(Conflict::Abandoned {
            session_id: created.session_id,
            detail: detail.clone(),
        });
    }
    if created.schema_version != SCHEMA_VERSION {
        return Some(Conflict::Schema {
            session_id: created.session_id,
            active: created.schema_version.clone(),
            supported: SCHEMA_VERSION.to_string(),
        });
    }
    if created.origin != request.origin {
        return Some(Conflict::Setup {
            session_id: created.session_id,
            active: created.origin.label().to_string(),
            requested: request.origin.label().to_string(),
        });
    }
    if created.parent != request.parent {
        return Some(Conflict::Parent {
            session_id: created.session_id,
            active: created.parent.clone(),
            requested: request.parent.clone(),
        });
    }
    if created.profile != request.profile {
        return Some(Conflict::Profile {
            session_id: created.session_id,
            active_sha256: created.profile.sha256.clone(),
            requested_sha256: request.profile.sha256.clone(),
        });
    }
    if created.mode != request.mode {
        return Some(Conflict::Mode {
            session_id: created.session_id,
            active: created.mode,
            requested: request.mode,
        });
    }
    if !matches!(&created.origin, Origin::Historical { .. })
        && created.cursor.as_ref() != Some(&request.origin_cursor)
    {
        return Some(Conflict::OriginCursor {
            session_id: created.session_id,
            active: created.cursor.clone(),
            requested: request.origin_cursor.clone(),
        });
    }
    if replay.cursor.as_ref() != Some(&request.cursor) {
        return Some(Conflict::Cursor {
            session_id: created.session_id,
            active: replay.cursor.clone(),
            requested: request.cursor.clone(),
        });
    }
    None
}

fn settle_epoch(
    intent: &AttemptIntent,
    result: AttemptResult,
    receipt: &EpochReceipt,
) -> (AttemptResult, ServerEpoch) {
    let Some(after) = receipt.after.as_ref() else {
        return (
            AttemptResult::Indeterminate {
                phase: None,
                evidence: None,
                detail: "controller epoch could not be captured after the edge".to_string(),
            },
            receipt.before.clone(),
        );
    };
    if after == &receipt.before {
        return (result, receipt.before.clone());
    }
    if admits_epoch_change(intent, &result, &receipt.before, after) {
        return (result, after.clone());
    }
    (
        AttemptResult::Indeterminate {
            phase: None,
            evidence: None,
            detail: "controller epoch changed across an edge that does not admit checkout mutation"
                .to_string(),
        },
        receipt.before.clone(),
    )
}

fn admits_epoch_change(
    intent: &AttemptIntent,
    result: &AttemptResult,
    before: &ServerEpoch,
    after: &ServerEpoch,
) -> bool {
    let edge = matches!(
        (intent.expected, result),
        (
            WalkPhase::R1,
            AttemptResult::Committed {
                phase: WalkPhase::R2a,
                ..
            }
        ) | (
            WalkPhase::R12,
            AttemptResult::Committed {
                phase: WalkPhase::R13b | WalkPhase::R13c,
                ..
            }
        )
    );
    edge && before.protocol_version == after.protocol_version
        && before.transition_graph_version == after.transition_graph_version
        && before.repo_root == after.repo_root
        && before.exe_path == after.exe_path
        && before.exe_modified_unix_ms == after.exe_modified_unix_ms
}

fn validate_epoch(
    intent: &AttemptIntent,
    result: &AttemptResult,
    receipt: &EpochReceipt,
) -> Result<(), Error> {
    if receipt.before != intent.epoch {
        return Err(Error::ResultMismatch {
            detail: "transition epoch receipt does not start at the admitted epoch".to_string(),
        });
    }
    let Some(after) = receipt.after.as_ref() else {
        if matches!(result, AttemptResult::Indeterminate { .. }) {
            return Ok(());
        }
        return Err(Error::ResultMismatch {
            detail: "terminal transition result is missing its observed after epoch".to_string(),
        });
    };
    if after == &receipt.before
        || admits_epoch_change(intent, result, &receipt.before, after)
        || matches!(result, AttemptResult::Indeterminate { .. })
    {
        return Ok(());
    }
    Err(Error::ResultMismatch {
        detail: "controller epoch changed across a result that did not admit checkout mutation"
            .to_string(),
    })
}

fn validate_cursor_evidence(
    created: &Created,
    session_id: SessionId,
    intent: &AttemptIntent,
    result: &AttemptResult,
    evidence: Option<&CursorEvidence>,
    epoch: Option<&EpochReceipt>,
) -> Result<(), Error> {
    match (result, evidence) {
        (AttemptResult::Committed { phase, evidence }, Some(certificate)) => {
            let epoch = epoch.ok_or_else(|| Error::ResultMismatch {
                detail: "v4 committed result is missing its observed epoch receipt".to_string(),
            })?;
            certificate
                .validate(
                    session_id,
                    &created.parent,
                    &created.profile,
                    intent,
                    *phase,
                    epoch,
                )
                .map_err(|error| Error::ResultMismatch {
                    detail: error.to_string(),
                })?;
            let cursor = certificate
                .cursor()
                .map_err(|error| Error::ResultMismatch {
                    detail: error.to_string(),
                })?;
            if cursor.phase != *phase || cursor.evidence != *evidence {
                return Err(Error::ResultMismatch {
                    detail: "committed cursor does not match its canonical evidence certificate"
                        .to_string(),
                });
            }
            Ok(())
        }
        (AttemptResult::Committed { .. }, None) => Err(Error::ResultMismatch {
            detail: "v4 committed result is missing its cursor evidence certificate".to_string(),
        }),
        (_, Some(_)) => Err(Error::ResultMismatch {
            detail: "non-committed result cannot contain a cursor evidence certificate".to_string(),
        }),
        (_, None) => Ok(()),
    }
}

fn validate_result(intent: &AttemptIntent, result: &AttemptResult) -> Result<(), Error> {
    let (phase, evidence) = match result {
        AttemptResult::Committed { phase, evidence } => {
            if !intent.targets.contains(phase) {
                return Err(Error::ResultMismatch {
                    detail: format!(
                        "committed phase {phase} is not one of the admitted targets {:?}",
                        intent.targets
                    ),
                });
            }
            (*phase, Some(evidence))
        }
        AttemptResult::Rejected {
            phase,
            evidence,
            detail,
        }
        | AttemptResult::Cancelled {
            phase,
            evidence,
            detail,
        } => {
            if *phase != intent.expected {
                return Err(Error::ResultMismatch {
                    detail: format!(
                        "non-committed phase {phase} does not match admitted source {}",
                        intent.expected
                    ),
                });
            }
            if evidence != &intent.evidence {
                return Err(Error::ResultMismatch {
                    detail: format!(
                        "non-committed evidence {evidence} does not match admitted source {}",
                        intent.evidence
                    ),
                });
            }
            if detail.trim().is_empty() {
                return Err(Error::ResultMismatch {
                    detail: "non-committed result requires a detail".to_string(),
                });
            }
            (*phase, Some(evidence))
        }
        AttemptResult::Indeterminate {
            phase,
            evidence,
            detail,
        } => {
            if detail.trim().is_empty() {
                return Err(Error::ResultMismatch {
                    detail: "indeterminate result requires a detail".to_string(),
                });
            }
            match (phase, evidence) {
                (None, None) => (intent.expected, None),
                (Some(phase), Some(evidence)) if *phase == intent.expected => {
                    if evidence != &intent.evidence {
                        return Err(Error::ResultMismatch {
                            detail: format!(
                                "indeterminate source evidence {evidence} does not match admitted source {}",
                                intent.evidence
                            ),
                        });
                    }
                    (*phase, Some(evidence))
                }
                (Some(phase), Some(evidence)) if intent.targets.contains(phase) => {
                    (*phase, Some(evidence))
                }
                _ => {
                    return Err(Error::ResultMismatch {
                        detail: format!(
                            "indeterminate result must omit both phase and evidence or bind both to {} or one of {:?}",
                            intent.expected, intent.targets
                        ),
                    });
                }
            }
        }
    };
    if evidence.is_some_and(|hash| hash.0.trim().is_empty()) {
        return Err(Error::ResultMismatch {
            detail: format!("result at phase {phase} has an empty evidence hash"),
        });
    }
    Ok(())
}

fn validate_result_current(intent: &AttemptIntent, result: &AttemptResult) -> Result<(), Error> {
    validate_result(intent, result)?;
    if let AttemptResult::Committed { phase, .. } = result
        && target_requires_live_api(*phase)
        && !intent.allow_live_api
    {
        return Err(Error::ResultMismatch {
            detail: format!(
                "committed phase {phase} requires live-provider admission on the transition intent"
            ),
        });
    }
    let evidence = match result {
        AttemptResult::Committed { evidence, .. }
        | AttemptResult::Rejected { evidence, .. }
        | AttemptResult::Cancelled { evidence, .. } => Some(evidence),
        AttemptResult::Indeterminate { evidence, .. } => evidence.as_ref(),
    };
    if let Some(hash) = evidence
        && let Err(detail) = validate_hash(hash)
    {
        return Err(Error::ResultMismatch {
            detail: format!("result evidence is invalid: {detail}"),
        });
    }
    Ok(())
}

fn validate_hash(hash: &ContentHash) -> Result<(), String> {
    let bytes = hash.0.as_bytes();
    if bytes.len() != 64 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err("evidence must be a 64-character SHA-256 digest".to_string());
    }
    if bytes.iter().any(u8::is_ascii_uppercase) {
        return Err("evidence SHA-256 digest must use canonical lowercase hex".to_string());
    }
    Ok(())
}

fn certify_resolution(
    replay: &Replay,
    resolution: RecoveryResolution,
) -> Result<RecoveryResolution, Error> {
    match resolution {
        RecoveryResolution::ResolveAttempt {
            transition_id,
            result,
            epoch,
            ..
        } => {
            let (result, evidence) =
                certify_attempt(replay, transition_id, result, epoch.as_ref())?;
            Ok(RecoveryResolution::ResolveAttempt {
                transition_id,
                result,
                evidence,
                epoch,
            })
        }
        RecoveryResolution::AcceptHandoff {
            acceptance,
            result,
            epoch,
            ..
        } => {
            let transition_id = acceptance.attempt().transition();
            let (result, evidence) = certify_attempt(replay, transition_id, result, Some(&epoch))?;
            Ok(RecoveryResolution::AcceptHandoff {
                acceptance,
                result,
                evidence,
                epoch,
            })
        }
        resolution => Ok(resolution),
    }
}

fn certify_attempt(
    replay: &Replay,
    transition_id: TransitionId,
    result: AttemptResult,
    epoch: Option<&EpochReceipt>,
) -> Result<(AttemptResult, Option<CursorEvidence>), Error> {
    let attempt = replay
        .attempts
        .get(&transition_id)
        .ok_or_else(|| Error::InvalidResolution {
            detail: format!("recovery transition {transition_id} was not recorded"),
        })?;
    let created = replay
        .created
        .as_ref()
        .ok_or_else(|| Error::InvalidResolution {
            detail: "attempt recovery has no session creation authority".to_string(),
        })?;
    if let Attempt::Finished(receipt) = attempt {
        match (&result, &receipt.result) {
            (
                AttemptResult::Committed { phase, .. },
                AttemptResult::Committed {
                    phase: recorded, ..
                },
            ) if phase == recorded => {
                return Ok((receipt.result.clone(), receipt.evidence.clone()));
            }
            (_, AttemptResult::Indeterminate { .. }) => {}
            _ => {
                return Err(Error::InvalidResolution {
                    detail: "recovery cannot recertify or contradict an already-terminal attempt"
                        .to_string(),
                });
            }
        }
    }
    let (result, evidence) = match result {
        AttemptResult::Committed {
            phase,
            evidence: witness,
        } => {
            let epoch = epoch.ok_or_else(|| Error::InvalidResolution {
                detail: "committed recovery requires an observed epoch receipt".to_string(),
            })?;
            let evidence = CursorEvidence::new(
                created.session_id,
                &created.parent,
                &created.profile,
                attempt.intent(),
                phase,
                &witness,
                epoch,
            )
            .map_err(|error| Error::InvalidResolution {
                detail: error.to_string(),
            })?;
            let cursor = evidence
                .cursor()
                .map_err(|error| Error::InvalidResolution {
                    detail: error.to_string(),
                })?;
            (
                AttemptResult::Committed {
                    phase,
                    evidence: cursor.evidence,
                },
                Some(evidence),
            )
        }
        result => (result, None),
    };
    Ok((result, evidence))
}

fn validate_recovery_evidence(
    replay: &Replay,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    let (transition_id, result, evidence, epoch) = match resolution {
        RecoveryResolution::ResolveAttempt {
            transition_id,
            result,
            evidence,
            epoch,
        } => (*transition_id, result, evidence.as_ref(), epoch.as_ref()),
        RecoveryResolution::AcceptHandoff {
            acceptance,
            result,
            evidence,
            epoch,
        } => (
            acceptance.attempt().transition(),
            result,
            evidence.as_ref(),
            Some(epoch),
        ),
        _ => return Ok(()),
    };
    let attempt = replay
        .attempts
        .get(&transition_id)
        .ok_or_else(|| Error::InvalidResolution {
            detail: format!("recovery transition {transition_id} was not recorded"),
        })?;
    let created = replay
        .created
        .as_ref()
        .ok_or_else(|| Error::InvalidResolution {
            detail: "attempt recovery has no session creation authority".to_string(),
        })?;
    validate_cursor_evidence(
        created,
        created.session_id,
        attempt.intent(),
        result,
        evidence,
        epoch,
    )
    .map_err(|error| Error::InvalidResolution {
        detail: error.to_string(),
    })
}

fn validate_resolution(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    match (cause, resolution) {
        (RecoveryCause::OwnerLost { .. }, RecoveryResolution::AbandonOwner) => Ok(()),
        (
            RecoveryCause::AttemptPending { .. } | RecoveryCause::AttemptIndeterminate { .. },
            RecoveryResolution::AbandonSession { detail },
        ) if !detail.trim().is_empty() => Ok(()),
        (
            RecoveryCause::AttemptPending { intent, .. }
            | RecoveryCause::AttemptIndeterminate { intent, .. },
            RecoveryResolution::ResolveAttempt {
                transition_id,
                result,
                ..
            },
        ) if *transition_id == intent.transition_id
            && !matches!(result, AttemptResult::Indeterminate { .. }) =>
        {
            validate_result(intent, result)
        }
        (
            RecoveryCause::AttemptPending {
                session_id,
                fence,
                intent,
            }
            | RecoveryCause::AttemptIndeterminate {
                session_id,
                fence,
                intent,
                ..
            },
            RecoveryResolution::AcceptHandoff {
                acceptance, result, ..
            },
        ) => validate_handoff_resolution(*session_id, *fence, intent, acceptance, result),
        (
            RecoveryCause::OwnerLost {
                session_id, fence, ..
            },
            RecoveryResolution::AcceptHandoff {
                acceptance, result, ..
            },
        ) => {
            let transition_id = acceptance.attempt().transition();
            let receipt = replay
                .attempts
                .get(&transition_id)
                .and_then(|attempt| match attempt {
                    Attempt::Finished(receipt) => Some(receipt),
                    Attempt::Pending { .. } | Attempt::Recovered { .. } => None,
                })
                .ok_or_else(|| Error::InvalidResolution {
                    detail: "lost-owner handoff recovery requires an exact finished R12 attempt"
                        .to_string(),
                })?;
            if receipt.fence != *fence || &receipt.result != result {
                return Err(Error::InvalidResolution {
                    detail:
                        "lost-owner handoff recovery contradicts the recorded transition receipt"
                            .to_string(),
                });
            }
            validate_handoff_resolution(*session_id, *fence, &receipt.intent, acceptance, result)
        }
        (
            RecoveryCause::EpochChanged {
                prior, requested, ..
            },
            RecoveryResolution::AdmitEpoch { prior: from, next },
        ) if from == prior && next == requested && from != next => Ok(()),
        _ => Err(Error::InvalidResolution {
            detail: format!(
                "resolution {resolution:?} does not terminalize recovery cause {cause:?}"
            ),
        }),
    }?;

    if let RecoveryResolution::AbandonOwner = resolution
        && replay
            .active
            .as_ref()
            .and_then(|owner| replay.unresolved_for(owner.fence))
            .is_some()
    {
        return Err(Error::InvalidResolution {
            detail: "cannot abandon an owner with an unresolved attempt".to_string(),
        });
    }
    Ok(())
}

fn validate_handoff_resolution(
    session_id: SessionId,
    fence: Fence,
    intent: &AttemptIntent,
    acceptance: &HandoffAcceptance,
    result: &AttemptResult,
) -> Result<(), Error> {
    acceptance
        .ready()
        .validate_persisted()
        .map_err(|detail| Error::InvalidResolution { detail })?;
    let attempt = acceptance.attempt();
    if attempt.session() != session_id
        || attempt.fence() != fence
        || attempt.transition() != intent.transition_id
        || attempt.allow_live_api() != intent.allow_live_api
        || attempt.allow_git_changes() != intent.allow_git_changes
        || intent.expected != WalkPhase::R12
        || !intent.targets.contains(&WalkPhase::R13b)
        || !intent.allow_git_changes
        || !matches!(
            result,
            AttemptResult::Committed {
                phase: WalkPhase::R13b,
                ..
            }
        )
    {
        return Err(Error::InvalidResolution {
            detail: "handoff recovery does not bind the exact admitted R12->R13b attempt"
                .to_string(),
        });
    }
    validate_result_current(intent, result)
}

fn validate_resolution_current(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    validate_resolution_epoch(replay, cause, resolution, true)?;
    validate_recovery_evidence(replay, resolution)
}

fn validate_resolution_v3(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    if matches!(
        resolution,
        RecoveryResolution::AcceptHandoff { .. } | RecoveryResolution::AbandonSession { .. }
    ) {
        return Err(Error::InvalidResolution {
            detail: "recovered handoff authority requires the current session schema".to_string(),
        });
    }
    if let RecoveryResolution::ResolveAttempt { evidence, .. } = resolution
        && evidence.is_some()
    {
        return Err(Error::InvalidResolution {
            detail: "v3 attempt recovery cannot contain v4 cursor evidence".to_string(),
        });
    }
    validate_resolution_epoch(replay, cause, resolution, true)
}

fn validate_resolution_v2(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    if matches!(
        resolution,
        RecoveryResolution::AcceptHandoff { .. } | RecoveryResolution::AbandonSession { .. }
    ) {
        return Err(Error::InvalidResolution {
            detail: "recovered handoff authority requires the current session schema".to_string(),
        });
    }
    validate_resolution_epoch(replay, cause, resolution, false)
}

fn validate_resolution_epoch(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
    require_live_api: bool,
) -> Result<(), Error> {
    validate_resolution(replay, cause, resolution)?;
    let attempt_fields = match resolution {
        RecoveryResolution::ResolveAttempt {
            transition_id,
            result,
            epoch,
            ..
        } => Some((*transition_id, result, epoch.as_ref())),
        RecoveryResolution::AcceptHandoff {
            acceptance,
            result,
            epoch,
            ..
        } => Some((acceptance.attempt().transition(), result, Some(epoch))),
        _ => None,
    };
    if let Some((transition_id, result, epoch)) = attempt_fields {
        let Some(attempt) = replay.attempts.get(&transition_id) else {
            return Ok(());
        };
        let intent = attempt.intent();
        if require_live_api {
            validate_result_current(intent, result)?;
        } else {
            validate_result(intent, result)?;
        }
        let epoch = epoch.ok_or_else(|| Error::InvalidResolution {
            detail: "epoch-aware attempt recovery requires the observed epoch receipt".to_string(),
        })?;
        validate_recovery_epoch(intent, result, epoch)?;
        if let Attempt::Finished(receipt) = attempt
            && let Some(recorded) = receipt
                .epoch
                .as_ref()
                .and_then(|epoch| epoch.after.as_ref())
            && epoch.after.as_ref() != Some(recorded)
        {
            return Err(Error::InvalidResolution {
                detail: "attempt recovery contradicted the previously observed after epoch"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn validate_recovery_epoch(
    intent: &AttemptIntent,
    result: &AttemptResult,
    receipt: &EpochReceipt,
) -> Result<(), Error> {
    if receipt.before != intent.epoch || receipt.after.is_none() {
        return Err(Error::InvalidResolution {
            detail: "attempt recovery requires complete epoch evidence from the admitted epoch"
                .to_string(),
        });
    }
    match result {
        AttemptResult::Committed { .. } => validate_epoch(intent, result, receipt),
        AttemptResult::Rejected { .. } | AttemptResult::Cancelled { .. } => Ok(()),
        AttemptResult::Indeterminate { .. } => Err(Error::InvalidResolution {
            detail: "attempt recovery must terminalize the indeterminate result".to_string(),
        }),
    }
}

fn load_entries(path: &Path) -> Result<Load, Error> {
    let mut file = match OpenOptions::new().read(true).open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(Load::Ready {
                lines: Vec::new(),
                head: JournalHead::empty(),
            });
        }
        Err(source) => {
            return Err(Error::Open {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    lock_shared(&file, path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if bytes.is_empty() {
        return Ok(Load::Ready {
            lines: Vec::new(),
            head: JournalHead::empty(),
        });
    }

    let mut lines = Vec::new();
    let mut start = 0_usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let record = &bytes[start..index];
        let number = lines.len() + 1;
        if record.is_empty() {
            return Ok(Load::Damaged {
                head: JournalHead::new(lines.len(), &bytes[..start]),
                lines,
                cause: Damage::Malformed {
                    line: number,
                    detail: "blank control-journal line".to_string(),
                },
                tail: Tail {
                    valid_len: start as u64,
                    bytes: bytes[start..].to_vec(),
                },
            });
        }
        let entry = match serde_json::from_slice(record) {
            Ok(entry) => entry,
            Err(source) => {
                return Ok(Load::Damaged {
                    head: JournalHead::new(lines.len(), &bytes[..start]),
                    lines,
                    cause: Damage::Malformed {
                        line: number,
                        detail: source.to_string(),
                    },
                    tail: Tail {
                        valid_len: start as u64,
                        bytes: bytes[start..].to_vec(),
                    },
                });
            }
        };
        lines.push(Line { number, entry });
        start = index + 1;
    }
    if start != bytes.len() {
        let tail = bytes[start..].to_vec();
        let hash = content_hash(&tail);
        return Ok(Load::Damaged {
            head: JournalHead::new(lines.len(), &bytes[..start]),
            lines,
            cause: Damage::Truncated {
                line: bytes.iter().filter(|byte| **byte == b'\n').count() + 1,
                tail: hash,
            },
            tail: Tail {
                valid_len: start as u64,
                bytes: tail,
            },
        });
    }
    Ok(Load::Ready {
        head: JournalHead::new(lines.len(), &bytes),
        lines,
    })
}

fn append_entry(
    path: &Path,
    entry: &Entry,
    source: &JournalHead,
) -> Result<(JournalAppendReceipt, JournalHead), Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    create_dir_synced(parent)?;
    let payload_json = serde_json::to_string(entry).map_err(Error::Serialize)?;
    let byte_len = payload_json.len();
    let content_sha256 = sha256_hex(payload_json.as_bytes());
    let mut line = payload_json.as_bytes().to_vec();
    line.push(b'\n');

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .map_err(|source| Error::Open {
            path: path.to_path_buf(),
            source,
        })?;
    lock_exclusive(&file, path)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| Error::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let mut prior = Vec::new();
    file.read_to_end(&mut prior).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let actual_index = prior.iter().filter(|byte| **byte == b'\n').count();
    if !prior.is_empty() && !prior.ends_with(b"\n") {
        let tail_start = prior
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        return Err(Error::Replay(Damage::Truncated {
            line: actual_index + 1,
            tail: content_hash(&prior[tail_start..]),
        }));
    }
    if actual_index != source.index {
        return Err(Error::JournalRevision {
            path: path.to_path_buf(),
            expected: source.index,
            actual: actual_index,
        });
    }
    let actual_hash = content_hash(&prior);
    if actual_hash != source.hash {
        return Err(Error::JournalPrefix {
            path: path.to_path_buf(),
            expected: source.hash.clone(),
            actual: actual_hash,
        });
    }
    let byte_start = u64::try_from(prior.len()).map_err(|_| Error::JournalRevision {
        path: path.to_path_buf(),
        expected: source.index,
        actual: actual_index,
    })?;
    file.write_all(&line)
        .map_err(|source| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?;
    file.sync_all().map_err(|source| Error::AppendUncertain {
        path: path.to_path_buf(),
        detail: source.to_string(),
    })?;
    file.seek(SeekFrom::Start(byte_start))
        .map_err(|source| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?;
    let mut published = vec![0; line.len()];
    file.read_exact(&mut published)
        .map_err(|source| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?;
    let published_len = file
        .metadata()
        .map_err(|source| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?
        .len();
    let line_len = u64::try_from(line.len()).map_err(|_| Error::AppendUncertain {
        path: path.to_path_buf(),
        detail: "journal record length does not fit the durable extent".to_string(),
    })?;
    let expected_len = byte_start
        .checked_add(line_len)
        .ok_or_else(|| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: "journal length overflow after append".to_string(),
        })?;
    if published != line || published_len != expected_len {
        return Err(Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: "durable append readback did not match the admitted record extent".to_string(),
        });
    }
    sync_dir(parent).map_err(|source| Error::AppendUncertain {
        path: path.to_path_buf(),
        detail: source.to_string(),
    })?;

    prior.extend_from_slice(&line);
    let next = JournalHead::new(source.index + 1, &prior);
    Ok((
        JournalAppendReceipt {
            path: path.to_path_buf(),
            source_event_index: source.index,
            source_line: source.index + 1,
            byte_start,
            byte_len,
            content_sha256,
            payload_json,
        },
        next,
    ))
}

fn preserve_tail(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes).map_err(|source| Error::Write {
                path: path.to_path_buf(),
                source,
            })?;
            file.sync_all().map_err(|source| Error::Sync {
                path: path.to_path_buf(),
                source,
            })?;
            sync_dir(path.parent().unwrap_or_else(|| Path::new(".")))
        }
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path).map_err(|source| Error::Read {
                path: path.to_path_buf(),
                source,
            })?;
            if existing == bytes {
                Ok(())
            } else {
                Err(Error::RepairUnsupported {
                    detail: format!(
                        "tail evidence '{}' already exists with different bytes",
                        path.display()
                    ),
                })
            }
        }
        Err(source) => Err(Error::Open {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Atomically replace one damaged final record with its durable repair marker.
fn replace_tail(
    path: &Path,
    source: &JournalHead,
    tail: &Tail,
    marker: &Entry,
    epoch: &ServerEpoch,
) -> Result<(), Error> {
    let mut journal = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|source| Error::Open {
            path: path.to_path_buf(),
            source,
        })?;
    lock_exclusive(&journal, path)?;
    let mut bytes = Vec::new();
    journal
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let valid_len = usize::try_from(tail.valid_len).map_err(|_| Error::RepairUnsupported {
        detail: "valid journal prefix does not fit this platform".to_string(),
    })?;
    if valid_len > bytes.len() {
        return Err(Error::RepairUnsupported {
            detail: format!(
                "valid journal prefix {valid_len} exceeds journal length {}",
                bytes.len()
            ),
        });
    }
    let actual_index = bytes[..valid_len]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    if actual_index != source.index {
        return Err(Error::JournalRevision {
            path: path.to_path_buf(),
            expected: source.index,
            actual: actual_index,
        });
    }
    let actual_head = content_hash(&bytes[..valid_len]);
    if actual_head != source.hash {
        return Err(Error::JournalPrefix {
            path: path.to_path_buf(),
            expected: source.hash.clone(),
            actual: actual_head,
        });
    }
    let actual_tail = &bytes[valid_len..];
    if actual_tail != tail.bytes {
        return Err(Error::RepairMismatch {
            expected: content_hash(&tail.bytes).0,
            actual: content_hash(actual_tail).0,
        });
    }
    let payload = serde_json::to_vec(marker).map_err(Error::Serialize)?;
    let mut replacement = bytes[..valid_len].to_vec();
    replacement.extend_from_slice(&payload);
    replacement.push(b'\n');
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(JOURNAL_FILE);
    let repair_path = path.with_file_name(format!(".{name}.repair-{}.tmp", Uuid::new_v4()));
    let mut repair = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&repair_path)
        .map_err(|source| Error::Open {
            path: repair_path.clone(),
            source,
        })?;
    if let Err(error) = lock_exclusive(&repair, &repair_path) {
        drop(repair);
        let _ = fs::remove_file(&repair_path);
        return Err(error);
    }
    if let Err(source) = repair.write_all(&replacement) {
        drop(repair);
        let _ = fs::remove_file(&repair_path);
        return Err(Error::Write {
            path: repair_path,
            source,
        });
    }
    if let Err(source) = repair.sync_all() {
        drop(repair);
        let _ = fs::remove_file(&repair_path);
        return Err(Error::Sync {
            path: repair_path,
            source,
        });
    }
    if let Err(error) = ensure_fresh(epoch) {
        drop(repair);
        let _ = fs::remove_file(&repair_path);
        return Err(error);
    }
    if let Err(source) = fs::rename(&repair_path, path) {
        drop(repair);
        let _ = fs::remove_file(&repair_path);
        return Err(Error::Write {
            path: path.to_path_buf(),
            source,
        });
    }
    let published = sync_dir(path.parent().unwrap_or_else(|| Path::new(".")))
        .and_then(|()| {
            let actual = fs::read(path).map_err(|source| Error::Read {
                path: path.to_path_buf(),
                source,
            })?;
            if actual != replacement {
                return Err(Error::AppendUncertain {
                    path: path.to_path_buf(),
                    detail: "journal repair readback did not match the durable replacement"
                        .to_string(),
                });
            }
            Ok(())
        })
        .map_err(|error| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: error.to_string(),
        });
    drop(repair);
    drop(journal);
    published
}

fn damaged_path(journal: &Path, hash: &ContentHash) -> PathBuf {
    let prefix = hash.0.get(..16).unwrap_or(hash.0.as_str());
    let name = journal
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(JOURNAL_FILE);
    journal.with_file_name(format!("{name}.damaged-{prefix}.bin"))
}

fn coordinate_key(parent: &ParentIdentity) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parent.campaign_id().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(parent.node_id().as_bytes());
    hasher.update([0]);
    hasher.update(parent.generation().to_be_bytes());
    format!("{:x}", hasher.finalize())
}

fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash(sha256_hex(bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Create a directory tree and durably publish every newly created directory
/// entry from the nearest pre-existing ancestor down to `path`.
fn create_dir_synced(path: &Path) -> Result<(), Error> {
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        match fs::metadata(current) {
            Ok(_) => break,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                missing.push(current.to_path_buf());
            }
            Err(source) => {
                return Err(Error::CreateDir {
                    path: current.to_path_buf(),
                    source,
                });
            }
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent.as_os_str().is_empty() {
            current = Path::new(".");
        } else {
            current = parent;
        }
    }
    fs::create_dir_all(path).map_err(|source| Error::CreateDir {
        path: path.to_path_buf(),
        source,
    })?;
    for created in missing.iter().rev() {
        sync_dir(created.parent().unwrap_or_else(|| Path::new(".")))?;
    }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), Error> {
    let dir = File::open(path).map_err(|source| Error::Open {
        path: path.to_path_buf(),
        source,
    })?;
    dir.sync_all().map_err(|source| Error::Sync {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn try_lock(file: &File, path: &Path) -> Result<bool, Error> {
    // SAFETY: `file` owns a valid descriptor for this call and the successful
    // descriptor is moved into `Lease` or `RecoveryLease` without replacement.
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
    Err(Error::Lock {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn try_lock_shared(file: &File, path: &Path) -> Result<bool, Error> {
    // SAFETY: `file` owns a valid descriptor and remains alive for this
    // nonblocking publication-barrier probe.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) };
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
    Err(Error::Lock {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn lock_shared(file: &File, path: &Path) -> Result<(), Error> {
    lock_journal(file, path, libc::LOCK_SH)
}

#[cfg(unix)]
fn lock_exclusive(file: &File, path: &Path) -> Result<(), Error> {
    lock_journal(file, path, libc::LOCK_EX)
}

#[cfg(unix)]
fn lock_journal(file: &File, path: &Path, operation: libc::c_int) -> Result<(), Error> {
    // SAFETY: `file` owns a valid descriptor and remains alive for the entire
    // protected journal read or append. The lock is released when it drops.
    let result = unsafe { libc::flock(file.as_raw_fd(), operation) };
    if result == 0 {
        return Ok(());
    }
    Err(Error::Lock {
        path: path.to_path_buf(),
        source: io::Error::last_os_error(),
    })
}

#[cfg(not(unix))]
fn try_lock(_file: &File, path: &Path) -> Result<bool, Error> {
    Err(Error::Unsupported {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
fn try_lock_shared(_file: &File, path: &Path) -> Result<bool, Error> {
    Err(Error::Unsupported {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
fn lock_shared(_file: &File, path: &Path) -> Result<(), Error> {
    Err(Error::Unsupported {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &File, path: &Path) -> Result<(), Error> {
    Err(Error::Unsupported {
        path: path.to_path_buf(),
    })
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("invalid controller claim: {detail}")]
    InvalidClaim { detail: String },
    #[error("invalid transition intent: {detail}")]
    InvalidIntent { detail: String },
    #[error("invalid transition result: {detail}")]
    ResultMismatch { detail: String },
    #[error("invalid recovery resolution: {detail}")]
    InvalidResolution { detail: String },
    #[error("failed to create controller-session directory '{path}': {source}")]
    CreateDir { path: PathBuf, source: io::Error },
    #[error("failed to open controller-session path '{path}': {source}")]
    Open { path: PathBuf, source: io::Error },
    #[error("failed to acquire controller-session lock '{path}': {source}")]
    Lock { path: PathBuf, source: io::Error },
    #[error("failed to read controller-session journal '{path}': {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to serialize controller-session entry: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write controller-session journal '{path}': {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to sync controller-session path '{path}': {source}")]
    Sync { path: PathBuf, source: io::Error },
    #[error("controller-session journal replay failed: {0:?}")]
    Replay(Damage),
    #[error(
        "controller-session journal '{path}' revision mismatch: expected {expected} records, found {actual}"
    )]
    JournalRevision {
        path: PathBuf,
        expected: usize,
        actual: usize,
    },
    #[error(
        "controller-session journal '{path}' prefix mismatch: expected {expected}, found {actual}"
    )]
    JournalPrefix {
        path: PathBuf,
        expected: ContentHash,
        actual: ContentHash,
    },
    #[error("controller-session fence exhausted for session {session_id}")]
    FenceExhausted { session_id: SessionId },
    #[error("transition {transition_id} already has a pending attempt")]
    AttemptPending { transition_id: TransitionId },
    #[error("transition {transition_id} reused an idempotency key with different intent")]
    AttemptMismatch { transition_id: TransitionId },
    #[error("transition retry generation is exhausted")]
    RetryExhausted,
    #[error("transition source cursor {requested:?} does not match committed cursor {active:?}")]
    CursorMismatch { active: Cursor, requested: Cursor },
    #[error("controller session {session_id} has no durable committed cursor")]
    CursorUnbound { session_id: SessionId },
    #[error("controller session {session_id} was permanently abandoned: {detail}")]
    Abandoned {
        session_id: SessionId,
        detail: String,
    },
    #[error("controller session schema '{active}' is read-only; mutation requires '{supported}'")]
    LegacySchema { active: String, supported: String },
    #[error("transition request used a different controller epoch")]
    EpochMismatch,
    #[error("controller epoch is no longer current: {detail}")]
    EpochStale { detail: String },
    #[error("controller-session append at '{path}' has an uncertain outcome: {detail}")]
    AppendUncertain { path: PathBuf, detail: String },
    #[error("journal repair is not admitted: {detail}")]
    RepairUnsupported { detail: String },
    #[error("journal tail hash mismatch: expected {expected}, actual {actual}")]
    RepairMismatch { expected: String, actual: String },
    #[error("recovery was already resolved")]
    RecoveryResolved,
    #[error("recovery must be resolved before takeover")]
    RecoveryPending,
    #[cfg(not(unix))]
    #[error("controller-session flock is unsupported for '{path}' on this platform")]
    Unsupported { path: PathBuf },
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        process::Command,
        sync::mpsc,
        time::Duration,
    };

    use std::os::unix::fs::MetadataExt;

    use ploke_records::{identity::ParentIdentityRecord, ids::CampaignId};

    use crate::cli::prototype1_state::successor::PredecessorAttempt;

    use crate::{
        cli::prototype1_state::{
            backend::GitCommit,
            identity, invocation,
            journal::{ActiveCheckoutAdvancedEntry, JournalEntry, PrototypeJournal, Streams},
            parent::ChildPlanFiles,
            profile,
            setup_admission::{
                SetupAdmissionIntent, SetupAdmissionState, SetupArtifactHashes, SetupCheckoutBase,
            },
            successor,
        },
        intervention::{
            PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeRecord, Prototype1NodeStatus,
            Prototype1RunnerRequest,
        },
    };

    use super::*;

    const COLLISION_FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/fixtures/prototype1-controller-collision-20260630"
    );

    fn parent() -> ParentIdentity {
        parent_in("campaign-session-test")
    }

    fn parent_in(campaign: &str) -> ParentIdentity {
        ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: CampaignId::from(campaign),
            parent_id: "node-parent".to_string(),
            node_id: "node-parent".to_string(),
            generation: 1,
            instance_id: Some("instance-1".to_string()),
            previous_parent_id: Some("node-root".to_string()),
            parent_node_id: Some("node-root".to_string()),
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("artifact-parent".to_string()),
            created_at: "2026-06-30T21:38:52Z".to_string(),
        })
    }

    fn profile() -> RunProfileCommitment {
        RunProfileCommitment {
            schema_version: "prototype1-run-profile-commitment.v1".to_string(),
            profile_path: PathBuf::from("prototype1/run-profile.toml"),
            sha256: "a".repeat(64),
            source_path: Some(PathBuf::from("operator-profile.toml")),
            admitted_at: "2026-06-30T20:00:00Z".to_string(),
        }
    }

    fn epoch(root: &Path) -> ServerEpoch {
        ServerEpoch::capture(root).expect("capture test epoch")
    }

    fn cursor(phase: WalkPhase, evidence: &str) -> Cursor {
        Cursor::new(phase, ContentHash::of(evidence)).expect("valid test cursor")
    }

    fn claim(root: &Path, mode: RunMode) -> Claim {
        claim_at(root, mode, cursor(WalkPhase::R7, "r7-evidence"))
    }

    fn claim_at(root: &Path, mode: RunMode, cursor: Cursor) -> Claim {
        Claim::historical(
            ContentHash::of("session-test-origin"),
            parent(),
            profile(),
            mode,
            cursor,
            epoch(root),
        )
    }

    fn admitted_setup(root: &Path) -> (Prototype1SetupAdmission, profile::AdmittedRunProfile) {
        let campaign_id = CampaignId::from("campaign-session-test");
        let node_id = "node-root".to_string();
        let branch = "prototype1-parent-session-test-gen0".to_string();
        let node_dir = root.join("campaign/prototype1/nodes/node-root");
        let binary_path = node_dir.join("bin/ploke-eval");
        let node = Prototype1NodeRecord {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            node_id: node_id.clone(),
            parent_node_id: None,
            generation: 0,
            instance_id: "instance-1".to_string(),
            source_state_id: "prototype1-root:campaign-session-test".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: branch.clone(),
            candidate_id: "root-parent".to_string(),
            target_relpath: PathBuf::from(".ploke/prototype1/parent_identity.json"),
            node_dir: node_dir.clone(),
            workspace_root: root.to_path_buf(),
            binary_path: binary_path.clone(),
            runner_request_path: node_dir.join("runner-request.json"),
            runner_result_path: node_dir.join("runner-result.json"),
            status: Prototype1NodeStatus::Planned,
            created_at: "2026-07-13T00:00:00Z".to_string(),
            updated_at: "2026-07-13T00:00:00Z".to_string(),
        };
        let request = Prototype1RunnerRequest {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            campaign_id: campaign_id.clone(),
            node_id: node_id.clone(),
            generation: 0,
            instance_id: "instance-1".to_string(),
            source_state_id: "prototype1-root:campaign-session-test".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            branch_id: branch.clone(),
            target_relpath: PathBuf::from(".ploke/prototype1/parent_identity.json"),
            workspace_root: root.to_path_buf(),
            binary_path,
            stop_on_error: false,
            runner_args: vec!["loop".to_string(), "prototype1-state".to_string()],
        };
        let identity = ParentIdentity::root_bootstrap(
            campaign_id.clone(),
            node_id,
            "instance-1",
            branch.clone(),
            Some(branch.clone()),
        );
        let profile_hash = "a".repeat(64);
        let intent = SetupAdmissionIntent {
            plan_hash: ContentHash("b".repeat(64)),
            campaign_id,
            manifest_path: root.join("campaign/campaign.json"),
            repo_root: root.to_path_buf(),
            artifact_branch: branch,
            batch_manifest: root.join("batch.json"),
            hashes: SetupArtifactHashes {
                manifest: ContentHash("c".repeat(64)),
                slice: ContentHash("d".repeat(64)),
                profile: ContentHash(profile_hash.clone()),
            },
            checkout: SetupCheckoutBase {
                branch: "main".to_string(),
                head: GitCommit("e".repeat(40)),
            },
            node,
            request,
            identity,
            started_at: RecordedAt(1_784_000_000_000),
        };
        let setup = Prototype1SetupAdmission::new(intent)
            .expect("valid setup intent")
            .complete(
                GitCommit(epoch(root).git_head.expect("test repository head")),
                RecordedAt(1_784_000_060_000),
            )
            .expect("complete setup");
        let profile = toml::from_str::<profile::Prototype1RunProfile>(
            r#"
schema_version = "prototype1-run-profile.v1"
name = "session-test"

[control]
mode = "continuous"
"#,
        )
        .expect("parse test profile");
        profile.validate().expect("validate test profile");
        let admitted = profile::AdmittedRunProfile {
            commitment: RunProfileCommitment {
                schema_version: "prototype1-run-profile-commitment.v1".to_string(),
                profile_path: root.join("campaign/prototype1/run-profile.toml"),
                sha256: profile_hash,
                source_path: None,
                admitted_at: "2026-07-13T00:00:00Z".to_string(),
            },
            profile,
        };
        (setup, admitted)
    }

    fn run_git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run test git command");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn init_repo(root: &Path) {
        run_git(root, &["init", "-q"]);
        fs::write(root.join("README.md"), "session fixture\n").expect("write repository fixture");
        run_git(root, &["add", "README.md"]);
        run_git(
            root,
            &[
                "-c",
                "user.name=Prototype Test",
                "-c",
                "user.email=prototype@example.invalid",
                "commit",
                "-qm",
                "initialize session fixture",
            ],
        );
        run_git(
            root,
            &["checkout", "-qb", "prototype1-parent-session-test-gen0"],
        );
    }

    fn acquired(outcome: Outcome) -> Lease<Idle> {
        match outcome {
            Outcome::Acquired(lease) => lease,
            other => panic!("expected acquired lease, got {other:?}"),
        }
    }

    fn abandon<S>(owner: Lease<S>) {
        #[cfg(unix)]
        {
            // The library test process stays alive and may have concurrent
            // subprocess activity. Explicitly model the kernel unlock that a
            // crashed controller process would receive before reclaiming its
            // still-active journal owner.
            let result = unsafe { libc::flock(owner.lock.as_raw_fd(), libc::LOCK_UN) };
            assert_eq!(result, 0, "unlock abandoned test owner");
        }
        drop(owner);
    }

    fn intent(lease: &Lease<Idle>) -> AttemptIntent {
        lease
            .intent_with_live_api(true, false)
            .expect("valid transition intent")
    }

    fn same_epoch(epoch: &ServerEpoch) -> Option<EpochReceipt> {
        Some(EpochReceipt {
            before: epoch.clone(),
            after: Some(epoch.clone()),
        })
    }

    fn successor_claim(root: &Path, runtime: RuntimeId, pid: u32) -> Claim {
        let (setup, admitted) = admitted_setup(root);
        let selected = parent();
        let branch = selected
            .artifact_branch()
            .expect("successor artifact branch")
            .to_string();
        run_git(root, &["checkout", "-qb", &branch]);
        let head = run_git(root, &["rev-parse", "HEAD"]);
        let invoke_path = root.join("successor-invocation.json");
        let binary_path = std::env::current_exe().expect("current test executable");
        let ready_path = root.join("successor-ready.json");
        let streams = Streams {
            stdout: root.join("successor.stdout"),
            stderr: root.join("successor.stderr"),
        };
        let invocation = invocation::Invocation {
            schema_version: invocation::SCHEMA_VERSION.to_string(),
            role: invocation::Role::Successor,
            campaign_id: selected.campaign_id().clone(),
            node_id: selected.node_id().to_string(),
            runtime_id: runtime,
            journal_path: root.join("prototype1-transition-journal.jsonl"),
            channel_root: Some(root.join("successor-channel")),
            node: None,
            request: None,
            resolved: None,
            active_parent_root: Some(root.to_path_buf()),
            run_profile: Some(admitted.commitment.clone()),
            predecessor_attempt: Some(PredecessorAttempt::new(
                SessionId::for_test(1),
                TransitionId::new(),
                Fence::for_test(1),
                true,
                true,
            )),
            created_at: "2026-07-13T00:00:00Z".to_string(),
        };
        let checkout = ActiveCheckoutAdvancedEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: selected.campaign_id().clone(),
            previous_parent_identity: Some(setup.intent.identity.clone()),
            selected_parent_identity: selected.clone(),
            active_parent_root: root.to_path_buf(),
            selected_branch: branch,
            installed_commit: head,
        };
        let spawned = successor::Record {
            runtime_id: Some(runtime),
            recorded_at: RecordedAt::now(),
            campaign_id: selected.campaign_id().clone(),
            node_id: selected.node_id().to_string(),
            state: successor::State::Spawned {
                pid,
                incarnation: invocation::process_incarnation(pid)
                    .expect("capture successor process"),
                active_parent_root: root.to_path_buf(),
                binary_path,
                invocation_path: invoke_path.clone(),
                ready_path,
                streams,
            },
        };
        let transfer = SuccessorOrigin::new(
            invoke_path,
            invocation,
            checkout,
            spawned,
            &selected,
            &admitted.commitment,
            root,
        )
        .expect("valid successor origin");
        let cursor = successor_cursor(&transfer, &selected, &admitted.commitment, root)
            .expect("successor R3 cursor");
        Claim::from_successor(
            transfer,
            SuccessorAuthority::Spawned,
            selected,
            &admitted,
            RunMode::Continuous,
            cursor,
            epoch(root),
        )
        .expect("valid successor claim")
        .with_pid(pid)
    }

    fn legacy_successor(mut request: Claim) -> Claim {
        let Origin::Successor { transfer } = &request.origin else {
            panic!("successor claim changed origin kind");
        };
        let mut value = serde_json::to_value(transfer).expect("serialize successor origin");
        value
            .get_mut("spawned")
            .and_then(|spawned| spawned.get_mut("state"))
            .and_then(|state| state.get_mut("spawned"))
            .and_then(serde_json::Value::as_object_mut)
            .expect("Spawned state object")
            .remove("incarnation");
        let transfer: SuccessorOrigin =
            serde_json::from_value(value).expect("decode legacy successor origin");
        let origin = successor_cursor(
            &transfer,
            &request.parent,
            &request.profile,
            &request.epoch.repo_root,
        )
        .expect("legacy successor origin cursor");
        request.origin = Origin::Successor { transfer };
        request.origin_cursor = origin.clone();
        request.cursor = origin;
        request
    }

    fn commit_edge(
        owner: Lease<Idle>,
        target: WalkPhase,
        live: bool,
        git: bool,
    ) -> (Lease<Idle>, AttemptReceipt) {
        let intent = owner
            .intent_with_live_api(live, git)
            .expect("valid edge intent");
        let pending = match owner.begin(intent).expect("begin edge") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("fresh edge must start"),
        };
        let finished = pending
            .finish(AttemptResult::Committed {
                phase: target,
                evidence: ContentHash::of(&format!("commit {target}")),
            })
            .expect("commit edge");
        match finished {
            Finished::Terminal { lease, receipt, .. } => (lease, receipt),
            Finished::Uncertain { .. } => panic!("unchanged epoch must commit terminally"),
        }
    }

    fn ready_receipt(owner: &Lease<Idle>, runtime: RuntimeId) -> ReadyReceipt {
        let commit = owner
            .prepare_ready(runtime)
            .expect("prepare exact R4c Ready commit");
        ReadyReceipt::new(
            invocation::SuccessorReadyRecord {
                schema_version: invocation::SUCCESSOR_READY_SCHEMA_VERSION.to_string(),
                campaign_id: owner.campaign_id().clone(),
                node_id: owner.parent().node_id().to_string(),
                runtime_id: invocation::record_runtime_id(runtime),
                pid: std::process::id(),
                incarnation: invocation::process_incarnation(std::process::id())
                    .expect("capture test process"),
                recorded_at: "2026-07-13T00:00:00Z".to_string(),
            },
            commit,
            None,
            None,
        )
        .expect("construct Ready receipt")
    }

    fn handoff_acceptance(attempt: PredecessorAttempt) -> HandoffAcceptance {
        let runtime = RuntimeId::new();
        let commit = ReadyCommit::new(
            SessionId::for_test(8_080),
            TransitionId::new(),
            Fence::for_test(3),
            cursor(WalkPhase::R4c, "successor-ready"),
            RunMode::Continuous,
        )
        .expect("construct Ready commit");
        let ready = ReadyReceipt::new(
            invocation::SuccessorReadyRecord {
                schema_version: invocation::SUCCESSOR_READY_SCHEMA_VERSION.to_string(),
                campaign_id: parent().campaign_id().clone(),
                node_id: parent().node_id().to_string(),
                runtime_id: invocation::record_runtime_id(runtime),
                pid: 8_080,
                incarnation: Some(invocation::ProcessIncarnation {
                    boot_id: uuid::Uuid::from_u128(1),
                    start_ticks: 1,
                }),
                recorded_at: "2026-07-13T00:00:00Z".to_string(),
            },
            commit,
            None,
            None,
        )
        .expect("construct handoff Ready");
        HandoffAcceptance::from_persisted(ready, attempt).expect("construct persisted acceptance")
    }

    fn committed_lines(root: &Path) -> Vec<Line> {
        let store = Store::new(root.join("control"));
        let owner = acquired(
            store
                .claim(claim(root, RunMode::Step))
                .expect("certificate fixture claim"),
        );
        let journal = owner.paths().journal().to_path_buf();
        let intent = owner
            .intent_with_live_api(true, false)
            .expect("certificate fixture intent");
        let pending = match owner.begin(intent).expect("certificate fixture begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("certificate fixture must start"),
        };
        let finished = pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R8,
                evidence: ContentHash::of("caller-hash-is-not-authority"),
            })
            .expect("certificate fixture finish");
        let Finished::Terminal { lease, .. } = finished else {
            panic!("certificate fixture must commit");
        };
        lease.release().expect("certificate fixture release");
        match load_entries(&journal).expect("load certificate fixture") {
            Load::Ready { lines, .. } => lines,
            Load::Damaged { cause, .. } => panic!("certificate fixture is damaged: {cause:?}"),
        }
    }

    fn write_entries(path: &Path, entries: &[Entry]) {
        let mut head = JournalHead::empty();
        for entry in entries {
            (_, head) = append_entry(path, entry, &head).expect("write journal fixture");
        }
    }

    fn remove_branch(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                fields.remove("active_branch");
                for value in fields.values_mut() {
                    remove_branch(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    remove_branch(value);
                }
            }
            _ => {}
        }
    }

    fn write_legacy(path: &Path, entries: &[Entry]) {
        fs::create_dir_all(path.parent().expect("legacy journal parent"))
            .expect("create legacy journal parent");
        let mut payload = String::new();
        for entry in entries {
            let mut value = serde_json::to_value(entry).expect("serialize legacy entry");
            remove_branch(&mut value);
            payload.push_str(&serde_json::to_string(&value).expect("encode legacy entry"));
            payload.push('\n');
        }
        fs::write(path, payload).expect("write legacy journal");
    }

    fn edit_finished(lines: &mut [Line], edit: impl FnOnce(&mut serde_json::Value)) {
        let line = lines
            .iter_mut()
            .find(|line| matches!(line.entry, Entry::Finished { .. }))
            .expect("finished journal entry");
        let mut value = serde_json::to_value(&line.entry).expect("serialize finished entry");
        edit(&mut value);
        line.entry = serde_json::from_value(value).expect("deserialize edited entry");
    }

    fn stage_fixture(name: &str, destination: &Path, expected: &str) -> Vec<u8> {
        let source = Path::new(COLLISION_FIXTURE).join(name);
        let bytes = fs::read(&source).expect("read historical fixture");
        let original = bytes
            .strip_suffix(b"\n")
            .expect("repository fixture must add exactly one trailing LF");
        assert_eq!(sha256_hex(original), expected, "fixture digest for {name}");
        fs::create_dir_all(destination.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::write(destination, original).expect("stage historical bytes");
        original.to_vec()
    }

    fn stage_exact_fixture(name: &str, destination: &Path, expected: &str) -> Vec<u8> {
        let source = Path::new(COLLISION_FIXTURE).join(name);
        let bytes = fs::read(&source).expect("read historical fixture");
        assert_eq!(sha256_hex(&bytes), expected, "fixture digest for {name}");
        fs::create_dir_all(destination.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::write(destination, &bytes).expect("stage exact historical bytes");
        bytes
    }

    #[test]
    fn lock_inode_is_stable_and_fence_increments() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));

        let first = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("first claim"),
        );
        let session_id = first.session_id();
        assert_eq!(first.fence().get(), 1);
        let paths = first.paths().clone();
        let inode = fs::metadata(paths.lock()).expect("lock metadata").ino();
        first.release().expect("first release");

        let second = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("second claim"),
        );
        assert_eq!(second.session_id(), session_id);
        assert_eq!(second.fence().get(), 2);
        assert_eq!(
            fs::metadata(paths.lock()).expect("lock metadata").ino(),
            inode,
            "the lock inode must never be replaced"
        );
        second.release().expect("second release");
    }

    #[test]
    fn guarded_claim_rejects_version_change_under_lock() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let first = acquired(
            store
                .claim_version(claim(temp.path(), RunMode::Step), &SessionVersion::empty())
                .expect("guarded initial claim"),
        );
        first.release().expect("release initial owner");
        let stale = store
            .inspect(&parent())
            .expect("inspect initial release")
            .expect("session snapshot");
        let stale = SessionVersion {
            session_id: stale.created.as_ref().map(Created::session_id),
            cursor: stale.cursor,
            journal_revision: stale.journal_revision,
        };

        acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("intervening claim"),
        )
        .release()
        .expect("intervening release");
        let journal = store.paths(&parent()).journal;
        let before = fs::read(&journal).expect("read before stale guarded claim");
        let error = store
            .claim_version(claim(temp.path(), RunMode::Step), &stale)
            .expect_err("stale guarded claim must fail before acquisition");
        assert!(error.to_string().contains("version changed"), "{error}");
        assert_eq!(
            fs::read(journal).expect("read after stale guarded claim"),
            before,
            "stale guarded admission must not append a fence"
        );
    }

    #[test]
    fn live_owner_returns_typed_lock_conflict() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );

        let conflict = store
            .claim(claim(temp.path(), RunMode::Continuous))
            .expect("competing claim");
        assert!(matches!(
            conflict,
            Outcome::Conflict(Conflict::Locked { .. })
        ));
        owner.release().expect("owner release");
    }

    #[test]
    fn inspect_missing_session_does_not_create_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let paths = store.paths(&parent());
        assert!(!paths.root().exists());

        assert!(store.inspect(&parent()).expect("inspect missing").is_none());
        assert!(
            !paths.root().exists(),
            "inspection must not create authority paths"
        );
    }

    #[test]
    fn two_readers_inspect_active_session_without_writes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let journal = owner.paths().journal().to_path_buf();
        let before = fs::read(&journal).expect("read journal before inspection");

        let first = store
            .inspect(owner.parent())
            .expect("first inspect")
            .expect("first snapshot");
        let second = store
            .inspect(owner.parent())
            .expect("second inspect")
            .expect("second snapshot");

        assert_eq!(first, second);
        assert_eq!(first.journal_revision, 2);
        assert_eq!(first.cursor, Some(owner.cursor().clone()));
        assert_eq!(
            first.active.as_ref().map(|active| active.fence),
            Some(Fence(1))
        );
        assert_eq!(
            fs::read(&journal).expect("read journal after inspection"),
            before
        );
        owner.release().expect("owner release");
    }

    #[test]
    fn inspection_waits_for_the_journal_publication_barrier() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let journal = owner.paths().journal().to_path_buf();
        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&journal)
            .expect("open journal barrier");
        lock_exclusive(&guard, &journal).expect("hold publication barrier");

        let reader = store.clone();
        let parent = owner.parent().clone();
        let (send, receive) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            send.send(reader.inspect(&parent)).expect("send inspection");
        });
        assert!(matches!(
            receive.recv_timeout(Duration::from_millis(25)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        // SAFETY: `guard` owns the descriptor locked above and remains alive.
        assert_eq!(
            unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&guard), libc::LOCK_UN) },
            0
        );
        let snapshot = receive
            .recv_timeout(Duration::from_secs(1))
            .expect("inspection resumes after publication")
            .expect("inspect published journal")
            .expect("session snapshot");
        assert_eq!(snapshot.journal_revision, 2);
        thread.join().expect("join inspection thread");
        owner.release().expect("owner release");
    }

    #[test]
    fn journal_idle_detects_publication_barrier() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        assert!(
            store
                .journal_idle(owner.parent())
                .expect("idle journal probe")
        );
        let journal = owner.paths().journal().to_path_buf();
        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&journal)
            .expect("open journal barrier");
        lock_exclusive(&guard, &journal).expect("hold publication barrier");
        assert!(
            !store
                .journal_idle(owner.parent())
                .expect("busy journal probe")
        );
        drop(guard);
        assert!(
            store
                .journal_idle(owner.parent())
                .expect("released journal probe")
        );
        owner.release().expect("owner release");
    }

    #[test]
    fn append_uncertainty_requires_replay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let failure = failure(
            owner,
            Error::AppendUncertain {
                path: temp.path().join("control-journal.jsonl"),
                detail: "injected ambiguous sync outcome".to_string(),
            },
        );
        assert!(matches!(failure, Failure::Uncertain { .. }));

        let recovery = store
            .claim(claim(temp.path(), RunMode::Continuous))
            .expect("replay after uncertainty");
        assert!(matches!(
            recovery,
            Outcome::Recoverable(RecoveryLease {
                cause: Some(RecoveryCause::OwnerLost { .. }),
                ..
            })
        ));
    }

    #[test]
    fn append_rejects_stale_revision() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let journal = owner.paths().journal().to_path_buf();
        let before = fs::read(&journal).expect("read admitted journal");
        let entry = Entry::Released {
            session_id: owner.session_id(),
            fence: owner.fence(),
            ready: None,
            recorded_at: RecordedAt::now(),
        };

        let stale = JournalHead {
            index: 1,
            hash: owner.head.hash.clone(),
        };
        let error = append_entry(&journal, &entry, &stale).expect_err("reject stale revision");
        assert!(matches!(
            error,
            Error::JournalRevision {
                expected: 1,
                actual: 2,
                ..
            }
        ));
        assert_eq!(fs::read(&journal).expect("read unchanged journal"), before);
        owner.release().expect("owner release");
    }

    #[test]
    fn append_rejects_rewrite() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let journal = owner.paths().journal().to_path_buf();
        let prior = fs::read_to_string(&journal).expect("read admitted journal");
        let rewritten = prior.replacen("continuous", "step", 1);
        assert_eq!(
            prior.lines().count(),
            rewritten.lines().count(),
            "fixture rewrite must preserve the line count"
        );
        fs::write(&journal, &rewritten).expect("rewrite journal prefix");
        let entry = Entry::Released {
            session_id: owner.session_id(),
            fence: owner.fence(),
            ready: None,
            recorded_at: RecordedAt::now(),
        };

        let error = append_entry(&journal, &entry, &owner.head).expect_err("reject prefix rewrite");
        assert!(matches!(error, Error::JournalPrefix { .. }));
        assert_eq!(
            fs::read_to_string(&journal).expect("read rejected rewrite"),
            rewritten
        );
        drop(owner);
    }

    #[test]
    fn stale_finish_is_uncertain() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let intent = owner
            .intent_with_live_api(true, false)
            .expect("live transition intent");
        let pending = match owner.begin(intent).expect("begin transition") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("transition must start"),
        };
        let journal = pending.paths().journal().to_path_buf();
        let prior = fs::read_to_string(&journal).expect("read pending journal");
        let rewritten = prior.replacen("\"step\"", "\"continuous\"", 1);
        fs::write(&journal, rewritten).expect("rewrite pending prefix");

        let failure = pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R8,
                evidence: ContentHash::of("effect-witness"),
            })
            .expect_err("stale finish must lose authority");
        assert!(matches!(
            failure,
            Failure::Uncertain {
                source: Error::JournalPrefix { .. }
            }
        ));
    }

    #[test]
    fn historical_collision_blocks_second_driver() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let prototype_root = temp.path().join("campaign/prototype1");
        let invocation_path = prototype_root.join("successor-invocation.json");
        let journal_path = prototype_root.join("transition-journal.jsonl");
        let plan_path = prototype_root.join("messages/child-plan/node-2fe75acd9e9cf6c3.json");
        let query_path = prototype_root.join("evidence/child-plan-db-query.json");
        let identity_bytes = stage_fixture(
            "parent_identity.json",
            &identity::parent_identity_path(&repo_root),
            "f44e95b0ab26ab1e33f2874c486c4586804dd7f08df56d8319e0f41701391376",
        );
        let profile_bytes = stage_fixture(
            "run-profile.commitment.json",
            &prototype_root.join("run-profile.commitment.json"),
            "441c051ea9e0eb3f3fe2914838f4a7571ba6fc3a1dcbd63a32ade50ac2afa7e9",
        );
        let invocation_bytes = stage_fixture(
            "successor-invocation.json",
            &invocation_path,
            "b6e3d9b9dc1b76d94637a37873b0232194f56ca098561a17968d72e5aeeafb46",
        );
        let journal_bytes = stage_exact_fixture(
            "transition-journal.jsonl",
            &journal_path,
            "b1fc907a7b56eaa240a6252a5a82458c217dad9234da4b77aa09b4f833d1b373",
        );
        let plan_bytes = stage_fixture(
            "child-plan.json",
            &plan_path,
            "361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f",
        );
        let query_bytes = stage_exact_fixture(
            "child-plan-db-query.json",
            &query_path,
            "68ac3ef851ec2a0ea1e193f1f081cb6644786ef242fd11f804793e4ce7824629",
        );

        let parent = identity::load_parent_identity(&repo_root).expect("load parent identity");
        let commitment = profile::load_admitted_commitment_from_prototype_root(&prototype_root)
            .expect("load profile commitment")
            .expect("historical commitment");
        let successor =
            match invocation::load_authority(&invocation_path).expect("load successor authority") {
                invocation::InvocationAuthority::Successor(successor) => successor,
                invocation::InvocationAuthority::Child(_) => panic!("fixture must be a successor"),
            };
        assert_eq!(successor.campaign_id(), parent.campaign_id());
        assert_eq!(successor.node_id(), parent.node_id());
        assert_eq!(
            successor.as_invocation().run_profile.as_ref(),
            Some(&commitment)
        );
        assert_eq!(
            successor.runtime_id().to_string(),
            "a96b5766-b5e0-43a8-99d8-b81936cb5449"
        );

        let entries = PrototypeJournal::new(&journal_path)
            .load_entries()
            .expect("load historical transition journal");
        assert_eq!(entries.len(), 49, "complete June 30 journal fixture");
        let runtime = successor.runtime_id();
        assert!(
            entries.iter().any(|entry| {
                matches!(
                    entry,
                    JournalEntry::Successor(record)
                        if record.runtime_id == Some(runtime)
                            && record.campaign_id == *parent.campaign_id()
                            && record.node_id == parent.node_id()
                            && matches!(
                                &record.state,
                                successor::State::Spawned { pid, .. } if *pid == 186_748
                            )
                )
            }),
            "journal must authenticate the successor spawn coordinate"
        );
        assert!(
            entries.iter().any(|entry| {
                matches!(
                    entry,
                    JournalEntry::ParentStarted(record)
                        if record.handoff_runtime_id == Some(runtime)
                            && record.pid == 186_748
                            && record.parent_identity == parent
                )
            }),
            "journal must authenticate the successor parent start"
        );
        assert!(
            entries.iter().any(|entry| {
                matches!(
                    entry,
                    JournalEntry::SuccessorHandoff(record)
                        if record.runtime_id == runtime
                            && record.campaign_id == *parent.campaign_id()
                            && record.node_id == parent.node_id()
                            && record.pid == 186_748
                )
            }),
            "journal must authenticate the successor handoff"
        );

        let plan: ChildPlanFiles =
            serde_json::from_slice(&plan_bytes).expect("load typed historical child plan");
        plan.validate_receiver(&parent)
            .expect("child plan belongs to historical parent");
        assert_eq!(plan.parent_node_id(), parent.node_id());
        assert_eq!(plan.child_generation(), 2);
        assert_eq!(plan.children().len(), 3);
        assert!(plan.contains_child("node-5a626eeb21d0c85a"));

        let query: serde_json::Value =
            serde_json::from_slice(&query_bytes).expect("load saved DB query response");
        assert_eq!(query["type"].as_str(), Some("walk_db_query"));
        assert_eq!(
            query["campaign_id"].as_str(),
            Some("p1-gated-parent-3g1x3-p3-20260630-174316")
        );
        let db_plan = query["rows"]
            .as_array()
            .expect("DB query rows")
            .iter()
            .find(|row| row["parent_node_id"].as_str() == Some(parent.node_id()))
            .expect("generation-2 DB child plan");
        assert_eq!(
            db_plan["plan_id"].as_str(),
            Some("9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04")
        );
        let db_hash = db_plan["message_sha256"]
            .as_str()
            .expect("DB message digest");
        assert_eq!(
            db_hash,
            "be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499"
        );
        let db_child = db_plan["child_node_id"].as_str().expect("DB child node");
        assert_eq!(db_child, "node-e9be81e08bda07a1");
        let file_hash = sha256_hex(&plan_bytes);
        assert_eq!(
            file_hash,
            "361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f"
        );
        assert_ne!(file_hash, db_hash, "DB and overwritten file must drift");
        assert!(
            !plan.contains_child(db_child),
            "DB child must be absent from the overwritten file"
        );

        let mut source_bytes = identity_bytes;
        source_bytes.extend_from_slice(&profile_bytes);
        source_bytes.extend_from_slice(&invocation_bytes);
        source_bytes.extend_from_slice(&journal_bytes);
        source_bytes.extend_from_slice(&plan_bytes);
        source_bytes.extend_from_slice(&query_bytes);
        let source = content_hash(&source_bytes);
        let epoch = epoch(&repo_root);
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(
                    Claim::historical(
                        source.clone(),
                        parent.clone(),
                        commitment.clone(),
                        RunMode::Continuous,
                        cursor(WalkPhase::R7, "historical-r7"),
                        epoch.clone(),
                    )
                    .with_runtime(successor.runtime_id()),
                )
                .expect("autonomous successor claim"),
        );

        let plan_before = fs::read(&plan_path).expect("read child plan before collision");
        let mut mutation_ran = false;
        let competing = store
            .claim(Claim::historical(
                source,
                parent,
                commitment,
                RunMode::Step,
                cursor(WalkPhase::R7, "historical-r7"),
                epoch,
            ))
            .expect("walk driver claim");
        match competing {
            Outcome::Conflict(Conflict::Locked { .. }) => {}
            Outcome::Acquired(lease) => {
                mutation_ran = true;
                lease.release().expect("unexpected lease release");
            }
            other => panic!("expected typed lock conflict, got {other:?}"),
        }
        assert!(
            !mutation_ran,
            "the competing driver must not reach mutation"
        );
        assert_eq!(
            fs::read(&plan_path).expect("read child plan after conflict"),
            plan_before,
            "typed conflict must precede any child-plan mutation"
        );
        owner.release().expect("owner release");
    }

    #[test]
    fn established_mode_rejects_a_different_driver_mode() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("continuous claim"),
        )
        .release()
        .expect("continuous release");

        let outcome = store
            .claim(claim(temp.path(), RunMode::Step))
            .expect("stepped claim");
        assert!(matches!(
            outcome,
            Outcome::Conflict(Conflict::Mode {
                active: RunMode::Continuous,
                requested: RunMode::Step,
                ..
            })
        ));
    }

    #[test]
    fn established_session_rejects_a_different_cursor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("initial claim"),
        )
        .release()
        .expect("initial release");

        let outcome = store
            .claim(claim_at(
                temp.path(),
                RunMode::Step,
                cursor(WalkPhase::R6, "different-r6"),
            ))
            .expect("drifted claim");
        assert!(matches!(
            outcome,
            Outcome::Conflict(Conflict::Cursor {
                active: Some(Cursor {
                    phase: WalkPhase::R7,
                    ..
                }),
                requested: Cursor {
                    phase: WalkPhase::R6,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn production_claim_requires_exact_admission() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let (setup, admitted) = admitted_setup(temp.path());
        let active = setup.intent.identity.clone();
        let origin = initial_cursor(&setup, &active, &admitted.commitment)
            .expect("derive setup-bound origin cursor");
        let valid = Claim::from_setup(
            setup.clone(),
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect("valid production claim");
        let store = Store::new(temp.path().join("control"));
        acquired(store.claim(valid).expect("persist production claim"))
            .release()
            .expect("release production claim");
        let replayed = acquired(
            store
                .claim(
                    Claim::from_setup(
                        setup.clone(),
                        active.clone(),
                        &admitted,
                        RunMode::Continuous,
                        origin.clone(),
                        epoch(temp.path()),
                    )
                    .expect("rebuild exact production claim"),
                )
                .expect("replay production claim"),
        );
        assert_eq!(replayed.fence(), Fence(2));
        replayed.release().expect("release replayed claim");

        let mut incomplete = setup.clone();
        incomplete.state = SetupAdmissionState::Admitting;
        let error = Claim::from_setup(
            incomplete,
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("incomplete setup must fail");
        assert!(error.to_string().contains("completed setup admission"));

        let mut bad_schema = setup.clone();
        bad_schema.schema_version = "prototype1-setup-admission.invalid".to_string();
        let error = Claim::from_setup(
            bad_schema,
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("invalid setup schema must fail");
        assert!(error.to_string().contains("setup authority is invalid"));

        let mut bad_node = setup.clone();
        bad_node.intent.node.node_id = "different-root".to_string();
        let error = Claim::from_setup(
            bad_node,
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("inconsistent setup carrier must fail");
        assert!(error.to_string().contains("setup authority is invalid"));

        let mut drifted = admitted.clone();
        drifted.commitment.sha256 = "0".repeat(64);
        let error = Claim::from_setup(
            setup.clone(),
            active.clone(),
            &drifted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("profile drift must fail");
        assert!(error.to_string().contains("profile hash"));

        let error = Claim::from_setup(
            setup.clone(),
            parent_in("other-campaign"),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("campaign drift must fail");
        assert!(error.to_string().contains("does not match parent campaign"));

        let error = Claim::from_setup(
            setup.clone(),
            parent(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("same-campaign parent drift must fail");
        assert!(
            error
                .to_string()
                .contains("does not match controller parent")
        );

        let error = Claim::from_setup(
            setup.clone(),
            active.clone(),
            &admitted,
            RunMode::Step,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect_err("mode drift must fail");
        assert!(error.to_string().contains("profile mode"));

        let other = tempfile::tempdir().expect("other tempdir");
        let error = Claim::from_setup(
            setup,
            active,
            &admitted,
            RunMode::Continuous,
            origin,
            epoch(other.path()),
        )
        .expect_err("repository drift must fail");
        assert!(
            error
                .to_string()
                .contains("does not match controller epoch")
        );
    }

    #[test]
    fn checkout_gates_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let (setup, admitted) = admitted_setup(temp.path());
        let active = setup.intent.identity.clone();
        let origin =
            initial_cursor(&setup, &active, &admitted.commitment).expect("derive setup origin");
        let created = |session_id| Entry::Created {
            schema_version: SCHEMA_VERSION.to_string(),
            session_id,
            origin: Origin::Admitted {
                setup: setup.clone(),
            },
            parent: active.clone(),
            profile: admitted.commitment.clone(),
            mode: RunMode::Continuous,
            cursor: Some(origin.clone()),
            recorded_at: RecordedAt::now(),
        };
        let branch_resume = Store::new(temp.path().join("branch-resume"));
        write_entries(
            branch_resume.paths(&active).journal(),
            &[created(SessionId::new())],
        );
        let head_resume = Store::new(temp.path().join("head-resume"));
        write_entries(
            head_resume.paths(&active).journal(),
            &[created(SessionId::new())],
        );

        run_git(temp.path(), &["checkout", "-qb", "wrong-branch"]);
        let request = Claim::from_setup(
            setup.clone(),
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect("construct wrong-branch claim");
        let error = Store::new(temp.path().join("branch-control"))
            .claim(request)
            .expect_err("wrong branch must block session creation");
        assert!(error.to_string().contains("setup branch"));
        let request = Claim::from_setup(
            setup.clone(),
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect("construct wrong-branch resume");
        let error = branch_resume
            .claim(request)
            .expect_err("wrong branch must block Created-only recovery");
        assert!(error.to_string().contains("setup branch"));

        run_git(
            temp.path(),
            &["checkout", "prototype1-parent-session-test-gen0"],
        );
        fs::write(temp.path().join("README.md"), "changed head\n")
            .expect("change repository fixture");
        run_git(temp.path(), &["add", "README.md"]);
        run_git(
            temp.path(),
            &[
                "-c",
                "user.name=Prototype Test",
                "-c",
                "user.email=prototype@example.invalid",
                "commit",
                "-qm",
                "change session fixture",
            ],
        );
        let request = Claim::from_setup(
            setup.clone(),
            active.clone(),
            &admitted,
            RunMode::Continuous,
            origin.clone(),
            epoch(temp.path()),
        )
        .expect("construct wrong-head claim");
        let error = Store::new(temp.path().join("head-control"))
            .claim(request)
            .expect_err("wrong head must block session creation");
        assert!(error.to_string().contains("setup head"));
        let request = Claim::from_setup(
            setup,
            active,
            &admitted,
            RunMode::Continuous,
            origin,
            epoch(temp.path()),
        )
        .expect("construct wrong-head resume");
        let error = head_resume
            .claim(request)
            .expect_err("wrong head must block Created-only recovery");
        assert!(error.to_string().contains("setup head"));
    }

    #[test]
    fn lost_owner_recovery_retains_lock_and_issues_next_fence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(
                    claim(temp.path(), RunMode::Continuous)
                        .with_runtime(RuntimeId::new())
                        .with_pid(4242),
                )
                .expect("owner claim"),
        );
        let session_id = owner.session_id();
        abandon(owner);

        let mut recovery = match store
            .claim(claim(temp.path(), RunMode::Continuous).with_pid(5252))
            .expect("recovery claim")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        assert!(matches!(
            recovery.cause(),
            Some(RecoveryCause::OwnerLost {
                session_id: found,
                fence: Fence(1),
                pid: 4242,
                ..
            }) if *found == session_id
        ));
        assert!(matches!(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("claim while recovery held"),
            Outcome::Conflict(Conflict::Locked { .. })
        ));
        recovery = recovery
            .resolve(RecoveryResolution::AbandonOwner)
            .expect("abandon lost owner");
        let next = recovery.take_over().expect("take over recovered session");
        assert_eq!(next.session_id(), session_id);
        assert_eq!(next.fence().get(), 2);
        next.release().expect("release recovered owner");
    }

    #[test]
    fn prior_graph_v1_replays_read_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let requested = claim_at(
            temp.path(),
            RunMode::Continuous,
            cursor(WalkPhase::R13b, "legacy-r13b-evidence"),
        );
        let mut prior = requested.epoch.clone();
        prior.protocol_version = 3;
        prior.transition_graph_version = GRAPH_VERSION_V1.to_string();
        prior.active_branch = None;
        let session_id = SessionId::new();
        let fence = Fence(1);
        let transition_id = TransitionId::new();
        let intent = AttemptIntent {
            transition_id,
            expected: WalkPhase::R12,
            targets: vec![WalkPhase::R13a, WalkPhase::R13b],
            allow_live_api: false,
            allow_git_changes: true,
            epoch: prior.clone(),
            evidence: ContentHash("legacy-r12-evidence".to_string()),
            retry: 0,
        };
        let entries = [
            Entry::Created {
                schema_version: SCHEMA_VERSION_V1.to_string(),
                session_id,
                origin: requested.origin.clone(),
                parent: requested.parent.clone(),
                profile: requested.profile.clone(),
                mode: requested.mode,
                cursor: None,
                recorded_at: RecordedAt::now(),
            },
            Entry::Acquired {
                session_id,
                fence,
                epoch: prior.clone(),
                runtime_id: None,
                pid: 4242,
                incarnation: None,
                recorded_at: RecordedAt::now(),
            },
            Entry::Began {
                session_id,
                fence,
                intent,
                recorded_at: RecordedAt::now(),
            },
            Entry::Finished {
                session_id,
                transition_id,
                fence,
                result: AttemptResult::Committed {
                    phase: WalkPhase::R13b,
                    evidence: ContentHash("legacy-r13b-evidence".to_string()),
                },
                evidence: None,
                epoch: None,
                recorded_at: RecordedAt::now(),
            },
            Entry::Released {
                session_id,
                fence,
                ready: None,
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&requested.parent).journal;
        write_legacy(&journal, &entries);

        let snapshot = store
            .inspect(&requested.parent)
            .expect("inspect v1 journal")
            .expect("v1 snapshot");
        assert_eq!(
            snapshot.cursor,
            Some(Cursor {
                phase: WalkPhase::R13b,
                evidence: ContentHash("legacy-r13b-evidence".to_string()),
            })
        );
        assert!(matches!(
            store.claim(requested).expect("legacy claim"),
            Outcome::Conflict(Conflict::Schema {
                active,
                supported,
                ..
            }) if active == SCHEMA_VERSION_V1 && supported == SCHEMA_VERSION
        ));
    }

    #[test]
    fn live_result_from_v2_replays_read_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let target = cursor(WalkPhase::R8, "legacy-v2-r8-evidence");
        let request = claim_at(temp.path(), RunMode::Step, target.clone());
        let session_id = SessionId::new();
        let fence = Fence(1);
        let source = cursor(WalkPhase::R7, "legacy-v2-r7-evidence");
        let mut prior = request.epoch.clone();
        prior.protocol_version = 3;
        prior.active_branch = None;
        let targets = current_targets(source.phase, false);
        let intent = AttemptIntent {
            transition_id: transition_id(
                session_id,
                &source,
                &targets,
                false,
                false,
                &prior.transition_graph_version,
                0,
            )
            .expect("legacy v2 transition id"),
            expected: source.phase,
            targets,
            allow_live_api: false,
            allow_git_changes: false,
            epoch: prior.clone(),
            evidence: source.evidence.clone(),
            retry: 0,
        };
        let transition_id = intent.transition_id;
        let began = Entry::Began {
            session_id,
            fence,
            intent,
            recorded_at: RecordedAt::now(),
        };
        let mut value = serde_json::to_value(began).expect("serialize v2 began entry");
        value
            .get_mut("intent")
            .and_then(serde_json::Value::as_object_mut)
            .expect("began intent object")
            .remove("allow_live_api");
        let began: Entry = serde_json::from_value(value).expect("deserialize legacy v2 entry");
        let Entry::Began { intent, .. } = &began else {
            panic!("legacy began entry changed variant");
        };
        assert!(!intent.allow_live_api);
        let epoch = EpochReceipt {
            before: prior.clone(),
            after: Some(prior.clone()),
        };
        let entries = [
            Entry::Created {
                schema_version: SCHEMA_VERSION_V2.to_string(),
                session_id,
                origin: request.origin.clone(),
                parent: request.parent.clone(),
                profile: request.profile.clone(),
                mode: request.mode,
                cursor: Some(source),
                recorded_at: RecordedAt::now(),
            },
            Entry::Acquired {
                session_id,
                fence,
                epoch: prior,
                runtime_id: None,
                pid: 4242,
                incarnation: None,
                recorded_at: RecordedAt::now(),
            },
            began,
            Entry::Finished {
                session_id,
                transition_id,
                fence,
                result: AttemptResult::Committed {
                    phase: WalkPhase::R8,
                    evidence: target.evidence.clone(),
                },
                evidence: None,
                epoch: Some(epoch),
                recorded_at: RecordedAt::now(),
            },
            Entry::Released {
                session_id,
                fence,
                ready: None,
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&request.parent).journal;
        write_legacy(&journal, &entries);

        let snapshot = store
            .inspect(&request.parent)
            .expect("inspect v2 journal")
            .expect("v2 snapshot");
        assert_eq!(snapshot.cursor, Some(target));
        assert!(matches!(
            store.claim(request).expect("legacy v2 claim"),
            Outcome::Conflict(Conflict::Schema {
                active,
                supported,
                ..
            }) if active == SCHEMA_VERSION_V2 && supported == SCHEMA_VERSION
        ));
    }

    #[test]
    fn v3_replay_is_readonly() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let target = cursor(WalkPhase::R8, "legacy-v3-r8-evidence");
        let request = claim_at(temp.path(), RunMode::Step, target.clone());
        let session_id = SessionId::new();
        let fence = Fence(1);
        let source = cursor(WalkPhase::R7, "legacy-v3-r7-evidence");
        let mut prior = request.epoch.clone();
        prior.protocol_version = 3;
        prior.active_branch = None;
        let intent = AttemptIntent::with_live_api(session_id, source.clone(), false, prior.clone())
            .expect("legacy v3 intent");
        let transition_id = intent.transition_id;
        let entries = [
            Entry::Created {
                schema_version: SCHEMA_VERSION_V3.to_string(),
                session_id,
                origin: request.origin.clone(),
                parent: request.parent.clone(),
                profile: request.profile.clone(),
                mode: request.mode,
                cursor: Some(source),
                recorded_at: RecordedAt::now(),
            },
            Entry::Acquired {
                session_id,
                fence,
                epoch: prior.clone(),
                runtime_id: None,
                pid: 4242,
                incarnation: None,
                recorded_at: RecordedAt::now(),
            },
            Entry::Began {
                session_id,
                fence,
                intent,
                recorded_at: RecordedAt::now(),
            },
            Entry::Finished {
                session_id,
                transition_id,
                fence,
                result: AttemptResult::Committed {
                    phase: WalkPhase::R8,
                    evidence: target.evidence.clone(),
                },
                evidence: None,
                epoch: Some(EpochReceipt {
                    before: prior.clone(),
                    after: Some(prior),
                }),
                recorded_at: RecordedAt::now(),
            },
            Entry::Released {
                session_id,
                fence,
                ready: None,
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&request.parent).journal;
        write_legacy(&journal, &entries);

        let snapshot = store
            .inspect(&request.parent)
            .expect("inspect v3 journal")
            .expect("v3 snapshot");
        assert_eq!(snapshot.cursor, Some(target));
        assert!(matches!(
            store.claim(request).expect("legacy v3 claim"),
            Outcome::Conflict(Conflict::Schema {
                active,
                supported,
                ..
            }) if active == SCHEMA_VERSION_V3 && supported == SCHEMA_VERSION
        ));
    }

    #[test]
    fn successor_pid_reuse_cannot_create_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let mut request = successor_claim(temp.path(), RuntimeId::new(), std::process::id());
        request
            .incarnation
            .as_mut()
            .expect("current successor incarnation")
            .start_ticks += 1;

        let error = Store::new(temp.path().join("control"))
            .claim(request)
            .expect_err("reused numeric PID must not claim successor authority");
        assert!(
            error.to_string().contains("exact process incarnation"),
            "{error}"
        );
    }

    #[test]
    fn legacy_spawn_origin_replays_v4_but_cannot_claim_v5() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let runtime = RuntimeId::new();
        let request = legacy_successor(successor_claim(temp.path(), runtime, std::process::id()));
        let current = Store::new(temp.path().join("current-control"));
        let error = current
            .claim(request.clone())
            .expect_err("legacy Spawned origin must not create a v5 session");
        assert!(
            error.to_string().contains("has no process incarnation"),
            "{error}"
        );

        let legacy = Store::new(temp.path().join("legacy-control"));
        let session_id = SessionId::new();
        let fence = Fence(1);
        let entries = [
            Entry::Created {
                schema_version: SCHEMA_VERSION_V4.to_string(),
                session_id,
                origin: request.origin.clone(),
                parent: request.parent.clone(),
                profile: request.profile.clone(),
                mode: request.mode,
                cursor: Some(request.origin_cursor.clone()),
                recorded_at: RecordedAt::now(),
            },
            Entry::Acquired {
                session_id,
                fence,
                epoch: request.epoch.clone(),
                runtime_id: Some(runtime),
                pid: std::process::id(),
                incarnation: None,
                recorded_at: RecordedAt::now(),
            },
            Entry::Released {
                session_id,
                fence,
                ready: None,
                recorded_at: RecordedAt::now(),
            },
        ];
        write_entries(legacy.paths(&request.parent).journal(), &entries);

        let snapshot = legacy
            .inspect(&request.parent)
            .expect("inspect legacy v4 journal")
            .expect("legacy v4 snapshot");
        assert_eq!(snapshot.cursor, Some(request.origin_cursor.clone()));
        assert!(matches!(
            legacy.claim(request).expect("legacy v4 claim"),
            Outcome::Conflict(Conflict::Schema {
                active,
                supported,
                ..
            }) if active == SCHEMA_VERSION_V4 && supported == SCHEMA_VERSION
        ));
    }

    #[test]
    fn pending_attempt_requires_explicit_cancelled_reconciliation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let intent = intent(&owner);
        let transition_id = intent.transition_id;
        let epoch = same_epoch(&intent.epoch);
        let pending = match owner.begin(intent).expect("begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new transition should start"),
        };
        abandon(pending);

        let mut recovery = match store
            .claim(claim(temp.path(), RunMode::Continuous))
            .expect("recovery claim")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        assert!(matches!(
            recovery.cause(),
            Some(RecoveryCause::AttemptPending { intent, .. })
                if intent.transition_id == transition_id
        ));
        recovery = recovery
            .resolve(RecoveryResolution::ResolveAttempt {
                transition_id,
                result: AttemptResult::Cancelled {
                    phase: WalkPhase::R7,
                    evidence: ContentHash::of("r7-evidence"),
                    detail: "no committed R8 evidence found".to_string(),
                },
                evidence: None,
                epoch,
            })
            .expect("reconcile pending attempt");
        recovery
            .take_over()
            .expect("take over after reconciliation")
            .release()
            .expect("release reconciled owner");
    }

    #[test]
    fn unresolved_attempt_can_only_abandon_session_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let request = claim(temp.path(), RunMode::Continuous);
        let owner = acquired(store.claim(request.clone()).expect("owner claim"));
        let transition_id = owner
            .intent_with_live_api(true, false)
            .expect("effectful intent")
            .transition_id;
        let intent = owner
            .intent_with_live_api(true, false)
            .expect("same effectful intent");
        let pending = match owner.begin(intent).expect("begin effectful attempt") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("fresh effectful attempt must start"),
        };
        abandon(pending);

        let recovery = match store.claim(request.clone()).expect("recovery claim") {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        recovery
            .abandon_session("provider effect cannot be proven absent".to_string())
            .expect("permanently abandon unresolved session");

        let snapshot = store
            .inspect(&parent())
            .expect("inspect abandoned session")
            .expect("session snapshot");
        assert!(snapshot.active.is_none());
        assert_eq!(
            snapshot.abandoned.as_deref(),
            Some("provider effect cannot be proven absent")
        );
        assert!(matches!(
            snapshot.attempts.as_slice(),
            [Attempt::Pending { intent, .. }] if intent.transition_id == transition_id
        ));
        assert!(matches!(
            store.claim(request).expect("probe abandoned session"),
            Outcome::Conflict(Conflict::Abandoned { .. })
        ));
    }

    #[test]
    fn indeterminate_attempt_blocks_idle_authority() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let intent = intent(&owner);
        let transition_id = intent.transition_id;
        let pending = match owner.begin(intent).expect("begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new transition should start"),
        };
        let finished = pending
            .finish(AttemptResult::Indeterminate {
                phase: None,
                evidence: None,
                detail: "effect boundary could not be reconstructed".to_string(),
            })
            .expect("record indeterminate result");
        let Finished::Uncertain { lease, receipt, .. } = finished else {
            panic!("indeterminate result must not restore idle authority");
        };
        assert_eq!(lease.cursor(), &cursor(WalkPhase::R7, "r7-evidence"));
        let snapshot = store
            .inspect(&parent())
            .expect("inspect indeterminate")
            .expect("session snapshot");
        assert_eq!(snapshot.cursor, Some(cursor(WalkPhase::R7, "r7-evidence")));
        abandon(lease);

        let recovery = match store
            .claim(claim(temp.path(), RunMode::Step))
            .expect("recovery claim")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        assert!(matches!(
            recovery.cause(),
            Some(RecoveryCause::AttemptIndeterminate { intent, .. })
                if intent.transition_id == transition_id
        ));
        let recovery = recovery
            .resolve(RecoveryResolution::ResolveAttempt {
                transition_id,
                result: AttemptResult::Cancelled {
                    phase: WalkPhase::R7,
                    evidence: ContentHash::of("r7-evidence"),
                    detail: "reconstruction found no committed R8 evidence".to_string(),
                },
                evidence: None,
                epoch: receipt.epoch,
            })
            .expect("reconcile indeterminate result");
        recovery
            .take_over()
            .expect("take over reconciled session")
            .release()
            .expect("release reconciled owner");
    }

    #[test]
    fn missing_after_epoch_is_recovered_by_a_separate_observation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let request = claim(temp.path(), RunMode::Step);
        let session_id = SessionId::new();
        let fence = Fence(1);
        let intent = AttemptIntent::with_live_api(
            session_id,
            request.cursor.clone(),
            false,
            request.epoch.clone(),
        )
        .expect("valid transition intent");
        let transition_id = intent.transition_id;
        let missing = EpochReceipt {
            before: intent.epoch.clone(),
            after: None,
        };
        let entries = [
            Entry::Created {
                schema_version: SCHEMA_VERSION.to_string(),
                session_id,
                origin: request.origin.clone(),
                parent: request.parent.clone(),
                profile: request.profile.clone(),
                mode: request.mode,
                cursor: Some(request.cursor.clone()),
                recorded_at: RecordedAt::now(),
            },
            Entry::Acquired {
                session_id,
                fence,
                epoch: request.epoch.clone(),
                runtime_id: None,
                pid: 4242,
                incarnation: request.incarnation.clone(),
                recorded_at: RecordedAt::now(),
            },
            Entry::Began {
                session_id,
                fence,
                intent: intent.clone(),
                recorded_at: RecordedAt::now(),
            },
            Entry::Finished {
                session_id,
                transition_id,
                fence,
                result: AttemptResult::Indeterminate {
                    phase: None,
                    evidence: None,
                    detail: "controller epoch could not be captured after the edge".to_string(),
                },
                evidence: None,
                epoch: Some(missing.clone()),
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&request.parent).journal;
        write_entries(&journal, &entries);

        let recovery = match store.claim(request).expect("recovery claim") {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        let observed = same_epoch(&intent.epoch);
        let recovery = recovery
            .resolve(RecoveryResolution::ResolveAttempt {
                transition_id,
                result: AttemptResult::Cancelled {
                    phase: WalkPhase::R7,
                    evidence: ContentHash::of("r7-evidence"),
                    detail: "reconstruction found no committed R8 evidence".to_string(),
                },
                evidence: None,
                epoch: observed.clone(),
            })
            .expect("record separate recovery observation");
        recovery
            .take_over()
            .expect("take over recovered session")
            .release()
            .expect("release recovered owner");

        let snapshot = store
            .inspect(&parent())
            .expect("inspect recovered attempt")
            .expect("session snapshot");
        let Attempt::Recovered { prior, receipt } = &snapshot.attempts[0] else {
            panic!("attempt must retain its original receipt and recovery observation");
        };
        assert!(matches!(
            prior.as_ref(),
            Attempt::Finished(prior) if prior.epoch == Some(missing)
        ));
        assert_eq!(receipt.epoch, observed);
        assert!(matches!(receipt.result, AttemptResult::Cancelled { .. }));
    }

    #[test]
    fn rejected_result_retains_committed_cursor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let intent = intent(&owner);
        let first_id = intent.transition_id;
        let pending = match owner.begin(intent).expect("begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new transition should start"),
        };
        let finished = pending
            .finish(AttemptResult::Rejected {
                phase: WalkPhase::R7,
                evidence: ContentHash::of("r7-evidence"),
                detail: "typed edge rejected before commit".to_string(),
            })
            .expect("record rejection");
        let Finished::Terminal { lease, .. } = finished else {
            panic!("rejection must restore idle authority");
        };
        assert_eq!(lease.cursor(), &cursor(WalkPhase::R7, "r7-evidence"));
        let retry = lease
            .intent_with_live_api(true, false)
            .expect("retry rejected edge");
        assert_eq!(retry.retry, 1);
        assert_ne!(retry.transition_id, first_id);
        lease.release().expect("release owner");
    }

    #[test]
    fn cancelled_result_retains_committed_cursor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let intent = intent(&owner);
        let first_id = intent.transition_id;
        let pending = match owner.begin(intent).expect("begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new transition should start"),
        };
        let finished = pending
            .finish(AttemptResult::Cancelled {
                phase: WalkPhase::R7,
                evidence: ContentHash::of("r7-evidence"),
                detail: "cancelled before the effect boundary".to_string(),
            })
            .expect("record cancellation");
        let Finished::Terminal { lease, .. } = finished else {
            panic!("cancellation must restore idle authority");
        };
        assert_eq!(lease.cursor(), &cursor(WalkPhase::R7, "r7-evidence"));
        let retry = lease
            .intent_with_live_api(true, false)
            .expect("retry cancelled edge");
        assert_eq!(retry.retry, 1);
        assert_ne!(retry.transition_id, first_id);
        let pending = match lease.begin(retry).expect("begin retry") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("retry must start a new attempt"),
        };
        let finished = pending
            .finish(AttemptResult::Cancelled {
                phase: WalkPhase::R7,
                evidence: ContentHash::of("r7-evidence"),
                detail: "cancel retry before the effect boundary".to_string(),
            })
            .expect("record retry cancellation");
        let Finished::Terminal { lease, .. } = finished else {
            panic!("retry cancellation must restore idle authority");
        };
        lease.release().expect("release owner");
    }

    #[test]
    fn recovered_commit_advances_cursor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let intent = owner
            .intent_with_live_api(true, false)
            .expect("valid live transition intent");
        let transition_id = intent.transition_id;
        let epoch = same_epoch(&intent.epoch);
        let pending = match owner.begin(intent).expect("begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new transition should start"),
        };
        abandon(pending);

        let recovery = match store
            .claim(claim(temp.path(), RunMode::Step))
            .expect("recovery claim")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        let recovery = recovery
            .resolve(RecoveryResolution::ResolveAttempt {
                transition_id,
                result: AttemptResult::Committed {
                    phase: WalkPhase::R8,
                    evidence: ContentHash::of("recovered-r8"),
                },
                evidence: None,
                epoch,
            })
            .expect("resolve committed attempt");
        let owner = recovery.take_over().expect("take over recovered session");
        let committed_cursor = owner.cursor().clone();
        assert_eq!(committed_cursor.phase, WalkPhase::R8);
        let snapshot = store
            .inspect(&parent())
            .expect("inspect recovered cursor")
            .expect("recovered session snapshot");
        let receipt = snapshot
            .attempts
            .iter()
            .find_map(Attempt::terminal)
            .expect("recovered terminal receipt");
        let receipt_cursor = receipt
            .evidence
            .as_ref()
            .expect("recovered commit certificate")
            .cursor()
            .expect("hash recovered certificate");
        assert_eq!(receipt_cursor, committed_cursor);
        owner.release().expect("release recovered owner");
    }

    #[test]
    fn branch_intent_records_observed_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim_at(
                    temp.path(),
                    RunMode::Step,
                    cursor(WalkPhase::R1, "r1-evidence"),
                ))
                .expect("owner claim"),
        );
        let intent = owner.intent(false).expect("valid branch intent");
        assert_eq!(intent.targets, vec![WalkPhase::R2a, WalkPhase::R3]);
        let pending = match owner.begin(intent).expect("begin branch edge") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new branch edge should start"),
        };
        let finished = pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R3,
                evidence: ContentHash::of("r3-evidence"),
            })
            .expect("finish observed branch");
        let Finished::Terminal { lease, receipt, .. } = finished else {
            panic!("committed branch must restore idle authority");
        };
        assert!(matches!(
            receipt.result,
            AttemptResult::Committed {
                phase: WalkPhase::R3,
                ..
            }
        ));
        let epoch = receipt.epoch.expect("v2 attempt has epoch receipt");
        assert_eq!(epoch.after.as_ref(), Some(&epoch.before));
        lease.release().expect("release branch owner");
    }

    #[test]
    fn begin_rejects_cursor_mismatch_before_append() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let journal = owner.paths().journal().to_path_buf();
        let before = fs::read(&journal).expect("read journal");
        let mismatch = AttemptIntent::new(
            owner.session_id(),
            cursor(WalkPhase::R6, "different-source"),
            false,
            owner.epoch().clone(),
        )
        .expect("valid mismatched intent");

        let Failure::Retained { authority, source } = owner
            .begin(mismatch)
            .expect_err("cursor mismatch must fail closed")
        else {
            panic!("cursor mismatch is known before append");
        };
        assert!(matches!(source, Error::CursorMismatch { .. }));
        assert_eq!(fs::read(&journal).expect("read unchanged journal"), before);
        authority.release().expect("release retained authority");
    }

    #[test]
    fn transition_id_is_deterministic_and_domain_separated() {
        let temp = tempfile::tempdir().expect("tempdir");
        let epoch = epoch(temp.path());
        let session = SessionId::new();
        let source = cursor(WalkPhase::R3, "r3-evidence");
        let first = AttemptIntent::new(session, source.clone(), false, epoch.clone())
            .expect("first intent");
        let repeated = AttemptIntent::new(session, source.clone(), false, epoch.clone())
            .expect("repeated intent");
        let other_session =
            AttemptIntent::new(SessionId::new(), source.clone(), false, epoch.clone())
                .expect("other session intent");
        let other_cursor = AttemptIntent::new(
            session,
            cursor(WalkPhase::R6, "r6-evidence"),
            false,
            epoch.clone(),
        )
        .expect("other cursor intent");
        let other_capability = AttemptIntent::new(session, source.clone(), true, epoch.clone())
            .expect("other capability intent");
        let live = AttemptIntent::with_live_api(session, source, false, epoch)
            .expect("live-provider intent");
        let mut restarted = repeated.epoch.clone();
        restarted.exe_modified_unix_ms = restarted.exe_modified_unix_ms.map(|value| value + 1);
        let same_key = AttemptIntent::new(session, repeated.cursor(), false, restarted)
            .expect("restart intent");
        let retry = AttemptIntent::with_retry(
            session,
            repeated.cursor(),
            false,
            false,
            repeated.epoch.clone(),
            1,
        )
        .expect("retry intent");

        assert_eq!(first.transition_id, repeated.transition_id);
        assert_eq!(first.transition_id, same_key.transition_id);
        assert_ne!(first.transition_id, other_session.transition_id);
        assert_ne!(first.transition_id, other_cursor.transition_id);
        assert_ne!(first.transition_id, other_capability.transition_id);
        assert_ne!(first.transition_id, live.transition_id);
        assert_ne!(first.transition_id, retry.transition_id);
    }

    #[test]
    fn v2_tables_stay_aligned() {
        assert_eq!(TRANSITION_GRAPH_VERSION, GRAPH_VERSION_V2);
        for edge in crate::cli::prototype1_state::edge::ControlEdge::ALL {
            assert_eq!(
                crate::cli::prototype1_state::edge::ControlEdge::for_graph(
                    GRAPH_VERSION_V2,
                    edge.from(),
                    edge.to(),
                ),
                Some(edge)
            );
            assert!(
                v2_targets(edge.from(), true)
                    .expect("v2 source phase")
                    .contains(&edge.to()),
                "v2 target table is missing {edge}"
            );
            if !edge.requires_checkout() {
                assert!(
                    v2_targets(edge.from(), false)
                        .expect("v2 source phase")
                        .contains(&edge.to()),
                    "v2 non-checkout table is missing {edge}"
                );
            }
        }
    }

    #[test]
    fn epoch_changes_cursor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session_id = SessionId::new();
        let before = epoch(temp.path());
        let intent = AttemptIntent::with_live_api(
            session_id,
            cursor(WalkPhase::R12, "r12-evidence"),
            true,
            before.clone(),
        )
        .expect("checkout transition intent");
        let witness = ContentHash::of("checkout-effect");
        let mut first_after = before.clone();
        first_after.git_head = Some("a".repeat(40));
        let first_epoch = EpochReceipt {
            before: before.clone(),
            after: Some(first_after),
        };
        let mut second_after = before.clone();
        second_after.git_head = Some("b".repeat(40));
        let second_epoch = EpochReceipt {
            before,
            after: Some(second_after),
        };
        let first = CursorEvidence::new(
            session_id,
            &parent(),
            &profile(),
            &intent,
            WalkPhase::R13b,
            &witness,
            &first_epoch,
        )
        .expect("first checkout certificate")
        .cursor()
        .expect("first checkout cursor");
        let second = CursorEvidence::new(
            session_id,
            &parent(),
            &profile(),
            &intent,
            WalkPhase::R13b,
            &witness,
            &second_epoch,
        )
        .expect("second checkout certificate")
        .cursor()
        .expect("second checkout cursor");

        assert_ne!(first, second);
    }

    #[test]
    fn r12_intent_preserves_git_changes_authority() {
        let temp = tempfile::tempdir().expect("tempdir");
        let blocked = AttemptIntent::new(
            SessionId::new(),
            cursor(WalkPhase::R12, "r12-evidence"),
            false,
            epoch(temp.path()),
        )
        .expect("R13a remains admitted without checkout authority");
        assert_eq!(blocked.targets, vec![WalkPhase::R13a]);
        assert!(!blocked.allow_git_changes);
        let error = validate_result(
            &blocked,
            &AttemptResult::Committed {
                phase: WalkPhase::R13b,
                evidence: ContentHash::of("r13b-evidence"),
            },
        )
        .expect_err("R13b must not commit without checkout authority");
        assert!(
            error
                .to_string()
                .contains("not one of the admitted targets")
        );

        let admitted = AttemptIntent::new(
            SessionId::new(),
            cursor(WalkPhase::R12, "r12-evidence"),
            true,
            epoch(temp.path()),
        )
        .expect("explicit checkout authority admits all typed outcomes");
        assert_eq!(
            admitted.targets,
            vec![WalkPhase::R13a, WalkPhase::R13b, WalkPhase::R13c]
        );
        assert!(!admitted.allow_live_api);
        assert!(admitted.allow_git_changes);
    }

    #[test]
    fn live_target_rejects_an_unadmitted_result() {
        let temp = tempfile::tempdir().expect("tempdir");
        for phase in [WalkPhase::R5, WalkPhase::R7, WalkPhase::R10] {
            let error = AttemptIntent::new(
                SessionId::new(),
                cursor(phase, phase.as_str()),
                false,
                epoch(temp.path()),
            )
            .expect_err("provider edge must fail before durable admission");
            assert!(
                error.to_string().contains("live-provider authority"),
                "{error}"
            );
        }
        let result = AttemptResult::Committed {
            phase: WalkPhase::R8,
            evidence: ContentHash::of("r8-evidence"),
        };
        let admitted = AttemptIntent::with_live_api(
            SessionId::new(),
            cursor(WalkPhase::R7, "r7-evidence"),
            false,
            epoch(temp.path()),
        )
        .expect("live intent");
        validate_result_current(&admitted, &result).expect("live R8 result");
    }

    #[test]
    fn epoch_change_only_advances_admitted_checkout_edges() {
        let temp = tempfile::tempdir().expect("tempdir");
        let before = epoch(temp.path());
        let mut after = before.clone();
        after.git_head = Some("changed-checkout".to_string());

        let r1 = AttemptIntent::new(
            SessionId::new(),
            cursor(WalkPhase::R1, "r1-evidence"),
            false,
            before.clone(),
        )
        .expect("R1 intent");
        let receipt = EpochReceipt {
            before: before.clone(),
            after: Some(after.clone()),
        };
        let committed = AttemptResult::Committed {
            phase: WalkPhase::R2a,
            evidence: ContentHash::of("r2a-evidence"),
        };
        let (result, epoch) = settle_epoch(&r1, committed.clone(), &receipt);
        assert_eq!(result, committed);
        assert_eq!(epoch, after);

        let mut rebuilt = after.clone();
        rebuilt.exe_modified_unix_ms = Some(
            rebuilt
                .exe_modified_unix_ms
                .unwrap_or_default()
                .saturating_add(1),
        );
        let rebuilt_receipt = EpochReceipt {
            before: before.clone(),
            after: Some(rebuilt),
        };
        let (result, epoch) = settle_epoch(&r1, committed, &rebuilt_receipt);
        assert!(matches!(result, AttemptResult::Indeterminate { .. }));
        assert_eq!(epoch, before);

        let disallowed = AttemptResult::Committed {
            phase: WalkPhase::R3,
            evidence: ContentHash::of("r3-evidence"),
        };
        let (result, epoch) = settle_epoch(&r1, disallowed, &receipt);
        assert!(matches!(result, AttemptResult::Indeterminate { .. }));
        assert_eq!(epoch, before);

        let r12 = AttemptIntent::new(
            SessionId::new(),
            cursor(WalkPhase::R12, "r12-evidence"),
            true,
            before.clone(),
        )
        .expect("R12 intent");
        for phase in [WalkPhase::R13b, WalkPhase::R13c] {
            let committed = AttemptResult::Committed {
                phase,
                evidence: ContentHash::of(phase.as_str()),
            };
            let (result, epoch) = settle_epoch(&r12, committed.clone(), &receipt);
            assert_eq!(result, committed);
            assert_eq!(epoch, after);
        }
        let stopped = AttemptResult::Committed {
            phase: WalkPhase::R13a,
            evidence: ContentHash::of("r13a-evidence"),
        };
        let (result, epoch) = settle_epoch(&r12, stopped, &receipt);
        assert!(matches!(result, AttemptResult::Indeterminate { .. }));
        assert_eq!(epoch, before);
    }

    #[test]
    fn recovery_cannot_commit_an_illegal_epoch_change() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session_id = SessionId::new();
        let intent = AttemptIntent::with_live_api(
            session_id,
            cursor(WalkPhase::R7, "r7-evidence"),
            false,
            epoch(temp.path()),
        )
        .expect("R7 intent");
        let mut after = intent.epoch.clone();
        after.git_head = Some("unexpected-checkout".to_string());
        let epoch = EpochReceipt {
            before: intent.epoch.clone(),
            after: Some(after),
        };
        let fence = Fence(1);
        let receipt = AttemptReceipt {
            intent: intent.clone(),
            fence,
            result: AttemptResult::Indeterminate {
                phase: None,
                evidence: None,
                detail: "unexpected epoch change".to_string(),
            },
            evidence: None,
            epoch: Some(epoch.clone()),
        };
        let mut replay = Replay::default();
        replay
            .attempts
            .insert(intent.transition_id, Attempt::Finished(receipt));
        let cause = RecoveryCause::AttemptIndeterminate {
            session_id,
            fence,
            intent: intent.clone(),
            detail: "unexpected epoch change".to_string(),
        };
        let resolution = RecoveryResolution::ResolveAttempt {
            transition_id: intent.transition_id,
            result: AttemptResult::Committed {
                phase: WalkPhase::R8,
                evidence: ContentHash::of("r8-evidence"),
            },
            evidence: None,
            epoch: Some(epoch),
        };

        let error = validate_resolution_current(&replay, &cause, &resolution)
            .expect_err("illegal epoch drift must remain indeterminate");
        assert!(
            error
                .to_string()
                .contains("did not admit checkout mutation")
        );
    }

    #[test]
    fn recovery_cannot_contradict_a_recorded_after_epoch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session_id = SessionId::new();
        let intent = AttemptIntent::with_live_api(
            session_id,
            cursor(WalkPhase::R7, "r7-evidence"),
            false,
            epoch(temp.path()),
        )
        .expect("R7 intent");
        let recorded = EpochReceipt {
            before: intent.epoch.clone(),
            after: Some(intent.epoch.clone()),
        };
        let mut changed = intent.epoch.clone();
        changed.git_head = Some("later-observation".to_string());
        let observed = EpochReceipt {
            before: intent.epoch.clone(),
            after: Some(changed),
        };
        let fence = Fence(1);
        let receipt = AttemptReceipt {
            intent: intent.clone(),
            fence,
            result: AttemptResult::Indeterminate {
                phase: None,
                evidence: None,
                detail: "effect boundary remained uncertain".to_string(),
            },
            evidence: None,
            epoch: Some(recorded),
        };
        let mut replay = Replay::default();
        replay
            .attempts
            .insert(intent.transition_id, Attempt::Finished(receipt));
        let cause = RecoveryCause::AttemptIndeterminate {
            session_id,
            fence,
            intent: intent.clone(),
            detail: "effect boundary remained uncertain".to_string(),
        };
        let resolution = RecoveryResolution::ResolveAttempt {
            transition_id: intent.transition_id,
            result: AttemptResult::Cancelled {
                phase: WalkPhase::R7,
                evidence: ContentHash::of("r7-evidence"),
                detail: "no committed R8 evidence found".to_string(),
            },
            evidence: None,
            epoch: Some(observed),
        };

        let error = validate_resolution_current(&replay, &cause, &resolution)
            .expect_err("recovery must preserve a concrete after epoch");
        assert!(error.to_string().contains("contradicted"));
    }

    #[test]
    fn current_graph_replay_rejects_self_declared_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let invalid = AttemptIntent {
            transition_id: TransitionId::new(),
            expected: WalkPhase::R0,
            targets: vec![WalkPhase::R14b],
            allow_live_api: false,
            allow_git_changes: false,
            epoch: owner.epoch().clone(),
            evidence: ContentHash::of("r0-evidence"),
            retry: 0,
        };
        append_entry(
            owner.paths().journal(),
            &Entry::Began {
                session_id: owner.session_id(),
                fence: owner.fence(),
                intent: invalid,
                recorded_at: RecordedAt::now(),
            },
            &owner.head,
        )
        .expect("stage impossible durable target");
        drop(owner);

        let recovery = store
            .claim(claim(temp.path(), RunMode::Step))
            .expect("replay impossible target");
        assert!(matches!(
            recovery,
            Outcome::Recoverable(RecoveryLease {
                cause: Some(RecoveryCause::Journal(Damage::Sequence { .. })),
                ..
            })
        ));
    }

    #[test]
    fn replay_rejects_began_cursor_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let mismatch = AttemptIntent::new(
            owner.session_id(),
            cursor(WalkPhase::R6, "mismatched-r6"),
            false,
            owner.epoch().clone(),
        )
        .expect("valid mismatched intent");
        append_entry(
            owner.paths().journal(),
            &Entry::Began {
                session_id: owner.session_id(),
                fence: owner.fence(),
                intent: mismatch,
                recorded_at: RecordedAt::now(),
            },
            &owner.head,
        )
        .expect("stage mismatched durable cursor");
        drop(owner);

        assert!(matches!(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("replay mismatched cursor"),
            Outcome::Recoverable(RecoveryLease {
                cause: Some(RecoveryCause::Journal(Damage::Sequence { .. })),
                ..
            })
        ));
    }

    #[test]
    fn empty_v1_session_is_read_only_and_cursor_unbound() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let request = claim(temp.path(), RunMode::Step);
        let session_id = SessionId::new();
        append_entry(
            store.paths(&request.parent).journal(),
            &Entry::Created {
                schema_version: SCHEMA_VERSION_V1.to_string(),
                session_id,
                origin: request.origin.clone(),
                parent: request.parent.clone(),
                profile: request.profile.clone(),
                mode: request.mode,
                cursor: None,
                recorded_at: RecordedAt::now(),
            },
            &JournalHead::empty(),
        )
        .expect("write empty v1 session");

        let snapshot = store
            .inspect(&request.parent)
            .expect("inspect v1 session")
            .expect("v1 snapshot");
        assert_eq!(snapshot.cursor, None);
        assert!(matches!(
            store.claim(request).expect("claim v1 session"),
            Outcome::Conflict(Conflict::Schema { active, .. })
                if active == SCHEMA_VERSION_V1
        ));
    }

    #[test]
    fn replay_requires_certificate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut lines = committed_lines(temp.path());
        edit_finished(&mut lines, |value| {
            value
                .as_object_mut()
                .expect("finished entry object")
                .remove("evidence");
        });

        assert!(matches!(
            Replay::from_lines(&lines),
            Err(Damage::Sequence { detail, .. })
                if detail.contains("missing its cursor evidence certificate")
        ));
    }

    #[test]
    fn replay_rejects_tampering() {
        let temp = tempfile::tempdir().expect("tempdir");
        let admitted = committed_lines(temp.path());
        for field in [
            "session_id",
            "parent",
            "profile",
            "graph_version",
            "prior",
            "intent",
            "edge",
            "witness",
            "epoch",
            "cursor_hash",
        ] {
            let mut lines = admitted.clone();
            edit_finished(&mut lines, |value| match field {
                "session_id" => {
                    value["evidence"]["session_id"] = serde_json::json!(Uuid::new_v4().to_string());
                }
                "parent" => {
                    value["evidence"]["parent"]["node_id"] = serde_json::json!("other-parent");
                }
                "profile" => {
                    value["evidence"]["profile"]["sha256"] = serde_json::json!("0".repeat(64));
                }
                "graph_version" => {
                    value["evidence"]["graph_version"] = serde_json::json!(GRAPH_VERSION_V1);
                }
                "prior" => {
                    value["evidence"]["prior"]["evidence"] = serde_json::json!("0".repeat(64));
                }
                "intent" => {
                    value["evidence"]["intent"]["allow_live_api"] = serde_json::json!(false);
                }
                "edge" => {
                    value["evidence"]["edge"] = serde_json::json!("r8_to_r9");
                }
                "witness" => {
                    value["evidence"]["witness"] = serde_json::json!("0".repeat(64));
                }
                "epoch" => {
                    value["evidence"]["epoch"]["after"]["protocol_version"] =
                        serde_json::json!(999);
                }
                "cursor_hash" => {
                    value["result"]["evidence"] = serde_json::json!("0".repeat(64));
                }
                _ => unreachable!("enumerated tamper field"),
            });

            assert!(
                matches!(Replay::from_lines(&lines), Err(Damage::Sequence { .. })),
                "tampered {field} must fail closed"
            );
        }
    }

    #[test]
    fn replay_rejects_noncommit_certificate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut lines = committed_lines(temp.path());
        let source = cursor(WalkPhase::R7, "r7-evidence");
        edit_finished(&mut lines, |value| {
            value["result"] = serde_json::json!({
                "status": "cancelled",
                "phase": "r7",
                "evidence": source.evidence,
                "detail": "cancelled before the effect boundary"
            });
        });

        assert!(matches!(
            Replay::from_lines(&lines),
            Err(Damage::Sequence { detail, .. })
                if detail.contains("non-committed result cannot contain")
        ));
    }

    #[test]
    fn committed_transition_id_replays_without_a_second_attempt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("first claim"),
        );
        let original = owner
            .intent_with_live_api(true, false)
            .expect("valid live transition intent");
        let transition_id = original.transition_id;
        let pending = match owner.begin(original.clone()).expect("begin") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("new transition should start"),
        };
        let first = pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R8,
                evidence: ContentHash::of("committed-r8"),
            })
            .expect("finish");
        let Finished::Terminal { lease, receipt, .. } = first else {
            panic!("committed attempt must restore idle authority");
        };
        let committed_cursor = receipt
            .evidence
            .as_ref()
            .expect("committed cursor certificate")
            .cursor()
            .expect("hash committed certificate");
        assert_eq!(lease.cursor(), &committed_cursor);
        lease.release().expect("first release");
        let snapshot = store
            .inspect(&parent())
            .expect("inspect committed cursor")
            .expect("session snapshot");
        assert_eq!(snapshot.cursor, Some(committed_cursor.clone()));

        let second = acquired(
            store
                .claim(claim_at(temp.path(), RunMode::Step, committed_cursor))
                .expect("second claim"),
        );
        let existing = second.begin(original).expect("idempotent begin");
        match existing {
            Begin::Existing { lease, receipt } => {
                assert_eq!(receipt.intent.transition_id, transition_id);
                assert_eq!(receipt.fence, Fence(1));
                assert!(matches!(
                    receipt.result,
                    AttemptResult::Committed {
                        phase: WalkPhase::R8,
                        ..
                    }
                ));
                lease.release().expect("second release");
            }
            Begin::Started { .. } => panic!("idempotent transition must not restart"),
        }
    }

    #[test]
    fn inspection_preserves_attempt_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let first = owner.intent_with_live_api(true, false).expect("R7 intent");
        let first_id = first.transition_id;
        let pending = match owner.begin(first).expect("begin R7") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("R7 attempt must start"),
        };
        let owner = match pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R8,
                evidence: ContentHash::of("ordered-r8"),
            })
            .expect("finish R7")
        {
            Finished::Terminal { lease, .. } => lease,
            Finished::Uncertain { .. } => panic!("R7 commit must be terminal"),
        };
        let second = owner.intent(false).expect("R8 intent");
        let second_id = second.transition_id;
        let pending = match owner.begin(second).expect("begin R8") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("R8 attempt must start"),
        };
        let owner = match pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R9,
                evidence: ContentHash::of("ordered-r9"),
            })
            .expect("finish R8")
        {
            Finished::Terminal { lease, .. } => lease,
            Finished::Uncertain { .. } => panic!("R8 commit must be terminal"),
        };
        owner.release().expect("release owner");

        let snapshot = store
            .inspect(&parent())
            .expect("inspect ordered attempts")
            .expect("session snapshot");
        let ids = snapshot
            .attempts
            .iter()
            .map(|attempt| match attempt {
                Attempt::Pending { intent, .. } => intent.transition_id,
                Attempt::Finished(receipt) => receipt.intent.transition_id,
                Attempt::Recovered { receipt, .. } => receipt.intent.transition_id,
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![first_id, second_id]);
        let receipt = snapshot
            .attempts
            .last()
            .and_then(Attempt::terminal)
            .expect("last ordered receipt");
        let committed_cursor = receipt
            .evidence
            .as_ref()
            .expect("ordered commit certificate")
            .cursor()
            .expect("hash ordered certificate");
        assert_eq!(committed_cursor.phase, WalkPhase::R9);
        assert_eq!(snapshot.cursor, Some(committed_cursor));
    }

    #[test]
    fn truncated_final_record_is_preserved_before_takeover() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let lease = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("first claim"),
        );
        let journal = lease.paths().journal().to_path_buf();
        lease.release().expect("release");

        let tail = b"{\"kind\":";
        let mut file = OpenOptions::new()
            .append(true)
            .open(&journal)
            .expect("open journal");
        file.write_all(tail).expect("write partial entry");
        file.sync_all().expect("sync partial entry");

        let recovery = match store
            .claim(claim(temp.path(), RunMode::Step))
            .expect("claim damaged journal")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        let hash = content_hash(tail);
        assert!(matches!(
            recovery.cause(),
            Some(RecoveryCause::Journal(Damage::Truncated { tail, .. })) if tail == &hash
        ));
        let repaired = recovery.repair_tail(&hash).expect("repair exact tail");
        assert_eq!(fs::read(&repaired.evidence).expect("tail evidence"), tail);
        let recovery = repaired.lease;
        assert!(recovery.cause().is_none());
        let next = recovery.take_over().expect("take over repaired journal");
        assert_eq!(next.fence().get(), 2);
        next.release().expect("release repaired owner");
    }

    #[test]
    fn tail_repair_rejects_prefix_rewrite() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let lease = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("first claim"),
        );
        let journal = lease.paths().journal().to_path_buf();
        lease.release().expect("release");

        let tail = b"{\"kind\":";
        let mut file = OpenOptions::new()
            .append(true)
            .open(&journal)
            .expect("open journal");
        file.write_all(tail).expect("write partial entry");
        file.sync_all().expect("sync partial entry");
        drop(file);

        let recovery = match store
            .claim(claim(temp.path(), RunMode::Step))
            .expect("claim damaged journal")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        let hash = content_hash(tail);
        let mut bytes = fs::read(&journal).expect("read journal for rewrite");
        bytes[0] = if bytes[0] == b'{' { b'[' } else { b'{' };
        fs::write(&journal, bytes).expect("rewrite admitted prefix");

        assert!(matches!(
            recovery.repair_tail(&hash),
            Err(Failure::Uncertain {
                source: Error::JournalPrefix { .. }
            })
        ));
    }

    #[test]
    fn ready_commit_requires_atomic_release() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let runtime = RuntimeId::new();
        let request = successor_claim(temp.path(), runtime, std::process::id());
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(store.claim(request).expect("claim successor session"));
        let (owner, _) = commit_edge(owner, WalkPhase::R4a, false, false);
        let (owner, receipt) = commit_edge(owner, WalkPhase::R4c, false, false);

        let active = store
            .inspect(&parent())
            .expect("inspect active R4c")
            .expect("successor session exists");
        let error = active
            .ready_commit(runtime)
            .expect_err("active R4c must not publish Ready");
        assert!(error.to_string().contains("no atomic"), "{error}");

        let session = owner.session_id();
        let fence = owner.fence();
        let committed = owner.cursor().clone();
        let ready = ready_receipt(&owner, runtime);
        owner
            .release_ready(ready.clone())
            .expect("atomically release Ready");
        let released = store
            .inspect(&parent())
            .expect("inspect released R4c")
            .expect("successor session exists");
        let commit = released
            .ready_commit(runtime)
            .expect("atomic release admits Ready");
        assert_eq!(commit.session_id(), session);
        assert_eq!(commit.transition_id(), receipt.intent.transition_id);
        assert_eq!(commit.fence(), fence);
        assert_eq!(commit.cursor(), &committed);
        assert_eq!(
            released
                .ready_epoch(runtime, &ready)
                .expect("recover exact Ready epoch"),
            receipt
                .epoch
                .as_ref()
                .and_then(|epoch| epoch.after.as_ref())
                .expect("terminal R4c epoch")
        );
        assert_eq!(
            released
                .ready_receipt(runtime)
                .expect("validate Ready")
                .expect("Ready is persisted"),
            &ready
        );
    }

    #[test]
    fn ordinary_r4c_release_is_not_ready() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let runtime = RuntimeId::new();
        let request = successor_claim(temp.path(), runtime, std::process::id());
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(store.claim(request).expect("claim successor session"));
        let (owner, _) = commit_edge(owner, WalkPhase::R4a, false, false);
        let (owner, _) = commit_edge(owner, WalkPhase::R4c, false, false);
        owner.release().expect("ordinary clean release");

        let snapshot = store
            .inspect(&parent())
            .expect("inspect ordinary release")
            .expect("successor session exists");
        assert!(
            snapshot
                .ready_receipt(runtime)
                .expect("inspect Ready")
                .is_none()
        );
        let error = snapshot
            .ready_commit(runtime)
            .expect_err("ordinary release cannot manufacture Ready");
        assert!(error.to_string().contains("no atomic"), "{error}");
    }

    #[test]
    fn recovery_cannot_substitute_for_ready_release() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let runtime = RuntimeId::new();
        let request = successor_claim(temp.path(), runtime, std::process::id());
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(request.clone())
                .expect("claim successor session"),
        );
        let (owner, _) = commit_edge(owner, WalkPhase::R4a, false, false);
        let (owner, _) = commit_edge(owner, WalkPhase::R4c, false, false);
        let cursor = owner.cursor().clone();
        abandon(owner);

        let mut reclaim = request.with_pid(4_343);
        reclaim.cursor = cursor;
        let recovery = match store.claim(reclaim).expect("recover abandoned R4c owner") {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        let recovery = recovery
            .resolve(RecoveryResolution::AbandonOwner)
            .expect("abandon lost owner");
        let abandoned = store
            .inspect(&parent())
            .expect("inspect abandoned R4c")
            .expect("successor session exists");
        let error = abandoned
            .ready_commit(runtime)
            .expect_err("abandonment is not a clean release");
        assert!(error.to_string().contains("no atomic"), "{error}");

        recovery
            .take_over()
            .expect("take over abandoned session")
            .release()
            .expect("release later fence");
        let later = store
            .inspect(&parent())
            .expect("inspect later release")
            .expect("successor session exists");
        let error = later
            .ready_commit(runtime)
            .expect_err("a later fence cannot release the R4c commit fence");
        assert!(error.to_string().contains("no atomic"), "{error}");
    }

    #[test]
    fn persisted_ready_revalidation_requires_its_exact_release() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let runtime = RuntimeId::new();
        let request = successor_claim(temp.path(), runtime, std::process::id());
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(request.clone())
                .expect("claim successor session"),
        );
        let (owner, _) = commit_edge(owner, WalkPhase::R4a, false, false);
        let (owner, receipt) = commit_edge(owner, WalkPhase::R4c, false, false);
        let persisted = ReadyCommit::new(
            owner.session_id(),
            receipt.intent.transition_id,
            receipt.fence,
            owner.cursor().clone(),
            RunMode::Continuous,
        )
        .expect("construct persisted Ready commit");
        let cursor = owner.cursor().clone();
        abandon(owner);

        let mut reclaim = request.with_pid(4_343);
        reclaim.cursor = cursor;
        let recovery = match store.claim(reclaim).expect("recover abandoned R4c owner") {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        recovery
            .resolve(RecoveryResolution::AbandonOwner)
            .expect("abandon R4c owner")
            .take_over()
            .expect("take over abandoned session")
            .release()
            .expect("release later fence");

        let snapshot = store
            .inspect(&parent())
            .expect("inspect later release")
            .expect("successor session exists");
        let error = snapshot
            .validate_ready_commit(runtime, &persisted)
            .expect_err("later release must not revalidate an unreleased R4c commit");
        assert!(error.to_string().contains("no atomic"), "{error}");
    }

    #[test]
    fn recovered_handoff_is_accepted_without_a_clean_release() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let request = claim_at(
            temp.path(),
            RunMode::Continuous,
            cursor(WalkPhase::R12, "handoff-r12"),
        )
        .with_pid(4_242);
        let owner = acquired(store.claim(request).expect("claim predecessor session"));
        let session_id = owner.session_id();
        let fence = owner.fence();
        let intent = owner
            .intent_with_live_api(true, true)
            .expect("R12 handoff intent");
        let attempt = PredecessorAttempt::new(
            session_id,
            intent.transition_id,
            fence,
            intent.allow_live_api,
            intent.allow_git_changes,
        );
        let acceptance = handoff_acceptance(attempt);
        let pending = match owner.begin(intent.clone()).expect("begin R12 handoff") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("fresh handoff must start"),
        };
        abandon(pending);

        let snapshot = store
            .inspect(&parent())
            .expect("inspect stranded handoff")
            .expect("predecessor session exists");
        let created = snapshot.created.expect("session creation authority");
        let next_epoch = epoch(temp.path());
        let request = Claim::from_recovery(
            &created,
            snapshot.cursor.expect("committed R12 cursor"),
            next_epoch.clone(),
        )
        .expect("construct recovery-only claim");
        let recovery = match store
            .claim_recovery(request)
            .expect("claim exact handoff recovery")
        {
            RecoveryOutcome::Required(recovery) => recovery,
            other => panic!("expected handoff recovery, got {other:?}"),
        };
        let permit = HandoffPermit::for_test(
            acceptance.clone(),
            ContentHash::of("recovered handoff witness"),
            EpochReceipt {
                before: intent.epoch,
                after: Some(next_epoch),
            },
        );
        let journal = store.paths(&parent()).journal;
        let before = fs::read_to_string(&journal)
            .expect("read pending recovery journal")
            .lines()
            .count();
        let prepared = recovery
            .prepare_handoff(permit)
            .expect("prepare exact recovered handoff");
        let during = fs::read_to_string(&journal)
            .expect("read prepared recovery journal")
            .lines()
            .count();
        assert_eq!(during, before, "preparation must not append recovery");
        prepared.commit().expect("accept exact recovered handoff");

        let snapshot = store
            .inspect(&parent())
            .expect("inspect recovered handoff")
            .expect("predecessor session exists");
        assert_eq!(
            snapshot.cursor.as_ref().map(|cursor| cursor.phase),
            Some(WalkPhase::R13b)
        );
        assert!(snapshot.active.is_none());
        assert!(!snapshot.releases.contains(&fence));
        assert!(snapshot.recoveries.contains(&fence));
        assert_eq!(snapshot.handoffs.get(&fence), Some(&acceptance));
        assert!(
            snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("validate recovered transfer")
        );

        let before = fs::read_to_string(&journal)
            .expect("read recovered journal")
            .lines()
            .count();
        let request = Claim::from_recovery(
            snapshot
                .created
                .as_ref()
                .expect("session creation authority"),
            snapshot.cursor.clone().expect("recovered R13b cursor"),
            epoch(temp.path()),
        )
        .expect("construct resolved recovery probe");
        assert!(matches!(
            store
                .claim_recovery(request)
                .expect("probe completed recovery"),
            RecoveryOutcome::Resolved
        ));
        let after = fs::read_to_string(journal)
            .expect("reread recovered journal")
            .lines()
            .count();
        assert_eq!(after, before, "resolved probe must not mint a fence");
    }

    #[test]
    fn committed_handoff_recovery_preserves_later_cursor() {
        for final_phase in [WalkPhase::R13b, WalkPhase::R14b] {
            let temp = tempfile::tempdir().expect("tempdir");
            let store = Store::new(temp.path().join("control"));
            let request = claim_at(
                temp.path(),
                RunMode::Continuous,
                cursor(WalkPhase::R12, "handoff-r12"),
            )
            .with_pid(4_242);
            let owner = acquired(store.claim(request).expect("claim predecessor session"));
            let session_id = owner.session_id();
            let fence = owner.fence();
            let intent = owner
                .intent_with_live_api(false, true)
                .expect("R12 handoff intent");
            let attempt = PredecessorAttempt::new(
                session_id,
                intent.transition_id,
                fence,
                intent.allow_live_api,
                intent.allow_git_changes,
            );
            let acceptance = handoff_acceptance(attempt);
            let witness = ContentHash::of("committed predecessor handoff");
            let before = intent.epoch.clone();
            let pending = match owner.begin(intent).expect("begin R12 handoff") {
                Begin::Started { lease, .. } => lease,
                Begin::Existing { .. } => panic!("fresh handoff must start"),
            };
            let finished = pending
                .finish(AttemptResult::Committed {
                    phase: WalkPhase::R13b,
                    evidence: witness.clone(),
                })
                .expect("commit R12 handoff");
            let Finished::Terminal {
                lease: mut owner, ..
            } = finished
            else {
                panic!("R12 handoff must finish terminally");
            };
            if final_phase == WalkPhase::R14b {
                let intent = owner
                    .intent_with_live_api(false, true)
                    .expect("R13b finalization intent");
                let pending = match owner.begin(intent).expect("begin R13b finalization") {
                    Begin::Started { lease, .. } => lease,
                    Begin::Existing { .. } => panic!("fresh finalization must start"),
                };
                let finished = pending
                    .finish(AttemptResult::Committed {
                        phase: WalkPhase::R14b,
                        evidence: ContentHash::of("committed predecessor finalization"),
                    })
                    .expect("commit R14b finalization");
                let Finished::Terminal { lease, .. } = finished else {
                    panic!("R13b finalization must finish terminally");
                };
                owner = lease;
            }
            drop(owner);

            let snapshot = store
                .inspect(&parent())
                .expect("inspect committed crash window")
                .expect("predecessor session exists");
            assert_eq!(
                snapshot.cursor.as_ref().map(|cursor| cursor.phase),
                Some(final_phase)
            );
            snapshot
                .committed_handoff(session_id, fence)
                .expect("exact committed handoff receipt");
            let request = Claim::from_recovery(
                snapshot
                    .created
                    .as_ref()
                    .expect("session creation authority"),
                snapshot.cursor.clone().expect("committed cursor"),
                epoch(temp.path()),
            )
            .expect("construct committed recovery claim");
            let recovery = match store
                .claim_recovery(request)
                .expect("claim committed handoff recovery")
            {
                RecoveryOutcome::Required(recovery) => recovery,
                other => panic!("expected committed handoff recovery, got {other:?}"),
            };
            assert!(matches!(
                recovery.cause(),
                Some(RecoveryCause::OwnerLost { .. })
            ));
            let permit = HandoffPermit::for_test(
                acceptance.clone(),
                witness,
                EpochReceipt {
                    before,
                    after: Some(epoch(temp.path())),
                },
            );
            recovery
                .prepare_handoff(permit)
                .expect("prepare committed handoff recovery")
                .commit()
                .expect("commit handoff recovery");

            let snapshot = store
                .inspect(&parent())
                .expect("inspect recovered committed handoff")
                .expect("predecessor session exists");
            assert_eq!(
                snapshot.cursor.as_ref().map(|cursor| cursor.phase),
                Some(final_phase)
            );
            assert!(snapshot.active.is_none());
            assert!(!snapshot.releases.contains(&fence));
            assert!(snapshot.recoveries.contains(&fence));
            assert!(
                snapshot
                    .accepted_handoff(&parent(), &acceptance)
                    .expect("validate recovered committed transfer")
            );
        }
    }

    #[test]
    fn accepted_handoff_requires_exact_released_attempt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let request = claim_at(
            temp.path(),
            RunMode::Continuous,
            cursor(WalkPhase::R12, "handoff-r12"),
        )
        .with_pid(4_242);
        let owner = acquired(
            store
                .claim(request.clone())
                .expect("claim predecessor session"),
        );
        let session = owner.session_id();
        let fence = owner.fence();
        let intent = owner
            .intent_with_live_api(true, true)
            .expect("R12 handoff intent");
        let attempt = PredecessorAttempt::new(
            session,
            intent.transition_id,
            fence,
            intent.allow_live_api,
            intent.allow_git_changes,
        );
        let acceptance = handoff_acceptance(attempt.clone());
        let pending = match owner.begin(intent).expect("begin R12 handoff") {
            Begin::Started { lease, .. } => lease,
            Begin::Existing { .. } => panic!("fresh handoff must start"),
        };
        let snapshot = store
            .inspect(&parent())
            .expect("inspect pending handoff")
            .expect("predecessor session exists");
        assert!(
            !snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("pending handoff is not accepted")
        );

        let finished = pending
            .finish(AttemptResult::Committed {
                phase: WalkPhase::R13b,
                evidence: ContentHash::of("commit predecessor handoff"),
            })
            .expect("commit R12 to R13b");
        let Finished::Terminal { lease: owner, .. } = finished else {
            panic!("unchanged epoch must commit terminally");
        };
        let cursor = owner.cursor().clone();
        let snapshot = store
            .inspect(&parent())
            .expect("inspect unreleased handoff")
            .expect("predecessor session exists");
        assert!(
            !snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("unreleased handoff is not accepted")
        );

        owner.release().expect("release exact handoff fence");
        let snapshot = store
            .inspect(&parent())
            .expect("inspect released handoff")
            .expect("predecessor session exists");
        let wrong = PredecessorAttempt::new(session, TransitionId::new(), fence, true, true);
        let wrong = handoff_acceptance(wrong);
        assert!(
            !snapshot
                .accepted_handoff(&parent(), &wrong)
                .expect("wrong attempt is not accepted")
        );
        assert!(
            snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("exact committed and released handoff is accepted")
        );

        let mut next = request.with_pid(4_343);
        next.cursor = cursor;
        let owner = acquired(store.claim(next.clone()).expect("claim later fence"));
        let snapshot = store
            .inspect(&parent())
            .expect("inspect later active fence")
            .expect("predecessor session exists");
        assert!(
            snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("later finalization owner does not revoke accepted handoff")
        );
        drop(owner);
        let recovery = match store
            .claim(next.with_pid(4_444))
            .expect("recover later fence")
        {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        let recovery = recovery
            .resolve(RecoveryResolution::AbandonOwner)
            .expect("abandon later fence");
        let snapshot = store
            .inspect(&parent())
            .expect("inspect recovered handoff")
            .expect("predecessor session exists");
        assert!(
            snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("later recovery does not revoke accepted handoff")
        );
        recovery
            .take_over()
            .expect("take over later recovery")
            .release()
            .expect("release recovery fence");
        let snapshot = store
            .inspect(&parent())
            .expect("inspect post-recovery release")
            .expect("predecessor session exists");
        assert!(
            snapshot
                .accepted_handoff(&parent(), &acceptance)
                .expect("accepted handoff remains monotonic")
        );
    }
}
