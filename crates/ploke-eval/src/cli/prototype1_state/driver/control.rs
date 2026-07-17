//! One owned Prototype 1 parent state and one meaning of a forward step.
//!
//! The controller session admits one exact attempt, this module reconstructs
//! its typed source state, dispatches one direct edge, and the same session
//! records the result. Frontends never call a raw edge dispatcher.

use std::path::{Path, PathBuf};

use crate::{
    ResolvedCampaignConfig, campaign_manifest_path,
    cli::{Prototype1StateCommand, prototype1_state::cli_facing::Prototype1StateRunShape},
    replay::tool_loop::OuterAttempt,
    spec::PrepareError,
};

use super::{
    super::{
        control_evidence::{SuccessorOrigin, initial_cursor, successor_cursor},
        edge::ControlEdge,
        event::{ContentHash, TransitionId},
        identity::{ParentIdentity, load_parent_identity_optional},
        invocation::{self, InvocationAuthority},
        journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
        live_edges::{
            r0_to_r1, r1_to_r2a_or_r3, r2a_to_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis,
            r4c_to_r5, r5_to_r6, r6_to_r7, r7_to_r8_linked, r8_to_r9, r9_to_r10, r10_to_r11,
            r11_to_r12, r12_to_r13, r13_to_r14,
        },
        profile::{self, RunMode},
        session::{
            AdmissionFailure, AttemptIntent, AttemptReceipt, Begin, Claim, Conflict, EpochReceipt,
            Failure, Fence, FinishFailure, Finished, Idle, Lease, Outcome, RecoveryCause,
            RecoveryLease, RecoveryOutcome, SessionId, SessionSnapshot, Store, SuccessorAuthority,
        },
        setup_admission::{load_setup_admission, setup_admission_path},
        successor,
        typestate::{
            self, AsyncStepInput, R0, R1, R2a, R3, R4a, R4bGenesisChecked, R4cReady, R5, R6, R7,
            R8, R9, R10, R11FanoutComplete, R11aRejectedOnly, R12, R13aStopped,
            R13bHandoffCommitted, R13cHandoffIncomplete, R14aFinalStopped, R14bFinalHandoff,
            StepInput,
        },
        walk::{endpoint, epoch::ServerEpoch, phase::WalkPhase, protocol::SessionVersion},
    },
    reconstruct::{self, EarlyState},
};

type RunShape = Prototype1StateRunShape;
type CampaignConfig = ResolvedCampaignConfig;

const EFFECT_SCHEMA: &str = "prototype1-control-effect.v1";

/// The concrete R-state authority owned by the admitted controller.
pub(crate) enum ControlState {
    Empty,
    R0(R0),
    R1(R1<RunShape, CampaignConfig>),
    R2a(R2a<RunShape, CampaignConfig>),
    R3(R3<RunShape, CampaignConfig>),
    R4a(R4a<RunShape, CampaignConfig>),
    R4b(R4bGenesisChecked<RunShape, CampaignConfig>),
    R4c(R4cReady<RunShape, CampaignConfig>),
    R5(R5<RunShape, CampaignConfig>),
    R6(R6<RunShape, CampaignConfig>),
    R7(R7<RunShape, CampaignConfig>),
    R8(R8<RunShape, CampaignConfig>),
    R9(R9<RunShape, CampaignConfig>),
    R10(R10<RunShape, CampaignConfig>),
    R11a(R11aRejectedOnly<RunShape, CampaignConfig>),
    R11(R11FanoutComplete<RunShape, CampaignConfig>),
    R12(R12<RunShape, CampaignConfig>),
    R13a(R13aStopped<RunShape, CampaignConfig>),
    R13b(R13bHandoffCommitted<RunShape, CampaignConfig>),
    R13c(R13cHandoffIncomplete<RunShape, CampaignConfig>),
    R14a(R14aFinalStopped<RunShape, CampaignConfig>),
    R14b(R14bFinalHandoff<RunShape, CampaignConfig>),
    /// Durable evidence identifies a cursor, but exact typed authority cannot
    /// be reconstructed safely.
    Blocked {
        phase: WalkPhase,
        detail: String,
    },
    /// A consuming transition failed after its input value moved.
    Failed {
        phase: WalkPhase,
        detail: String,
    },
}

impl ControlState {
    pub(crate) fn new(command: Prototype1StateCommand) -> Self {
        Self::R0(typestate::R0::new(command))
    }

    pub(crate) fn phase(&self) -> WalkPhase {
        match self {
            Self::Empty => WalkPhase::Empty,
            Self::R0(_) => WalkPhase::R0,
            Self::R1(_) => WalkPhase::R1,
            Self::R2a(_) => WalkPhase::R2a,
            Self::R3(_) => WalkPhase::R3,
            Self::R4a(_) => WalkPhase::R4a,
            Self::R4b(_) => WalkPhase::R4b,
            Self::R4c(_) => WalkPhase::R4c,
            Self::R5(_) => WalkPhase::R5,
            Self::R6(_) => WalkPhase::R6,
            Self::R7(_) => WalkPhase::R7,
            Self::R8(_) => WalkPhase::R8,
            Self::R9(_) => WalkPhase::R9,
            Self::R10(_) => WalkPhase::R10,
            Self::R11a(_) => WalkPhase::R11a,
            Self::R11(_) => WalkPhase::R11,
            Self::R12(_) => WalkPhase::R12,
            Self::R13a(_) => WalkPhase::R13a,
            Self::R13b(_) => WalkPhase::R13b,
            Self::R13c(_) => WalkPhase::R13c,
            Self::R14a(_) => WalkPhase::R14a,
            Self::R14b(_) => WalkPhase::R14b,
            Self::Blocked { phase, .. } | Self::Failed { phase, .. } => *phase,
        }
    }

    pub(crate) fn from_reconstructed(state: EarlyState) -> Self {
        match state {
            EarlyState::R1(r1) => Self::R1(r1),
            EarlyState::R3(r3) => Self::R3(r3),
            EarlyState::R4a(r4a) => Self::R4a(r4a),
            EarlyState::R4b(r4b) => Self::R4b(r4b),
            EarlyState::R4c(r4c) => Self::R4c(r4c),
            EarlyState::R5(r5) => Self::R5(r5),
            EarlyState::R6(r6) => Self::R6(r6),
            EarlyState::R7(r7) => Self::R7(r7),
            EarlyState::R8(r8) => Self::R8(r8),
            EarlyState::R9(r9) => Self::R9(r9),
            EarlyState::R10(r10) => Self::R10(r10),
            EarlyState::R11a(r11a) => Self::R11a(r11a),
            EarlyState::R11(r11) => Self::R11(r11),
            EarlyState::R12(r12) => Self::R12(r12),
            EarlyState::R13a(r13a) => Self::R13a(r13a),
            EarlyState::R13b(r13b) => Self::R13b(r13b),
            EarlyState::R13c(r13c) => Self::R13c(r13c),
            EarlyState::R14a(r14a) => Self::R14a(r14a),
            EarlyState::R14b(r14b) => Self::R14b(r14b),
        }
    }

    pub(crate) fn complete(&self) -> bool {
        matches!(self, Self::R14a(_) | Self::R14b(_))
    }
}

/// Explicit side-effect capabilities admitted for one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepAdmission {
    pub(crate) live: bool,
    pub(crate) checkout: bool,
}

impl StepAdmission {
    pub(crate) const fn new(live: bool, checkout: bool) -> Self {
        Self { live, checkout }
    }

    pub(crate) const fn continuous() -> Self {
        Self {
            live: true,
            checkout: true,
        }
    }
}

/// Sealed authority for one exact, already-admitted controller attempt.
///
/// Fields and construction stay private to this module. Session methods accept
/// a borrowed permit, so crate siblings can use the common operation but cannot
/// open or terminalize attempts themselves.
pub(crate) struct ControlPermit {
    session_id: SessionId,
    fence: Fence,
    transition_id: TransitionId,
    repo_root: PathBuf,
    expected: WalkPhase,
    targets: Vec<WalkPhase>,
    allow_live_api: bool,
    allow_git_changes: bool,
}

/// Opaque proof that the recovery driver validated one exact dead Ready offer
/// against both successor and predecessor durable sessions.
pub(crate) struct HandoffPermit {
    acceptance: successor::HandoffAcceptance,
    witness: ContentHash,
    epoch: EpochReceipt,
}

impl HandoffPermit {
    fn new(
        acceptance: successor::HandoffAcceptance,
        witness: ContentHash,
        epoch: EpochReceipt,
    ) -> Self {
        Self {
            acceptance,
            witness,
            epoch,
        }
    }

