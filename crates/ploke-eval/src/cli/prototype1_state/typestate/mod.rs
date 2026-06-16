#![allow(dead_code)]

//! Global Prototype 1 runtime typestate map.
//!
//! This module is intentionally not wired into the live controller yet. It maps
//! the observed `run_prototype1_state_turn` execution phases onto one structural
//! `Runtime<...>` carrier so we can review the intended type boundaries before
//! moving implementation code.
//!
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

use std::{marker::PhantomData, path::PathBuf};

use ploke_records::ids::CampaignId;

use crate::{
    cli::Prototype1StateCommand,
    intervention::{
        CompleteBaseline, Prototype1ChildBudget, Prototype1ChildScheduleMode,
        Prototype1ContinuationDecision, Prototype1SearchPolicy,
    },
    successor_selection::SuccessorDecision,
};

use super::{
    c1, c2, c3, c4,
    cli_facing::{
        PlannedChildOutcome, PlannedChildren, Prototype1StateReport, SelectionSealMaterial,
    },
    history::{self as history_model, Block, LineageState},
    identity::ParentIdentity,
    inner::{self, Crown, Received},
    journal::{ParentStartedEntry, PrototypeJournal},
    parent as parent_role, successor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Private;

/// Global runtime carrier, parameterized by each independently meaningful axis.
#[derive(Debug)]
pub(crate) struct Runtime<
    Phase,
    Role,
    Context,
    Plan,
    Children,
    History,
    Evidence,
    Continuation,
    Report,
> {
    phase: Phase,
    role: Role,
    context: Context,
    plan: Plan,
    children: Children,
    history: History,
    evidence: Evidence,
    continuation: Continuation,
    report: Report,
    _private: Private,
}

/// Role axis before a concrete `Parent<S>` / `Child<S>` carrier exists.
///
/// Once the parent role is constructed, the `Runtime` role parameter uses the
/// existing `parent::Parent<S>` carrier directly instead of wrapping it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeRole<Kind, State> {
    _kind: PhantomData<Kind>,
    _state: PhantomData<State>,
    _private: Private,
}

/// Run context axis: command-only first, then collected command-derived inputs.
#[derive(Debug)]
pub(crate) struct Context<State> {
    state: State,
    _private: Private,
}

impl<State> Context<State> {
    fn new(state: State) -> Self {
        Self {
            state,
            _private: Private,
        }
    }

    fn into_state(self) -> State {
        self.state
    }
}

/// Child-plan axis split into authority and schedule dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Plan<Authority, Schedule> {
    _authority: PhantomData<Authority>,
    _schedule: PhantomData<Schedule>,
    _private: Private,
}

/// Child-set axis split from the per-child C1-C5 attempt chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Children<Set, Attempt> {
    _set: PhantomData<Set>,
    _attempt: PhantomData<Attempt>,
    _private: Private,
}

/// History axis, following the History docs rather than a single flat status.
///
/// - `Startup` tracks current startup/admission evidence.
/// - `Head` tracks whether the lineage-head projection has been observed or
///   advanced by this runtime.
/// - `Epoch` tracks the in-flight Crown/Block handoff authority state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct History<Startup, Head, Epoch> {
    _startup: PhantomData<Startup>,
    _head: PhantomData<Head>,
    _epoch: PhantomData<Epoch>,
    _private: Private,
}

/// Readiness/evidence axis for facts that are real prerequisites but not yet
/// first-class role/History/child carriers in the live implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Evidence<ParentStart, Baseline, Policy, Selection, Completion> {
    _parent_start: PhantomData<ParentStart>,
    _baseline: PhantomData<Baseline>,
    _policy: PhantomData<Policy>,
    _selection: PhantomData<Selection>,
    _completion: PhantomData<Completion>,
    _private: Private,
}

/// Continuation axis: selection value, continuation decision, and handoff
/// record are separate facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Continuation<Selection, Decision, Handoff> {
    _selection: PhantomData<Selection>,
    _decision: PhantomData<Decision>,
    _handoff: PhantomData<Handoff>,
    _private: Private,
}

/// Report axis. Report facts and emitted report are separate from successor
/// completion/handoff evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Report<State> {
    _state: PhantomData<State>,
    _private: Private,
}

// -----------------------------------------------------------------------------
// Typed transition combinators
// -----------------------------------------------------------------------------
//
// These types are the functional-programming side of the typestate map.
// `Runtime<...>` names a state. `Transition<From, To, F>` names a typed arrow
// from one state to another. `Step::then` composes adjacent arrows.
//
// This intentionally does not create named transition structs such as
// `ResolveParentIdentity` or `RunChildFanout`. If a transition deserves a name
// later, that name should be attached as metadata or documentation around a
// `Transition<From, To, _>`, not encoded by flattening the edge into a new type
// name.

