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

    /// Human typestate delta for a transition between two admitted phases.
    pub(crate) fn changes_from(self, from: WalkPhase) -> &'static [&'static str] {
        match (from, self) {
            (WalkPhase::R0, WalkPhase::R1) => &[
                "phase: phase::R0 -> phase::R1",
                "context: Context<context::Command<Prototype1StateCommand>> -> Context<context::Collected<RunShape, CampaignConfig>>",
            ],
            (WalkPhase::R1, WalkPhase::R2a) => &[
                "phase: phase::R1 -> phase::R2a",
                "role: RuntimeRole<role::Unknown, role::Unresolved> -> RuntimeRole<role::Parent, role::Initialized<ParentIdentity>>",
                "history: History<startup::None, head::Unobserved, epoch::None> -> History<startup::Identity<ParentIdentity>, head::Unobserved, epoch::None>",
                "report: Report<report::None> -> Report<report::Identity<ParentIdentity>>",
            ],
            (WalkPhase::R1, WalkPhase::R3) => &[
                "phase: phase::R1 -> phase::R3",
                "role: RuntimeRole<role::Unknown, role::Unresolved> -> RuntimeRole<role::Parent, role::Identity<ParentIdentity>>",
                "history: History<startup::None, head::Unobserved, epoch::None> -> History<startup::Pending, head::Unobserved, epoch::None>",
            ],
            (WalkPhase::R3, WalkPhase::R4a) => &[
                "phase: phase::R3 -> phase::R4a",
                "role: RuntimeRole<role::Parent, role::Identity<ParentIdentity>> -> parent::Parent<parent::Unchecked>",
            ],
            (WalkPhase::R4a, WalkPhase::R4b) => &[
                "phase: phase::R4a -> phase::R4b",
                "role: parent::Parent<parent::Unchecked> -> parent::Parent<parent::Checked>",
            ],
            (WalkPhase::R4a, WalkPhase::R4c) => &[
                "phase: phase::R4a -> phase::R4c",
                "role: parent::Parent<parent::Unchecked> -> parent::Parent<parent::Ready>",
                "history: History<startup::Pending, head::Unobserved, epoch::None> -> History<startup::Validated<Any>, head::FromStartup, epoch::None>",
            ],
            (WalkPhase::R4b, WalkPhase::R4c) => &[
                "phase: phase::R4b -> phase::R4c",
                "role: parent::Parent<parent::Checked> -> parent::Parent<parent::Ready>",
                "history: History<startup::Pending, head::Unobserved, epoch::None> -> History<startup::Validated<Any>, head::FromStartup, epoch::None>",
            ],
            (WalkPhase::R4c, WalkPhase::R5) => &[
                "phase: phase::R4c -> phase::R5",
                "evidence: Evidence<parent_start::None, ...> -> Evidence<parent_start::Recorded<ParentStartedEntry>, ...>",
                "side effect: appends parent-start/resource entries to the transition journal",
            ],
            _ => &["no admitted typestate delta for this phase pair"],
        }
    }

    /// Human-readable Rust typestate alias for the current phase.
    pub(crate) fn typestate(self) -> &'static str {
        match self {
            WalkPhase::Empty => "(no Runtime value is held yet)",
            WalkPhase::R0 => R0_TYPE,
            WalkPhase::R1 => R1_TYPE,
            WalkPhase::R2a => R2A_TYPE,
            WalkPhase::R3 => R3_TYPE,
            WalkPhase::R4a => R4A_TYPE,
            WalkPhase::R4b => R4B_TYPE,
            WalkPhase::R4c => R4C_TYPE,
            WalkPhase::R5 => R5_TYPE,
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

const R0_TYPE: &str = r#"Runtime<
    phase::R0,
    RuntimeRole<role::Unknown, role::Unresolved>,
    Context<context::Command<Prototype1StateCommand>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<history_axis::startup::None, history_axis::head::Unobserved, history_axis::epoch::None>,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;

const R1_TYPE: &str = r#"Runtime<
    phase::R1,
    RuntimeRole<role::Unknown, role::Unresolved>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<history_axis::startup::None, history_axis::head::Unobserved, history_axis::epoch::None>,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;

const R2A_TYPE: &str = r#"Runtime<
    phase::R2a,
    RuntimeRole<role::Parent, role::Initialized<ParentIdentity>>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Identity<ParentIdentity>,
        history_axis::head::Unobserved,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::Identity<ParentIdentity>>,
>;"#;

const R3_TYPE: &str = r#"Runtime<
    phase::R3,
    RuntimeRole<role::Parent, role::Identity<ParentIdentity>>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Pending,
        history_axis::head::Unobserved,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;

const R4A_TYPE: &str = r#"Runtime<
    phase::R4a,
    parent::Parent<parent::Unchecked>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Pending,
        history_axis::head::Unobserved,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;

const R4B_TYPE: &str = r#"Runtime<
    phase::R4b,
    parent::Parent<parent::Checked>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Pending,
        history_axis::head::Unobserved,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;

const R4C_TYPE: &str = r#"Runtime<
    phase::R4c,
    parent::Parent<parent::Ready>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::None,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;

const R5_TYPE: &str = r#"Runtime<
    phase::R5,
    parent::Parent<parent::Ready>,
    Context<context::Collected<RunShape, CampaignConfig>>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::None,
        evidence::policy::None,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;"#;
