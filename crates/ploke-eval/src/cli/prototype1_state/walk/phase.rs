//! Phase vocabulary used by the debug walk server protocol.
//!
//! These values are runtime cursors for CLI/server communication. They are not
//! replacements for the real Rust typestate aliases; the controller maps each
//! phase to an owned `WalkState` variant carrying the corresponding typed value.

use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::cli::prototype1_state::typestate::{
    R0_SHAPE, R1_SHAPE, R2A_SHAPE, R3_SHAPE, R4A_SHAPE, R4B_SHAPE, R4C_SHAPE, R5_SHAPE, R6_SHAPE,
    R7_SHAPE, R8_SHAPE, R9_SHAPE, R10_SHAPE, R11_SHAPE, R11A_SHAPE, R12_SHAPE, R13A_SHAPE,
    R14A_SHAPE, RuntimeAxisDelta, RuntimeShape,
};

/// Serializable cursor for the early Prototype 1 typestate walk.
///
/// The current server slice intentionally stops stepping at `R14a` after the
/// stopped/no-selection final report. That is enough to validate socket
/// lifecycle, in-memory stepping, branching, stale-server guards, and setup
/// edges before successor handoff.
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
    /// Parent baseline established or loaded.
    R6,
    /// Run policy and child-planning budget ready.
    R7,
    /// Existing child-plan authority received; parent is selectable.
    R8,
    /// Child schedule and budget are shaped.
    R9,
    /// Successor-selection strategy is ready.
    R10,
    /// Rejected-only selection evidence projected.
    R11a,
    /// Child fanout complete.
    R11,
    /// Report facts projected from rejected-only or child fanout evidence.
    R12,
    /// Continuation stopped without successor handoff.
    R13a,
    /// Final report emitted for stopped/no-selection continuation.
    R14a,
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

const R5_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r5_to_r6",
    phase: WalkPhase::R6,
    detail: "establish or load parent baseline",
}];

const R6_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r6_to_r7",
    phase: WalkPhase::R7,
    detail: "derive run policy and child-planning budget",
}];

const R7_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r7_to_r8 --watch",
    phase: WalkPhase::R8,
    detail: "resolve live child-plan authority; may wait on provider/harness work",
}];

const R8_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r8_to_r9",
    phase: WalkPhase::R9,
    detail: "shape child schedule and budget",
}];

const R9_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r9_to_r10",
    phase: WalkPhase::R10,
    detail: "resolve successor-selection strategy",
}];

const R10_NEXT: &[WalkNextStep] = &[
    WalkNextStep {
        edge: "r10_to_r11 --watch",
        phase: WalkPhase::R11a,
        detail: "project rejected-only selection evidence",
    },
    WalkNextStep {
        edge: "r10_to_r11 --watch",
        phase: WalkPhase::R11,
        detail: "run live child fanout and collect outcomes",
    },
];

const R11A_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r11_to_r12",
    phase: WalkPhase::R12,
    detail: "project rejected-only report facts",
}];

const R11_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r11_to_r12",
    phase: WalkPhase::R12,
    detail: "project child outcome report facts",
}];

const R12_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r12_to_r13",
    phase: WalkPhase::R13a,
    detail: "record no-selection stopped continuation; selected-successor handoff remains blocked",
}];

