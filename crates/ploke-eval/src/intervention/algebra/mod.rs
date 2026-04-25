//! Minimal intervention substrate for Prototype 1.
//!
//! This layer intentionally starts small and only names the objects we need to
//! mediate safe configuration transitions:
//! - [`Configuration`]: one joint artifact/binary world-state
//! - [`Surface`]: a bounded read/edit mediation layer over the artifact-bearing
//!   part of a configuration
//! - [`Intervention`]: a typed transition from one configuration state to the
//!   next, with typed failure and explicit journal commit
//! - [`RecordStore`]: an append-only external journal used to persist
//!   transition records outside the running binaries
//!
//! Richer objects such as history, trajectories, procedures, and protocols can
//! be layered on top once the first concrete transitions are implemented.

use serde::{Deserialize, Serialize};

/// Append-only journal phase for one committed transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitPhase {
    Before,
    After,
}

/// Failure either during the transition itself or while committing its journal
/// entries.
#[derive(Debug)]
pub enum CommitError<TransitionError, RecordError> {
    Transition(TransitionError),
    Record {
        phase: CommitPhase,
        source: RecordError,
    },
}

/// Committed result of one intervention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<To, Rejected> {
    Advanced(To),
    Rejected(Rejected),
}

/// One joint artifact/binary world-state.
///
/// A concrete configuration value represents one realized pairing of:
/// - the artifact state visible in the world
/// - the binary state currently associated with that artifact world
///
/// If either side changes, the system has moved to a different configuration.
pub trait Configuration {
    /// The underlying artifact-bearing state, such as a source tree snapshot.
    type ArtifactState;

    /// The binary-side state associated with that artifact world.
    type BinaryState;

    /// Read-only access to the artifact-bearing side of the configuration.
    fn artifact_state(&self) -> &Self::ArtifactState;

    /// Read-only access to the binary-side state of the configuration.
    fn binary_state(&self) -> &Self::BinaryState;
}

/// A bounded mediation layer over the artifact-bearing part of a configuration.
///
/// A surface defines what part of a configuration is addressable and readable.
/// Write paths should remain mediated by concrete interventions rather than
/// being exposed as a generally available mutation API on the surface itself.
pub trait Surface<C: Configuration> {
    /// Stable target identity within the bounded surface.
    type Target;

    /// Read-side projection used by procedures and interventions.
    type ReadView;

    /// Surface-specific failure type for target resolution or view production.
    type Error;

    /// Produce the bounded read-side projection for `target` over `config`.
    fn read_view(&self, config: &C, target: &Self::Target) -> Result<Self::ReadView, Self::Error>;
}

/// Append-only external journal for transition records.
///
/// This is the durable communication seam that survives across parent/child
/// binaries and keeps global transition policy outside the in-memory runtime of
/// any one process.
pub trait RecordStore {
    /// One append-only journal entry.
    type Entry;

    /// Store-specific append failure.
    type Error;

    /// Append one new record to the journal.
    fn append(&mut self, entry: Self::Entry) -> Result<(), Self::Error>;
}

/// Typed transition from one configuration state to the next.
///
/// Concrete interventions mediate one bounded world transition. They consume a
/// source configuration, may record append-only evidence, and either produce a
/// successor configuration or fail with a typed error.
pub trait Intervention<From, To>
where
    From: Configuration,
    To: Configuration,
{
    /// The bounded surface this transition reads or edits over the source
    /// configuration.
    type Surface: Surface<From>;

    /// The concrete journal used to commit this transition.
    type Journal: RecordStore;

    /// Typed transition failure.
    type Error;

    /// Typed committed non-success outcome.
    type Rejected;

    /// Consume one source configuration, append a `before` entry, perform the
    /// bounded change, append an `after` entry, and only then yield the
    /// successor configuration.
    fn transition(
        &self,
        from: From,
        records: &mut Self::Journal,
    ) -> Result<
        Outcome<To, Self::Rejected>,
        CommitError<Self::Error, <Self::Journal as RecordStore>::Error>,
    >;
}
