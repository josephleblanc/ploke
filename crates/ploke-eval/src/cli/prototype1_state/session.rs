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
    io::{self, Write},
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

const SCHEMA_VERSION: &str = "prototype1-control-session.v1";
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
        Ok(Self {
            origin: Origin::Admitted { setup },
            parent,
            profile: admitted.commitment.clone(),
            mode,
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
        epoch: ServerEpoch,
    ) -> Self {
        Self {
            origin: Origin::Historical { source },
            parent,
            profile,
            mode,
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
        validate_resolution(replay, &cause, &resolution)?;
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
                recorded_at: RecordedAt::now(),
            };
            if let Err(source) = append_entry(&self.paths.journal, &entry, next_index) {
                self.replay = Some(replay);
                return Err(failure(self, source));
            }
            replay.created = Some(Created {
                session_id,
                origin: self.request.origin.clone(),
                parent: self.request.parent.clone(),
                profile: self.request.profile.clone(),
                mode: self.request.mode,
            });
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
        let lock = self.lock.take().expect("recovery lease owns its lock");
        Ok(Lease {
            lock,
            paths: self.paths,
            session_id,
            parent: self.request.parent,
            mode: self.request.mode,
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
    /// Durably admit one exact typed edge and evidence revision.
    pub(crate) fn begin(self, intent: AttemptIntent) -> Result<Begin, AdmissionFailure> {
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
        if let Err(source) = intent.validate_current() {
            return Err(failure(self, source));
        }
        if let Some(attempt) = self.attempts.get(&intent.transition_id).cloned() {
            return match attempt {
                Attempt::Pending { .. } => Err(failure(
                    self,
                    Error::AttemptPending {
                        transition_id: intent.transition_id,
                    },
                )),
                Attempt::Finished(receipt) if receipt.intent == intent => Ok(Begin::Existing {
                    lease: self,
                    receipt,
                }),
                Attempt::Finished(_) => Err(failure(
                    self,
                    Error::AttemptMismatch {
                        transition_id: intent.transition_id,
                    },
                )),
            };
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
        let result = match self.epoch.ensure_not_stale_now() {
            Ok(()) => result,
            Err(source) => AttemptResult::Indeterminate {
                phase: None,
                evidence: None,
                detail: format!("controller epoch changed while the edge was running: {source}"),
            },
        };
        if let Err(source) = validate_result(&self.state.intent, &result) {
            return Err(failure(self, source));
        }
        let transition_id = self.state.intent.transition_id;
        let entry = Entry::Finished {
            session_id: self.session_id,
            transition_id,
            fence: self.fence,
            result: result.clone(),
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
        };
        let mut attempts = self.attempts;
        attempts.insert(transition_id, Attempt::Finished(receipt.clone()));
        let Lease {
            lock,
            paths,
            session_id,
            parent,
            mode,
            epoch,
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
                    epoch,
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
                    epoch,
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
    pub(crate) allow_git_changes: bool,
    pub(crate) epoch: ServerEpoch,
    pub(crate) evidence: ContentHash,
}

impl AttemptIntent {
    pub(crate) fn new(
        transition_id: TransitionId,
        expected: WalkPhase,
        allow_git_changes: bool,
        epoch: ServerEpoch,
        evidence: ContentHash,
    ) -> Result<Self, Error> {
        let targets = current_targets(expected, allow_git_changes);
        let intent = Self {
            transition_id,
            expected,
            targets,
            allow_git_changes,
            epoch,
            evidence,
        };
        intent.validate_current()?;
        Ok(intent)
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

#[derive(Debug, Clone)]
struct Created {
    session_id: SessionId,
    origin: Origin,
    parent: ParentIdentity,
    profile: RunProfileCommitment,
    mode: RunMode,
}

#[derive(Debug, Clone)]
struct Owner {
    session_id: SessionId,
    fence: Fence,
    epoch: ServerEpoch,
    runtime_id: Option<RuntimeId>,
    pid: u32,
}

#[derive(Debug, Clone)]
enum Attempt {
    Pending { fence: Fence, intent: AttemptIntent },
    Finished(AttemptReceipt),
}

#[derive(Debug, Clone, Default)]
struct Replay {
    created: Option<Created>,
    active: Option<Owner>,
    last_epoch: Option<ServerEpoch>,
    max_fence: u64,
    attempts: BTreeMap<TransitionId, Attempt>,
}

impl Replay {
    fn from_lines(lines: &[Line]) -> Result<Self, Damage> {
        let mut replay = Self::default();
        for line in lines {
            replay.apply(line)?;
        }
        Ok(replay)
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
                ..
            } => {
                if schema_version != SCHEMA_VERSION {
                    return Err(sequence(format!(
                        "unsupported session schema '{schema_version}'"
                    )));
                }
                if self.created.is_some() || line.number != 1 {
                    return Err(sequence(
                        "session creation must be the first and only created entry".to_string(),
                    ));
                }
                self.created = Some(Created {
                    session_id: *session_id,
                    origin: origin.clone(),
                    parent: parent.clone(),
                    profile: profile.clone(),
                    mode: *mode,
                });
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
            }
            Entry::Finished {
                session_id,
                transition_id,
                fence,
                result,
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
                validate_result(intent, result).map_err(|error| sequence(error.to_string()))?;
                let receipt = AttemptReceipt {
                    intent: intent.clone(),
                    fence: *fence,
                    result: result.clone(),
                };
                self.attempts
                    .insert(*transition_id, Attempt::Finished(receipt));
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
                validate_resolution(self, &cause, resolution)
                    .map_err(|error| sequence(error.to_string()))?;
                match resolution {
                    RecoveryResolution::AbandonOwner => {}
                    RecoveryResolution::ResolveAttempt {
                        transition_id,
                        result,
                    } => {
                        let intent = match self.attempts.get(transition_id) {
                            Some(Attempt::Pending { intent, .. }) => intent.clone(),
                            Some(Attempt::Finished(receipt)) => receipt.intent.clone(),
                            None => {
                                return Err(sequence(format!(
                                    "recovery transition {transition_id} was not recorded"
                                )));
                            }
                        };
                        self.attempts.insert(
                            *transition_id,
                            Attempt::Finished(AttemptReceipt {
                                intent,
                                fence: *fence,
                                result: result.clone(),
                            }),
                        );
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
    None
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

fn load_entries(path: &Path) -> Result<Load, Error> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(Load::Ready(Vec::new()));
        }
        Err(source) => {
            return Err(Error::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
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
    let byte_start = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(source) if source.kind() == io::ErrorKind::NotFound => 0,
        Err(source) => {
            return Err(Error::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
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
    let write_repair = || -> Result<(), Error> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&repair_path)
            .map_err(|source| Error::Open {
                path: repair_path.clone(),
                source,
            })?;
        file.write_all(&bytes[..valid_len])
            .and_then(|()| file.write_all(&payload))
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|source| Error::Write {
                path: repair_path.clone(),
                source,
            })?;
        file.sync_all().map_err(|source| Error::Sync {
            path: repair_path.clone(),
            source,
        })
    };
    if let Err(error) = write_repair() {
        let _ = fs::remove_file(&repair_path);
        return Err(error);
    }
    if let Err(error) = ensure_fresh(epoch) {
        let _ = fs::remove_file(&repair_path);
        return Err(error);
    }
    if let Err(source) = fs::rename(&repair_path, path) {
        let _ = fs::remove_file(&repair_path);
        return Err(Error::Write {
            path: path.to_path_buf(),
            source,
        });
    }
    sync_dir(path.parent().unwrap_or_else(|| Path::new("."))).map_err(|error| {
        Error::AppendUncertain {
            path: path.to_path_buf(),
            detail: error.to_string(),
        }
    })
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

#[cfg(not(unix))]
fn try_lock(_file: &File, path: &Path) -> Result<bool, Error> {
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

    fn claim(root: &Path, mode: RunMode) -> Claim {
        Claim::historical(
            ContentHash::of("session-test-origin"),
            parent(),
            profile(),
            mode,
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

    fn intent(root: &Path, transition_id: TransitionId) -> AttemptIntent {
        AttemptIntent::new(
            transition_id,
            WalkPhase::R7,
            false,
            epoch(root),
            ContentHash::of("r7-evidence"),
        )
        .expect("valid R7 to R8 intent")
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
    fn production_claim_requires_exact_admission() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (setup, admitted) = admitted_setup(temp.path());
        let valid = Claim::from_setup(
            setup.clone(),
            parent(),
            &admitted,
            RunMode::Continuous,
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
            epoch(temp.path()),
        )
        .expect_err("profile drift must fail");
        assert!(error.to_string().contains("profile hash"));

        let error = Claim::from_setup(
            setup.clone(),
            parent_in("other-campaign"),
            &admitted,
            RunMode::Continuous,
            epoch(temp.path()),
        )
        .expect_err("campaign drift must fail");
        assert!(error.to_string().contains("does not match parent campaign"));

        let error = Claim::from_setup(
            setup.clone(),
            parent(),
            &admitted,
            RunMode::Step,
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
    fn prior_graph_replays_before_epoch_admission() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let requested = claim(temp.path(), RunMode::Continuous);
        let next_epoch = requested.epoch.clone();
        let mut prior = next_epoch.clone();
        prior.transition_graph_version = GRAPH_VERSION_V1.to_string();
        let session_id = SessionId::new();
        let fence = Fence(1);
        let transition_id = TransitionId::new();
        let intent = AttemptIntent {
            transition_id,
            expected: WalkPhase::R12,
            targets: vec![WalkPhase::R13a, WalkPhase::R13b],
            allow_git_changes: true,
            epoch: prior.clone(),
            evidence: ContentHash::of("legacy-r12-evidence"),
        };
        let entries = [
            Entry::Created {
                schema_version: SCHEMA_VERSION.to_string(),
                session_id,
                origin: requested.origin.clone(),
                parent: requested.parent.clone(),
                profile: requested.profile.clone(),
                mode: requested.mode,
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
                    evidence: ContentHash::of("legacy-r13b-evidence"),
                },
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

        let mut recovery = match store.claim(requested).expect("recovery claim") {
            Outcome::Recoverable(recovery) => recovery,
            other => panic!("expected recovery lease, got {other:?}"),
        };
        assert!(matches!(
            recovery.cause(),
            Some(RecoveryCause::EpochChanged {
                prior,
                requested,
                ..
            }) if prior.transition_graph_version == GRAPH_VERSION_V1 && requested == &next_epoch
        ));
        recovery = recovery
            .resolve(RecoveryResolution::AdmitEpoch {
                prior,
                next: next_epoch.clone(),
            })
            .expect("admit replacement epoch");
        assert!(recovery.cause().is_none());
        let next = recovery.take_over().expect("take over admitted epoch");
        assert_eq!(next.epoch(), &next_epoch);
        assert_eq!(next.fence().get(), 2);
        next.release().expect("release replacement owner");
    }

    #[test]
    fn pending_attempt_requires_explicit_cancelled_reconciliation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let transition_id = TransitionId::new();
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Continuous))
                .expect("owner claim"),
        );
        let pending = match owner
            .begin(intent(temp.path(), transition_id))
            .expect("begin")
        {
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
        let transition_id = TransitionId::new();
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let pending = match owner
            .begin(intent(temp.path(), transition_id))
            .expect("begin")
        {
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
        let Finished::Uncertain { lease, .. } = finished else {
            panic!("indeterminate result must not restore idle authority");
        };
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
            })
            .expect("reconcile indeterminate result");
        recovery
            .take_over()
            .expect("take over reconciled session")
            .release()
            .expect("release reconciled owner");
    }

    #[test]
    fn branch_intent_records_observed_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("owner claim"),
        );
        let intent = AttemptIntent::new(
            TransitionId::new(),
            WalkPhase::R1,
            false,
            epoch(temp.path()),
            ContentHash::of("r1-evidence"),
        )
        .expect("valid branch intent");
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
        lease.release().expect("release branch owner");
    }

    #[test]
    fn r12_intent_preserves_git_changes_authority() {
        let temp = tempfile::tempdir().expect("tempdir");
        let blocked = AttemptIntent::new(
            TransitionId::new(),
            WalkPhase::R12,
            false,
            epoch(temp.path()),
            ContentHash::of("r12-evidence"),
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
            TransitionId::new(),
            WalkPhase::R12,
            true,
            epoch(temp.path()),
            ContentHash::of("r12-evidence"),
        )
        .expect("explicit checkout authority admits all typed outcomes");
        assert_eq!(
            admitted.targets,
            vec![WalkPhase::R13a, WalkPhase::R13b, WalkPhase::R13c]
        );
        assert!(admitted.allow_git_changes);
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
            allow_git_changes: false,
            epoch: owner.epoch().clone(),
            evidence: ContentHash::of("r0-evidence"),
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
    fn committed_transition_id_replays_without_a_second_attempt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Store::new(temp.path().join("control"));
        let transition_id = TransitionId::new();
        let owner = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("first claim"),
        );
        let pending = match owner
            .begin(intent(temp.path(), transition_id))
            .expect("begin")
        {
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
        lease.release().expect("first release");

        let second = acquired(
            store
                .claim(claim(temp.path(), RunMode::Step))
                .expect("second claim"),
        );
        let existing = second
            .begin(intent(temp.path(), transition_id))
            .expect("idempotent begin");
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
