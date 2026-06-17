//! In-memory typestate walker used by the local debug server.
//!
//! `WalkController` owns exactly one `WalkState` value. Each step consumes that
//! state and calls the canonical direct edge functions from `live_edges`; this
//! module should not duplicate transition semantics.

use crate::{
    ResolvedCampaignConfig,
    cli::prototype1_state::{
        cli_facing::Prototype1StateRunShape,
        live_edges::{r0_to_r1, r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis},
        typestate::{self, R0, R1, R2a, R3, R4a, R4bGenesisChecked, R4cReady, StepInput},
    },
    spec::PrepareError,
};

use super::{phase::WalkPhase, protocol::WalkStartConfig};

type RunShape = Prototype1StateRunShape;
type CampaignConfig = ResolvedCampaignConfig;

/// Single-session in-memory controller for early Prototype 1 typestate phases.
///
/// The first server slice admits only setup/startup phases through `R4c` so the
/// socket lifecycle can be tested before exposing child fanout or handoff.
pub(crate) struct WalkController {
    state: WalkState,
    steps: usize,
}

/// Owned typestate value currently held by the server.
///
/// The enum is intentionally private: external callers address state through
/// `WalkPhase`, while only the controller can consume and replace typed values.
enum WalkState {
    Empty,
    R0(R0),
    R1(R1<RunShape, CampaignConfig>),
    R2a(R2a<RunShape, CampaignConfig>),
    R3(R3<RunShape, CampaignConfig>),
    R4a(R4a<RunShape, CampaignConfig>),
    R4b(R4bGenesisChecked<RunShape, CampaignConfig>),
    R4c(R4cReady<RunShape, CampaignConfig>),
    /// A consuming transition failed after the previous typed value was moved.
    ///
    /// Rust cannot restore the consumed value after an edge returns `Err`, so
    /// the server keeps an inspectable failed cursor and requires a fresh walk.
    Failed {
        phase: WalkPhase,
        detail: String,
    },
}

impl WalkController {
    /// Create an empty controller with no active walk.
    pub(crate) fn new() -> Self {
        Self {
            state: WalkState::Empty,
            steps: 0,
        }
    }

    /// Return the current protocol-visible phase cursor.
    pub(crate) fn phase(&self) -> WalkPhase {
        self.state.phase()
    }

    /// Produce a short human-readable summary for `show` and `health`.
    pub(crate) fn describe(&self) -> String {
        match &self.state {
            WalkState::Failed { phase, detail } => format!(
                "prototype1-state walk failed while advancing from {phase}; steps={}; detail={detail}",
                self.steps
            ),
            _ => format!(
                "prototype1-state walk is at {}; steps={}",
                self.phase(),
                self.steps
            ),
        }
    }

