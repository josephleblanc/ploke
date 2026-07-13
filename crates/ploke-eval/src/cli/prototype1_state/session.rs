//! Durable, process-exclusive controller sessions for Prototype 1 parents.
//!
//! The kernel lock is the live authority primitive. The append-only journal is
//! durable evidence about session creation, fences, transition attempts, clean
//! release, and explicit recovery. A journal record never substitutes for the
//! held file descriptor.

use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
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
    event::{ContentHash, RecordedAt, RuntimeId, TransitionId},
    identity::ParentIdentity,
    journal::JournalAppendReceipt,
    profile::{AdmittedRunProfile, RunMode, RunProfileCommitment},
    setup_admission::Prototype1SetupAdmission,
    walk::{
        epoch::{ServerEpoch, TRANSITION_GRAPH_VERSION},
        phase::WalkPhase,
    },
};

const SCHEMA_VERSION_V1: &str = "prototype1-control-session.v1";
const SCHEMA_VERSION_V2: &str = "prototype1-control-session.v2";
const SCHEMA_VERSION: &str = "prototype1-control-session.v3";
const TRANSITION_KEY_VERSION_V1: &str = "prototype1-control-transition-key.v1";
const TRANSITION_KEY_VERSION: &str = "prototype1-control-transition-key.v2";
const LOCK_FILE: &str = "controller.lock";
const JOURNAL_FILE: &str = "control-journal.jsonl";
const GRAPH_VERSION_V1: &str = "walk-r0-r14a-v1";

/// Durable identity for one parent-scoped controller session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SessionId(Uuid);