    pub(crate) fn into_parts(self) -> (successor::HandoffAcceptance, ContentHash, EpochReceipt) {
        (self.acceptance, self.witness, self.epoch)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        acceptance: successor::HandoffAcceptance,
        witness: ContentHash,
        epoch: EpochReceipt,
    ) -> Self {
        Self::new(acceptance, witness, epoch)
    }
}

impl ControlPermit {
    fn new(lease: &Lease<Idle>, intent: &AttemptIntent) -> Self {
        Self {
            session_id: lease.session_id(),
            fence: lease.fence(),
            transition_id: intent.transition_id,
            repo_root: intent.epoch.repo_root.clone(),
            expected: intent.expected,
            targets: intent.targets.clone(),
            allow_live_api: intent.allow_live_api,
            allow_git_changes: intent.allow_git_changes,
        }
    }

    pub(crate) const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub(crate) const fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub(crate) const fn expected(&self) -> WalkPhase {
        self.expected
    }

    pub(crate) fn targets(&self) -> &[WalkPhase] {
        &self.targets
    }

    pub(crate) const fn allow_live_api(&self) -> bool {
        self.allow_live_api
    }

    pub(crate) const fn allow_git_changes(&self) -> bool {
        self.allow_git_changes
    }

    pub(crate) fn handoff_attempt(&self) -> Result<successor::PredecessorAttempt, PrepareError> {
        if self.expected != WalkPhase::R12 || !self.targets.contains(&WalkPhase::R13b) {
            return Err(PrepareError::InvalidBatchSelection {
                detail:
                    "only an admitted R12->R13b controller attempt may acknowledge successor Ready"
                        .to_string(),
            });
        }
        Ok(successor::PredecessorAttempt::new(
            self.session_id,
            self.transition_id,
            self.fence,
            self.allow_live_api,
            self.allow_git_changes,
        ))
    }
}

#[cfg(test)]
pub(crate) async fn test_r12_stop(
    repo_root: &Path,
    r12: R12<RunShape, CampaignConfig>,
) -> Result<ControlStep, PrepareError> {
    let permit = ControlPermit {
        session_id: SessionId::for_test(0x1213),
        fence: Fence::for_test(12),
        transition_id: TransitionId::new(),
        repo_root: repo_root.to_path_buf(),
        expected: WalkPhase::R12,
        targets: vec![WalkPhase::R13a],
        allow_live_api: false,
        allow_git_changes: false,
    };
    advance(repo_root, ControlState::R12(r12), &permit)
        .await
        .map_err(|failure| failure.error)
}

#[cfg(test)]
pub(crate) fn test_r12_denied(
    repo_root: &Path,
    r12: R12<RunShape, CampaignConfig>,
) -> Result<(), PrepareError> {
    let permit = ControlPermit {
        session_id: SessionId::for_test(0x1213),
        fence: Fence::for_test(12),
        transition_id: TransitionId::new(),
        repo_root: repo_root.to_path_buf(),
        expected: WalkPhase::R12,
        targets: vec![WalkPhase::R13a],
        allow_live_api: false,
        allow_git_changes: false,
    };
    r12_to_r13(r12, &permit).map(|_| ())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlTransition {
    from: WalkPhase,
    to: WalkPhase,
}

impl ControlTransition {
    pub(crate) const fn from(self) -> WalkPhase {
        self.from
    }

    pub(crate) const fn to(self) -> WalkPhase {
        self.to
    }
}

/// Opaque proof that the canonical driver returned success for one admitted
/// session attempt. Only the controlled driver path can bind an attempt id.
pub(crate) struct ControlEffect {
    session_id: SessionId,
    fence: Fence,
    repo_root: PathBuf,
    transition_id: TransitionId,
    transition: ControlTransition,
    witness: ContentHash,
}

impl ControlEffect {
    pub(crate) const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub(crate) const fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub(crate) const fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn transition(&self) -> ControlTransition {
        self.transition
    }

    pub(crate) fn witness(&self) -> &ContentHash {
        &self.witness
    }
}

pub(crate) struct ControlStep {
    state: ControlState,
    effect: ControlEffect,
}

impl ControlStep {
    pub(crate) fn state(&self) -> &ControlState {
        &self.state
    }

    pub(crate) const fn transition(&self) -> ControlTransition {
        self.effect.transition
    }

    pub(crate) fn into_state(self) -> ControlState {
        self.state
    }

    pub(super) fn into_parts(self) -> (ControlState, ControlEffect) {
        (self.state, self.effect)
    }
}

pub(crate) struct StepFailure {
    state: ControlState,
    error: PrepareError,
}

/// Idempotent or newly finished result from the sole session mutation path.
pub(crate) enum ControlAdvance {
    Existing {
        lease: Lease<Idle>,
        receipt: AttemptReceipt,
    },
    Finished {
        finished: Finished,
        state: ControlState,
    },
}

impl std::fmt::Debug for ControlAdvance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Existing { lease, receipt } => formatter
                .debug_struct("Existing")
                .field("lease", lease)
                .field("receipt", receipt)
                .finish(),
            Self::Finished { finished, state } => formatter
                .debug_struct("Finished")
                .field("finished", finished)
                .field("phase", &state.phase())
                .finish(),
        }
    }
}

/// Opaque proof that the exact predecessor attempt accepted Ready and cleanly
/// relinquished its fenced controller authority.
pub(crate) struct PredecessorRelease(());

#[cfg(test)]
impl PredecessorRelease {
    pub(crate) const fn for_test() -> Self {
        Self(())
    }
}

/// Mutation admission for a generic walk-server endpoint.
pub(crate) enum ServerAdmission {
    /// A generation-zero setup/controller does not require predecessor transfer.
    Open,
    /// The exact predecessor handoff was committed and cleanly released.
    Transferred(PredecessorRelease),
    /// This successor remains inspectable, but mutation authority has not moved.
    Pending,
}

/// One explicit production reconciliation admitted by the operator.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDirective {
    Inspect,
    AbandonOwner,
    AbandonSession,
    AdmitEpoch,
}

/// Authority-preserving failures before or while persisting one controlled
/// attempt. An invoked edge never returns idle authority on failure.
#[derive(Debug)]
pub(crate) enum ControlFailure {
    Reconstruct {
        lease: Lease<Idle>,
        source: PrepareError,
    },
    Admission(AdmissionFailure),
    Persist(FinishFailure),
}

/// Resolve the completed setup authority and claim the one controller session
/// for the active checkout parent.
pub(crate) fn claim_controller(
    repo_root: &Path,
    expected_mode: RunMode,
) -> Result<Lease<Idle>, PrepareError> {
    claim_controller_at(repo_root, expected_mode, None)
}

/// Claim only if the durable session remains at the exact version observed by
/// a socket mutation before it was queued.
pub(crate) fn claim_controller_version(
    repo_root: &Path,
    expected_mode: RunMode,
    expected: &SessionVersion,
) -> Result<Lease<Idle>, PrepareError> {
    claim_controller_at(repo_root, expected_mode, Some(expected))
}

fn claim_controller_at(
    repo_root: &Path,
    expected_mode: RunMode,
    expected: Option<&SessionVersion>,
) -> Result<Lease<Idle>, PrepareError> {
    let (parent, manifest, admitted) = controller_inputs(repo_root)?;
    let store = Store::for_manifest(&manifest);
    let inspected = store.inspect(&parent).map_err(session_error)?;
    let handoff = inspected
        .as_ref()
        .and_then(|snapshot| snapshot.created.as_ref())
        .and_then(|created| created.handoff_path())
        .map(Path::to_path_buf);
    if let Some(path) = handoff {
        return claim_successor_with(
            repo_root,
            expected_mode,
            &path,
            parent,
            manifest,
            admitted,
            inspected,
            expected,
        );
    }
    let setup_path = setup_admission_path(&manifest);
    let setup =
        load_setup_admission(&setup_path)?.ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "controller session requires completed setup admission '{}'",
                setup_path.display()
            ),
        })?;
    let actual_mode = admitted.profile.control.mode;
    if actual_mode != expected_mode {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested {expected_mode:?} controller but the admitted run profile requires {actual_mode:?}"
            ),
        });
    }
    let origin = initial_cursor(&setup, &parent, &admitted.commitment).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: source.to_string(),
        }
    })?;
    let cursor = match inspected {
        Some(snapshot) => snapshot.cursor.unwrap_or(origin),
        None => {
            validate_fresh_session(repo_root)?;
            origin
        }
    };
    let epoch = ServerEpoch::capture(repo_root)?;
    let request = Claim::from_setup(setup, parent, &admitted, actual_mode, cursor, epoch)
        .map_err(session_error)?;
    acquire_claim(&store, request, expected)
}