/// A typed, fallible arrow from `From` to `To`.
///
/// The function `F` is stored as data so callers can close over the environment
/// they need for one transition: paths, backend handles, profile data, or test
/// fixtures. The type parameters still enforce adjacency:
///
/// ```ignore
/// Transition<R0, R1, _>
///     .then(Transition<R1, R3, _>) // ok
///
/// Transition<R0, R1, _>
///     .then(Transition<R4a, R5, _>) // type error: R1 != R4a
/// ```
///
/// A `Transition` has no authority by itself. Authority remains in the consumed
/// `From` value. Applying the transition consumes `From` and produces `To`, just
/// like the existing `Parent<Unchecked> -> Parent<Checked>` and C1-C5 move-only
/// transitions.
#[must_use = "a Transition does nothing until Step::apply is called"]
pub(crate) struct Transition<From, To, F, Error = crate::spec::PrepareError> {
    f: F,
    _from: PhantomData<From>,
    _to: PhantomData<To>,
    _error: PhantomData<Error>,
    _private: Private,
}

impl<From, To, F, Error> Transition<From, To, F, Error>
where
    F: FnOnce(From) -> Result<To, Error>,
{
    /// Build a typed transition from a closure or function.
    ///
    /// In practice, closure argument annotations are usually enough to let Rust
    /// infer `From` and `To`:
    ///
    /// ```ignore
    /// let edge = Transition::new(|r0: R0| -> Result<R1, PrepareError> {
    ///     // collect command-derived inputs here
    ///     todo!()
    /// });
    /// ```
    pub(crate) fn new(f: F) -> Self {
        Self {
            f,
            _from: PhantomData,
            _to: PhantomData,
            _error: PhantomData,
            _private: Private,
        }
    }
}

/// Convenience constructor for `Transition::new`.
///
/// This makes a transition pipeline read like a small functional program:
///
/// ```ignore
/// let pipeline = transition(|r0: R0| -> Result<R1, PrepareError> { todo!() })
///     .then(transition(|r1: R1| -> Result<R3, PrepareError> { todo!() }))
///     .then(transition(|r3: R3| -> Result<R4a, PrepareError> { todo!() }));
///
/// let r4a = pipeline.apply(r0)?;
/// ```
pub(crate) fn transition<From, To, F, Error>(f: F) -> Transition<From, To, F, Error>
where
    F: FnOnce(From) -> Result<To, Error>,
{
    Transition::new(f)
}

/// A value that can advance one typed state to another.
///
/// This trait is intentionally tiny. It is the Rust equivalent of a Kleisli
/// arrow for `Result`:
///
/// ```text
/// From -> Result<To, Error>
/// ```
///
/// The associated `Error` lets the scaffold remain generic. Most live
/// `prototype1-state` transitions will probably use `PrepareError`, but tests
/// and pure experiments can use smaller error types.
pub(crate) trait Step<From>: Sized {
    /// The state produced by this step.
    type To;

    /// The failure type for this step.
    type Error;

    /// Consume `from` and either produce the next typed state or fail before the
    /// state transition happens.
    fn apply(self, from: From) -> Result<Self::To, Self::Error>;

    /// Compose this step with a second step whose input is exactly this step's
    /// output.
    ///
    /// The type checker enforces adjacency. This is the piece that gives the
    /// map functional-programming ergonomics without erasing the typestate
    /// guarantees.
    fn then<Next>(self, next: Next) -> Chain<Self, Next, Self::To>
    where
        Next: Step<Self::To, Error = Self::Error>,
    {
        Chain {
            first: self,
            second: next,
            _mid: PhantomData,
            _private: Private,
        }
    }
}

impl<From, To, F, Error> Step<From> for Transition<From, To, F, Error>
where
    F: FnOnce(From) -> Result<To, Error>,
{
    type To = To;
    type Error = Error;

    fn apply(self, from: From) -> Result<Self::To, Self::Error> {
        (self.f)(from)
    }
}

/// A composed pair of adjacent steps.
///
/// `Chain<A, B, Mid>` represents:
///
/// ```text
/// From --A--> Mid --B--> To
/// ```
///
/// The `Mid` parameter is explicit for the same reason C1-C5 use explicit type
/// aliases: it keeps the intermediate state visible in type errors and review.
#[must_use = "a Chain does nothing until Step::apply is called"]
pub(crate) struct Chain<First, Second, Mid> {
    first: First,
    second: Second,
    _mid: PhantomData<Mid>,
    _private: Private,
}

impl<From, Mid, First, Second> Step<From> for Chain<First, Second, Mid>
where
    First: Step<From, To = Mid>,
    Second: Step<Mid, Error = First::Error>,
{
    type To = Second::To;
    type Error = First::Error;

    fn apply(self, from: From) -> Result<Self::To, Self::Error> {
        let mid = self.first.apply(from)?;
        self.second.apply(mid)
    }
}

/// Branching is represented by making `To` a sum type.
///
/// This is a documentation-only example of the intended style; we do not need a
/// special branch combinator until a real transition wants one.
///
/// ```ignore
/// enum StartupBranch {
///     Genesis(R4bGenesisChecked),
///     Predecessor(R4cPredecessorReady),
/// }
///
/// let startup = transition(|r4a: R4a| -> Result<StartupBranch, PrepareError> {
///     // choose the branch from handoff invocation / startup evidence
///     todo!()
/// });
/// ```
///
/// This keeps the control-flow fork explicit without inventing a flattened
/// state name like `GenesisOrPredecessorStartupResolved`.
type _BranchingDocumentationOnly = ();

