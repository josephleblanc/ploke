//! Stable semantic identity for each concrete parent typestate edge.
//!
//! Operator command hints such as `--watch` and `--allow git-changes` are not
//! transition identity. Durable session evidence uses this closed vocabulary
//! so UI formatting can change without changing historical cursor hashes.

use serde::{Deserialize, Serialize};

use super::walk::phase::WalkPhase;

pub(crate) const GRAPH_VERSION_V1: &str = "walk-r0-r14a-v1";
pub(crate) const GRAPH_VERSION_V2: &str = "walk-r0-r14a-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlEdge {
    StartToR0,
    R0ToR1,
    R1ToR2a,
    R1ToR3,
    R2aToR3,
    R3ToR4a,
    R4aToR4b,
    R4aToR4c,
    R4bToR4c,
    R4cToR5,
    R5ToR6,
    R6ToR7,
    R7ToR8,
    R8ToR9,
    R9ToR10,
    R10ToR11a,
    R10ToR11,
    R11aToR12,
    R11ToR12,
    R12ToR13a,
    R12ToR13b,
    R12ToR13c,
    R13aToR14a,
    R13bToR14b,
}

impl ControlEdge {
    pub(crate) const ALL: [Self; 24] = [
        Self::StartToR0,
        Self::R0ToR1,
        Self::R1ToR2a,
        Self::R1ToR3,
        Self::R2aToR3,
        Self::R3ToR4a,
        Self::R4aToR4b,
        Self::R4aToR4c,
        Self::R4bToR4c,
        Self::R4cToR5,
        Self::R5ToR6,
        Self::R6ToR7,
        Self::R7ToR8,
        Self::R8ToR9,
        Self::R9ToR10,
        Self::R10ToR11a,
        Self::R10ToR11,
        Self::R11aToR12,
        Self::R11ToR12,
        Self::R12ToR13a,
        Self::R12ToR13b,
        Self::R12ToR13c,
        Self::R13aToR14a,
        Self::R13bToR14b,
    ];

    pub const fn from(self) -> WalkPhase {
        match self {
            Self::StartToR0 => WalkPhase::Empty,
            Self::R0ToR1 => WalkPhase::R0,
            Self::R1ToR2a | Self::R1ToR3 => WalkPhase::R1,
            Self::R2aToR3 => WalkPhase::R2a,
            Self::R3ToR4a => WalkPhase::R3,
            Self::R4aToR4b | Self::R4aToR4c => WalkPhase::R4a,
            Self::R4bToR4c => WalkPhase::R4b,
            Self::R4cToR5 => WalkPhase::R4c,
            Self::R5ToR6 => WalkPhase::R5,
            Self::R6ToR7 => WalkPhase::R6,
            Self::R7ToR8 => WalkPhase::R7,
            Self::R8ToR9 => WalkPhase::R8,
            Self::R9ToR10 => WalkPhase::R9,
            Self::R10ToR11a | Self::R10ToR11 => WalkPhase::R10,
            Self::R11aToR12 => WalkPhase::R11a,
            Self::R11ToR12 => WalkPhase::R11,
            Self::R12ToR13a | Self::R12ToR13b | Self::R12ToR13c => WalkPhase::R12,
            Self::R13aToR14a => WalkPhase::R13a,
            Self::R13bToR14b => WalkPhase::R13b,
        }
    }