/// Inspect or terminalize one exact durable controller recovery cause.
///
/// Pending and indeterminate attempts deliberately remain unresolved here.
/// Reconstructing the admitted source does not prove that a provider,
/// checkout, process, or partially persisted edge effect did not occur. Those
/// attempts require an edge-specific no-effect certificate or specialized
/// reconciliation rather than a generic cancellation; the only generic
/// terminal action permanently abandons the session without restoring
/// mutation authority.
pub(crate) fn recover_controller(
    repo_root: &Path,
    expected_mode: RunMode,
    directive: RecoveryDirective,
) -> Result<String, PrepareError> {
    let epoch = ServerEpoch::capture(repo_root)?;
    recover_controller_at(repo_root, expected_mode, directive, &epoch, None)
}

fn recover_controller_at(
    repo_root: &Path,
    expected_mode: RunMode,
    directive: RecoveryDirective,
    epoch: &ServerEpoch,
    expected: Option<&SessionVersion>,
) -> Result<String, PrepareError> {
    let (parent, manifest, admitted) = controller_inputs(repo_root)?;
    if admitted.profile.control.mode != expected_mode {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested {expected_mode:?} recovery but the admitted run profile requires {:?}",
                admitted.profile.control.mode
            ),
        });
    }
    let store = Store::for_manifest(&manifest);
    let snapshot = store
        .inspect(&parent)
        .map_err(session_error)?
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "controller recovery has no durable session".to_string(),
        })?;
    let created = snapshot
        .created
        .as_ref()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "controller recovery session has no creation authority".to_string(),
        })?;
    let cursor = snapshot
        .cursor
        .clone()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "controller recovery session has no committed cursor".to_string(),
        })?;
    let request = Claim::from_recovery(created, cursor, epoch.clone()).map_err(session_error)?;
    let outcome = match expected {
        Some(expected) => store.claim_recovery_version(request, expected),
        None => store.claim_recovery(request),
    }
    .map_err(session_error)?;
    let recovery = match outcome {
        RecoveryOutcome::Resolved => {
            return Ok("controller recovery: no unresolved cause".to_string());
        }
        RecoveryOutcome::Conflict(Conflict::Locked { path }) => {
            return Err(PrepareError::RecoveryInProgress { path });
        }
        RecoveryOutcome::Conflict(conflict) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!("controller recovery claim conflicted: {conflict:?}"),
            });
        }
        RecoveryOutcome::Required(recovery) => recovery,
    };
    let cause = recovery
        .cause()
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "controller recovery lock has no unresolved cause".to_string(),
        })?;
    if directive == RecoveryDirective::Inspect {
        return Ok(format!("controller recovery required: {cause:?}"));
    }
    let abandon = directive == RecoveryDirective::AbandonSession;
    match directive {
        RecoveryDirective::Inspect => unreachable!("handled above"),
        RecoveryDirective::AbandonOwner => {
            ensure_owner_gone(&cause)?;
            recovery.abandon_owner().map_err(recovery_error)?;
        }
        RecoveryDirective::AbandonSession => recovery
            .abandon_session(format!(
                "operator permanently abandoned unresolved controller state: {cause:?}"
            ))
            .map_err(recovery_error)?,
        RecoveryDirective::AdmitEpoch => recovery.admit_epoch().map_err(recovery_error)?,
    }
    if abandon {
        Ok(format!(
            "controller session permanently abandoned at {cause:?}; preserve this run as evidence and start a fresh run"
        ))
    } else {
        Ok(format!(
            "controller recovery resolved {cause:?}; retry the original command to claim a new fence"
        ))
    }
}

fn ensure_owner_gone(cause: &RecoveryCause) -> Result<(), PrepareError> {
    let RecoveryCause::OwnerLost {
        pid, incarnation, ..
    } = cause
    else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("owner abandonment cannot resolve recovery cause {cause:?}"),
        });
    };
    let expected = incarnation.as_ref().ok_or_else(|| PrepareError::InvalidBatchSelection {
        detail: "legacy controller owner has no process incarnation; automatic abandonment is unavailable"
            .to_string(),
    })?;
    let actual = invocation::process_incarnation(*pid).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: format!("cannot inspect controller owner process {pid}: {source}"),
        }
    })?;
    if actual.as_ref() == Some(expected) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "controller owner process {pid} still has its recorded incarnation; abandonment is refused"
            ),
        });
    }
    Ok(())
}

/// Resolve recovery in the mode already committed by the admitted run profile.
/// Socket clients do not choose a competing controller mode.
pub(crate) fn recover_admitted_controller(
    repo_root: &Path,
    directive: RecoveryDirective,
) -> Result<String, PrepareError> {
    let (_, _, admitted) = controller_inputs(repo_root)?;
    recover_controller(repo_root, admitted.profile.control.mode, directive)
}

/// Resolve the exact recovery cause admitted by a socket mutation guard and
/// the server epoch the client actually authorized.
pub(crate) fn recover_admitted_version(
    repo_root: &Path,
    directive: RecoveryDirective,
    epoch: &ServerEpoch,
    expected: &SessionVersion,
) -> Result<String, PrepareError> {
    let (_, _, admitted) = controller_inputs(repo_root)?;
    recover_controller_at(
        repo_root,
        admitted.profile.control.mode,
        directive,
        epoch,
        Some(expected),
    )
}

/// Claim the admitted controller mode for an ancillary mutation that does not
/// itself advance the outer R-state cursor.
pub(crate) fn claim_active(repo_root: &Path) -> Result<Lease<Idle>, PrepareError> {
    let (_, _, admitted) = controller_inputs(repo_root)?;
    claim_controller(repo_root, admitted.profile.control.mode)
}

/// Claim an installed successor session from its exact invocation/runtime
/// transfer rather than reusing the generation-zero setup origin.
pub(crate) fn claim_successor(
    repo_root: &Path,
    expected_mode: RunMode,
    invocation_path: &Path,
) -> Result<Lease<Idle>, PrepareError> {
    let (parent, manifest, admitted) = controller_inputs(repo_root)?;
    let store = Store::for_manifest(&manifest);
    let inspected = store.inspect(&parent).map_err(session_error)?;
    claim_successor_with(
        repo_root,
        expected_mode,
        invocation_path,
        parent,
        manifest,
        admitted,
        inspected,
        None,
    )
}

fn claim_successor_with(
    repo_root: &Path,
    expected_mode: RunMode,
    invocation_path: &Path,
    parent: ParentIdentity,
    manifest: PathBuf,
    admitted: profile::AdmittedRunProfile,
    inspected: Option<SessionSnapshot>,
    expected: Option<&SessionVersion>,
) -> Result<Lease<Idle>, PrepareError> {
    let actual_mode = admitted.profile.control.mode;
    if actual_mode != expected_mode {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested {expected_mode:?} controller but the admitted run profile requires {actual_mode:?}"
            ),
        });
    }
    let (transfer, lifecycle) = load_successor_origin(
        repo_root,
        &manifest,
        invocation_path,
        &parent,
        &admitted.commitment,
    )?;
    let origin = successor_cursor(&transfer, &parent, &admitted.commitment, repo_root).map_err(
        |source| PrepareError::InvalidBatchSelection {
            detail: source.to_string(),
        },
    )?;
    let epoch = ServerEpoch::capture(repo_root)?;
    let store = Store::for_manifest(&manifest);
    let (cursor, authority) = match inspected {
        Some(snapshot) => {
            if lifecycle.terminal() {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "cannot reclaim successor controller session after runtime lifecycle {}",
                        lifecycle.label()
                    ),
                });
            }
            let cursor = snapshot.cursor.clone().unwrap_or(origin);
            if matches!(cursor.phase, WalkPhase::R3 | WalkPhase::R4a) {
                if !matches!(&lifecycle, SuccessorLifecycle::Spawned) {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "successor bootstrap at {} cannot use runtime lifecycle {}",
                            cursor.phase,
                            lifecycle.label()
                        ),
                    });
                }
                if std::process::id() != transfer.spawned_pid()
                    || !same_path(&epoch.exe_path, transfer.binary_path())
                {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: "only the exact spawned successor process may resume before its durable R4c Ready boundary"
                            .to_string(),
                    });
                }
                (cursor, SuccessorAuthority::Spawned)
            } else {
                let receipt = lifecycle.ready_receipt().ok_or_else(|| {
                    PrepareError::InvalidBatchSelection {
                        detail: "successor has not published a durable R4c Ready receipt"
                            .to_string(),
                    }
                })?;
                validate_ready_session(&snapshot, &transfer, receipt)?;
                predecessor_release(&store, &transfer, &lifecycle)?.ok_or_else(|| {
                    PrepareError::InvalidBatchSelection {
                        detail: "successor controller transfer is pending: predecessor has not accepted this Ready receipt, committed R12->R13b, and cleanly released that fence"
                            .to_string(),
                    }
                })?;
                let acceptance =
                    lifecycle
                        .acceptance()
                        .ok_or_else(|| PrepareError::InvalidBatchSelection {
                            detail: "successor transfer has no exact predecessor acceptance"
                                .to_string(),
                        })?;
                (cursor, SuccessorAuthority::Transferred(acceptance.clone()))
            }
        }
        None if !matches!(&lifecycle, SuccessorLifecycle::Spawned) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "cannot create a fresh successor controller session after this runtime recorded {}; preserve the historical run as read-only or recover it explicitly",
                    lifecycle.label()
                ),
            });
        }
        None => (origin, SuccessorAuthority::Spawned),
    };
    let request = Claim::from_successor(
        transfer,
        authority,
        parent,
        &admitted,
        actual_mode,
        cursor,
        epoch,
    )
    .map_err(session_error)?;
    acquire_claim(&store, request, expected)
}

