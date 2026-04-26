//! Typed prototype1 configuration states.
//!
//! These states are only partially wired into the live controller. They exist
//! so the Parent -> Child -> Completed child -> Selected successor -> Successor
//! bootstrap seam can be modeled explicitly as move-only configuration
//! transitions while the older non-trampoline controller is being replaced.
//! Live configuration values should be constructed through trusted
//! loaders/transitions in their defining modules rather than by ad hoc struct
//! literals elsewhere.
//!
//! ## Contents
//!
//! This crate contains the defining types and transitions that govern the self-improvement loop
//! protocol.
//!
//! ## Design constraints imposed by long-term planning
//!
//!  - Do not assume one global active Parent.
//!  - Do not store “current best branch” as a singleton.
//!  - Do not overwrite evaluation state; append observations and decisions.
//!  - Do not store scores without evaluator/eval-set/policy identity.
//!  - Do not make the analysis engine part of the trusted root.
//!  - Do not let a runtime’s self-report become promotion without independent verification.
//!  - Do not couple worktree layout to the semantic tree.
//!  - Do not make successor authority imply global authority.
//!
//! ## Key terms and types
//!
//! The types in this module are reflections of concepts for a complex structure of
//! self-propagating, self-evaluating configurations of artifact/runtime pairs that exist in a
//! branching tree structure with a shared append-only history.
//!
//! Configuration: The operating world of one runtime. This includes type-state information that
//! determines what is and is not an admissible intervention in that runtime.
//!
//! Artifact: An Artifact is the collection of files used to generate a Runtime. This is not an
//! immutable object, and can be operated over by a Runtime by making an Intervention that applies a
//! Patch. Applying a Patch to an Artifact produces another Artifact.
//!
//! Runtime: An executing process from a binary. A Runtime is built from an Artifact, and shares an
//! "Identity" relationship with the Artifact that generated it. A Runtime may perform any
//! Intervention, but the admissible Interventions are restricted by the current Configuration in
//! which the Runtime exists. A Runtime may operate over the Surface of an Artifact to produce a
//! Patch, and apply the Patch to create a new Artifact. The Artifact the Runtime operates over to
//! produce a Patch may or may not be the same Artifact with created the Runtime. Notably, a Runtime
//! cannot be evaluated externally, and must itself produce the Record containing the metrics used
//! to evaluate it. This is because the Metrics are of a Procedure the Runtime is the operational
//! environment for.
//!
//! Journal: The append-only group of Records that are produced by all Runtimes. The Journal
//! contains evaluations, handoff documents, Records of Interventions, and Journals of a given
//! Runtime's trajectory.
//!
//! Tree: The branching structure that contains the lineage of all Artifacts. This structure is
//! based on the conceptual framework of a git tree, but is abstracted from it and backend agnostic.
//! The important characteristics of a Tree are that it contains the edges formed by applying a
//! Patch to an Artifact, thereby preserving the lineage of all Artifacts. Notably, a Tree allows
//! for a Runtime to operate on any Artifact, regardless of whether that is the same Artifact that
//! produced the Runtime, a descendent of the Runtime's Artifact, or an ancestor of the Runtime's Artifact.
//! A Tree is operated over by a Runtime through an Intervention when the Runtime performs actions
//! like checking out a branch, restoring an Artifact, or commiting a new Artifact. These operations
//! also change the Artifact, and therefore are Interventions over the Configuration.
//!
//! Intervention: Any change to the Configuration. This means changing the Artifact or Runtime.
//! Every Intervention must include a Record before and after the Intervention, which is added to
//! the Journal.
//!
//! Actions: A Runtime may perform an Action that may or may not be an Intervention.
//!     - create: generate a new Artifact. This is done by generating, staging, and applying a
//!     Patch. Only the last step of "applying" the Patch is an Intervention, because it changes the
//!     Configuration of which Artifact the Runtime is operating over. A 'create' may or may not
//!     generate a worktree for the Child Runtime.
//!     - select: determine the next Runtime to become the Parent Runtime. This involves accessing
//!     Child self-evaluations through the Journal, executing a Policy over the view of that
//!     Journal, and choosing the Artifact in the Tree to become the next Parent Runtime.
//!     - update: changes the Configuration from the previous Artifact to the next Artifact used to
//!     generate the Parent Runtime. This involves checking out that Artifact from the Tree.
//!     - build: builds a Successor Runtime from the selected Artifact in the Configuration, and
//!     launches the Successor Runtime that may become the next Parent Runtime.
//!     - cleanup: cleans up child worktrees
//!     - exit: Parent Runtime terminates, possibly after performing the handshake with the next
//!     Parent Runtime. If no next Parent Runtime is selected, the looping protocol is over.
//!
//!
//! ## Runtime: Parent/Child/Successor
//!
//! Responsibilities for different roles of runtime.
//!
//! Parent Runtime:
//!     - creates, selects, updates, builds, cleans up, exits
//!     - mutates Tree state, but must not destructively mutate Tree history
//!         - (e.g. change branch, remove worktree, but not "reset --hard")
//!
//! Child Runtime:
//!     - self-evaluates
//!     - records evaluations
//!     - acknowledges its invocation
//!     - does not mutate Tree state
//!
//! Successor Runtime:
//!     - fresh binary built from the selected Child artifact
//!     - acknowledges handoff with Parent Runtime
//!     - becomes the next Parent only after policy and bootstrap validation
//!
//! The persisted invocation contract for the live Prototype 1 seam is
//! narrower than the full long-term role vocabulary but already has two
//! executable fresh-binary roles:
//!
//! - `Child`: leaf evaluator that records one self-evaluation and exits.
//! - `Successor`: selected-continuation bootstrap that acknowledges handoff
//!   within a bounded standby window. The successor is not yet a fully
//!   autonomous next-generation controller, but it must be built from the
//!   selected artifact rather than reusing the old parent binary.
//!
//! ## Actions
//!
//! ### create
//!
//! Has several sub-steps:
//! 1. Read the accessible Surface
//! 2. Generate a Patch
//! - create worktree?
//!     - on fail, follow policy
//! - apply patch in worktree?
//!     - on fail, follow policy
//! - validate patch in worktree with cargo check?
//!     - on fail, follow policy
//! - build child binary in worktree?
//!     - on fail, follow policy
//! - commit patch in worktree?
//!     - on fail, follow policy
//! - write handshake to Child with shared Journal location in worktree?
//!     - await on Child with timeout
//!     - on fail, follow policy
//! - read handshake from Child in Journal?
//!     - await on Child with timeout
//!     - on fail, follow policy
//! - read evaluation of Child in Journal
//!     - await on Child termination with timeout
//!     - on fail, follow policy
//! - optionally clean up worktree as per Policy
//!     - on fail, follow policy
//!
//! ### select
//! 1. reads Journal to load Child self-evaluations
//!     - on fail, follow policy
//! 2. executes a Policy to choose the Successor
//!
//! ### update
//! 1. checkout Successor Artifact branch/node from Tree
//!
//! ## build
//! 1. use cargo check
//!     - on fail, follow Policy
//! 2. use cargo build
//!     - on fail, follow Policy
//! 3. execute Successor Runtime
//! 4. handoff with Successor Runtime
//!     - await on acknowledge
//!     - on fail, follow Policy
//!
//! ### exit
//! 1. record final status
//! 2. terminate
//!
//! Prototype 1 persisted state is rooted under
//! `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`.
//!
//! ## Implementation map
//!
//! Prototype 1 currently spans two overlapping implementation paths:
//!
//! - [`crate::cli`]
//!   Operator-facing `loop prototype1` command surface and the live controller
//!   orchestration around campaigns, branches, and scheduler policy.
//! - [`crate::cli::prototype1_process`]
//!   Parent/child/successor process seam: stage one node workspace, build one
//!   child binary, spawn one runner, read back persisted results, and bootstrap
//!   the selected successor binary after policy accepts it.
//! - [`crate::intervention`]
//!   Durable scheduler-owned records for node registration, runner requests and
//!   results, branch registry, continuation policy, and evaluation summaries.
//! - [`crate::cli::prototype1_state::c1`] through [`crate::cli::prototype1_state::c4`]
//!   Typed transition sketch for materialize -> build -> spawn -> observe.
//! - [`crate::cli::prototype1_state::backend`]
//!   Backend-managed child workspace realization, currently implemented with
//!   git worktrees rooted under each node directory.
//! - [`crate::cli::prototype1_state::invocation`]
//!   Attempt-scoped bootstrap contracts for fresh binaries, including child and
//!   successor invocations plus successor-ready acknowledgements.
//! - [`crate::cli::prototype1_state::journal`]
//!   Append-only typed transition journal and replay helpers.
//!
//! The important current split is:
//!
//! - the older `loop prototype1` controller still owns much of the multi-branch
//!   evaluation, ranking, and reporting machinery
//! - the typed state path defines the intended trampoline authority carriers and
//!   attempt-level protocol surface
//! - cleanup work should migrate useful policy/history/evaluation pieces from
//!   the old controller into the typed state model rather than extending the old
//!   non-trampoline path as the final architecture
//!
//! ## Persisted artifact map
//!
//! Campaign-scoped Prototype 1 state lives under:
//!
//! ```text
//! ~/.ploke-eval/campaigns/<campaign-id>/prototype1/
//! ```
//!
//! The main durable files and directories there are:
//!
//! - `scheduler.json`
//!   Scheduler frontier, completed/failed node ids, node summaries, and search
//!   policy.
//! - `branches.json`
//!   Synthesized branch registry for one source state, including proposed
//!   content, apply ids, and latest evaluation summaries.
//! - `evaluations/<branch-id>.json`
//!   Persisted branch-vs-baseline comparison results for treatment campaigns.
//! - `transition-journal.jsonl`
//!   Append-only typed transition log for child-ready and successor-handoff
//!   events.
//! - `nodes/<node-id>/node.json`
//!   Durable node summary owned by the scheduler.
//! - `nodes/<node-id>/runner-request.json`
//!   Mutable node-level request used by the live runner seam.
//! - `nodes/<node-id>/runner-result.json`
//!   Latest node-level runner outcome used by the live controller.
//! - `nodes/<node-id>/invocations/<runtime-id>.json`
//!   Attempt-scoped bootstrap contract for one fresh child or successor
//!   process.
//! - `nodes/<node-id>/results/<runtime-id>.json`
//!   Attempt-scoped result record for one invocation runtime.
//! - `nodes/<node-id>/successor-ready/<runtime-id>.json`
//!   Successor acknowledgement file written after detached successor handoff.
//! - `nodes/<node-id>/worktree/`
//!   Backend-managed node-owned child workspace root when the worktree path is
//!   realized through [`backend`].
//! - `nodes/<node-id>/bin/` and `nodes/<node-id>/target/`
//!   Build artifacts from the live child-build path rather than authoritative
//!   scheduler state.
//!
//! Related state also exists outside the campaign-local `prototype1/` subtree:
//!
//! - `~/.ploke-eval/campaigns/<campaign-id>/campaign.json`
//!   Baseline campaign manifest reused by the live controller path.
//! - `~/.ploke-eval/campaigns/<campaign-id>/closure-state.json`
//!   Closure-derived campaign state reused by the live controller path.
//! - `~/.ploke-eval/instances/prototype1/<campaign-id>/...`
//!   Baseline and treatment run records referenced by evaluation artifacts.
//!
//! ## Current persistence gaps
//!
//! The persisted surface above is enough to recover individual node outcomes,
//! but it is not yet the full state model needed for safe fan-out. In
//! particular, the live implementation still lacks:
//!
//! - a controller lease or epoch proving which parent owns the campaign
//! - a first-class attempt record that unifies invocation, pid, workspace
//!   lease, heartbeat, and terminal status
//! - a durable scheduler decision for selected successors distinct from
//!   per-node `keep` evaluations
//! - explicit bounded fan-out policy state such as concurrent-child budget,
//!   total-completed budget, and absolute deadline
//!
pub(crate) mod authority;
pub(crate) mod backend;
pub(crate) mod c1;
pub(crate) mod c2;
pub(crate) mod c3;
pub(crate) mod c4;
pub(crate) mod cli_facing;
pub(crate) mod event;
pub(crate) mod invocation;
pub(crate) mod journal;
pub(crate) mod record;
pub(crate) mod workspace;
