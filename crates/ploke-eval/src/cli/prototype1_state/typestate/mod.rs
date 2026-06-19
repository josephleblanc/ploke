#![allow(dead_code)]

//! Global Prototype 1 runtime typestate map.
//!
//! This module is now part of the live Prototype 1 parent driver. The batch
//! `prototype1-state` path enters through `driver::advance::run_to_terminal`,
//! and the `walk` operator/debugger surface steps the same typed edge functions
//! through `walk::controller::WalkController`. The map is still evolving, but it
//! is no longer a detached design sketch.
//!
//! The central carrier maps a parent turn onto one structural `Runtime<...>`
//! value instead of encoding phase facts in loose locals or flattened names.
//! The shape deliberately follows the existing C1-C5 pattern in `c1.rs`:
//!
//! ```text
//! Prototype<Running, ArtifactWorld, ChildState, AckState>
//! Runtime<Phase, Role, Context, Plan, Children, History, Evidence, Continuation, Report>
//! ```
//!
//! In other words: do not flatten several facts into one long state name. If a
//! state has multiple dimensions, represent those dimensions as nested type
//! parameters.
//!
//! History caveat: `history/mod.rs` names the intended authority sequence as
//! `Startup<Observed> -> Startup<Genesis | Predecessor> -> Startup<Validated>
//! -> Parent<Ruling>`. The live `prototype1-state` path is not there yet; it
//! currently advances through `Parent<Ready>` and `Parent<Selectable>`. This map
//! records the current live path while keeping the intended History model visible
//! in the `History<Startup, Head, Epoch>` axis.

#[cfg(test)]
mod tests;

mod aliases;
pub(crate) mod context;
mod runtime;
mod shape;
mod transition;

mod axes;

#[allow(unused_imports)]
pub(crate) use aliases::{
    ChildAttemptC1, ChildAttemptC2, ChildAttemptC3, ChildAttemptC4, ChildAttemptC5,
    HandoffBlockOpen, HandoffBlockSealed, HandoffCrownLocked, HandoffLineageObserved, R0, R0_SHAPE,
    R1, R1_SHAPE, R1Branch, R2A_SHAPE, R2a, R2aParts, R3, R3_SHAPE, R3Parts, R4A_SHAPE, R4B_SHAPE,
    R4C_SHAPE, R4a, R4aParts, R4aStartupBranch, R4bGenesisChecked, R4bParts, R4cParts, R4cReady,
    R5, R5_SHAPE, R6, R6_SHAPE, R7, R7_SHAPE, R8, R8_SHAPE, R9, R9_SHAPE, R10, R10_SHAPE,
    R10FanoutBranch, R11_SHAPE, R11A_SHAPE, R11FanoutComplete, R11aRejectedOnly, R12, R12_SHAPE,
    R12ContinuationBranch, R13A_SHAPE, R13B_SHAPE, R13aStopped, R13bHandoffCommitted, R14A_SHAPE,
    R14B_SHAPE, R14FinalBranch, R14aFinalStopped, R14bFinalHandoff, ReadyParts, RetiredParts,
    SelectableParts,
};
pub(crate) use axes::{children, continuation, evidence, history_axis, phase, plan, report, role};
#[allow(unused_imports)]
pub(crate) use runtime::{
    Children, Context, Continuation, Evidence, History, Plan, Report, Runtime, RuntimeRole,
};
pub(crate) use shape::{RuntimeAxisDelta, RuntimeShape};
#[allow(unused_imports)]
pub(crate) use transition::{
    AsyncStep, AsyncStepInput, AsyncTransition, Chain, Step, StepInput, Transition,
    async_transition, transition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::cli::prototype1_state::typestate) struct Private;

// Review notes from mapping existing code to the phase diagram:
//
// - `Parent<S>`, C1-C5, `Child<S>`, `Crown<S>`, `Block<S>`, and `Startup<S>`
//   are the strong existing typestate islands.
// - `PlannedChildren` is useful but structurally awkward for this global map
//   because it bundles `Parent<Selectable>` together with plan/child data. A
//   future Runtime carrier should likely split those fields across axes.
// - `Prototype1StateRunShape` and `ActiveSelectionStrategy` are now carried as
//   value-level facts in the collected runtime context. Their marker-level shape
//   remains intentionally small so the typestate aliases do not depend on every
//   private projection detail in `cli_facing.rs`.
// - `history/mod.rs` names the intended authority sequence as
//   `Startup<Observed>
//      -> Startup<Genesis | Predecessor> -> Startup<Validated>
//      -> Parent<Ruling>`.
//   The current live path is weaker/different: it enters
//   `Parent<Ready>` and later `Parent<Selectable>`.
// - `Startup<Validated>` is the live startup gate, but it erases whether the
//   validated startup came from genesis or predecessor. This map preserves that
//   distinction in `R4cReady<Kind>`, then uses `startup::Any` once branches
//   converge.
// - Stopped/no-successor completion still has no separate `Parent<Stopped>`
//   carrier; R13a/R14a retain `Parent<Selectable>` while report/completion axes
//   record terminal facts.
// - Selected-successor handoff is represented by R13b/R14b with
//   `Parent<Retired>` plus History/continuation/handoff axes. The `walk` CLI
//   admits this path only with `--watch --allow git-changes`.
// - `Prototype1ContinuationDecision` remains a value-level decision carried in
//   the continuation axis; the authority to mutate checkout/History still comes
//   from consuming the typed parent state plus the explicit operator gate.
// - `successor::Record` is durable transition evidence for successor selection,
//   ready, stopped, handoff, and completion events; it is not by itself a
//   `Successor<S>` authority carrier.