/// Inspect whether the predecessor has completed the exact durable transfer
/// that permits this successor to acquire post-R4c mutation authority.
pub(crate) fn successor_transfer_release(
    repo_root: &Path,
    expected_mode: RunMode,
    invocation_path: &Path,
) -> Result<Option<PredecessorRelease>, PrepareError> {
    let (parent, manifest, admitted) = controller_inputs(repo_root)?;
    let actual_mode = admitted.profile.control.mode;
    if actual_mode != expected_mode {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested {expected_mode:?} controller but the admitted run profile requires {actual_mode:?}"
            ),
        });
    }
    let store = Store::for_manifest(&manifest);
    let Some(snapshot) = store.inspect(&parent).map_err(session_error)? else {
        return Ok(None);
    };
    let (transfer, lifecycle) = load_successor_origin(
        repo_root,
        &manifest,
        invocation_path,
        &parent,
        &admitted.commitment,
    )?;
    if lifecycle.terminal() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor transfer cannot open after runtime lifecycle {}",
                lifecycle.label()
            ),
        });
    }
    let receipt = match lifecycle.ready_receipt() {
        Some(receipt) => receipt,
        None if matches!(lifecycle, SuccessorLifecycle::Spawned) => {
            let Some(receipt) = snapshot
                .ready_receipt(transfer.runtime_id())
                .map_err(session_error)?
            else {
                return Ok(None);
            };
            validate_ready_session(&snapshot, &transfer, receipt)?;
            return Ok(None);
        }
        None => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "successor has not published a durable R4c Ready receipt".to_string(),
            });
        }
    };
    validate_ready_session(&snapshot, &transfer, receipt)?;
    predecessor_release(&store, &transfer, &lifecycle)
}

/// Derive a generic walk server's mutation gate from durable lineage/session
/// evidence. A successor checkout never opens merely because an operator
/// launched `walk serve` directly.
pub(crate) fn walk_server_admission(repo_root: &Path) -> Result<ServerAdmission, PrepareError> {
    let Some(parent) = load_parent_identity_optional(repo_root)? else {
        return Ok(ServerAdmission::Open);
    };
    if parent.generation() == 0 && parent.previous_parent_id().is_none() {
        return Ok(ServerAdmission::Open);
    }
    let manifest = campaign_manifest_path(parent.campaign_id())?;
    let admitted = profile::load_admitted_run_profile(&manifest)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk server mutation requires an admitted run profile for '{}'",
                manifest.display()
            ),
        }
    })?;
    let store = Store::for_manifest(&manifest);
    let Some(snapshot) = store.inspect(&parent).map_err(session_error)? else {
        return Ok(ServerAdmission::Pending);
    };
    if let Some(damage) = snapshot.damage.as_ref() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk server cannot derive mutation authority from a damaged controller session: {damage:?}"
            ),
        });
    }
    let created = snapshot
        .created
        .as_ref()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "walk server controller session has no creation authority".to_string(),
        })?;
    let Some(handoff) = created.handoff_path() else {
        return Ok(ServerAdmission::Pending);
    };
    let Some(cursor) = snapshot.cursor.as_ref() else {
        return Ok(ServerAdmission::Pending);
    };
    if matches!(cursor.phase, WalkPhase::R3 | WalkPhase::R4a) {
        return Ok(ServerAdmission::Pending);
    }
    match successor_transfer_release(repo_root, admitted.profile.control.mode, handoff)? {
        Some(release) => Ok(ServerAdmission::Transferred(release)),
        None => Ok(ServerAdmission::Pending),
    }
}

/// Read the exact session-persisted Ready receipt for a spawned successor.
/// Transition-journal and runtime-channel records are projections that may be
/// absent after a crash without losing the atomic R4c release authority.
pub(crate) fn persisted_successor_ready(
    repo_root: &Path,
    invocation_path: &Path,
) -> Result<Option<successor::ReadyReceipt>, PrepareError> {
    let (parent, manifest, admitted) = controller_inputs(repo_root)?;
    let (transfer, lifecycle) = load_successor_origin(
        repo_root,
        &manifest,
        invocation_path,
        &parent,
        &admitted.commitment,
    )?;
    let store = Store::for_manifest(&manifest);
    let Some(snapshot) = store.inspect(&parent).map_err(session_error)? else {
        return match lifecycle {
            SuccessorLifecycle::Spawned => Ok(None),
            _ => Err(PrepareError::InvalidBatchSelection {
                detail: "successor Ready projection exists without a controller session"
                    .to_string(),
            }),
        };
    };
    let persisted = snapshot
        .ready_receipt(transfer.runtime_id())
        .map_err(session_error)?
        .cloned();
    match (persisted, lifecycle) {
        (None, SuccessorLifecycle::Spawned) => Ok(None),
        (None, SuccessorLifecycle::Ready(_) | SuccessorLifecycle::Acknowledged(_)) => {
            Err(PrepareError::InvalidBatchSelection {
                detail: "successor Ready projection has no atomic session authority".to_string(),
            })
        }
        (Some(receipt), SuccessorLifecycle::Spawned) => {
            validate_ready_session(&snapshot, &transfer, &receipt)?;
            Ok(Some(receipt))
        }
        (Some(receipt), SuccessorLifecycle::Ready(projected)) if projected == receipt => {
            validate_ready_session(&snapshot, &transfer, &receipt)?;
            Ok(Some(receipt))
        }
        (Some(receipt), SuccessorLifecycle::Acknowledged(acceptance))
            if acceptance.ready() == &receipt =>
        {
            validate_ready_session(&snapshot, &transfer, &receipt)?;
            Ok(Some(receipt))
        }
        (Some(_), SuccessorLifecycle::Ready(_) | SuccessorLifecycle::Acknowledged(_)) => {
            Err(PrepareError::InvalidBatchSelection {
                detail: "successor Ready projection conflicts with atomic session authority"
                    .to_string(),
            })
        }
        (_, terminal) => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor runtime reached {} before Ready reconciliation",
                terminal.label()
            ),
        }),
    }
}