    /// Start a new walk from `R0` and optionally advance to an early boundary.
    pub(crate) fn start(
        &mut self,
        config: WalkStartConfig,
        until: WalkPhase,
    ) -> Result<WalkPhase, PrepareError> {
        if !matches!(self.state, WalkState::Empty | WalkState::Failed { .. }) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "prototype1-state walk is already started at {}; stop and restart the server to begin a new walk",
                    self.phase()
                ),
            });
        }
        ensure_supported_target(until)?;
        self.state = WalkState::R0(typestate::R0::new(config.into_state_command()));
        self.steps = 0;
        self.advance_until(until)?;
        Ok(self.phase())
    }

    /// Advance the current walk by one edge or until a requested early phase.
    pub(crate) fn step(&mut self, until: Option<WalkPhase>) -> Result<WalkPhase, PrepareError> {
        let target = until.unwrap_or_else(|| self.phase().next().unwrap_or(self.phase()));
        ensure_supported_target(target)?;
        if self.phase() == target {
            return Ok(self.phase());
        }
        if until.is_some() {
            self.advance_until(target)?;
        } else {
            self.step_once()?;
        }
        Ok(self.phase())
    }

    fn advance_until(&mut self, target: WalkPhase) -> Result<(), PrepareError> {
        let mut guard = 0_u8;
        while self.phase() != target {
            guard += 1;
            if guard > 16 {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "prototype1-state walk exceeded early-step guard while advancing to {target}"
                    ),
                });
            }
            self.step_once()?;
        }
        Ok(())
    }

    fn step_once(&mut self) -> Result<(), PrepareError> {
        let state = std::mem::replace(&mut self.state, WalkState::Empty);
        let previous = state.phase();
        let next = match state {
            WalkState::Empty => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "prototype1-state walk has not been started; run start first"
                        .to_string(),
                });
            }
            WalkState::Failed { phase, detail } => {
                self.state = WalkState::Failed { phase, detail };
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "prototype1-state walk is failed; start a new walk to continue"
                        .to_string(),
                });
            }
            WalkState::R0(r0) => r0.advance(r0_to_r1).map(WalkState::R1),
            WalkState::R1(r1) => r1.advance(r1_to_r2a_or_r3).map(|branch| match branch {
                typestate::R1Branch::R2a(r2a) => WalkState::R2a(r2a),
                typestate::R1Branch::R3(r3) => WalkState::R3(r3),
            }),
            WalkState::R2a(r2a) => {
                self.state = WalkState::R2a(r2a);
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "prototype1-state walk reached R2a parent-identity initialization boundary; no next early step is defined".to_string(),
                });
            }
            WalkState::R3(r3) => r3.advance(r3_to_r4a).map(WalkState::R4a),
            WalkState::R4a(r4a) => r4a.advance(r4a_to_r4b_or_r4c).map(|branch| match branch {
                typestate::R4aStartupBranch::GenesisChecked(r4b) => WalkState::R4b(r4b),
                typestate::R4aStartupBranch::PredecessorReady(r4c) => WalkState::R4c(r4c),
            }),
            WalkState::R4b(r4b) => r4b.advance(r4b_to_r4c_genesis).map(WalkState::R4c),
            WalkState::R4c(r4c) => {
                self.state = WalkState::R4c(r4c);
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "prototype1-state walk reached R4c ready-parent boundary; later live phases are not admitted by this debug server slice yet".to_string(),
                });
            }
        };
        match next {
            Ok(state) => {
                self.state = state;
                self.steps += 1;
                Ok(())
            }
            Err(error) => {
                let detail = error.to_string();
                self.state = WalkState::Failed {
                    phase: previous,
                    detail,
                };
                Err(error)
            }
        }
    }
}

impl WalkState {
    fn phase(&self) -> WalkPhase {
        match self {
            WalkState::Empty => WalkPhase::Empty,
            WalkState::R0(_) => WalkPhase::R0,
            WalkState::R1(_) => WalkPhase::R1,
            WalkState::R2a(_) => WalkPhase::R2a,
            WalkState::R3(_) => WalkPhase::R3,
            WalkState::R4a(_) => WalkPhase::R4a,
            WalkState::R4b(_) => WalkPhase::R4b,
            WalkState::R4c(_) => WalkPhase::R4c,
            WalkState::Failed { phase, .. } => *phase,
        }
    }
}

trait NextPhase {
    fn next(self) -> Option<WalkPhase>;
}

impl NextPhase for WalkPhase {
    fn next(self) -> Option<WalkPhase> {
        match self {
            WalkPhase::Empty => Some(WalkPhase::R0),
            WalkPhase::R0 => Some(WalkPhase::R1),
            WalkPhase::R1 => Some(WalkPhase::R3),
            WalkPhase::R2a => None,
            WalkPhase::R3 => Some(WalkPhase::R4a),
            WalkPhase::R4a => Some(WalkPhase::R4c),
            WalkPhase::R4b => Some(WalkPhase::R4c),
            WalkPhase::R4c => None,
        }
    }
}

fn ensure_supported_target(target: WalkPhase) -> Result<(), PrepareError> {
    if target.is_early_boundary() {
        Ok(())
    } else {
        Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1-state walk target {target} is not supported by the early server slice"
            ),
        })
    }
}