/// Async transitions should use the same shape later, but should not be added
/// until live transitions need them.
///
/// The async analogue is likely:
///
/// ```ignore
/// trait AsyncStep<From> {
///     type To;
///     type Error;
///     type Fut: Future<Output = Result<Self::To, Self::Error>>;
///
///     fn apply(self, from: From) -> Self::Fut;
/// }
/// ```
///
/// Keeping the synchronous `Step` first makes the state graph easy to review
/// before introducing future/lifetime complexity.
type _AsyncDocumentationOnly = ();

/// R0: CLI dispatch / failure hook.
///
/// Initial carrier:
/// - `Prototype1StateCommand` enters `Context<Command<Prototype1StateCommand>>`.
/// - Runtime role starts as `RuntimeRole<Unknown, Unresolved>`.
///
/// The process has started and received a `Prototype1StateCommand`, but the
/// runtime role is not known yet in the evalnomicon `Role<State>` sense.
pub(crate) type R0 = Runtime<
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
>;

/// R1: prelude and coordinates.
///
/// Axis changes:
/// - `Context<Command<Prototype1StateCommand>> -> Context<Collected>`.
/// - Role remains `RuntimeRole<Unknown, Unresolved>`.
///
/// Existing code currently materializes these as loose locals: repo root,
/// campaign id, manifest path, run shape, resolved campaign config, journal
/// path/writer, and active monitor target. `Prototype1StateRunShape` exists but
/// is private to `cli_facing.rs`; if this map becomes implementation, that
/// shape should move behind this context axis instead of being duplicated.
pub(crate) type R1<RunShape = (), CampaignConfig = ()> = Runtime<
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
>;

impl R0 {
    /// Construct the initial runtime-state carrier from the raw CLI command.
    ///
    /// This is intentionally the only constructor for R0. At this point the
    /// process has not entered a concrete evalnomicon role, and no repo or
    /// campaign coordinates have been collected.
    pub(crate) fn new(command: Prototype1StateCommand) -> Self {
        Self {
            phase: phase::R0 { _private: Private },
            role: RuntimeRole {
                _kind: PhantomData,
                _state: PhantomData,
                _private: Private,
            },
            context: Context::new(context::Command::new(command)),
            plan: Plan {
                _authority: PhantomData,
                _schedule: PhantomData,
                _private: Private,
            },
            children: Children {
                _set: PhantomData,
                _attempt: PhantomData,
                _private: Private,
            },
            history: History {
                _startup: PhantomData,
                _head: PhantomData,
                _epoch: PhantomData,
                _private: Private,
            },
            evidence: Evidence {
                _parent_start: PhantomData,
                _baseline: PhantomData,
                _policy: PhantomData,
                _selection: PhantomData,
                _completion: PhantomData,
                _private: Private,
            },
            continuation: Continuation {
                _selection: PhantomData,
                _decision: PhantomData,
                _handoff: PhantomData,
                _private: Private,
            },
            report: Report {
                _state: PhantomData,
                _private: Private,
            },
            _private: Private,
        }
    }

    /// Consume R0 and recover the command for the R0 -> R1 collection edge.
    ///
    /// This is not a general escape hatch for role-bearing states. R0 has no
    /// role authority yet; it is just the command payload plus empty axes.
    pub(crate) fn into_command(self) -> Prototype1StateCommand {
        self.context.into_state().into_inner()
    }
}

impl<RunShape, CampaignConfig> R1<RunShape, CampaignConfig> {
    /// Construct R1 from collected live-loop inputs.
    ///
    /// This is the target constructor for the first typed transition:
    /// `R0 -> R1<RunShape, CampaignConfig>`.
    pub(crate) fn from_collected(collected: context::Collected<RunShape, CampaignConfig>) -> Self {
        Self {
            phase: phase::R1 { _private: Private },
            role: RuntimeRole {
                _kind: PhantomData,
                _state: PhantomData,
                _private: Private,
            },
            context: Context::new(collected),
            plan: Plan {
                _authority: PhantomData,
                _schedule: PhantomData,
                _private: Private,
            },
            children: Children {
                _set: PhantomData,
                _attempt: PhantomData,
                _private: Private,
            },
            history: History {
                _startup: PhantomData,
                _head: PhantomData,
                _epoch: PhantomData,
                _private: Private,
            },
            evidence: Evidence {
                _parent_start: PhantomData,
                _baseline: PhantomData,
                _policy: PhantomData,
                _selection: PhantomData,
                _completion: PhantomData,
                _private: Private,
            },
            continuation: Continuation {
                _selection: PhantomData,
                _decision: PhantomData,
                _handoff: PhantomData,
                _private: Private,
            },
            report: Report {
                _state: PhantomData,
                _private: Private,
            },
            _private: Private,
        }
    }

    /// Temporary migration seam back to the existing live implementation.
    ///
    /// Once the subsequent R-states are wired, callers should advance through
    /// typed transitions instead of extracting the collected payload.
    pub(crate) fn into_collected(self) -> context::Collected<RunShape, CampaignConfig> {
        self.context.into_state()
    }
}

