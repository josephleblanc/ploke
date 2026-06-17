//! Phase vocabulary used by the debug walk server protocol.
//!
//! These values are runtime cursors for CLI/server communication. They are not
//! replacements for the real Rust typestate aliases; the controller maps each
//! phase to an owned `WalkState` variant carrying the corresponding typed value.

use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Serializable cursor for the early Prototype 1 typestate walk.
///
/// The first server slice intentionally stops at `R4c`: ready-parent startup
/// is enough to validate socket lifecycle, in-memory stepping, branching, and
/// stale-server guards before admitting child fanout or successor handoff.
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
}

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
        }
    }

    /// Returns whether this target is admitted by the current early server slice.
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
        )
    }
}

impl fmt::Display for WalkPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
