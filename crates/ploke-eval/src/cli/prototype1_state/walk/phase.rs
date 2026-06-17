//! Phase vocabulary used by the debug walk server protocol.
//!
//! These values are runtime cursors for CLI/server communication. They are not
//! replacements for the real Rust typestate aliases; the controller maps each
//! phase to an owned `WalkState` variant carrying the corresponding typed value.

use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::cli::prototype1_state::typestate::{
    R0_SHAPE, R1_SHAPE, R2A_SHAPE, R3_SHAPE, R4A_SHAPE, R4B_SHAPE, R4C_SHAPE, R5_SHAPE,
    RuntimeShape,
};

/// Serializable cursor for the early Prototype 1 typestate walk.
///
/// The current server slice intentionally stops at `R5`: parent-start evidence
/// is enough to validate socket lifecycle, in-memory stepping, branching,
/// stale-server guards, and the first journal-writing live edge before child
/// fanout or successor handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WalkPhase {
    /// No in-memory walk has been started.
    Empty,
    /// Raw `Prototype1StateCommand` captured as `typestate::R0`.
    R0,
    /// Command-derived campaign/run context collected.
    R1,
    /// Parent identity initialization branch completed.
    R2a,
    /// Existing parent identity resolved for a normal turn.
    R3,
    /// `Parent<Unchecked>` loaded for startup validation.
    R4a,
    /// Genesis startup checkout validated, before `Parent<Ready>`.
    R4b,
    /// Unified `Parent<Ready>` boundary for genesis or successor startup.
    R4c,
    /// Parent-start evidence recorded after `Parent<Ready>` is proven.
    R5,
}

/// One admitted edge that can follow a phase in the current server slice.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WalkNextStep {
    pub(crate) edge: &'static str,
    pub(crate) phase: WalkPhase,
    pub(crate) detail: &'static str,
}

const EMPTY_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "start",
    phase: WalkPhase::R0,
    detail: "create the initial command carrier",
}];

const R0_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r0_to_r1",
    phase: WalkPhase::R1,
    detail: "collect repo, campaign, and run context",
}];

const R1_NEXT: &[WalkNextStep] = &[
    WalkNextStep {
        edge: "r1_to_r2a_or_r3",
        phase: WalkPhase::R2a,
        detail: "initialize parent identity",
    },
    WalkNextStep {
        edge: "r1_to_r2a_or_r3",
        phase: WalkPhase::R3,
        detail: "resolve existing parent identity",
    },
];

const R3_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r3_to_r4a",
    phase: WalkPhase::R4a,
    detail: "load parent as unchecked",
}];

const R4A_NEXT: &[WalkNextStep] = &[
    WalkNextStep {
        edge: "r4a_to_r4b_or_r4c",
        phase: WalkPhase::R4b,
        detail: "genesis checkout validated",
    },
    WalkNextStep {
        edge: "r4a_to_r4b_or_r4c",
        phase: WalkPhase::R4c,
        detail: "predecessor startup ready",
    },
];

const R4B_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r4b_to_r4c_genesis",
    phase: WalkPhase::R4c,
    detail: "converge genesis startup to ready parent",
}];

const R4C_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r4c_to_r5",
    phase: WalkPhase::R5,
    detail: "record parent-start evidence",
}];

const NO_NEXT: &[WalkNextStep] = &[];

impl WalkPhase {
    /// Stable lowercase spelling used by table output and `Display`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            WalkPhase::Empty => "empty",
            WalkPhase::R0 => "r0",
            WalkPhase::R1 => "r1",
            WalkPhase::R2a => "r2a",
            WalkPhase::R3 => "r3",
            WalkPhase::R4a => "r4a",
            WalkPhase::R4b => "r4b",
            WalkPhase::R4c => "r4c",
            WalkPhase::R5 => "r5",
        }
    }

    /// Short human label for the phase.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            WalkPhase::Empty => "no active walk",
            WalkPhase::R0 => "command captured",
            WalkPhase::R1 => "run context collected",
            WalkPhase::R2a => "parent identity initialized",
            WalkPhase::R3 => "parent identity resolved",
            WalkPhase::R4a => "parent unchecked",
            WalkPhase::R4b => "genesis checked",
            WalkPhase::R4c => "parent ready",
            WalkPhase::R5 => "parent start recorded",
        }
    }

    /// Returns whether this target is admitted by the current server slice.
    pub(crate) fn is_early_boundary(self) -> bool {
        matches!(
            self,
            WalkPhase::R0
                | WalkPhase::R1
                | WalkPhase::R2a
                | WalkPhase::R3
                | WalkPhase::R4a
                | WalkPhase::R4b
                | WalkPhase::R4c
                | WalkPhase::R5
        )
    }

    /// Admitted next edges from this phase.
    pub(crate) fn next_steps(self) -> &'static [WalkNextStep] {
        match self {
            WalkPhase::Empty => EMPTY_NEXT,
            WalkPhase::R0 => R0_NEXT,
            WalkPhase::R1 => R1_NEXT,
            WalkPhase::R2a => NO_NEXT,
            WalkPhase::R3 => R3_NEXT,
            WalkPhase::R4a => R4A_NEXT,
            WalkPhase::R4b => R4B_NEXT,
            WalkPhase::R4c => R4C_NEXT,
            WalkPhase::R5 => NO_NEXT,
        }
    }

    /// Canonical edge name for a transition between two admitted phases.
    pub(crate) fn edge_from(self, from: WalkPhase) -> Option<&'static str> {
        self.next_from(from).map(|step| step.edge)
    }

    /// Human typestate deltas for a transition between two admitted phases.
    pub(crate) fn changes_from(self, from: WalkPhase) -> Vec<String> {
        let mut deltas = match (from.shape(), self.shape()) {
            (Some(from), Some(to)) => to.render_deltas_from(from),
            (None, Some(_)) | (Some(_), None) => vec![format!("phase: {from} -> {self}")],
            (None, None) => Vec::new(),
        };
        if matches!((from, self), (WalkPhase::R4c, WalkPhase::R5)) {
            deltas.push(
                "side effect: appends parent-start/resource entries to the transition journal"
                    .to_string(),
            );
        }
        if deltas.is_empty() {
            deltas.push("no admitted typestate delta for this phase pair".to_string());
        }
        deltas
    }

    /// Human-readable Rust typestate alias for the current phase.
    pub(crate) fn typestate(self) -> String {
        self.shape()
            .map(RuntimeShape::render)
            .unwrap_or_else(|| "(no Runtime value is held yet)".to_string())
    }

    fn shape(self) -> Option<RuntimeShape> {
        match self {
            WalkPhase::Empty => None,
            WalkPhase::R0 => Some(R0_SHAPE),
            WalkPhase::R1 => Some(R1_SHAPE),
            WalkPhase::R2a => Some(R2A_SHAPE),
            WalkPhase::R3 => Some(R3_SHAPE),
            WalkPhase::R4a => Some(R4A_SHAPE),
            WalkPhase::R4b => Some(R4B_SHAPE),
            WalkPhase::R4c => Some(R4C_SHAPE),
            WalkPhase::R5 => Some(R5_SHAPE),
        }
    }

    fn next_from(self, from: WalkPhase) -> Option<&'static WalkNextStep> {
        from.next_steps().iter().find(|step| step.phase == self)
    }
}

impl fmt::Display for WalkPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
