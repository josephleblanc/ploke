use std::marker::PhantomData;

use super::Private;

// ANCHOR: prototype1_runtime_product
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
    pub(super) phase: Phase,
    pub(super) role: Role,
    pub(super) context: Context,
    pub(super) plan: Plan,
    pub(super) children: Children,
    pub(super) history: History,
    pub(super) evidence: Evidence,
    pub(super) continuation: Continuation,
    pub(super) report: Report,
    pub(super) _private: Private,
}
// ANCHOR_END: prototype1_runtime_product

// ANCHOR: prototype1_runtime_axes
/// Role axis before a concrete `Parent<S>` / `Child<S>` carrier exists.
///
/// Once the parent role is constructed, the `Runtime` role parameter uses the
/// existing `parent::Parent<S>` carrier directly instead of wrapping it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeRole<Kind, State> {
    pub(super) state: State,
    pub(super) _kind: PhantomData<Kind>,
    pub(super) _private: Private,
}

impl<Kind, State> RuntimeRole<Kind, State> {
    pub(super) fn new(state: State) -> Self {
        Self {
            state,
            _kind: PhantomData,
            _private: Private,
        }
    }

    pub(super) fn into_state(self) -> State {
        self.state
    }
}

/// Run context axis: command-only first, then collected command-derived inputs.
#[derive(Debug)]
pub(crate) struct Context<State> {
    pub(super) state: State,
    pub(super) _private: Private,
}

impl<State> Context<State> {
    pub(super) fn new(state: State) -> Self {
        Self {
            state,
            _private: Private,
        }
    }

    pub(super) fn state(&self) -> &State {
        &self.state
    }

    pub(super) fn into_state(self) -> State {
        self.state
    }
}

/// Child-plan axis split into authority and schedule dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Plan<Authority, Schedule> {
    pub(super) _authority: PhantomData<Authority>,
    pub(super) _schedule: PhantomData<Schedule>,
    pub(super) _private: Private,
}

/// Child-set axis split from the per-child C1-C5 attempt chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Children<Set, Attempt> {
    pub(super) _set: PhantomData<Set>,
    pub(super) _attempt: PhantomData<Attempt>,
    pub(super) _private: Private,
}

/// History axis, following the History docs rather than a single flat status.
///
/// - `Startup` tracks current startup/admission evidence.
/// - `Head` tracks whether the lineage-head projection has been observed or
///   advanced by this runtime.
/// - `Epoch` tracks the in-flight Crown/Block handoff authority state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct History<Startup, Head, Epoch> {
    pub(super) _startup: PhantomData<Startup>,
    pub(super) _head: PhantomData<Head>,
    pub(super) _epoch: PhantomData<Epoch>,
    pub(super) _private: Private,
}

/// Readiness/evidence axis for facts that are real prerequisites but not yet
/// first-class role/History/child carriers in the live implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Evidence<ParentStart, Baseline, Policy, Selection, Completion> {
    pub(super) _parent_start: PhantomData<ParentStart>,
    pub(super) _baseline: PhantomData<Baseline>,
    pub(super) _policy: PhantomData<Policy>,
    pub(super) _selection: PhantomData<Selection>,
    pub(super) _completion: PhantomData<Completion>,
    pub(super) _private: Private,
}

/// Continuation axis: selection value, continuation decision, and handoff
/// record are separate facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Continuation<Selection, Decision, Handoff> {
    pub(super) _selection: PhantomData<Selection>,
    pub(super) _decision: PhantomData<Decision>,
    pub(super) _handoff: PhantomData<Handoff>,
    pub(super) _private: Private,
}

/// Report axis. Report facts and emitted report are separate from successor
/// completion/handoff evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Report<State> {
    pub(super) _state: PhantomData<State>,
    pub(super) _private: Private,
}
// ANCHOR_END: prototype1_runtime_axes