/// R2a: gen0 parent identity initialization terminal branch.
///
/// Axis changes:
/// - `RuntimeRole<Unknown, Unresolved>`
///   `-> RuntimeRole<Parent, Initialized<ParentIdentity>>`.
/// - `Report<None> -> Report<Identity<ParentIdentity>>`.
///
/// This returns before the normal parent runtime path. ADR 007 says the future
/// version should create a genesis History block; current code only writes and
/// commits checkout parent identity.
pub(crate) type R2a = Runtime<
    phase::R2a,
    RuntimeRole<role::Parent, role::Initialized<ParentIdentity>>,
    Context<context::Collected>,
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
>;

/// R3: parent identity source resolved.
///
/// Axis changes:
/// - `RuntimeRole<Unknown, Unresolved>`
///   `-> RuntimeRole<Parent, Identity<ParentIdentity>>`.
/// - `History<startup::None, head::Unobserved, epoch::None>`
///   `-> History<startup::Pending, head::Unobserved, epoch::None>`.
///
/// The concrete parent carrier has not been constructed yet. The role axis is a
/// parent-role candidate with `ParentIdentity` evidence.
pub(crate) type R3 = Runtime<
    phase::R3,
    RuntimeRole<role::Parent, role::Identity<ParentIdentity>>,
    Context<context::Collected>,
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
>;

/// R4a: parent loaded/constructed as unchecked.
///
/// Axis changes:
/// - `RuntimeRole<Parent, Identity<ParentIdentity>>`
///   `-> Parent<Unchecked>`.
///
/// First concrete parent role carrier. This corresponds to the diagram edge
/// `load parent -> parent unchecked` and the code transition
/// `ParentIdentity -> Parent<Unchecked>`.
pub(crate) type R4a = Runtime<
    phase::R4a,
    parent_role::Parent<parent_role::Unchecked>,
    Context<context::Collected>,
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
>;

/// R4b: genesis checkout validated.
///
/// Axis changes:
/// - `Parent<Unchecked> -> Parent<Checked>`.
///
/// Genesis path only: `Parent<Unchecked> -> Parent<Checked>` after active
/// checkout validation. Predecessor startup does not pass through this parent
/// state in the current code.
pub(crate) type R4bGenesisChecked = Runtime<
    phase::R4b,
    parent_role::Parent<parent_role::Checked>,
    Context<context::Collected>,
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
>;

/// R4c: parent startup complete in the current live implementation.
///
/// Axis changes:
/// - Genesis: `Parent<Checked> -> Parent<Ready>`.
/// - Predecessor: `Parent<Unchecked> -> Parent<Ready>`.
/// - `History<startup::Pending, head::Unobserved, epoch::None>`
///   `-> History<startup::Validated<Kind>, head::FromStartup, epoch::None>`.
///
/// Both startup branches converge here as `Parent<Ready>`. This is not yet the
/// intended History-docs `Parent<Ruling>` state. The `Kind` parameter records
/// which startup branch produced readiness, because existing `Startup<Validated>`
/// erases that once consumed.
pub(crate) type R4cReady<Kind> = Runtime<
    phase::R4c,
    parent_role::Parent<parent_role::Ready>,
    Context<context::Collected>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Validated<Kind>,
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
>;

pub(crate) type R4cGenesisReady = R4cReady<parent_role::Genesis>;
pub(crate) type R4cPredecessorReady = R4cReady<parent_role::Predecessor>;

/// R5: parent-start evidence recorded.
///
/// Axis changes:
/// - `Evidence<parent_start::None, ...>`
///   `-> Evidence<parent_start::Recorded<ParentStartedEntry>, ...>`.
///
/// Existing carrier: `ParentStartedEntry` in the transition journal, plus a
/// resource sample for `ParentStart`.
pub(crate) type R5 = Runtime<
    phase::R5,
    parent_role::Parent<parent_role::Ready>,
    Context<context::Collected>,
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
>;

/// R6: parent baseline established.
///
/// Axis changes:
/// - `Evidence<..., baseline::None, ...>`
///   `-> Evidence<..., baseline::Ready<CompleteBaseline>, ...>`.
///
/// Existing carrier: `CompleteBaseline = Baseline<baseline::Complete>`.
pub(crate) type R6 = Runtime<
    phase::R6,
    parent_role::Parent<parent_role::Ready>,
    Context<context::Collected>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
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
>;

/// R7: policy and child budget ready.
///
/// Axis changes:
/// - `Evidence<..., policy::None, ...>`
///   `-> Evidence<..., policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>, ...>`.
///
/// Existing carriers: `Prototype1SearchPolicy`, `Prototype1ChildBudget`, and
/// `Prototype1ChildScheduleMode`. The live code currently stores these in
/// locals and sometimes derives them from an admitted run profile.
pub(crate) type R7 = Runtime<
    phase::R7,
    parent_role::Parent<parent_role::Ready>,
    Context<context::Collected>,
    Plan<plan::authority::None, plan::schedule::None>,
    Children<children::set::None, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::None,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;