/// Complete the exact predecessor transfer when the original Ready owner died
/// after publishing atomic session authority but before acknowledgement.
///
/// The recovery lock is acquired before either journal is changed. An exact
/// pre-existing acknowledgement is accepted on retry; conflicting lifecycle
/// or session evidence remains a blocker.
pub(crate) fn recover_successor_handoff(
    repo_root: &Path,
    expected_mode: RunMode,
    invocation_path: &Path,
) -> Result<PredecessorRelease, PrepareError> {
    let (parent, manifest, admitted) = controller_inputs(repo_root)?;
    let actual_mode = admitted.profile.control.mode;
    if actual_mode != expected_mode {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested {expected_mode:?} controller but the admitted run profile requires {actual_mode:?}"
            ),
        });
    }
    let store = Store::for_manifest(&manifest);
    let (transfer, lifecycle) = load_successor_origin(
        repo_root,
        &manifest,
        invocation_path,
        &parent,
        &admitted.commitment,
    )?;
    if lifecycle.terminal() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor handoff recovery cannot use terminal runtime lifecycle {}",
                lifecycle.label()
            ),
        });
    }
    if let Some(release) = predecessor_release(&store, &transfer, &lifecycle)? {
        return Ok(release);
    }

    let successor = store
        .inspect(&parent)
        .map_err(session_error)?
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor handoff recovery has no successor controller session".to_string(),
        })?;
    let ready = successor
        .ready_receipt(transfer.runtime_id())
        .map_err(session_error)?
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor handoff recovery has no atomic Ready receipt".to_string(),
        })?;
    validate_ready_session(&successor, &transfer, &ready)?;
    let ready_epoch = successor
        .ready_epoch(transfer.runtime_id(), &ready)
        .map_err(session_error)?
        .clone();
    let current_epoch = ServerEpoch::capture(repo_root)?;
    if current_epoch != ready_epoch {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor handoff recovery refused because the checkout or controller epoch changed after durable Ready"
                .to_string(),
        });
    }
    ensure_ready_owner_gone(&ready)?;
    if expected_mode == RunMode::Step {
        let expected = ready
            .predecessor()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "Step handoff recovery has no persisted predecessor endpoint".to_string(),
            })?;
        if let Some(active) = endpoint::load(repo_root)?
            && &active != expected
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "Step handoff recovery predecessor changed from pid {} at '{}' to pid {} at '{}'",
                    expected.pid(),
                    expected.socket().display(),
                    active.pid(),
                    active.socket().display()
                ),
            });
        }
    }

    let predecessor =
        transfer
            .predecessor()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "successor handoff recovery is missing predecessor identity".to_string(),
            })?;
    let predecessor_snapshot = store
        .inspect(predecessor)
        .map_err(session_error)?
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor handoff recovery has no predecessor controller session".to_string(),
        })?;
    let created = predecessor_snapshot.created.as_ref().ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: "predecessor recovery session is missing creation authority".to_string(),
        }
    })?;
    let cursor =
        predecessor_snapshot
            .cursor
            .clone()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "predecessor recovery session has no committed cursor".to_string(),
            })?;
    if !matches!(
        cursor.phase,
        WalkPhase::R12 | WalkPhase::R13b | WalkPhase::R14b
    ) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "stranded successor Ready requires predecessor cursor R12, R13b, or R14b, found {}",
                cursor.phase
            ),
        });
    }
    let request = Claim::from_recovery(created, cursor.clone(), ready_epoch.clone())
        .map_err(session_error)?;
    let recovery = match store.claim_recovery(request).map_err(session_error)? {
        RecoveryOutcome::Required(recovery) => recovery,
        RecoveryOutcome::Conflict(conflict) => {
            let (_, settled) = load_successor_origin(
                repo_root,
                &manifest,
                invocation_path,
                &parent,
                &admitted.commitment,
            )?;
            if let Some(release) = predecessor_release(&store, &transfer, &settled)? {
                return Ok(release);
            }
            if let Conflict::Locked { path } = conflict {
                return Err(PrepareError::RecoveryInProgress { path });
            }
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor handoff recovery conflicted with live authority: {conflict:?}"
                ),
            });
        }
        RecoveryOutcome::Resolved => {
            let (_, settled) = load_successor_origin(
                repo_root,
                &manifest,
                invocation_path,
                &parent,
                &admitted.commitment,
            )?;
            if let Some(release) = predecessor_release(&store, &transfer, &settled)? {
                return Ok(release);
            }
            return Err(PrepareError::InvalidBatchSelection {
                detail: "predecessor session no longer has a recoverable handoff fence".to_string(),
            });
        }
    };
    let (session_id, fence, intent) = match recovery.cause() {
        Some(RecoveryCause::AttemptPending {
            session_id,
            fence,
            intent,
        })
        | Some(RecoveryCause::AttemptIndeterminate {
            session_id,
            fence,
            intent,
            ..
        }) => (*session_id, *fence, intent.clone()),
        Some(RecoveryCause::OwnerLost {
            session_id, fence, ..
        }) => {
            let receipt = predecessor_snapshot
                .committed_handoff(*session_id, *fence)
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail:
                        "lost predecessor owner has no exact committed R12->R13b handoff attempt"
                            .to_string(),
                })?;
            (*session_id, *fence, receipt.intent)
        }
        cause => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor handoff recovery requires the exact pending, indeterminate, or committed R12 attempt, found {cause:?}"
                ),
            });
        }
    };
    let attempt = successor::PredecessorAttempt::new(
        session_id,
        intent.transition_id,
        fence,
        intent.allow_live_api,
        intent.allow_git_changes,
    );
    let acceptance = match lifecycle.acceptance() {
        Some(existing) if existing.ready() == &ready && existing.attempt() == &attempt => {
            existing.clone()
        }
        Some(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "persisted successor acknowledgement conflicts with the recoverable predecessor attempt"
                    .to_string(),
            });
        }
        None => successor::HandoffAcceptance::from_persisted(ready.clone(), attempt.clone())
            .map_err(|detail| PrepareError::InvalidBatchSelection { detail })?,
    };
    let edge = ControlEdge::from_phases(WalkPhase::R12, WalkPhase::R13b)
        .expect("R12->R13b is a retained control edge");
    let witness = effect_witness(session_id, fence, intent.transition_id, edge);
    let epoch = EpochReceipt {
        before: intent.epoch.clone(),
        after: Some(ready_epoch),
    };
    let prepared = recovery
        .prepare_handoff(HandoffPermit::new(acceptance.clone(), witness, epoch))
        .map_err(recovery_error)?;

    let invocation = match invocation::load_executable(invocation_path)? {
        InvocationAuthority::Successor(invocation) => invocation,
        InvocationAuthority::Child(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "handoff recovery invocation grants child authority".to_string(),
            });
        }
    };
    if invocation.predecessor_attempt() != Some(&attempt) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "handoff recovery invocation does not preserve the exact admitted predecessor attempt"
                .to_string(),
        });
    }
    let ready_path = invocation
        .channel_endpoints()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "handoff recovery invocation has no Ready channel".to_string(),
        })?
        .child_to_parent()
        .path()
        .to_path_buf();
    let mut journal = PrototypeJournal::new(invocation.journal_path().to_path_buf());
    let projected = journal
        .project_ready(successor::Record::ready(
            &invocation,
            ready.record().pid,
            ready_path,
            ready.clone(),
        ))
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_ready_recovery",
            detail: source.to_string(),
        })?;
    if projected != ready {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "reconciled Ready projection differs from atomic session authority".to_string(),
        });
    }
    journal
        .project_handoff(transfer.runtime_id(), acceptance.clone())
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_successor_handoff_recovery",
            detail: source.to_string(),
        })?;

    let expected_phase = if cursor.phase == WalkPhase::R12 {
        WalkPhase::R13b
    } else {
        cursor.phase
    };
    let rebuilt = reconstruct::reconstruct_at(repo_root, expected_phase)?;
    if rebuilt.blocked.is_some()
        || rebuilt.state.as_ref().map(EarlyState::phase) != Some(expected_phase)
    {
        let detail = rebuilt
            .blocked
            .map_or_else(|| rebuilt.blockers.join("; "), |blocked| blocked.detail);
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff acknowledgement did not reconstruct exact {expected_phase} authority: {detail}"
            ),
        });
    }
    prepared.commit().map_err(recovery_error)?;

    let recovered = SuccessorLifecycle::Acknowledged(acceptance);
    predecessor_release(&store, &transfer, &recovered)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: "recovered predecessor handoff did not produce transfer authority".to_string(),
        }
    })
}

fn recovery_error(failure: Failure<RecoveryLease>) -> PrepareError {
    match failure {
        Failure::Retained { source, .. } | Failure::Uncertain { source } => session_error(source),
    }
}

fn ensure_ready_owner_gone(receipt: &successor::ReadyReceipt) -> Result<(), PrepareError> {
    let record = receipt.record();
    let expected = record.incarnation.as_ref().ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: "legacy successor Ready has no process incarnation; automatic recovery is unavailable"
                .to_string(),
        }
    })?;
    let actual = invocation::process_incarnation(record.pid).map_err(|source| {
        PrepareError::DatabaseSetup {
            phase: "prototype1_successor_owner_probe",
            detail: source.to_string(),
        }
    })?;
    if actual.as_ref() == Some(expected) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "cannot recover successor Ready while its exact owner process {} remains alive",
                record.pid
            ),
        });
    }
    Ok(())
}