const R13A_NEXT: &[WalkNextStep] = &[WalkNextStep {
    edge: "r13_to_r14",
    phase: WalkPhase::R14a,
    detail: "emit stopped/no-selection final report",
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
            WalkPhase::R6 => "r6",
            WalkPhase::R7 => "r7",
            WalkPhase::R8 => "r8",
            WalkPhase::R9 => "r9",
            WalkPhase::R10 => "r10",
            WalkPhase::R11a => "r11a",
            WalkPhase::R11 => "r11",
            WalkPhase::R12 => "r12",
            WalkPhase::R13a => "r13a",
            WalkPhase::R14a => "r14a",
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
            WalkPhase::R6 => "parent baseline ready",
            WalkPhase::R7 => "policy and child budget ready",
            WalkPhase::R8 => "child-plan authority received",
            WalkPhase::R9 => "child schedule ready",
            WalkPhase::R10 => "selection strategy ready",
            WalkPhase::R11a => "rejected-only selection evidence ready",
            WalkPhase::R11 => "child fanout complete",
            WalkPhase::R12 => "report facts ready",
            WalkPhase::R13a => "stopped continuation ready",
            WalkPhase::R14a => "final stopped report emitted",
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
                | WalkPhase::R6
                | WalkPhase::R7
                | WalkPhase::R8
                | WalkPhase::R9
                | WalkPhase::R10
                | WalkPhase::R11a
                | WalkPhase::R11
                | WalkPhase::R12
                | WalkPhase::R13a
                | WalkPhase::R14a
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
            WalkPhase::R5 => R5_NEXT,
            WalkPhase::R6 => R6_NEXT,
            WalkPhase::R7 => R7_NEXT,
            WalkPhase::R8 => R8_NEXT,
            WalkPhase::R9 => R9_NEXT,
            WalkPhase::R10 => R10_NEXT,
            WalkPhase::R11a => R11A_NEXT,
            WalkPhase::R11 => R11_NEXT,
            WalkPhase::R12 => R12_NEXT,
            WalkPhase::R13a => R13A_NEXT,
            WalkPhase::R14a => NO_NEXT,
        }
    }

    /// Canonical edge name for a transition between two admitted phases.
    pub(crate) fn edge_from(self, from: WalkPhase) -> Option<&'static str> {
        self.next_from(from).map(|step| step.edge)
    }

    /// Human typestate deltas for a transition between two admitted phases.
    pub(crate) fn changes_from(self, from: WalkPhase) -> Vec<String> {
        let mut deltas = self
            .axis_deltas_from(from)
            .into_iter()
            .map(|delta| render_axis_delta(&delta))
            .collect::<Vec<_>>();
        if matches!((from, self), (WalkPhase::R4c, WalkPhase::R5)) {
            deltas.push(
                "side effect: appends parent-start/resource entries to the transition journal"
                    .to_string(),
            );
        }
        if matches!((from, self), (WalkPhase::R5, WalkPhase::R6)) {
            deltas.push(
                "side effect: may advance eval/protocol closure before loading parent baseline"
                    .to_string(),
            );
        }
        if matches!((from, self), (WalkPhase::R7, WalkPhase::R8)) {
            deltas.push(
                "side effect: may publish or receive child-plan authority and wait on provider/harness work"
                    .to_string(),
            );
        }
        if matches!((from, self), (WalkPhase::R10, WalkPhase::R11a)) {
            deltas.push(
                "side effect: projects rejected-only selection evidence without child fanout"
                    .to_string(),
            );
        }
        if matches!((from, self), (WalkPhase::R10, WalkPhase::R11)) {
            deltas.push(
                "side effect: runs live child fanout and may spawn or observe child runtimes"
                    .to_string(),
            );
        }
        if matches!(
            (from, self),
            (WalkPhase::R11a | WalkPhase::R11, WalkPhase::R12)
        ) {
            deltas.push(
                "projection: assembles report facts without emitting the final report".to_string(),
            );
        }
        if matches!((from, self), (WalkPhase::R12, WalkPhase::R13a)) {
            deltas.push(
                "side effect: may record no-selection stopped continuation; successor handoff is blocked in walk".to_string(),
            );
        }
        if matches!((from, self), (WalkPhase::R13a, WalkPhase::R14a)) {
            deltas.push(
                "side effect: emits final report and records parent-complete evidence".to_string(),
            );
        }
        if deltas.is_empty() {
            deltas.push("no admitted typestate delta for this phase pair".to_string());
        }
        deltas
    }

    /// Structured changed axes for a transition between two admitted phases.
    pub(crate) fn axis_deltas_from(self, from: WalkPhase) -> Vec<RuntimeAxisDelta> {
        match (from.shape(), self.shape()) {
            (Some(from), Some(to)) => to.axis_deltas_from(from),
            _ => Vec::new(),
        }
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
            WalkPhase::R6 => Some(R6_SHAPE),
            WalkPhase::R7 => Some(R7_SHAPE),
            WalkPhase::R8 => Some(R8_SHAPE),
            WalkPhase::R9 => Some(R9_SHAPE),
            WalkPhase::R10 => Some(R10_SHAPE),
            WalkPhase::R11a => Some(R11A_SHAPE),
            WalkPhase::R11 => Some(R11_SHAPE),
            WalkPhase::R12 => Some(R12_SHAPE),
            WalkPhase::R13a => Some(R13A_SHAPE),
            WalkPhase::R14a => Some(R14A_SHAPE),
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

fn render_axis_delta(delta: &RuntimeAxisDelta) -> String {
    let combined = format!("{}: {} -> {}", delta.label, delta.from, delta.to);
    if combined.len() <= 96 {
        combined
    } else {
        format!("{}:\n{}\n-> {}", delta.label, delta.from, delta.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r8_advertises_r9_schedule_step() {
        let steps = WalkPhase::R8.next_steps();

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].edge, "r8_to_r9");
        assert_eq!(steps[0].phase, WalkPhase::R9);
        assert_eq!(steps[0].detail, "shape child schedule and budget");
        assert_eq!(WalkPhase::R9.next_steps()[0].edge, "r9_to_r10");
    }

    #[test]
    fn r9_shape_records_schedule_ready_delta() {
        let deltas = WalkPhase::R9.axis_deltas_from(WalkPhase::R8);
        let plan = deltas
            .iter()
            .find(|delta| delta.label == "plan")
            .expect("R8 -> R9 should change the plan axis");

        assert!(plan.from.contains("plan::schedule::None"));
        assert!(
            plan.to.contains(
                "plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>"
            )
        );
    }

    #[test]
    fn r9_advertises_r10_strategy_step() {
        let steps = WalkPhase::R9.next_steps();

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].edge, "r9_to_r10");
        assert_eq!(steps[0].phase, WalkPhase::R10);
        assert_eq!(steps[0].detail, "resolve successor-selection strategy");
        assert_eq!(WalkPhase::R10.next_steps()[0].edge, "r10_to_r11 --watch");
    }

    #[test]
    fn r10_advertises_watch_gated_r11_branches() {
        let steps = WalkPhase::R10.next_steps();

        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].edge, "r10_to_r11 --watch");
        assert_eq!(steps[0].phase, WalkPhase::R11a);
        assert_eq!(steps[1].edge, "r10_to_r11 --watch");
        assert_eq!(steps[1].phase, WalkPhase::R11);
        assert_eq!(WalkPhase::R11a.next_steps()[0].edge, "r11_to_r12");
        assert_eq!(WalkPhase::R11.next_steps()[0].edge, "r11_to_r12");
    }

    #[test]
    fn r10_shape_records_selection_strategy_delta() {
        let deltas = WalkPhase::R10.axis_deltas_from(WalkPhase::R9);
        let evidence = deltas
            .iter()
            .find(|delta| delta.label == "evidence")
            .expect("R9 -> R10 should change the evidence axis");

        assert!(evidence.from.contains("evidence::selection::Plan"));
        assert!(evidence.to.contains("evidence::selection::Strategy"));
    }

    #[test]
    fn r11_shapes_record_selection_evidence_delta() {
        for phase in [WalkPhase::R11a, WalkPhase::R11] {
            let deltas = phase.axis_deltas_from(WalkPhase::R10);
            let evidence = deltas
                .iter()
                .find(|delta| delta.label == "evidence")
                .expect("R10 -> R11 branch should change the evidence axis");

            assert!(evidence.from.contains("evidence::selection::Strategy"));
            assert!(evidence.to.contains("evidence::selection::Evidence"));
        }
    }

    #[test]
    fn r11_branches_advertise_r12_report_projection() {
        assert_eq!(WalkPhase::R11a.next_steps().len(), 1);
        assert_eq!(WalkPhase::R11a.next_steps()[0].phase, WalkPhase::R12);
        assert_eq!(WalkPhase::R11.next_steps().len(), 1);
        assert_eq!(WalkPhase::R11.next_steps()[0].phase, WalkPhase::R12);
        assert_eq!(WalkPhase::R12.next_steps()[0].edge, "r12_to_r13");
    }

    #[test]
    fn r12_shape_records_report_facts_delta() {
        for from in [WalkPhase::R11a, WalkPhase::R11] {
            let deltas = WalkPhase::R12.axis_deltas_from(from);
            let report = deltas
                .iter()
                .find(|delta| delta.label == "report")
                .expect("R11 branch -> R12 should change the report axis");

            assert!(report.from.contains("report::None"));
            assert!(report.to.contains("report::Facts"));
        }
    }

    #[test]
    fn r12_advertises_stopped_continuation_only() {
        let steps = WalkPhase::R12.next_steps();

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].edge, "r12_to_r13");
        assert_eq!(steps[0].phase, WalkPhase::R13a);
        assert!(steps[0].detail.contains("handoff remains blocked"));
        assert_eq!(WalkPhase::R13a.next_steps()[0].edge, "r13_to_r14");
    }

    #[test]
    fn r13a_shape_records_stopped_continuation_delta() {
        let deltas = WalkPhase::R13a.axis_deltas_from(WalkPhase::R12);
        let continuation = deltas
            .iter()
            .find(|delta| delta.label == "continuation")
            .expect("R12 -> R13a should change the continuation axis");

        assert!(continuation.from.contains("continuation::decision::None"));
        assert!(continuation.to.contains("continuation::decision::Stopped"));
    }

    #[test]
    fn r13a_advertises_r14a_final_report() {
        let steps = WalkPhase::R13a.next_steps();

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].edge, "r13_to_r14");
        assert_eq!(steps[0].phase, WalkPhase::R14a);
        assert!(WalkPhase::R14a.next_steps().is_empty());
    }

    #[test]
    fn r14a_shape_records_final_report_delta() {
        let deltas = WalkPhase::R14a.axis_deltas_from(WalkPhase::R13a);
        let report = deltas
            .iter()
            .find(|delta| delta.label == "report")
            .expect("R13a -> R14a should change the report axis");
        let evidence = deltas
            .iter()
            .find(|delta| delta.label == "evidence")
            .expect("R13a -> R14a should change completion evidence");

        assert!(report.from.contains("report::Facts"));
        assert!(report.to.contains("report::Emitted"));
        assert!(evidence.from.contains("evidence::completion::None"));
        assert!(evidence.to.contains("evidence::completion::Recorded"));
    }
}