/// R8: child-plan authority received and parent becomes selectable.
///
/// Axis changes:
/// - `Parent<Ready> -> Parent<Selectable>`.
/// - `Plan<authority::None, schedule::None>`
///   `-> Plan<authority::Received<Received<ChildPlan>>, schedule::None>`.
/// - `Children<set::None, attempt::None>`
///   `-> Children<set::Planned<ChildFiles>, attempt::None>`.
///
/// Existing carriers:
/// - `Received<ChildPlan>` is the cross-runtime message capability.
/// - `ChildFiles` is the per-child input before C1.
/// - `PlannedChildren` currently bundles `Parent<Selectable>` with plan/child
///   data; future implementation should likely split that bundle across axes.
pub(crate) type R8 = Runtime<
    phase::R8,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<plan::authority::Received<Received<parent_role::ChildPlan>>, plan::schedule::None>,
    Children<children::set::Planned<parent_role::ChildFiles>, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Plan<PlannedChildren>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;

/// R9: child schedule shaped.
///
/// Axis changes:
/// - `Plan<authority::Received<_>, schedule::None>`
///   `-> Plan<authority::Received<_>, schedule::Ready<Budget, Mode>>`.
///
/// No existing typestate island moves here, but the runnable child set has been
/// budgeted/truncated and schedule mode has been selected.
pub(crate) type R9 = Runtime<
    phase::R9,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Planned<parent_role::ChildFiles>, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Plan<PlannedChildren>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;

/// R10: selection strategy ready.
///
/// Axis changes:
/// - `Evidence<..., selection::Plan<PlannedChildren>, ...>`
///   `-> Evidence<..., selection::Strategy, ...>`.
///
/// Existing implementation type: `ActiveSelectionStrategy`, but it is private
/// to `cli_facing.rs`. The local `evidence::selection::Strategy` marker stands
/// in for that existing-but-not-reusable carrier.
pub(crate) type R10 = Runtime<
    phase::R10,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Planned<parent_role::ChildFiles>, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Strategy,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;

/// R11a: rejected-only branch projected into selection evidence.
///
/// Axis changes:
/// - `Children<set::Planned<ChildFiles>, attempt::None>`
///   `-> Children<set::Rejected, attempt::None>`.
/// - `Evidence<..., selection::Strategy, ...>`
///   `-> Evidence<..., selection::Evidence<SelectionSealMaterial>, ...>`.
///
pub(crate) type R11aRejectedOnly = Runtime<
    phase::R11a,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Rejected, children::attempt::None>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Evidence<SelectionSealMaterial>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::None,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;

/// R11b/R11c: child fanout complete.
///
/// Axis changes:
/// - `Children<set::Planned<ChildFiles>, attempt::None>`
///   `-> Children<set::Outcomes<PlannedChildOutcome>, attempt::Complete>`.
/// - Per child: `ChildFiles -> C1 -> C2 -> C3 -> C4 -> C5`.
/// - `Continuation<selection::None, ...>`
///   `-> Continuation<selection::Maybe<SuccessorDecision>, ...>`.
///
/// Existing carrier: `PlannedChildOutcome`. Inside fanout, each child walks the
/// C1-C5 chain listed below; after fanout the parent has a vector of outcomes.
pub(crate) type R11FanoutComplete = Runtime<
    phase::R11,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Outcomes<PlannedChildOutcome>, children::attempt::Complete>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Evidence<SelectionSealMaterial>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::Maybe<SuccessorDecision>,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::None>,
>;

/// R12: report-child/outcome projection ready.
///
/// Axis changes:
/// - `Children<set::Outcomes<PlannedChildOutcome>, attempt::Complete>`
///   `-> Children<set::Report<PlannedChildOutcome>, attempt::Complete>`.
/// - `Report<None> -> Report<Facts>`.
///
/// Existing final report carrier is `Prototype1StateReport`, but Stage 12 only
/// has report facts. The actual report is assembled/emitted at R14.
pub(crate) type R12 = Runtime<
    phase::R12,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Report<PlannedChildOutcome>, children::attempt::Complete>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::FromStartup,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Evidence<SelectionSealMaterial>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::Maybe<SuccessorDecision>,
        continuation::decision::None,
        continuation::handoff::None,
    >,
    Report<report::Facts>,
>;

/// R13a: successor decision stops or selects no successor.
///
/// Axis changes:
/// - `History<startup::Validated<_>, head::FromStartup, epoch::None>`
///   `-> History<startup::Validated<_>, head::Read, epoch::None>`.
/// - `Continuation<selection::Maybe<_>, decision::None, handoff::None>`
///   `-> Continuation<selection::Maybe<_>, decision::Stopped<_>, handoff::None>`.
///
pub(crate) type R13aStopped = Runtime<
    phase::R13a,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Report<PlannedChildOutcome>, children::attempt::Complete>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::Read,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Evidence<SelectionSealMaterial>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::Maybe<SuccessorDecision>,
        continuation::decision::Stopped<Prototype1ContinuationDecision>,
        continuation::handoff::None,
    >,
    Report<report::Facts>,
