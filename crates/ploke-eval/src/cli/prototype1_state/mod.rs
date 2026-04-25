//! Typed prototype1 configuration states.
//!
//! These states are not wired into the live controller yet. They exist so the
//! parent/child seam can be modeled explicitly as move-only configuration
//! transitions before the runtime path is rewritten to use them.
//!
//! ## Contents
//!
//! This crate contains the defining types and transitions that govern the self-improvement loop
//! protocol.
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
//!     the Candidate Runtime's self-evaluation through the Journal, executing a Policy over the
//!     view of that Journal, and choosing the Artifact in the Tree to become the next Parent Runtime.
//!     - update: changes the Configuration from the previous Artifact to the next Artifact used to
//!     generate the Parent Runtime. This involves checking out that Artifact from the Tree.
//!     - build: builds a Candidate Runtime from the Artifact in the Configuration, and launches the
//!     Candidate Runtime that will become the next Parent Runtime.
//!     - cleanup: cleans up child worktrees
//!     - exit: Parent Runtime terminates, possibly after performing the handshake with the next
//!     Parent Runtime. If no next Parent Runtime is selected, the looping protocol is over.
//!
//!
//! ## Runtime: Parent/Child/Candidate
//!
//! Responsibilities for different roles of runtime.
//!
//! Parent Runtime:
//!     - creates, selects, updates, builds, cleans up, exits
//!     - mutates Tree state
//!
//! Child Runtime:
//!     - self-evaluates
//!     - records evaluations
//!     - performs handoffs with Parent
//!     - does not mutate Tree state
//!
//! Candidate Runtime:
//!     - promotes to Parent Runtime
//!     - performs handoff with Parent Runtime
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

pub(crate) mod backend;
pub(crate) mod c1;
pub(crate) mod c2;
pub(crate) mod c3;
pub(crate) mod c4;
pub(crate) mod event;
pub(crate) mod invocation;
pub(crate) mod journal;
pub(crate) mod workspace;