fn validate_ready_session(
    snapshot: &SessionSnapshot,
    transfer: &SuccessorOrigin,
    receipt: &successor::ReadyReceipt,
) -> Result<(), PrepareError> {
    receipt
        .validate_persisted()
        .map_err(|detail| PrepareError::InvalidBatchSelection { detail })?;
    let created = snapshot
        .created
        .as_ref()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor controller session is missing its creation authority".to_string(),
        })?;
    if created.successor_runtime() != Some(transfer.runtime_id())
        || created.predecessor() != transfer.predecessor()
        || created
            .handoff_path()
            .is_none_or(|path| !same_path(path, transfer.invocation_path()))
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor Ready receipt does not match its controller-session origin"
                .to_string(),
        });
    }
    snapshot
        .validate_ready_receipt(transfer.runtime_id(), receipt)
        .map_err(session_error)
}

fn predecessor_release(
    store: &Store,
    transfer: &SuccessorOrigin,
    lifecycle: &SuccessorLifecycle,
) -> Result<Option<PredecessorRelease>, PrepareError> {
    if !lifecycle.acknowledged() {
        return Ok(None);
    }
    let acceptance = lifecycle
        .acceptance()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor handoff has no fenced Ready acceptance".to_string(),
        })?;
    let predecessor =
        transfer
            .predecessor()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "successor checkout is missing its predecessor identity".to_string(),
            })?;
    let Some(snapshot) = store.inspect(predecessor).map_err(session_error)? else {
        return Ok(None);
    };
    let released = snapshot
        .accepted_handoff(predecessor, acceptance)
        .map_err(session_error)?;
    Ok(released.then_some(PredecessorRelease(())))
}

fn controller_inputs(
    repo_root: &Path,
) -> Result<(ParentIdentity, PathBuf, profile::AdmittedRunProfile), PrepareError> {
    let parent = load_parent_identity_optional(repo_root)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "no Prototype 1 parent identity exists under '{}'",
                repo_root.display()
            ),
        }
    })?;
    let manifest = campaign_manifest_path(parent.campaign_id())?;
    let admitted = profile::load_admitted_run_profile(&manifest)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "controller session requires an admitted run profile for '{}'",
                manifest.display()
            ),
        }
    })?;
    Ok((parent, manifest, admitted))
}

fn acquire_claim(
    store: &Store,
    request: Claim,
    expected: Option<&SessionVersion>,
) -> Result<Lease<Idle>, PrepareError> {
    let outcome = match expected {
        Some(expected) => store.claim_version(request, expected),
        None => store.claim(request),
    }
    .map_err(session_error)?;
    match outcome {
        Outcome::Acquired(lease) => Ok(lease),
        Outcome::Conflict(conflict) => Err(PrepareError::InvalidBatchSelection {
            detail: format!("controller session claim conflicted: {conflict:?}"),
        }),
        Outcome::Recoverable(recovery) => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "controller session requires explicit recovery before mutation: {:?}",
                recovery.cause()
            ),
        }),
    }
}

fn load_successor_origin(
    repo_root: &Path,
    manifest: &Path,
    invocation_path: &Path,
    parent: &ParentIdentity,
    profile: &profile::RunProfileCommitment,
) -> Result<(SuccessorOrigin, SuccessorLifecycle), PrepareError> {
    let invocation_path =
        invocation_path
            .canonicalize()
            .map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "cannot resolve successor invocation '{}': {source}",
                    invocation_path.display()
                ),
            })?;
    let invocation = match invocation::load_executable(&invocation_path)? {
        InvocationAuthority::Successor(invocation) => invocation,
        InvocationAuthority::Child(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "invocation '{}' grants child authority, not successor control",
                    invocation_path.display()
                ),
            });
        }
    };
    if invocation.campaign_id() != parent.campaign_id()
        || invocation.node_id() != parent.node_id()
        || invocation
            .active_parent_root()
            .is_none_or(|root| !same_path(root, repo_root))
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor invocation does not match the active parent checkout".to_string(),
        });
    }
    let journal_path = prototype1_transition_journal_path(manifest);
    if !same_path(invocation.journal_path(), &journal_path) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor invocation journal '{}' does not match campaign journal '{}'",
                invocation.journal_path().display(),
                journal_path.display()
            ),
        });
    }
    let entries = PrototypeJournal::new(journal_path)
        .load_entries()
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!("cannot load successor transfer journal: {source}"),
        })?;
    let runtime = invocation.runtime_id();
    let (spawn_index, spawned, lifecycle) = successor_lifecycle(&entries, parent, runtime)?;
    let checkout = entries
        .iter()
        .take(spawn_index)
        .rev()
        .find_map(|entry| match entry {
            JournalEntry::ActiveCheckoutAdvanced(entry)
                if entry.campaign_id == *parent.campaign_id()
                    && entry.selected_parent_identity == *parent
                    && same_path(&entry.active_parent_root, repo_root) =>
            {
                Some(entry.clone())
            }
            _ => None,
        })
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor controller origin is missing the installed checkout record"
                .to_string(),
        })?;
    if entries.iter().skip(spawn_index + 1).any(|entry| {
        matches!(
            entry,
            JournalEntry::ActiveCheckoutAdvanced(entry)
                if entry.campaign_id == *parent.campaign_id()
                    && entry.selected_parent_identity == *parent
        )
    }) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor checkout evidence was appended after the runtime was spawned; explicit reconciliation is required"
                .to_string(),
        });
    }
    let transfer = SuccessorOrigin::new(
        invocation_path,
        invocation.as_invocation().clone(),
        checkout,
        spawned,
        parent,
        profile,
        repo_root,
    )
    .map_err(|source| PrepareError::InvalidBatchSelection {
        detail: source.to_string(),
    })?;
    if lifecycle.acknowledged()
        && !entries.iter().any(|entry| {
            matches!(entry, JournalEntry::SuccessorHandoff(handoff) if transfer.matches_handoff(handoff))
        })
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor handoff acknowledgement does not match the exact spawned runtime"
                .to_string(),
        });
    }
    Ok((transfer, lifecycle))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SuccessorLifecycle {
    Spawned,
    Ready(successor::ReadyReceipt),
    Acknowledged(successor::HandoffAcceptance),
    TimedOut,
    Exited,
    Completed,
}

impl SuccessorLifecycle {
    const fn terminal(&self) -> bool {
        matches!(self, Self::TimedOut | Self::Exited | Self::Completed)
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::Spawned => "Spawned",
            Self::Ready(_) => "Ready",
            Self::Acknowledged(_) => "SuccessorHandoff",
            Self::TimedOut => "TimedOut",
            Self::Exited => "ExitedBeforeReady",
            Self::Completed => "Completed",
        }
    }

    fn ready_receipt(&self) -> Option<&successor::ReadyReceipt> {
        match self {
            Self::Ready(receipt) => Some(receipt),
            Self::Acknowledged(acceptance) => Some(acceptance.ready()),
            Self::Spawned | Self::TimedOut | Self::Exited | Self::Completed => None,
        }
    }

    const fn acknowledged(&self) -> bool {
        matches!(self, Self::Acknowledged(_))
    }

    fn acceptance(&self) -> Option<&successor::HandoffAcceptance> {
        match self {
            Self::Acknowledged(acceptance) => Some(acceptance),
            Self::Spawned | Self::Ready(_) | Self::TimedOut | Self::Exited | Self::Completed => {
                None
            }
        }
    }
}