>;

/// R13b: successor handoff committed.
///
/// Axis changes:
/// - `Parent<Selectable> -> Parent<Retired>`.
/// - `Children<set::Report<_>, attempt::Complete>`
///   `-> Children<set::Successor<_>, attempt::Complete>`.
/// - `History<startup::Validated<_>, head::FromStartup, epoch::None>`
///   `-> History<startup::Validated<_>, head::Advanced<Block<Sealed>>,`
///   `   epoch::Sealed<Block<Sealed>>>`.
/// - `Continuation<selection::Maybe<_>, decision::None, handoff::None>`
///   `-> Continuation<selection::Selected<_>, decision::Allowed<_>, handoff::Recorded<_>>`.
///
/// Existing carriers involved inside this phase include `LineageState`,
/// `Block<Open>`, `Crown<Locked>`, `Block<Sealed>`, successor journal records,
/// and `Parent<Retired>`. This alias represents the post-commit state.
pub(crate) type R13bHandoffCommitted = Runtime<
    phase::R13b,
    parent_role::Parent<parent_role::Retired>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Successor<PlannedChildOutcome>, children::attempt::Complete>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::Advanced<Block<history_model::block::Sealed>>,
        history_axis::epoch::Sealed<Block<history_model::block::Sealed>>,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Seal<SelectionSealMaterial>,
        evidence::completion::None,
    >,
    Continuation<
        continuation::selection::Selected<SuccessorDecision>,
        continuation::decision::Allowed<Prototype1ContinuationDecision>,
        continuation::handoff::Recorded<successor::Record>,
    >,
    Report<report::Facts>,
>;

/// R14a: final report after stopped/no-selection continuation.
///
/// Axis changes:
/// - `History<startup::Validated<_>, head::Read, epoch::None>`
///   `-> History<startup::Validated<_>, head::Unchanged, epoch::None>`.
/// - `Evidence<..., completion::None> -> Evidence<..., completion::Recorded>`.
/// - `Report<Facts> -> Report<Emitted<Prototype1StateReport>>`.
///
pub(crate) type R14aFinalStopped = Runtime<
    phase::R14,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Report<PlannedChildOutcome>, children::attempt::Complete>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::Unchanged,
        history_axis::epoch::None,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Evidence<SelectionSealMaterial>,
        evidence::completion::Recorded,
    >,
    Continuation<
        continuation::selection::Maybe<SuccessorDecision>,
        continuation::decision::Stopped<Prototype1ContinuationDecision>,
        continuation::handoff::None,
    >,
    Report<report::Emitted<Prototype1StateReport>>,
>;

/// R14b: final report after allowed successor handoff.
///
/// Axis changes:
/// - `Evidence<..., completion::None> -> Evidence<..., completion::Recorded>`.
/// - `Report<Facts> -> Report<Emitted<Prototype1StateReport>>`.
///
pub(crate) type R14bFinalHandoff = Runtime<
    phase::R14,
    parent_role::Parent<parent_role::Retired>,
    Context<context::Collected>,
    Plan<
        plan::authority::Received<Received<parent_role::ChildPlan>>,
        plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>,
    >,
    Children<children::set::Successor<PlannedChildOutcome>, children::attempt::Complete>,
    History<
        history_axis::startup::Validated<history_axis::startup::Any>,
        history_axis::head::Advanced<Block<history_model::block::Sealed>>,
        history_axis::epoch::Sealed<Block<history_model::block::Sealed>>,
    >,
    Evidence<
        evidence::parent_start::Recorded<ParentStartedEntry>,
        evidence::baseline::Ready<CompleteBaseline>,
        evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,
        evidence::selection::Seal<SelectionSealMaterial>,
        evidence::completion::Recorded,
    >,
    Continuation<
        continuation::selection::Selected<SuccessorDecision>,
        continuation::decision::Allowed<Prototype1ContinuationDecision>,
        continuation::handoff::Recorded<successor::Record>,
    >,
    Report<report::Emitted<Prototype1StateReport>>,
>;

/// Child-attempt typestate chain used within R11 fanout.
///
/// These are existing carriers, not new runtime states. Stop-after modes cut
/// this chain at C2/C3/C4; Complete can reach C5.
pub(crate) type ChildAttemptC1 = c1::C1;
pub(crate) type ChildAttemptC2 = c1::C2;
pub(crate) type ChildAttemptC3 = c2::C3;
pub(crate) type ChildAttemptC4 = c3::C4;
pub(crate) type ChildAttemptC5 = c4::C5;

/// History/Crown authority chain used within R13 handoff.
///
/// These existing carriers form the authority plane for sealing successor
/// handoff. They are listed separately because R13b is the post-commit state,
/// while these are the inner transition states.
pub(crate) type HandoffLineageObserved = History<
    history_axis::startup::Validated<history_axis::startup::Any>,
    history_axis::head::Observed<LineageState>,
    history_axis::epoch::None,
>;
pub(crate) type HandoffBlockOpen = History<
    history_axis::startup::Validated<history_axis::startup::Any>,
    history_axis::head::Observed<LineageState>,
    history_axis::epoch::Open<Block<history_model::block::Open>, Crown<inner::crown::Ruling>>,
