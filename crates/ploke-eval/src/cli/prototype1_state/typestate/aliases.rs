use std::marker::PhantomData;

use ploke_records::ids::CampaignId;

use crate::{
    cli::Prototype1StateCommand,
    intervention::{
        CompleteBaseline, Prototype1ChildBudget, Prototype1ChildScheduleMode,
        Prototype1ContinuationDecision, Prototype1SearchPolicy,
    },
    successor_selection::SuccessorDecision,
};

use super::super::{
    c1, c2, c3, c4,
    cli_facing::{PlannedChildOutcome, Prototype1StateReport, SelectionSealMaterial},
    history::{self as history_model, Block, LineageState},
    identity::ParentIdentity,
    inner::{self, Crown, Received},
    journal::ParentStartedEntry,
    parent as parent_role, successor,
};
use super::{
    Private, children, context, continuation, evidence, history_axis, phase, plan, report, role,
    runtime::{
        Children, Context, Continuation, Evidence, History, Plan, Report, Runtime, RuntimeRole,
    },
    shape::RuntimeShape,
};

macro_rules! runtime_alias {
    (
        $(#[$meta:meta])*
        $vis:vis type $name:ident $(<$($generic:ident $(= $default:ty)?),+ $(,)?>)? = Runtime<
            $phase:ty,
            $role:ty,
            $context:ty,
            $plan:ty,
            $children:ty,
            $history:ty,
            $evidence:ty,
            $continuation:ty,
            $report:ty $(,)?
        >;
        shape $shape:ident;
    ) => {
        $(#[$meta])*
        $vis type $name $(<$($generic $(= $default)?),+>)? = Runtime<
            $phase,
            $role,
            $context,
            $plan,
            $children,
            $history,
            $evidence,
            $continuation,
            $report,
        >;

        $vis const $shape: RuntimeShape = RuntimeShape {
            phase: stringify!($phase),
            role: stringify!($role),
            context: stringify!($context),
            plan: stringify!($plan),
            children: stringify!($children),
            history: stringify!($history),
            evidence: stringify!($evidence),
            continuation: stringify!($continuation),
            report: stringify!($report),
        };
    };
}

runtime_alias! {
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
    shape R0_SHAPE;
}

runtime_alias! {
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
    shape R1_SHAPE;
}

impl R0 {
    /// Construct the initial runtime-state carrier from the raw CLI command.
    ///
    /// This is intentionally the only constructor for R0. At this point the
    /// process has not entered a concrete evalnomicon role, and no repo or
    /// campaign coordinates have been collected.
    pub(crate) fn new(command: Prototype1StateCommand) -> Self {
        Self {
            phase: phase::R0,
            role: RuntimeRole::new(role::Unresolved),
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
            phase: phase::R1,
            role: RuntimeRole::new(role::Unresolved),
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
    pub(crate) fn campaign_id(&self) -> &CampaignId {
        self.context.state().campaign_id()
    }

    /// Temporary migration seam back to the existing live implementation.
    ///
    /// Once the subsequent R-states are wired, callers should advance through
    /// typed transitions instead of extracting the collected payload.
    pub(crate) fn into_collected(self) -> context::Collected<RunShape, CampaignConfig> {
        self.context.into_state()
    }
}

/// Structural branch after R1.
///
/// This is the first authoritative fork in the live parent loop:
///
/// - `R2a` is the `--init-parent-identity` terminal path.
/// - `R3` is the normal parent-runtime path with parent identity evidence.
///
/// The enum is intentionally named by the states it can produce, not by a
/// semantic transition label. The transition remains `Transition<R1, R1Branch,
/// _>`.
pub(crate) enum R1Branch<RunShape, CampaignConfig> {
    R2a(R2a<RunShape, CampaignConfig>),
    R3(R3<RunShape, CampaignConfig>),
}

/// Owned payload extracted from the R2a terminal branch.
pub(crate) struct R2aParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) identity: ParentIdentity,
}

/// Owned payload extracted from the R3 normal parent-runtime branch.
pub(crate) struct R3Parts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent_identity: ParentIdentity,
}

runtime_alias! {
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
    pub(crate) type R2a<RunShape = (), CampaignConfig = ()> = Runtime<
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
    >;
    shape R2A_SHAPE;
}

runtime_alias! {
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
    pub(crate) type R3<RunShape = (), CampaignConfig = ()> = Runtime<
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
    >;
    shape R3_SHAPE;
}

impl<RunShape, CampaignConfig> R2a<RunShape, CampaignConfig> {
    /// Build the terminal identity-initialization state from R1 context and the
    /// identity created by the live initialization path.
    pub(crate) fn from_collected_identity(
        collected: context::Collected<RunShape, CampaignConfig>,
        identity: ParentIdentity,
    ) -> Self {
        Self {
            phase: phase::R2a,
            role: RuntimeRole::new(role::Initialized::new(identity)),
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

    /// Temporary extraction for the still-live terminal printing code.
    pub(crate) fn into_parts(self) -> R2aParts<RunShape, CampaignConfig> {
        R2aParts {
            collected: self.context.into_state(),
            identity: self.role.into_state().into_inner(),
        }
    }
}

impl<RunShape, CampaignConfig> R3<RunShape, CampaignConfig> {
    /// Build the normal parent-runtime branch from R1 context and resolved
    /// parent identity evidence.
    pub(crate) fn from_collected_identity(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent_identity: ParentIdentity,
    ) -> Self {
        Self {
            phase: phase::R3,
            role: RuntimeRole::new(role::Identity::new(parent_identity)),
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

    /// Temporary extraction for the still-live parent-loading code.
    pub(crate) fn into_parts(self) -> R3Parts<RunShape, CampaignConfig> {
        R3Parts {
            collected: self.context.into_state(),
            parent_identity: self.role.into_state().into_inner(),
        }
    }
}

pub(crate) struct R4aParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent: parent_role::Parent<parent_role::Unchecked>,
}

pub(crate) struct R4bParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent: parent_role::Parent<parent_role::Checked>,
}

pub(crate) struct R4cParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent: parent_role::Parent<parent_role::Ready>,
}

pub(crate) enum R4aStartupBranch<RunShape, CampaignConfig> {
    GenesisChecked(R4bGenesisChecked<RunShape, CampaignConfig>),
    PredecessorReady(R4cReady<RunShape, CampaignConfig>),
}

runtime_alias! {
    /// R4a: parent loaded/constructed as unchecked.
    ///
    /// Axis changes:
    /// - `RuntimeRole<Parent, Identity<ParentIdentity>>`
    ///   `-> Parent<Unchecked>`.
    ///
    /// First concrete parent role carrier. This corresponds to the diagram edge
    /// `load parent -> parent unchecked` and the code transition
    /// `ParentIdentity -> Parent<Unchecked>`.
    pub(crate) type R4a<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R4a,
        parent_role::Parent<parent_role::Unchecked>,
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
    >;
    shape R4A_SHAPE;
}

runtime_alias! {
    /// R4b: genesis checkout validated.
    ///
    /// Axis changes:
    /// - `Parent<Unchecked> -> Parent<Checked>`.
    ///
    /// Genesis path only: `Parent<Unchecked> -> Parent<Checked>` after active
    /// checkout validation. Predecessor startup does not pass through this parent
    /// state in the current code.
    pub(crate) type R4bGenesisChecked<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R4b,
        parent_role::Parent<parent_role::Checked>,
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
    >;
    shape R4B_SHAPE;
}

runtime_alias! {
    /// R4c: parent startup complete in the current live implementation.
    ///
    /// Axis changes:
    /// - Genesis: `Parent<Checked> -> Parent<Ready>`.
    /// - Predecessor: `Parent<Unchecked> -> Parent<Ready>`.
    /// - `History<startup::Pending, head::Unobserved, epoch::None>`
    ///   `-> History<startup::Validated<Any>, head::FromStartup, epoch::None>`.
    ///
    /// Both startup branches converge here as the same induction invariant:
    /// `Parent<Ready>`. The runtime does not need a different post-startup state for
    /// genesis vs predecessor. Predecessor bookkeeping, when present, is ordinary
    /// value metadata in `context::Collected::handoff_invocation`.
    pub(crate) type R4cReady<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R4c,
        parent_role::Parent<parent_role::Ready>,
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
    >;
    shape R4C_SHAPE;
}

impl<RunShape, CampaignConfig> R4a<RunShape, CampaignConfig> {
    pub(crate) fn from_collected_parent(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent: parent_role::Parent<parent_role::Unchecked>,
    ) -> Self {
        Self {
            phase: phase::R4a,
            role: parent,
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

    pub(crate) fn into_parts(self) -> R4aParts<RunShape, CampaignConfig> {
        R4aParts {
            collected: self.context.into_state(),
            parent: self.role,
        }
    }
}

impl<RunShape, CampaignConfig> R4bGenesisChecked<RunShape, CampaignConfig> {
    pub(crate) fn from_collected_parent(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent: parent_role::Parent<parent_role::Checked>,
    ) -> Self {
        Self {
            phase: phase::R4b,
            role: parent,
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

    pub(crate) fn into_parts(self) -> R4bParts<RunShape, CampaignConfig> {
        R4bParts {
            collected: self.context.into_state(),
            parent: self.role,
        }
    }
}

impl<RunShape, CampaignConfig> R4cReady<RunShape, CampaignConfig> {
    pub(crate) fn from_collected_parent(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent: parent_role::Parent<parent_role::Ready>,
    ) -> Self {
        Self {
            phase: phase::R4c,
            role: parent,
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

    pub(crate) fn into_parts(self) -> R4cParts<RunShape, CampaignConfig> {
        R4cParts {
            collected: self.context.into_state(),
            parent: self.role,
        }
    }
}

pub(crate) struct ReadyParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent: parent_role::Parent<parent_role::Ready>,
}

pub(crate) struct SelectableParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent: parent_role::Parent<parent_role::Selectable>,
}

macro_rules! ready_state_impl {
    ($alias:ident, $phase:expr) => {
        impl<RunShape, CampaignConfig> $alias<RunShape, CampaignConfig> {
            pub(crate) fn from_collected_parent(
                collected: context::Collected<RunShape, CampaignConfig>,
                parent: parent_role::Parent<parent_role::Ready>,
            ) -> Self {
                Self {
                    phase: $phase,
                    role: parent,
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

            pub(crate) fn into_parts(self) -> ReadyParts<RunShape, CampaignConfig> {
                ReadyParts {
                    collected: self.context.into_state(),
                    parent: self.role,
                }
            }
        }
    };
}

ready_state_impl!(R5, phase::R5);
ready_state_impl!(R6, phase::R6);
ready_state_impl!(R7, phase::R7);

impl<RunShape, CampaignConfig> R8<RunShape, CampaignConfig> {
    pub(crate) fn from_collected_parent(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent: parent_role::Parent<parent_role::Selectable>,
    ) -> Self {
        Self {
            phase: phase::R8,
            role: parent,
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

    pub(crate) fn into_parts(self) -> SelectableParts<RunShape, CampaignConfig> {
        SelectableParts {
            collected: self.context.into_state(),
            parent: self.role,
        }
    }
}

runtime_alias! {
    /// R5: parent-start evidence recorded.
    ///
    /// Axis changes:
    /// - `Evidence<parent_start::None, ...>`
    ///   `-> Evidence<parent_start::Recorded<ParentStartedEntry>, ...>`.
    ///
    /// Existing carrier: `ParentStartedEntry` in the transition journal, plus a
    /// resource sample for `ParentStart`.
    pub(crate) type R5<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R5,
        parent_role::Parent<parent_role::Ready>,
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
    >;
    shape R5_SHAPE;
}

runtime_alias! {
    /// R6: parent baseline established.
    ///
    /// Axis changes:
    /// - `Evidence<..., baseline::None, ...>`
    ///   `-> Evidence<..., baseline::Ready<CompleteBaseline>, ...>`.
    ///
    /// Existing carrier: `CompleteBaseline = Baseline<baseline::Complete>`.
    pub(crate) type R6<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R6,
        parent_role::Parent<parent_role::Ready>,
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
    shape R6_SHAPE;
}

runtime_alias! {
    /// R7: policy and child budget ready.
    ///
    /// Axis changes:
    /// - `Evidence<..., policy::None, ...>`
    ///   `-> Evidence<..., policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>, ...>`.
    ///
    /// Existing carriers: `Prototype1SearchPolicy`, `Prototype1ChildBudget`, and
    /// `Prototype1ChildScheduleMode`. The live code currently stores these in
    /// locals and sometimes derives them from an admitted run profile.
    pub(crate) type R7<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R7,
        parent_role::Parent<parent_role::Ready>,
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
    shape R7_SHAPE;
}

runtime_alias! {
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
    pub(crate) type R8<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R8,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
            evidence::selection::Plan<context::ChildPlanFacts>,
            evidence::completion::None,
        >,
        Continuation<
            continuation::selection::None,
            continuation::decision::None,
            continuation::handoff::None,
        >,
        Report<report::None>,
    >;
    shape R8_SHAPE;
}

runtime_alias! {
    /// R9: child schedule shaped.
    ///
    /// Axis changes:
    /// - `Plan<authority::Received<_>, schedule::None>`
    ///   `-> Plan<authority::Received<_>, schedule::Ready<Budget, Mode>>`.
    ///
    /// No existing typestate island moves here, but the runnable child set has been
    /// budgeted/truncated and schedule mode has been selected.
    pub(crate) type R9<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R9,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
            evidence::selection::Plan<context::ChildPlanFacts>,
            evidence::completion::None,
        >,
        Continuation<
            continuation::selection::None,
            continuation::decision::None,
            continuation::handoff::None,
        >,
        Report<report::None>,
    >;
    shape R9_SHAPE;
}

runtime_alias! {
    /// R10: selection strategy ready.
    ///
    /// Axis changes:
    /// - `Evidence<..., selection::Plan<PlannedChildren>, ...>`
    ///   `-> Evidence<..., selection::Strategy, ...>`.
    ///
    /// Existing implementation type: `ActiveSelectionStrategy`, but it is private
    /// to `cli_facing.rs`. The local `evidence::selection::Strategy` marker stands
    /// in for that existing-but-not-reusable carrier.
    pub(crate) type R10<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R10,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
    shape R10_SHAPE;
}

macro_rules! selectable_state_impl {
    ($alias:ident, $phase:expr) => {
        impl<RunShape, CampaignConfig> $alias<RunShape, CampaignConfig> {
            pub(crate) fn from_collected_parent(
                collected: context::Collected<RunShape, CampaignConfig>,
                parent: parent_role::Parent<parent_role::Selectable>,
            ) -> Self {
                Self {
                    phase: $phase,
                    role: parent,
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

            pub(crate) fn into_parts(self) -> SelectableParts<RunShape, CampaignConfig> {
                SelectableParts {
                    collected: self.context.into_state(),
                    parent: self.role,
                }
            }
        }
    };
}

selectable_state_impl!(R9, phase::R9);
selectable_state_impl!(R10, phase::R10);

runtime_alias! {
    /// R11a: rejected-only branch projected into selection evidence.
    ///
    /// Axis changes:
    /// - `Children<set::Planned<ChildFiles>, attempt::None>`
    ///   `-> Children<set::Rejected, attempt::None>`.
    /// - `Evidence<..., selection::Strategy, ...>`
    ///   `-> Evidence<..., selection::Evidence<SelectionSealMaterial>, ...>`.
    ///
    pub(crate) type R11aRejectedOnly<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R11a,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
    shape R11A_SHAPE;
}

runtime_alias! {
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
    pub(crate) type R11FanoutComplete<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R11,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
    shape R11_SHAPE;
}

selectable_state_impl!(R11aRejectedOnly, phase::R11a);
selectable_state_impl!(R11FanoutComplete, phase::R11);

pub(crate) enum R10FanoutBranch<RunShape, CampaignConfig> {
    RejectedOnly(R11aRejectedOnly<RunShape, CampaignConfig>),
    FanoutComplete(R11FanoutComplete<RunShape, CampaignConfig>),
}

runtime_alias! {
    /// R12: report-child/outcome projection ready.
    ///
    /// Axis changes:
    /// - `Children<set::Outcomes<PlannedChildOutcome>, attempt::Complete>`
    ///   `-> Children<set::Report<PlannedChildOutcome>, attempt::Complete>`.
    /// - `Report<None> -> Report<Facts>`.
    ///
    /// Existing final report carrier is `Prototype1StateReport`, but Stage 12 only
    /// has report facts. The actual report is assembled/emitted at R14.
    pub(crate) type R12<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R12,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
    shape R12_SHAPE;
}

selectable_state_impl!(R12, phase::R12);

impl<RunShape, CampaignConfig> R12<RunShape, CampaignConfig> {
    pub(crate) fn has_successor_selection(&self) -> bool {
        self.context.state().has_successor_selection()
    }
}

runtime_alias! {
    /// R13a: successor decision stops or selects no successor.
    ///
    /// Axis changes:
    /// - `History<startup::Validated<_>, head::FromStartup, epoch::None>`
    ///   `-> History<startup::Validated<_>, head::Read, epoch::None>`.
    /// - `Continuation<selection::Maybe<_>, decision::None, handoff::None>`
    ///   `-> Continuation<selection::Maybe<_>, decision::Stopped<_>, handoff::None>`.
    ///
    pub(crate) type R13aStopped<RunShape = (), CampaignConfig = ()> = Runtime<
        phase::R13a,
        parent_role::Parent<parent_role::Selectable>,
        Context<context::Collected<RunShape, CampaignConfig>>,
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
    shape R13A_SHAPE;
}

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
pub(crate) type R13bHandoffCommitted<RunShape = (), CampaignConfig = ()> = Runtime<
    phase::R13b,
    parent_role::Parent<parent_role::Retired>,
    Context<context::Collected<RunShape, CampaignConfig>>,
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

selectable_state_impl!(R13aStopped, phase::R13a);

pub(crate) struct RetiredParts<RunShape, CampaignConfig> {
    pub(crate) collected: context::Collected<RunShape, CampaignConfig>,
    pub(crate) parent: parent_role::Parent<parent_role::Retired>,
}

impl<RunShape, CampaignConfig> R13bHandoffCommitted<RunShape, CampaignConfig> {
    pub(crate) fn from_collected_parent(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent: parent_role::Parent<parent_role::Retired>,
    ) -> Self {
        Self {
            phase: phase::R13b,
            role: parent,
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

    pub(crate) fn into_parts(self) -> RetiredParts<RunShape, CampaignConfig> {
        RetiredParts {
            collected: self.context.into_state(),
            parent: self.role,
        }
    }
}

pub(crate) enum R12ContinuationBranch<RunShape, CampaignConfig> {
    Stopped(R13aStopped<RunShape, CampaignConfig>),
    HandoffCommitted(R13bHandoffCommitted<RunShape, CampaignConfig>),
}

/// R14a: final report after stopped/no-selection continuation.
///
/// Axis changes:
/// - `History<startup::Validated<_>, head::Read, epoch::None>`
///   `-> History<startup::Validated<_>, head::Unchanged, epoch::None>`.
/// - `Evidence<..., completion::None> -> Evidence<..., completion::Recorded>`.
/// - `Report<Facts> -> Report<Emitted<Prototype1StateReport>>`.
///
pub(crate) type R14aFinalStopped<RunShape = (), CampaignConfig = ()> = Runtime<
    phase::R14,
    parent_role::Parent<parent_role::Selectable>,
    Context<context::Collected<RunShape, CampaignConfig>>,
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
pub(crate) type R14bFinalHandoff<RunShape = (), CampaignConfig = ()> = Runtime<
    phase::R14,
    parent_role::Parent<parent_role::Retired>,
    Context<context::Collected<RunShape, CampaignConfig>>,
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

selectable_state_impl!(R14aFinalStopped, phase::R14);

impl<RunShape, CampaignConfig> R14bFinalHandoff<RunShape, CampaignConfig> {
    pub(crate) fn from_collected_parent(
        collected: context::Collected<RunShape, CampaignConfig>,
        parent: parent_role::Parent<parent_role::Retired>,
    ) -> Self {
        Self {
            phase: phase::R14,
            role: parent,
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
}

pub(crate) enum R14FinalBranch<RunShape, CampaignConfig> {
    Stopped(R14aFinalStopped<RunShape, CampaignConfig>),
    Handoff(R14bFinalHandoff<RunShape, CampaignConfig>),
}

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
