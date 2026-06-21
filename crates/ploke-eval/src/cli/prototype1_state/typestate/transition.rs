//! Functional transition combinators for the Prototype 1 typestate graph.
//!
//! This module separates two concepts:
//!
//! - typed state values such as `R0`, `R1`, or `R4cReady`;
//! - typed arrows that consume one state value and produce the next.
//!
//! Direct functions like `r0_to_r1(r0) -> Result<R1, PrepareError>` implement
//! `Step<R0>` through the blanket impl below, while `Transition` remains
//! available when a caller wants an explicit transition value or a closure with
//! captured environment.

use std::{future::Future, marker::PhantomData};

use super::Private;

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

// ANCHOR: prototype1_transition_arrow
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
// ANCHOR_END: prototype1_transition_arrow

// ANCHOR: prototype1_step_trait
// ANCHOR: prototype1_step_trait_contract
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
// ANCHOR_END: prototype1_step_trait_contract

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

/// Treat any compatible direct function or closure as a typed step.
///
/// This is what lets live edges be plain functions while preserving
/// `Step::then` adjacency checks and `StepInput::advance` value-first syntax.
impl<From, To, F, Error> Step<From> for F
where
    F: FnOnce(From) -> Result<To, Error>,
{
    type To = To;
    type Error = Error;

    fn apply(self, from: From) -> Result<Self::To, Self::Error> {
        self(from)
    }
}
// ANCHOR_END: prototype1_step_trait

// ANCHOR: prototype1_step_input
/// Value-first helper for applying a typed step.
///
/// This is the typestate analogue of calling `x.map(f)`: the state value owns
/// the receiver position, while the edge function or transition value is passed
/// in as data.
pub(crate) trait StepInput: Sized {
    /// Consume `self` by applying a typed step.
    ///
    /// Prefer this at call sites that should read from the state outward:
    ///
    /// ```ignore
    /// let r1 = r0.advance(r0_to_r1)?;
    /// let r10 = r8.advance(r8_to_r9.then(r9_to_r10))?;
    /// ```
    fn advance<S>(self, step: S) -> Result<S::To, S::Error>
    where
        S: Step<Self>,
    {
        step.apply(self)
    }
}

/// Every sized state value can be the receiver for `advance`.
impl<T> StepInput for T {}
// ANCHOR_END: prototype1_step_input

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
///     GenesisChecked(R4bGenesisChecked),
///     Ready(R4cReady),
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

/// Async sibling of `Transition` for edges that must await live work.
///
/// This deliberately mirrors the synchronous shape instead of replacing it:
///
/// ```text
/// From -> Future<Output = Result<To, Error>>
/// ```
///
/// The first live use is the parent-baseline edge, because baseline closure work
/// is already async in the current controller.
#[must_use = "an AsyncTransition does nothing until AsyncStep::apply is awaited"]
pub(crate) struct AsyncTransition<From, To, F, Error = crate::spec::PrepareError> {
    f: F,
    _from: PhantomData<From>,
    _to: PhantomData<To>,
    _error: PhantomData<Error>,
    _private: Private,
}

impl<From, To, F, Fut, Error> AsyncTransition<From, To, F, Error>
where
    F: FnOnce(From) -> Fut,
    Fut: Future<Output = Result<To, Error>>,
{
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

/// Convenience constructor for async typed transitions.
pub(crate) fn async_transition<From, To, F, Fut, Error>(f: F) -> AsyncTransition<From, To, F, Error>
where
    F: FnOnce(From) -> Fut,
    Fut: Future<Output = Result<To, Error>>,
{
    AsyncTransition::new(f)
}

/// A value that can asynchronously advance one typed state to another.
pub(crate) trait AsyncStep<From>: Sized {
    /// The state produced when the returned future resolves successfully.
    type To;
    /// The failure type for this async edge.
    type Error;
    /// Future returned by applying this async edge.
    type Fut: Future<Output = Result<Self::To, Self::Error>>;

    /// Consume `from` and return the future that attempts the transition.
    fn apply(self, from: From) -> Self::Fut;
}

impl<From, To, F, Fut, Error> AsyncStep<From> for AsyncTransition<From, To, F, Error>
where
    F: FnOnce(From) -> Fut,
    Fut: Future<Output = Result<To, Error>>,
{
    type To = To;
    type Error = Error;
    type Fut = Fut;

    fn apply(self, from: From) -> Self::Fut {
        (self.f)(from)
    }
}

/// Treat any compatible async function or closure as an async typed step.
impl<From, To, F, Fut, Error> AsyncStep<From> for F
where
    F: FnOnce(From) -> Fut,
    Fut: Future<Output = Result<To, Error>>,
{
    type To = To;
    type Error = Error;
    type Fut = Fut;

    fn apply(self, from: From) -> Self::Fut {
        self(from)
    }
}

/// Value-first helper for applying an async typed step.
pub(crate) trait AsyncStepInput: Sized {
    /// Consume `self` by applying an async typed step and returning its future.
    ///
    /// ```ignore
    /// let r6 = r5.advance_async(r5_to_r6).await?;
    /// ```
    fn advance_async<S>(self, step: S) -> S::Fut
    where
        S: AsyncStep<Self>,
    {
        step.apply(self)
    }
}

/// Every sized state value can be the receiver for `advance_async`.
impl<T> AsyncStepInput for T {}