>;
pub(crate) type HandoffCrownLocked = History<
    history_axis::startup::Validated<history_axis::startup::Any>,
    history_axis::head::Observed<LineageState>,
    history_axis::epoch::Locked<Block<history_model::block::Open>, Crown<inner::crown::Locked>>,
>;
pub(crate) type HandoffBlockSealed = History<
    history_axis::startup::Validated<history_axis::startup::Any>,
    history_axis::head::Observed<LineageState>,
    history_axis::epoch::Sealed<Block<history_model::block::Sealed>>,
>;

pub(crate) mod phase {
    use super::Private;

    macro_rules! phase_marker {
        ($name:ident, $doc:literal) => {
            #[doc = $doc]
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct $name {
                pub(super) _private: Private,
            }
        };
    }

    phase_marker!(R0, "R0: CLI dispatch / failure hook.");
    phase_marker!(R1, "R1: prelude and turn coordinates.");
    phase_marker!(R2a, "R2a: gen0 identity init terminal branch.");
    phase_marker!(R3, "R3: parent identity source resolved.");
    phase_marker!(R4a, "R4a: ParentIdentity -> Parent<Unchecked>.");
    phase_marker!(R4b, "R4b: genesis checkout checked.");
    phase_marker!(R4c, "R4c: parent startup complete as Parent<Ready>.");
    phase_marker!(R5, "R5: parent-start evidence recorded.");
    phase_marker!(R6, "R6: parent baseline established.");
    phase_marker!(R7, "R7: planning policy and child budget ready.");
    phase_marker!(R8, "R8: child plan resolved and parent selectable.");
    phase_marker!(R9, "R9: child schedule shaped.");
    phase_marker!(R10, "R10: selection strategy ready.");
    phase_marker!(R11a, "R11a: rejected-only child branch.");
    phase_marker!(R11, "R11b/c: child fanout complete.");
    phase_marker!(R12, "R12: report projection facts ready.");
    phase_marker!(R13a, "R13a: successor continuation stopped.");
    phase_marker!(R13b, "R13b: successor handoff committed.");
    phase_marker!(R14, "R14: final report emitted.");
}

pub(crate) mod role {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Unknown {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Parent {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Child {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Successor {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Unresolved {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Identity<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Initialized<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod context {
    use super::{CampaignId, PathBuf, Private, Prototype1StateCommand, PrototypeJournal};

    /// Command payload before command-derived inputs have been collected.
    #[derive(Debug)]
    pub(crate) struct Command<T> {
        command: T,
        _private: Private,
    }

    impl<T> Command<T> {
        pub(super) fn new(command: T) -> Self {
            Self {
                command,
                _private: Private,
            }
        }

        pub(super) fn into_inner(self) -> T {
            self.command
        }
    }

    /// Repo/campaign/manifest/run-shape/journal inputs have been collected.
    ///
    /// This is intentionally generic over `RunShape` and `CampaignConfig`
    /// because the current concrete types live outside this module. The live
    /// controller can use `R1<Prototype1StateRunShape, ResolvedCampaignConfig>`
    /// without moving those definitions during this first wiring slice.
    #[derive(Debug)]
    pub(crate) struct Collected<RunShape = (), CampaignConfig = ()> {
        command: Prototype1StateCommand,
        repo_root: PathBuf,
        campaign_id: CampaignId,
        manifest_path: PathBuf,
        run_shape: RunShape,
        campaign_config: CampaignConfig,
        journal_path: PathBuf,
        journal: PrototypeJournal,
        _private: Private,
    }

    /// Owned payload extracted from `Context<Collected<...>>` at the temporary
    /// migration boundary back into the existing live implementation.
    ///
    /// Once later R-states are wired, this escape hatch should shrink or move
    /// behind typed transition methods.
    #[derive(Debug)]
    pub(crate) struct CollectedParts<RunShape, CampaignConfig> {
        pub(crate) command: Prototype1StateCommand,
        pub(crate) repo_root: PathBuf,
        pub(crate) campaign_id: CampaignId,
        pub(crate) manifest_path: PathBuf,
        pub(crate) run_shape: RunShape,
        pub(crate) campaign_config: CampaignConfig,
        pub(crate) journal_path: PathBuf,
        pub(crate) journal: PrototypeJournal,
    }

    impl<RunShape, CampaignConfig> Collected<RunShape, CampaignConfig> {
        pub(crate) fn new(
            command: Prototype1StateCommand,
            repo_root: PathBuf,
            campaign_id: CampaignId,
            manifest_path: PathBuf,
            run_shape: RunShape,
            campaign_config: CampaignConfig,
            journal_path: PathBuf,
            journal: PrototypeJournal,
        ) -> Self {
            Self {
                command,
                repo_root,
                campaign_id,
                manifest_path,
                run_shape,
                campaign_config,
                journal_path,
                journal,
                _private: Private,
            }
        }

