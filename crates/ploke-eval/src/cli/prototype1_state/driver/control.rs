//! One owned Prototype 1 parent state and one meaning of a forward step.
//!
//! Batch, walk, and later service adapters own this same state sum and call
//! [`advance_one`]. Frontends may decide how many steps to request, but they do
//! not dispatch R-state edges themselves.

use std::path::Path;

use crate::{
    ResolvedCampaignConfig,
    cli::{Prototype1StateCommand, prototype1_state::cli_facing::Prototype1StateRunShape},
    spec::PrepareError,
};

use super::{
    super::{
        live_edges::{
            r0_to_r1, r1_to_r2a_or_r3, r2a_to_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis,
            r4c_to_r5, r5_to_r6, r6_to_r7, r7_to_r8, r8_to_r9, r9_to_r10, r10_to_r11, r11_to_r12,
            r12_to_r13, r13_to_r14,
        },
        typestate::{
            self, AsyncStepInput, R0, R1, R2a, R3, R4a, R4bGenesisChecked, R4cReady, R5, R6, R7,
            R8, R9, R10, R11FanoutComplete, R11aRejectedOnly, R12, R13aStopped,
            R13bHandoffCommitted, R13cHandoffIncomplete, R14aFinalStopped, R14bFinalHandoff,
            StepInput,
        },
        walk::phase::WalkPhase,
    },
    reconstruct::{self, EarlyState},
};

type RunShape = Prototype1StateRunShape;
type CampaignConfig = ResolvedCampaignConfig;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlTransition {
    pub(crate) from: WalkPhase,
    pub(crate) to: WalkPhase,
}

pub(crate) struct ControlStep {
    pub(crate) state: ControlState,
    pub(crate) transition: ControlTransition,
}

pub(crate) struct StepFailure {
    pub(crate) state: ControlState,
    pub(crate) error: PrepareError,
}

/// Consume exactly one state and dispatch exactly one direct typed edge.
pub(crate) async fn advance_one(
    repo_root: &Path,
    state: ControlState,
    admission: StepAdmission,
) -> Result<ControlStep, StepFailure> {
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
        ControlState::R5(r5) => r5.advance_async(r5_to_r6).await.map(ControlState::R6),
        ControlState::R6(r6) => r6.advance(r6_to_r7).map(ControlState::R7),
        ControlState::R7(r7) => {
            if admission.live {
                r7.advance_async(r7_to_r8).await.map(ControlState::R8)
            } else {
                return retained(
                    ControlState::R7(r7),
                    "walk reached R7 policy-ready boundary; rerun `walk step --watch` to admit the live R8 child-plan authority edge",
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
                    "walk reached R10 selection-strategy boundary; rerun `walk step --watch` to admit the live R11 rejected-only/fanout edge",
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
            if r12.has_successor_selection() && !admission.live {
                return retained(
                    ControlState::R12(r12),
                    "walk reached R12 with selected-successor evidence; rerun `walk step --watch --allow git-changes` to admit R13b handoff",
                );
            }
            if r12.has_successor_selection() && !admission.checkout {
                return retained(
                    ControlState::R12(r12),
                    "walk R13b handoff installs the selected successor into the active checkout; rerun with `--allow git-changes`",
                );
            }
            r12.advance(r12_to_r13).map(|branch| match branch {
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
        Ok(state) => Ok(ControlStep {
            transition: ControlTransition {
                from: previous,
                to: state.phase(),
            },
            state,
        }),
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