fn successor_lifecycle(
    entries: &[JournalEntry],
    parent: &ParentIdentity,
    runtime: super::super::event::RuntimeId,
) -> Result<(usize, successor::Record, SuccessorLifecycle), PrepareError> {
    let mut spawned = None;
    let mut lifecycle = None;
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            JournalEntry::Successor(record)
                if record.campaign_id == *parent.campaign_id()
                    && record.node_id == parent.node_id()
                    && record.runtime_id == Some(runtime) =>
            {
                let next = match &record.state {
                    successor::State::Spawned { .. } if spawned.is_none() => {
                        spawned = Some((index, record.clone()));
                        SuccessorLifecycle::Spawned
                    }
                    successor::State::Ready {
                        pid,
                        ready_path,
                        controller,
                    } if lifecycle.as_ref() == Some(&SuccessorLifecycle::Spawned) => {
                        let receipt = controller.clone().ok_or_else(|| {
                            PrepareError::InvalidBatchSelection {
                                detail: "successor runtime recorded legacy Ready without a durable controller receipt"
                                    .to_string(),
                            }
                        })?;
                        receipt.validate_persisted().map_err(|detail| {
                            PrepareError::InvalidBatchSelection {
                                detail: format!("successor Ready receipt is invalid: {detail}"),
                            }
                        })?;
                        let ready = receipt.record();
                        let spawned_ready =
                            spawned
                                .as_ref()
                                .and_then(|(_, spawned)| match &spawned.state {
                                    successor::State::Spawned { ready_path, .. } => {
                                        Some(ready_path)
                                    }
                                    _ => None,
                                });
                        if ready.campaign_id != *parent.campaign_id()
                            || ready.node_id != parent.node_id()
                            || ready.runtime_id
                                != super::super::invocation::record_runtime_id(runtime)
                            || ready.pid != *pid
                            || spawned_ready != Some(ready_path)
                        {
                            return Err(PrepareError::InvalidBatchSelection {
                                detail: "successor Ready receipt does not match its runtime envelope or Spawned record"
                                    .to_string(),
                            });
                        }
                        SuccessorLifecycle::Ready(receipt)
                    }
                    successor::State::TimedOut { .. }
                        if lifecycle.as_ref() == Some(&SuccessorLifecycle::Spawned) =>
                    {
                        SuccessorLifecycle::TimedOut
                    }
                    successor::State::ExitedBeforeReady { .. }
                        if lifecycle.as_ref() == Some(&SuccessorLifecycle::Spawned) =>
                    {
                        SuccessorLifecycle::Exited
                    }
                    successor::State::Completed { .. }
                        if matches!(
                            lifecycle.as_ref(),
                            Some(
                                SuccessorLifecycle::Ready(_) | SuccessorLifecycle::Acknowledged(_)
                            )
                        ) =>
                    {
                        SuccessorLifecycle::Completed
                    }
                    state => {
                        return Err(PrepareError::InvalidBatchSelection {
                            detail: format!(
                                "invalid successor runtime lifecycle transition from {:?} to {state:?}",
                                lifecycle
                            ),
                        });
                    }
                };
                lifecycle = Some(next);
            }
            JournalEntry::SuccessorHandoff(handoff)
                if handoff.campaign_id == *parent.campaign_id()
                    && handoff.node_id == parent.node_id()
                    && handoff.runtime_id == runtime =>
            {
                let Some(SuccessorLifecycle::Ready(receipt)) = lifecycle.take() else {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail:
                            "successor handoff acknowledgement did not follow exact Ready evidence"
                                .to_string(),
                    });
                };
                let acceptance = handoff.acceptance.clone().ok_or_else(|| {
                    PrepareError::InvalidBatchSelection {
                        detail: "successor handoff is missing its fenced Ready acceptance"
                            .to_string(),
                    }
                })?;
                if acceptance.ready() != &receipt {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: "successor handoff acceptance names a different Ready receipt"
                            .to_string(),
                    });
                }
                lifecycle = Some(SuccessorLifecycle::Acknowledged(acceptance));
            }
            _ => {}
        }
    }
    let (index, spawned) = spawned.ok_or_else(|| PrepareError::InvalidBatchSelection {
        detail: "successor controller origin is missing the exact Spawned runtime record"
            .to_string(),
    })?;
    Ok((
        index,
        spawned,
        lifecycle.expect("Spawned evidence establishes a lifecycle"),
    ))
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

pub(crate) fn validate_fresh_session(repo_root: &Path) -> Result<(), PrepareError> {
    let origin = reconstruct::reconstruct_at(repo_root, WalkPhase::R3)?;
    if origin.blocked.is_some()
        || origin.state.as_ref().map(EarlyState::phase) != Some(WalkPhase::R3)
    {
        let detail = origin.blocked.map_or_else(
            || {
                if origin.blockers.is_empty() {
                    "setup authority cannot reconstruct the R3 controller origin".to_string()
                } else {
                    origin.blockers.join("; ")
                }
            },
            |blocked| blocked.detail,
        );
        return Err(PrepareError::InvalidBatchSelection { detail });
    }

    let latest = reconstruct::reconstruct_early(repo_root)?;
    if let Some(blocked) = latest.blocked {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "cannot create a controller session over blocked historical state at {}: {}",
                blocked.phase, blocked.detail
            ),
        });
    }
    let phase = latest.state.as_ref().map(EarlyState::phase);
    if phase.is_some_and(has_loop_effects) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "cannot create a fresh controller session at R3 because pre-session loop artifacts reconstruct through {}; preserve this run as read-only or migrate it explicitly",
                phase.expect("checked phase")
            ),
        });
    }
    Ok(())
}

const fn has_loop_effects(phase: WalkPhase) -> bool {
    matches!(
        phase,
        WalkPhase::R5
            | WalkPhase::R6
            | WalkPhase::R7
            | WalkPhase::R8
            | WalkPhase::R9
            | WalkPhase::R10
            | WalkPhase::R11a
            | WalkPhase::R11
            | WalkPhase::R12
            | WalkPhase::R13a
            | WalkPhase::R13b
            | WalkPhase::R13c
            | WalkPhase::R14a
            | WalkPhase::R14b
    )
}

/// Reconstruct, admit, execute, and persist exactly one typed edge.
pub(crate) async fn advance_controlled(
    lease: Lease<Idle>,
    intent: AttemptIntent,
) -> Result<ControlAdvance, ControlFailure> {
    if let Some(receipt) = lease.existing_receipt(&intent) {
        return Ok(ControlAdvance::Existing { lease, receipt });
    }
    let repo_root = lease.epoch().repo_root.clone();
    let handoff = lease.handoff_path().map(Path::to_path_buf);
    let snapshot = match handoff.as_deref() {
        Some(path) => reconstruct::reconstruct_handoff_at(&repo_root, intent.expected, path),
        None => reconstruct::reconstruct_at(&repo_root, intent.expected),
    };
    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(source) => return Err(ControlFailure::Reconstruct { lease, source }),
    };
    let state = match snapshot.state {
        Some(state) => ControlState::from_reconstructed(state),
        None => {
            let detail = snapshot.blocked.map_or_else(
                || {
                    if snapshot.blockers.is_empty() {
                        format!(
                            "durable reconstruction did not produce typed authority at {}",
                            intent.expected
                        )
                    } else {
                        snapshot.blockers.join("; ")
                    }
                },
                |blocked| blocked.detail,
            );
            return Err(ControlFailure::Reconstruct {
                lease,
                source: PrepareError::InvalidBatchSelection { detail },
            });
        }
    };
    let permit = ControlPermit::new(&lease, &intent);
    let pending = match lease.admit(&permit, intent) {
        Ok(Begin::Existing { lease, receipt }) => {
            return Ok(ControlAdvance::Existing { lease, receipt });
        }
        Ok(Begin::Started { lease, .. }) => lease,
        Err(failure) => return Err(ControlFailure::Admission(failure)),
    };

    let (finished, state) = match advance_admitted(&repo_root, state, &permit).await {
        Ok(step) => {
            let (state, effect) = step.into_parts();
            pending.commit(effect).map(|finished| (finished, state))
        }
        Err(failure) => {
            let StepFailure { state, error } = failure;
            let detail = error.to_string();
            if matches!(state, ControlState::Failed { .. }) {
                pending
                    .mark_indeterminate(&permit, detail)
                    .map(|finished| (finished, state))
            } else {
                pending
                    .reject(&permit, detail)
                    .map(|finished| (finished, state))
            }
        }
    }
    .map_err(ControlFailure::Persist)?;
    Ok(ControlAdvance::Finished { finished, state })
}

fn session_error(source: impl std::fmt::Display) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: format!("controller session error: {source}"),
    }
}

async fn advance_admitted(
    repo_root: &Path,
    state: ControlState,
    permit: &ControlPermit,
) -> Result<ControlStep, StepFailure> {
    let source = state.phase();
    if permit.repo_root() != repo_root || permit.expected() != source {
        return retained(
            state,
            "control permit does not match reconstructed transition authority",
        );
    }
    advance(repo_root, state, permit).await
}