        pub(crate) fn into_parts(self) -> CollectedParts<RunShape, CampaignConfig> {
            CollectedParts {
                command: self.command,
                repo_root: self.repo_root,
                campaign_id: self.campaign_id,
                manifest_path: self.manifest_path,
                run_shape: self.run_shape,
                campaign_config: self.campaign_config,
                journal_path: self.journal_path,
                journal: self.journal,
            }
        }
    }
}

pub(crate) mod plan {
    use super::{PhantomData, Private};

    pub(crate) mod authority {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Received<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod schedule {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Ready<Budget, Mode> {
            _budget: PhantomData<Budget>,
            _mode: PhantomData<Mode>,
            _private: Private,
        }
    }
}

pub(crate) mod children {
    use super::{PhantomData, Private};

    pub(crate) mod set {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Planned<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Rejected {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Outcomes<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Report<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Successor<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod attempt {
        use super::Private;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Complete {
            _private: Private,
        }
    }
}

pub(crate) mod history_axis {
    use super::{PhantomData, Private};

    pub(crate) mod startup {
        use super::{PhantomData, Private};

        /// Startup/admission has not been observed by this path.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        /// Current live startup is in progress or not yet validated.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Pending {
            _private: Private,
        }

        /// Current gen0 setup produced checkout identity only. It did not create
        /// the intended genesis History block.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Identity<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        /// Current live startup validated enough to enter `Parent<Ready>`.
        /// This is not yet the final `Parent<Ruling>` model.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Validated<Kind> {
            _kind: PhantomData<Kind>,
            _private: Private,
        }

        /// Branch kind intentionally no longer tracked after convergence.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Any {}
    }

    pub(crate) mod head {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Unobserved {
            _private: Private,
        }

        /// Startup validation consumed whatever head/absence observation it
        /// needed; no separate `LineageState` carrier remains in the live path.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct FromStartup {
            _private: Private,
        }

        /// History was read for candidate/selection purposes.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Read {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Observed<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Unchanged {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Advanced<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod epoch {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Open<Block, Crown> {
            _block: PhantomData<Block>,
            _crown: PhantomData<Crown>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Locked<Block, Crown> {
            _block: PhantomData<Block>,
            _crown: PhantomData<Crown>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Sealed<Block> {
            _block: PhantomData<Block>,
            _private: Private,
        }
    }
}

pub(crate) mod evidence {
    use super::{PhantomData, Private};

    pub(crate) mod parent_start {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Recorded<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod baseline {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Ready<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod policy {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Ready<Policy, Budget> {
            _policy: PhantomData<Policy>,
            _budget: PhantomData<Budget>,
            _private: Private,
        }
    }

    pub(crate) mod selection {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        /// `PlannedChildren` is the current live bundle produced by child-plan
        /// resolution. It includes fields that the global carrier would likely
        /// split apart later.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Plan<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        /// Placeholder for private `ActiveSelectionStrategy`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Strategy {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Evidence<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Seal<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod completion {
        use super::Private;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Recorded {
            _private: Private,
        }
    }
}

pub(crate) mod continuation {
    use super::{PhantomData, Private};

    pub(crate) mod selection {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Maybe<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Selected<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod decision {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Stopped<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Allowed<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }

    pub(crate) mod handoff {
        use super::{PhantomData, Private};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct None {
            _private: Private,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct Recorded<T> {
            _carrier: PhantomData<T>,
            _private: Private,
        }
    }
}

pub(crate) mod report {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Identity<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Facts {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Emitted<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

// Review notes from mapping existing code to the phase diagram:
//
// - `Parent<S>`, C1-C5, `Child<S>`, `Crown<S>`, `Block<S>`, and `Startup<S>`
//   are the strong existing typestate islands.
// - `PlannedChildren` is useful but structurally awkward for this global map
//   because it bundles `Parent<Selectable>` together with plan/child data. A
//   future Runtime carrier should likely split those fields across axes.
// - `Prototype1StateRunShape` and `ActiveSelectionStrategy` already exist, but
//   both are private to `cli_facing.rs`; the runtime-state map cannot reuse them
//   directly until those concepts move to a shared module or get replacement
//   carriers.
// - `history/mod.rs` names the intended authority sequence as
//   `Startup<Observed> -> Startup<Genesis | Predecessor> -> Startup<Validated>
//   -> Parent<Ruling>`. The current live path is weaker/different: it enters
//   `Parent<Ready>` and later `Parent<Selectable>`.
// - `Startup<Validated>` is the live startup gate, but it erases whether the
//   validated startup came from genesis or predecessor. This map preserves that
//   distinction in `R4cReady<Kind>`, then uses `startup::Any` once branches
//   converge.
// - Stopped/no-successor completion has no existing `Parent<Stopped>` carrier.
//   The live path ends with `Parent<Selectable>` still held/dropped. That is an
//   intentional review point before hardening terminal states.
// - `Prototype1ContinuationDecision` is a value-level decision, not yet an
//   unforgeable continuation gate token.
// - `successor::Record` is a durable projection of successor transitions, not a
//   `Successor<S>` authority carrier. The successor docs explicitly say the
//   successor is the incoming parent before handoff acknowledgement.