impl SessionId {
    fn new() -> Self {
        Self(Uuid::new_v4())
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
pub(crate) struct Cursor {
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
}

/// Monotonic token issued whenever a controller takes mutation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Fence(u64);

impl Fence {
    pub(crate) fn get(self) -> u64 {
        self.0
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
/// Production claims can only construct `Admitted`. `Historical` exists so a
/// real pre-Stage-1 incident can exercise this production store in regression
/// tests without inventing a setup receipt that never existed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
enum Origin {
    Admitted { setup: Prototype1SetupAdmission },
    Historical { source: ContentHash },
}

impl Origin {
    fn label(&self) -> &str {
        match self {
            Self::Admitted { setup } => &setup.intent.plan_hash.0,
            Self::Historical { source } => &source.0,
        }
    }
}

/// Request to claim mutation authority for one parent session.
#[derive(Debug, Clone)]
pub(crate) struct Claim {
    origin: Origin,
    parent: ParentIdentity,
    profile: RunProfileCommitment,
    mode: RunMode,
    cursor: Cursor,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    pid: u32,
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
        Ok(Self {
            origin: Origin::Admitted { setup },
            parent,
            profile: admitted.commitment.clone(),
            mode,
            cursor,
            epoch,
            runtime_id: None,
            pid: std::process::id(),
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
        Self {
            origin: Origin::Historical { source },
            parent,
            profile,
            mode,
            cursor,
            epoch,
            runtime_id: None,
            pid: std::process::id(),
        }
    }

    pub(crate) fn with_runtime(mut self, runtime_id: RuntimeId) -> Self {
        self.runtime_id = Some(runtime_id);
        self
    }

    #[cfg(test)]
    fn with_pid(mut self, pid: u32) -> Self {
        self.pid = pid;
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
    next_index: usize,
}

impl RecoveryLease {
    pub(crate) fn paths(&self) -> &Paths {
        &self.paths
    }

    pub(crate) fn cause(&self) -> Option<&RecoveryCause> {
        self.cause.as_ref()
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
            tail.valid_len,
            &entry,
            &self.request.epoch,
        )?;

        let Load::Ready(lines) = load_entries(&self.paths.journal)? else {
            return Err(Error::RepairUnsupported {
                detail: "journal remained damaged after exact tail repair".to_string(),
            });
        };
        let replay = Replay::from_lines(&lines).map_err(Error::Replay)?;
        self.next_index = lines.len();
        self.cause = replay
            .recovery()
            .or_else(|| replay.epoch_change(&self.request.epoch));
        self.replay = Some(replay);
        self.damage = None;
        Ok(evidence_path)
    }

    /// Resolve the old fence while retaining exclusive recovery authority.
    pub(crate) fn resolve(
        mut self,
        resolution: RecoveryResolution,
    ) -> Result<Self, RecoveryFailure> {
        match self.resolve_inner(resolution) {
            Ok(()) => Ok(self),
            Err(source) => Err(failure(self, source)),
        }
    }

    fn resolve_inner(&mut self, resolution: RecoveryResolution) -> Result<(), Error> {
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
            number: self.next_index + 1,
            entry: entry.clone(),
        };
        let mut next = replay.clone();
        next.apply(&line).map_err(Error::Replay)?;
        append_entry(&self.paths.journal, &entry, self.next_index)?;
        *replay = next;
        self.next_index += 1;
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
        let mut next_index = self.next_index;
        let session_id = if let Some(created) = replay.created.as_ref() {
            created.session_id
        } else {
            let session_id = SessionId::new();
            let entry = Entry::Created {
                schema_version: SCHEMA_VERSION.to_string(),
                session_id,
                origin: self.request.origin.clone(),
                parent: self.request.parent.clone(),
                profile: self.request.profile.clone(),
                mode: self.request.mode,
                cursor: Some(self.request.cursor.clone()),
                recorded_at: RecordedAt::now(),
            };
            if let Err(source) = append_entry(&self.paths.journal, &entry, next_index) {
                self.replay = Some(replay);
                return Err(failure(self, source));
            }
            replay.created = Some(Created {
                schema_version: SCHEMA_VERSION.to_string(),
                session_id,
                origin: self.request.origin.clone(),
                parent: self.request.parent.clone(),
                profile: self.request.profile.clone(),
                mode: self.request.mode,
            });
            replay.cursor = Some(self.request.cursor.clone());
            next_index += 1;
            self.next_index = next_index;
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
            recorded_at: RecordedAt::now(),
        };
        if let Err(source) = ensure_fresh(&self.request.epoch) {
            self.replay = Some(replay);
            return Err(failure(self, source));
        }
        if let Err(source) = append_entry(&self.paths.journal, &entry, next_index) {
            self.replay = Some(replay);
            return Err(failure(self, source));
        }
        next_index += 1;
        let cursor = replay
            .cursor
            .clone()
            .expect("mutable controller session has a committed cursor");
        let lock = self.lock.take().expect("recovery lease owns its lock");
        Ok(Lease {
            lock,
            paths: self.paths,
            session_id,
            parent: self.request.parent,
            mode: self.request.mode,
            cursor,
            epoch: self.request.epoch,
            runtime_id: self.request.runtime_id,
            fence,
            next_index,
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
    ResolveAttempt {
        transition_id: TransitionId,
        result: AttemptResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epoch: Option<EpochReceipt>,
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
    pub(crate) attempts: Vec<Attempt>,
    pub(crate) damage: Option<Damage>,
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
            Load::Ready(lines) => (lines, None),
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
            attempts,
            damage,
        }))
    }

    /// Try to claim the stable lock inode without waiting.
    pub(crate) fn claim(&self, request: Claim) -> Result<Outcome, Error> {
        let paths = self.paths(&request.parent);
        fs::create_dir_all(&paths.root).map_err(|source| Error::CreateDir {
            path: paths.root.clone(),
            source,
        })?;
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
            return Ok(Outcome::Conflict(Conflict::Locked {
                path: paths.lock.clone(),
            }));
        }

        let (lines, damage, cause) = match load_entries(&paths.journal)? {
            Load::Ready(lines) => (lines, None, None),
            Load::Damaged { lines, cause, tail } => {
                (lines, Some(tail), Some(RecoveryCause::Journal(cause)))
            }
        };
        let replay = match Replay::from_lines(&lines) {
            Ok(replay) => Some(replay),
            Err(cause) => {
                return Ok(Outcome::Recoverable(RecoveryLease {
                    lock: Some(lock),
                    paths,
                    request,
                    replay: None,
                    cause: Some(RecoveryCause::Journal(cause)),
                    damage,
                    next_index: lines.len(),
                }));
            }
        };
        if let Some(conflict) = binding_conflict(replay.as_ref().expect("replay"), &request) {
            return Ok(Outcome::Conflict(conflict));
        }
        if let Some(cause) = cause
            .or_else(|| replay.as_ref().and_then(Replay::recovery))
            .or_else(|| {
                replay
                    .as_ref()
                    .and_then(|replay| replay.epoch_change(&request.epoch))
            })
        {
            return Ok(Outcome::Recoverable(RecoveryLease {
                lock: Some(lock),
                paths,
                request,
                replay,
                cause: Some(cause),
                damage,
                next_index: lines.len(),
            }));
        }

        acquire_lease(
            lock,
            paths,
            request,
            replay.unwrap_or_default(),
            lines.len(),
        )
        .map(Outcome::Acquired)
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
    mode: RunMode,
    cursor: Cursor,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    fence: Fence,
    next_index: usize,
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

    pub(crate) fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) fn paths(&self) -> &Paths {
        &self.paths
    }
}

impl Lease<Idle> {
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

    /// Durably admit one exact typed edge and evidence revision.
    pub(crate) fn begin(self, intent: AttemptIntent) -> Result<Begin, AdmissionFailure> {
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
        let append = match append_entry(&self.paths.journal, &entry, self.next_index) {
            Ok(append) => append,
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
                mode: self.mode,
                cursor: self.cursor,
                epoch: self.epoch,
                runtime_id: self.runtime_id,
                fence: self.fence,
                next_index: self.next_index + 1,
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
            recorded_at: RecordedAt::now(),
        };
        match append_entry(&self.paths.journal, &entry, self.next_index) {
            Ok(receipt) => Ok(receipt),
            Err(source) => Err(failure(self, source)),
        }
    }
}

impl Lease<Pending> {
    pub(crate) fn intent(&self) -> &AttemptIntent {
        &self.state.intent
    }

    /// Finish the one attempt admitted under this lease and fence.
    pub(crate) fn finish(self, result: AttemptResult) -> Result<Finished, FinishFailure> {
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
            epoch: Some(epoch_receipt.clone()),
            recorded_at: RecordedAt::now(),
        };
        let append = match append_entry(&self.paths.journal, &entry, self.next_index) {
            Ok(append) => append,
            Err(source) => {
                return Err(failure(self, source));
            }
        };
        let uncertain = matches!(result, AttemptResult::Indeterminate { .. });
        let receipt = AttemptReceipt {
            intent: self.state.intent,
            fence: self.fence,
            result,
            epoch: Some(epoch_receipt),
        };
        let mut attempts = self.attempts;
        attempts.insert(transition_id, Attempt::Finished(receipt.clone()));
        let Lease {
            lock,
            paths,
            session_id,
            parent,
            mode,
            runtime_id,
            fence,
            next_index,
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
                    mode,
                    cursor: next_cursor,
                    epoch: next_epoch,
                    runtime_id,
                    fence,
                    next_index: next_index + 1,
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
                    mode,
                    cursor: next_cursor,
                    epoch: next_epoch,
                    runtime_id,
                    fence,
                    next_index: next_index + 1,
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
    expected
        .next_steps()
        .iter()
        .filter(|step| {
            allow_git_changes || !matches!(step.phase, WalkPhase::R13b | WalkPhase::R13c)
        })
        .map(|step| step.phase)
        .collect()
}

fn target_requires_live_api(phase: WalkPhase) -> bool {
    matches!(
        phase,
        WalkPhase::R8 | WalkPhase::R11a | WalkPhase::R11 | WalkPhase::R13b | WalkPhase::R13c
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
    if version == TRANSITION_GRAPH_VERSION {
        return Some(current_targets(expected, allow_git_changes));
    }
    if version == GRAPH_VERSION_V1 {
        return v1_targets(expected, allow_git_changes);
    }
    None
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
    if matches!(source, Error::AppendUncertain { .. }) {
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
        recorded_at: RecordedAt,
    },
}

#[derive(Debug, Clone)]
struct Line {
    number: usize,
    entry: Entry,
}

enum Load {
    Ready(Vec<Line>),
    Damaged {
        lines: Vec<Line>,
        cause: Damage,
        tail: Tail,
    },
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner {
    session_id: SessionId,
    fence: Fence,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    pid: u32,
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
    last_epoch: Option<ServerEpoch>,
    max_fence: u64,
    attempts: BTreeMap<TransitionId, Attempt>,
    attempt_order: Vec<TransitionId>,
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
                    (SCHEMA_VERSION | SCHEMA_VERSION_V2, Some(cursor)) => cursor
                        .validate()
                        .map_err(|error| sequence(error.to_string()))?,
                    (SCHEMA_VERSION | SCHEMA_VERSION_V2, None) => {
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
                self.created = Some(Created {
                    schema_version: schema_version.clone(),
                    session_id: *session_id,
                    origin: origin.clone(),
                    parent: parent.clone(),
                    profile: profile.clone(),
                    mode: *mode,
                });
                self.cursor = cursor.clone();
            }
            Entry::Acquired {
                session_id,
                fence,
                epoch,
                runtime_id,
                pid,
                ..
            } => {
                self.require_session(*session_id, line.number)?;
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
                self.active = Some(Owner {
                    session_id: *session_id,
                    fence: *fence,
                    epoch: epoch.clone(),
                    runtime_id: *runtime_id,
                    pid: *pid,
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
                if created.schema_version == SCHEMA_VERSION {
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
                if created.schema_version == SCHEMA_VERSION {
                    validate_result_current(&intent, result)
                        .map_err(|error| sequence(error.to_string()))?;
                } else {
                    validate_result(&intent, result)
                        .map_err(|error| sequence(error.to_string()))?;
                }
                match (created.schema_version.as_str(), epoch) {
                    (SCHEMA_VERSION | SCHEMA_VERSION_V2, Some(epoch)) => {
                        validate_epoch(&intent, result, epoch)
                            .map_err(|error| sequence(error.to_string()))?;
                    }
                    (SCHEMA_VERSION | SCHEMA_VERSION_V2, None) => {
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
                if created.schema_version == SCHEMA_VERSION {
                    validate_resolution_current(self, &cause, resolution)
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
                    RecoveryResolution::ResolveAttempt {
                        transition_id,
                        result,
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
                    RecoveryResolution::AdmitEpoch { .. } => {
                        return Err(sequence(
                            "epoch admission used an owner-recovery journal entry".to_string(),
                        ));
                    }
                }
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
                session_id, fence, ..
            } => {
                self.require_owner(*session_id, *fence, line.number)?;
                if self.unresolved_for(*fence).is_some() {
                    return Err(sequence(
                        "lease released with an unresolved transition attempt".to_string(),
                    ));
                }
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
    replay: Replay,
    mut next_index: usize,
) -> Result<Lease<Idle>, Error> {
    ensure_fresh(&request.epoch)?;
    let session_id = if let Some(created) = replay.created.as_ref() {
        created.session_id
    } else {
        let session_id = SessionId::new();
        let entry = Entry::Created {
            schema_version: SCHEMA_VERSION.to_string(),
            session_id,
            origin: request.origin.clone(),
            parent: request.parent.clone(),
            profile: request.profile.clone(),
            mode: request.mode,
            cursor: Some(request.cursor.clone()),
            recorded_at: RecordedAt::now(),
        };
        append_entry(&paths.journal, &entry, next_index)?;
        next_index += 1;
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
        recorded_at: RecordedAt::now(),
    };
    ensure_fresh(&request.epoch)?;
    append_entry(&paths.journal, &entry, next_index)?;
    next_index += 1;
    Ok(Lease {
        lock,
        paths,
        session_id,
        parent: request.parent,
        mode: request.mode,
        cursor: replay.cursor.unwrap_or(request.cursor),
        epoch: request.epoch,
        runtime_id: request.runtime_id,
        fence,
        next_index,
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

fn binding_conflict(replay: &Replay, request: &Claim) -> Option<Conflict> {
    let created = replay.created.as_ref()?;
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

fn validate_resolution(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    match (cause, resolution) {
        (RecoveryCause::OwnerLost { .. }, RecoveryResolution::AbandonOwner) => Ok(()),
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

fn validate_resolution_current(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    validate_resolution_epoch(replay, cause, resolution, true)
}

fn validate_resolution_v2(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
) -> Result<(), Error> {
    validate_resolution_epoch(replay, cause, resolution, false)
}

fn validate_resolution_epoch(
    replay: &Replay,
    cause: &RecoveryCause,
    resolution: &RecoveryResolution,
    require_live_api: bool,
) -> Result<(), Error> {
    validate_resolution(replay, cause, resolution)?;
    if let RecoveryResolution::ResolveAttempt {
        transition_id,
        result,
        epoch,
    } = resolution
    {
        let Some(attempt) = replay.attempts.get(transition_id) else {
            return Ok(());
        };
        let intent = attempt.intent();
        if require_live_api {
            validate_result_current(intent, result)?;
        } else {
            validate_result(intent, result)?;
        }
        let epoch = epoch.as_ref().ok_or_else(|| Error::InvalidResolution {
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
            return Ok(Load::Ready(Vec::new()));
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
        return Ok(Load::Ready(Vec::new()));
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
    Ok(Load::Ready(lines))
}

fn append_entry(
    path: &Path,
    entry: &Entry,
    source_event_index: usize,
) -> Result<JournalAppendReceipt, Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| Error::CreateDir {
        path: parent.to_path_buf(),
        source,
    })?;
    let existed = path.exists();
    let payload_json = serde_json::to_string(entry).map_err(Error::Serialize)?;
    let byte_len = payload_json.len();
    let content_sha256 = sha256_hex(payload_json.as_bytes());
    let mut line = payload_json.as_bytes().to_vec();
    line.push(b'\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| Error::Open {
            path: path.to_path_buf(),
            source,
        })?;
    lock_exclusive(&file, path)?;
    let byte_start = file
        .metadata()
        .map_err(|source| Error::Read {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    file.write_all(&line)
        .map_err(|source| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?;
    file.sync_all().map_err(|source| Error::AppendUncertain {
        path: path.to_path_buf(),
        detail: source.to_string(),
    })?;
    if !existed {
        sync_dir(parent).map_err(|source| Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?;
    }

    Ok(JournalAppendReceipt {
        path: path.to_path_buf(),
        source_event_index,
        source_line: source_event_index + 1,
        byte_start,
        byte_len,
        content_sha256,
        payload_json,
    })
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
    valid_len: u64,
    marker: &Entry,
    epoch: &ServerEpoch,
) -> Result<(), Error> {
    let bytes = fs::read(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let valid_len = usize::try_from(valid_len).map_err(|_| Error::RepairUnsupported {
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
    let payload = serde_json::to_vec(marker).map_err(Error::Serialize)?;
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
    if let Err(source) = repair
        .write_all(&bytes[..valid_len])
        .and_then(|()| repair.write_all(&payload))
        .and_then(|()| repair.write_all(b"\n"))
    {
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
    let published = sync_dir(path.parent().unwrap_or_else(|| Path::new("."))).map_err(|error| {
        Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: error.to_string(),
        }
    });
    drop(repair);
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
        sync::mpsc,
        time::Duration,
    };

    use std::os::unix::fs::MetadataExt;

    use ploke_records::{identity::ParentIdentityRecord, ids::CampaignId};

    use crate::{
        cli::prototype1_state::{
            backend::GitCommit,
            identity, invocation, profile,
            setup_admission::{
                SetupAdmissionIntent, SetupAdmissionState, SetupArtifactHashes, SetupCheckoutBase,
            },
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
            .complete(GitCommit("f".repeat(40)), RecordedAt(1_784_000_060_000))
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

    fn acquired(outcome: Outcome) -> Lease<Idle> {
        match outcome {
            Outcome::Acquired(lease) => lease,
            other => panic!("expected acquired lease, got {other:?}"),
        }
    }

    fn intent(lease: &Lease<Idle>) -> AttemptIntent {
        lease.intent(false).expect("valid transition intent")
    }

    fn same_epoch(epoch: &ServerEpoch) -> Option<EpochReceipt> {
        Some(EpochReceipt {
            before: epoch.clone(),
            after: Some(epoch.clone()),
        })
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
    fn historical_collision_blocks_second_driver() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let prototype_root = temp.path().join("campaign/prototype1");
        let invocation_path = prototype_root.join("successor-invocation.json");
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

        let mut source_bytes = identity_bytes;
        source_bytes.extend(profile_bytes);
        source_bytes.extend(invocation_bytes);
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
        let (setup, admitted) = admitted_setup(temp.path());
        let valid = Claim::from_setup(
            setup.clone(),
            parent(),
            &admitted,
            RunMode::Continuous,
            cursor(WalkPhase::R0, "setup-r0"),
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
                        parent(),
                        &admitted,
                        RunMode::Continuous,
                        cursor(WalkPhase::R0, "setup-r0"),
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
            parent(),
            &admitted,
            RunMode::Continuous,
            cursor(WalkPhase::R0, "setup-r0"),
            epoch(temp.path()),
        )
        .expect_err("incomplete setup must fail");
        assert!(error.to_string().contains("completed setup admission"));

        let mut drifted = admitted.clone();
        drifted.commitment.sha256 = "0".repeat(64);
        let error = Claim::from_setup(
            setup.clone(),
            parent(),
            &drifted,
            RunMode::Continuous,
            cursor(WalkPhase::R0, "setup-r0"),
            epoch(temp.path()),
        )
        .expect_err("profile drift must fail");
        assert!(error.to_string().contains("profile hash"));

        let error = Claim::from_setup(
            setup.clone(),
            parent_in("other-campaign"),
            &admitted,
            RunMode::Continuous,
            cursor(WalkPhase::R0, "setup-r0"),
            epoch(temp.path()),
        )
        .expect_err("campaign drift must fail");
        assert!(error.to_string().contains("does not match parent campaign"));

        let error = Claim::from_setup(
            setup.clone(),
            parent(),
            &admitted,
            RunMode::Step,
            cursor(WalkPhase::R0, "setup-r0"),
            epoch(temp.path()),
        )
        .expect_err("mode drift must fail");
        assert!(error.to_string().contains("profile mode"));

        let other = tempfile::tempdir().expect("other tempdir");
        let error = Claim::from_setup(
            setup,
            parent(),
            &admitted,
            RunMode::Continuous,
            cursor(WalkPhase::R0, "setup-r0"),
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
        drop(owner);

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
        prior.transition_graph_version = GRAPH_VERSION_V1.to_string();
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
                epoch: None,
                recorded_at: RecordedAt::now(),
            },
            Entry::Released {
                session_id,
                fence,
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&requested.parent).journal;
        for (index, entry) in entries.iter().enumerate() {
            append_entry(&journal, entry, index).expect("write legacy journal");
        }

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
        let intent = AttemptIntent::new(session_id, source.clone(), false, request.epoch.clone())
            .expect("legacy v2 intent");
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
            before: request.epoch.clone(),
            after: Some(request.epoch.clone()),
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
                epoch: request.epoch.clone(),
                runtime_id: None,
                pid: 4242,
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
                epoch: Some(epoch),
                recorded_at: RecordedAt::now(),
            },
            Entry::Released {
                session_id,
                fence,
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&request.parent).journal;
        for (index, entry) in entries.iter().enumerate() {
            append_entry(&journal, entry, index).expect("write v2 journal");
        }

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
        drop(pending);

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
        drop(lease);

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
        let intent = AttemptIntent::new(
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
                epoch: Some(missing.clone()),
                recorded_at: RecordedAt::now(),
            },
        ];
        let journal = store.paths(&request.parent).journal;
        for (index, entry) in entries.iter().enumerate() {
            append_entry(&journal, entry, index).expect("write incomplete epoch journal");
        }

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
        let retry = lease.intent(false).expect("retry rejected edge");
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
        let retry = lease.intent(false).expect("retry cancelled edge");
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
        drop(pending);

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
                epoch,
            })
            .expect("resolve committed attempt");
        let owner = recovery.take_over().expect("take over recovered session");
        assert_eq!(owner.cursor(), &cursor(WalkPhase::R8, "recovered-r8"));
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
        let source = cursor(WalkPhase::R7, "r7-evidence");
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

        let admitted = AttemptIntent::with_live_api(
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
        assert!(admitted.allow_live_api);
        assert!(admitted.allow_git_changes);
    }

    #[test]
    fn live_target_rejects_an_unadmitted_result() {
        let temp = tempfile::tempdir().expect("tempdir");
        let blocked = AttemptIntent::new(
            SessionId::new(),
            cursor(WalkPhase::R7, "r7-evidence"),
            false,
            epoch(temp.path()),
        )
        .expect("non-live intent remains inspectable");
        let result = AttemptResult::Committed {
            phase: WalkPhase::R8,
            evidence: ContentHash::of("r8-evidence"),
        };
        let error = validate_result_current(&blocked, &result)
            .expect_err("R8 must require live-provider admission");
        assert!(error.to_string().contains("live-provider admission"));

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
        let intent = AttemptIntent::new(
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
            2,
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
            2,
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
            0,
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
        let Finished::Terminal { lease, .. } = first else {
            panic!("committed attempt must restore idle authority");
        };
        assert_eq!(lease.cursor(), &cursor(WalkPhase::R8, "committed-r8"));
        lease.release().expect("first release");
        let snapshot = store
            .inspect(&parent())
            .expect("inspect committed cursor")
            .expect("session snapshot");
        assert_eq!(snapshot.cursor, Some(cursor(WalkPhase::R8, "committed-r8")));

        let second = acquired(
            store
                .claim(claim_at(
                    temp.path(),
                    RunMode::Step,
                    cursor(WalkPhase::R8, "committed-r8"),
                ))
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
        assert_eq!(snapshot.cursor, Some(cursor(WalkPhase::R9, "ordered-r9")));
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
}