    pub const fn to(self) -> WalkPhase {
        match self {
            Self::StartToR0 => WalkPhase::R0,
            Self::R0ToR1 => WalkPhase::R1,
            Self::R1ToR2a => WalkPhase::R2a,
            Self::R1ToR3 | Self::R2aToR3 => WalkPhase::R3,
            Self::R3ToR4a => WalkPhase::R4a,
            Self::R4aToR4b => WalkPhase::R4b,
            Self::R4aToR4c | Self::R4bToR4c => WalkPhase::R4c,
            Self::R4cToR5 => WalkPhase::R5,
            Self::R5ToR6 => WalkPhase::R6,
            Self::R6ToR7 => WalkPhase::R7,
            Self::R7ToR8 => WalkPhase::R8,
            Self::R8ToR9 => WalkPhase::R9,
            Self::R9ToR10 => WalkPhase::R10,
            Self::R10ToR11a => WalkPhase::R11a,
            Self::R10ToR11 => WalkPhase::R11,
            Self::R11aToR12 | Self::R11ToR12 => WalkPhase::R12,
            Self::R12ToR13a => WalkPhase::R13a,
            Self::R12ToR13b => WalkPhase::R13b,
            Self::R12ToR13c => WalkPhase::R13c,
            Self::R13aToR14a => WalkPhase::R14a,
            Self::R13bToR14b => WalkPhase::R14b,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::StartToR0 => "start_to_r0",
            Self::R0ToR1 => "r0_to_r1",
            Self::R1ToR2a => "r1_to_r2a",
            Self::R1ToR3 => "r1_to_r3",
            Self::R2aToR3 => "r2a_to_r3",
            Self::R3ToR4a => "r3_to_r4a",
            Self::R4aToR4b => "r4a_to_r4b",
            Self::R4aToR4c => "r4a_to_r4c",
            Self::R4bToR4c => "r4b_to_r4c",
            Self::R4cToR5 => "r4c_to_r5",
            Self::R5ToR6 => "r5_to_r6",
            Self::R6ToR7 => "r6_to_r7",
            Self::R7ToR8 => "r7_to_r8",
            Self::R8ToR9 => "r8_to_r9",
            Self::R9ToR10 => "r9_to_r10",
            Self::R10ToR11a => "r10_to_r11a",
            Self::R10ToR11 => "r10_to_r11",
            Self::R11aToR12 => "r11a_to_r12",
            Self::R11ToR12 => "r11_to_r12",
            Self::R12ToR13a => "r12_to_r13a",
            Self::R12ToR13b => "r12_to_r13b",
            Self::R12ToR13c => "r12_to_r13c",
            Self::R13aToR14a => "r13a_to_r14a",
            Self::R13bToR14b => "r13b_to_r14b",
        }
    }

    pub const fn requires_live(self) -> bool {
        matches!(
            self,
            Self::R5ToR6 | Self::R7ToR8 | Self::R10ToR11a | Self::R10ToR11
        )
    }

    pub const fn requires_checkout(self) -> bool {
        matches!(self, Self::R12ToR13b | Self::R12ToR13c)
    }

    pub fn from_phases(from: WalkPhase, to: WalkPhase) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|edge| edge.from() == from && edge.to() == to)
    }

    /// Resolve an edge through the retained graph table that admitted it.
    pub(crate) fn for_graph(graph: &str, from: WalkPhase, to: WalkPhase) -> Option<Self> {
        let edge = Self::from_phases(from, to)?;
        match graph {
            GRAPH_VERSION_V2 => Some(edge),
            GRAPH_VERSION_V1 if !matches!(edge, Self::R2aToR3 | Self::R12ToR13c) => Some(edge),
            _ => None,
        }
    }
}

impl std::fmt::Display for ControlEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::cli::prototype1_state::walk::epoch::TRANSITION_GRAPH_VERSION;

    use super::*;

    #[test]
    fn ids_and_serialized_spellings_are_stable_and_unique() {
        let mut ids = BTreeSet::new();
        for edge in ControlEdge::ALL {
            assert!(ids.insert(edge.id()), "duplicate edge id {}", edge.id());
            assert_eq!(
                serde_json::to_string(&edge).expect("serialize edge"),
                format!("\"{}\"", edge.id())
            );
            assert_eq!(ControlEdge::from_phases(edge.from(), edge.to()), Some(edge));
        }
    }

    #[test]
    fn retained_graph_table_does_not_invent_new_edges() {
        assert_eq!(TRANSITION_GRAPH_VERSION, GRAPH_VERSION_V2);
        assert_eq!(
            ControlEdge::for_graph(GRAPH_VERSION_V1, WalkPhase::R12, WalkPhase::R13c,),
            None
        );
        assert_eq!(
            ControlEdge::for_graph(TRANSITION_GRAPH_VERSION, WalkPhase::R12, WalkPhase::R13c,),
            Some(ControlEdge::R12ToR13c)
        );
    }

    #[test]
    fn provider_edges_require_live_capability() {
        assert!(ControlEdge::R5ToR6.requires_live());
        assert!(ControlEdge::R7ToR8.requires_live());
        assert!(ControlEdge::R10ToR11.requires_live());
        assert!(!ControlEdge::R12ToR13b.requires_live());
        assert!(!ControlEdge::R4cToR5.requires_live());
        assert!(!ControlEdge::R6ToR7.requires_live());
    }
}
