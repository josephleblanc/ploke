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
    type To;
    type Error;
    type Fut: Future<Output = Result<Self::To, Self::Error>>;

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