async fn advance(
    repo_root: &Path,
    state: ControlState,
    permit: &ControlPermit,
) -> Result<ControlStep, StepFailure> {
    let admission = StepAdmission::new(permit.allow_live_api(), permit.allow_git_changes());
    let previous = state.phase();
    let next = match state {
        ControlState::Empty => {
            return retained(
                ControlState::Empty,
                "walk has not been started; run start first",
            );
        }
        ControlState::Failed { phase, detail } => {
            return retained(
                ControlState::Failed { phase, detail },
                "walk is failed; start a new walk to continue",
            );
        }
        ControlState::Blocked { phase, detail } => {
            let message = format!(
                "walk is blocked at {phase}; repair the durable state or run reset before starting a different walk: {detail}"
            );
            return retained(ControlState::Blocked { phase, detail }, message);
        }
        ControlState::R0(r0) => r0.advance(r0_to_r1).map(ControlState::R1),
        ControlState::R1(r1) => r1.advance(r1_to_r2a_or_r3).map(|branch| match branch {
            typestate::R1Branch::R2a(r2a) => ControlState::R2a(r2a),
            typestate::R1Branch::R3(r3) => ControlState::R3(r3),
        }),
        ControlState::R2a(r2a) => r2a.advance(r2a_to_r3).map(ControlState::R3),
        ControlState::R3(r3) => r3.advance(r3_to_r4a).map(ControlState::R4a),
        ControlState::R4a(r4a) => r4a.advance(r4a_to_r4b_or_r4c).map(|branch| match branch {
            typestate::R4aStartupBranch::GenesisChecked(r4b) => ControlState::R4b(r4b),
            typestate::R4aStartupBranch::PredecessorReady(r4c) => ControlState::R4c(r4c),
        }),
        ControlState::R4b(r4b) => r4b.advance(r4b_to_r4c_genesis).map(ControlState::R4c),
        ControlState::R4c(r4c) => r4c.advance(r4c_to_r5).map(ControlState::R5),
        ControlState::R5(r5) => {
            if admission.live {
                r5.advance_async(r5_to_r6).await.map(ControlState::R6)
            } else {
                return retained(
                    ControlState::R5(r5),
                    "walk reached R5 baseline boundary; rerun with `--allow-live-api` to admit eval and protocol provider calls",
                );
            }
        }
        ControlState::R6(r6) => r6.advance(r6_to_r7).map(ControlState::R7),
        ControlState::R7(r7) => {
            if admission.live {
                let outer_attempt = OuterAttempt::new(permit.session_id(), permit.transition_id());
                r7.advance_async(|r7| r7_to_r8_linked(r7, outer_attempt))
                    .await
                    .map(ControlState::R8)
            } else {
                return retained(
                    ControlState::R7(r7),
                    "walk reached R7 policy-ready boundary; rerun with `--allow-live-api` to admit the live R8 child-plan authority edge",
                );
            }
        }
        ControlState::R8(r8) => r8.advance(r8_to_r9).map(ControlState::R9),
        ControlState::R9(r9) => r9.advance(r9_to_r10).map(ControlState::R10),
        ControlState::R10(r10) => {
            if admission.live {
                r10.advance_async(r10_to_r11)
                    .await
                    .map(|branch| match branch {
                        typestate::R10FanoutBranch::RejectedOnly(r11a) => ControlState::R11a(r11a),
                        typestate::R10FanoutBranch::FanoutComplete(r11) => ControlState::R11(r11),
                    })
            } else {
                return retained(
                    ControlState::R10(r10),
                    "walk reached R10 selection-strategy boundary; rerun with `--allow-live-api` to admit the live R11 rejected-only/fanout edge",
                );
            }
        }
        ControlState::R11a(r11a) => {
            r11_to_r12(typestate::R10FanoutBranch::RejectedOnly(r11a)).map(ControlState::R12)
        }
        ControlState::R11(r11) => {
            r11_to_r12(typestate::R10FanoutBranch::FanoutComplete(r11)).map(ControlState::R12)
        }
        ControlState::R12(r12) => {
            let handoff = match r12.preview_continuation() {
                Ok(decision) => {
                    decision.is_some_and(|decision| decision.disposition.allows_successor())
                }
                Err(error) => {
                    return Err(StepFailure {
                        state: ControlState::R12(r12),
                        error,
                    });
                }
            };
            if handoff && !admission.checkout {
                return retained(
                    ControlState::R12(r12),
                    "walk R13b handoff installs the selected successor into the active checkout; rerun with `--allow git-changes`",
                );
            }
            r12.advance(|r12| r12_to_r13(r12, permit))
                .map(|branch| match branch {
                    typestate::R12ContinuationBranch::Stopped(r13a) => ControlState::R13a(r13a),
                    typestate::R12ContinuationBranch::HandoffCommitted(r13b) => {
                        ControlState::R13b(r13b)
                    }
                    typestate::R12ContinuationBranch::HandoffIncomplete(r13c) => {
                        ControlState::R13c(r13c)
                    }
                })
        }
        ControlState::R13a(r13a) => r13_to_r14(typestate::R12ContinuationBranch::Stopped(r13a))
            .map(|branch| match branch {
                typestate::R14FinalBranch::Stopped(r14a) => ControlState::R14a(r14a),
                typestate::R14FinalBranch::Handoff(_) => {
                    unreachable!("R13a stopped branch cannot produce handoff final state")
                }
            }),
        ControlState::R13b(r13b) => {
            r13_to_r14(typestate::R12ContinuationBranch::HandoffCommitted(r13b)).map(|branch| {
                match branch {
                    typestate::R14FinalBranch::Stopped(_) => {
                        unreachable!("R13b handoff branch cannot produce stopped final state")
                    }
                    typestate::R14FinalBranch::Handoff(r14b) => ControlState::R14b(r14b),
                }
            })
        }
        ControlState::R13c(r13c) => {
            return retained(
                ControlState::R13c(r13c),
                "walk reached R13c with the predecessor retired and successor handoff incomplete; inspect and reconcile durable handoff evidence before continuing",
            );
        }
        ControlState::R14a(r14a) => {
            return retained(
                ControlState::R14a(r14a),
                "walk reached R14a final stopped-report boundary",
            );
        }
        ControlState::R14b(r14b) => {
            return retained(
                ControlState::R14b(r14b),
                "walk reached R14b final successor-handoff report boundary",
            );
        }
    };

    match next {
        Ok(state) => {
            let transition = ControlTransition {
                from: previous,
                to: state.phase(),
            };
            let edge = ControlEdge::from_phases(transition.from, transition.to)
                .expect("every successful typed advance has a stable semantic edge");
            Ok(ControlStep {
                state,
                effect: ControlEffect {
                    session_id: permit.session_id,
                    fence: permit.fence,
                    repo_root: repo_root.to_path_buf(),
                    transition_id: permit.transition_id,
                    transition,
                    witness: effect_witness(
                        permit.session_id,
                        permit.fence,
                        permit.transition_id,
                        edge,
                    ),
                },
            })
        }
        Err(error) => {
            let error = if previous == WalkPhase::R4a {
                PrepareError::DatabaseSetup {
                    phase: "prototype1_parent_checkout",
                    detail: reconstruct::format_r4a_blocker(repo_root, &error),
                }
            } else {
                error
            };
            let detail = error.to_string();
            Err(StepFailure {
                state: ControlState::Failed {
                    phase: previous,
                    detail,
                },
                error,
            })
        }
    }
}

fn effect_witness(
    session_id: SessionId,
    fence: Fence,
    transition_id: TransitionId,
    edge: ControlEdge,
) -> ContentHash {
    let stamp = serde_json::to_string(&(EFFECT_SCHEMA, session_id, fence, transition_id, edge))
        .expect("fixed control-effect stamp is serializable");
    ContentHash::of(&stamp)
}

fn retained<T>(state: ControlState, detail: T) -> Result<ControlStep, StepFailure>
where
    T: Into<String>,
{
    let detail = detail.into();
    Err(StepFailure {
        state,
        error: PrepareError::InvalidBatchSelection { detail },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::prototype1_state::invocation::ProcessIncarnation;

    #[test]
    fn abandonment_checks_incarnation() {
        let repo = tempfile::tempdir().expect("temp repo");
        let pid = std::process::id();
        let incarnation = invocation::process_incarnation(pid)
            .expect("inspect current process")
            .expect("current process incarnation");
        let cause = |incarnation| RecoveryCause::OwnerLost {
            session_id: SessionId::for_test(1),
            fence: Fence::for_test(1),
            pid,
            incarnation: Some(incarnation),
            runtime_id: None,
            epoch: ServerEpoch::capture(repo.path()).expect("capture epoch"),
        };

        let error = ensure_owner_gone(&cause(incarnation.clone()))
            .expect_err("live owner incarnation must block abandonment");
        assert!(
            error
                .to_string()
                .contains("still has its recorded incarnation")
        );

        let reused = ProcessIncarnation {
            boot_id: incarnation.boot_id,
            start_ticks: incarnation.start_ticks.saturating_add(1),
        };
        ensure_owner_gone(&cause(reused)).expect("different incarnation proves old owner gone");
    }
}
