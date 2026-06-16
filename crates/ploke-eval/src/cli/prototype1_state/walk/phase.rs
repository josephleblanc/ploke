use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WalkPhase {
    Empty,
    R0,
    R1,
    R2a,
    R3,
    R4a,
    R4b,
    R4c,
}

impl WalkPhase {
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
