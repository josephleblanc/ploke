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
//! Artifact: An Artifact is a checkout state: the collection of files that can
//! hydrate a Runtime. Every checkout is an Artifact, including the stable
//! active parent checkout and temporary child worktrees. A worktree path is a
//! handle to an Artifact, not the Artifact's semantic identity. Applying a
//! Patch to an Artifact produces another Artifact.
//!
//! Runtime: An executing process hydrated from an Artifact. Every Artifact is a
//! dehydrated Runtime, and every Runtime is a potential Parent if it is granted
//! parent authority. A Runtime may perform any Intervention, but the admissible
//! Interventions are restricted by the current Configuration in which the
//! Runtime exists. A Runtime may operate over the Surface of an Artifact to
//! produce a Patch, and apply the Patch to create a new Artifact. The Artifact
//! the Runtime operates over to produce a Patch may or may not be the same
//! Artifact that hydrated the Runtime. Notably, a Runtime cannot be evaluated
//! externally, and must itself produce the Record containing the metrics used
//! to evaluate it. This is because the Metrics are of a Procedure the Runtime
//! is the operational environment for.
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
//! ## Role and checkout model
//!
//! Roles are runtime authority states, not permanent filesystem locations.
//! The same stable checkout path can host a sequence of Parent runtimes as the
//! loop advances through selected Artifacts.
//!
//! The active parent root is the stable checkout a Parent currently controls.
//! In the single-parent prototype path this is the operator-provided repo root.
//! A Parent may create temporary child worktrees for candidate Artifacts, but
//! those worktrees are leaf evaluation/build surfaces. They are not where the
//! successor Parent should keep living after handoff.
//!
//! The intended single-successor loop is:
//!
//! ```text
//! Parent R1 running from active checkout A1
//!   -> create temporary child checkout A2
//!   -> hydrate/evaluate child runtime C2 from A2
//!   -> select A2 under policy
//!   -> update stable active checkout from A1 to A2
//!   -> hydrate successor runtime R2 from the active checkout
//!   -> hand off parent authority to R2
//!   -> R1 exits
//!   -> cleanup temporary child checkout/build products
//! ```
//!
//! This ordering is intentionally heavier than a one-child, one-branch demo.
//! It is the constrained case of the intended larger model: arbitrary
//! concurrent Parents may advance arbitrary branches in the same Tree while
//! arbitrary child leaf runtimes evaluate candidate Artifacts. Cleanup and
//! explicit handoff are therefore part of the semantic protocol, not optional
//! polish. Without updating the stable active checkout before handoff, a long
//! run would create one parent worktree per generation and eventually exhaust
//! disk.
//!
//! ## Planning note: generative graph, not only git ancestry
//!
//! The current prototype deliberately starts with an artificial editing
//! boundary: patch generation targets text included in a binary with
//! `include_str!`. That keeps the first loop small enough to observe. The
//! longer-term loop should not bake in that boundary. Patch generation should
//! become a backend-agnostic editing trait that can be driven by another
//! harness, including the `ploke-tui` coding-agent harness.
//!
//! In that fuller model, the Runtime does not merely mutate "its own" source
//! tree. A Runtime is an operator over an Artifact surface:
//!
//! ```text
//! Runtime -> Surface(Artifact) -> PatchAttempt
//! PatchAttempt + base Artifact -> derived Artifact
//! derived Artifact -> hydrated Runtime
//! ```
//!
//! Patch generation is probabilistic because it contains LLM calls. Repeating
//! the same operation coordinate can produce different patches:
//!
//! ```text
//! GeneratePatch(runtime=R1, target=A1, attempt=1) -> P1
//! GeneratePatch(runtime=R1, target=A1, attempt=2) -> P2
//! GeneratePatch(runtime=R1, target=A1, attempt=3) -> P3
//!
//! ApplyPatch(base=A1, patch=P1) -> A2
//! ApplyPatch(base=A1, patch=P2) -> A3
//! ApplyPatch(base=A1, patch=P3) -> A4
//! ```
//!
//! The important coordinate is therefore not just an Artifact. It is the pair
//! of the generator and the target:
//!
//! ```text
//! OperationCoordinate = (generator Runtime, target Artifact)
//! ```
//!
//! The common self-improvement path uses a generator that was hydrated from
//! the same Artifact it is operating over:
//!
//! ```text
//! (R1, A1) -> P1 -> A2 -> R2
//! (R2, A2) -> P2 -> A3 -> R3
//! ```
//!
//! but the graph must also admit cross-lineage operations:
//!
//! ```text
//! (R2, A1) -> P4 -> A5
//! ```
//!
//! Git ancestry can explain `A1 -> A5`, but it cannot explain that `R2` was
//! the generator. That provenance has to live in Prototype 1 records, not in
//! branch names or worktree layout.
//!
//! This implies three related graphs rather than one tree:
//!
//! - artifact graph: durable Artifact states connected by applied patches
//! - runtime derivation graph: which Artifact hydrated each Runtime
//! - operation graph: which Runtime operated over which Artifact to create a
//!   patch attempt
//!
//! A dirty worktree is only a provisional Artifact candidate. It should not be
//! treated as a durable graph node until it has a recoverable identity such as
//! a git commit, git tree id, content hash, or artifact manifest id. A binary
//! built from an uncommitted worktree may still run as a Runtime, but if the
//! source Artifact is later lost then that Runtime has degraded provenance. It
//! can still generate patches, but the system must not silently pretend the
//! missing Artifact is recoverable.
//!
//! Successor selection should eventually promote a graph coordinate or graph
//! node under a policy, not a global "current branch". The single-successor
//! path is a constrained case:
//!
//! ```text
//! parent actor at (R1, A1)
//!   -> generate A2, A3, A4
//!   -> hydrate/evaluate R2, R3, R4
//!   -> select A3/R3
//!   -> update the active checkout to A3
//!   -> hydrate R3 from that active checkout
//!   -> hand off parent authority to R3
//! ```
//!
//! Later policies may select from local children, local lineage, ancestor
//! lineage leaves, or the wider campaign graph. The persisted handoff and
//! selection records should therefore preserve generator runtime, target
//! artifact, selected artifact, selected runtime, candidate scope, oracle
//! identity, evaluation policy, and attempt identity.
//!
//! ### Near-term invariants implied by the future graph
//!
//! The full graph model is future work, but it should constrain the prototype
//! being built now. The point is not to implement every later capability in
//! the first loop. The point is to avoid storing state in a shape that will
//! make the later graph impossible or misleading.
//!
//! Prototype 1 code should preserve these invariants even while only one
//! successor lineage is executing:
//!
//! - Branch names and worktree paths are handles, not semantic identity. Any
//!   durable record that needs to identify an Artifact should carry an explicit
//!   artifact identity or enough backend identity to recover it.
//! - A child node is not just "the next branch". It is the result of a patch
//!   generation attempt by a specific Runtime over a specific target Artifact.
//!   Records should keep those identities separate when the information is
//!   available.
//! - Scheduler reports are not authority. A continuation or successor handoff
//!   should be represented by an immutable attempt-scoped selection record, not
//!   by the latest mutable scheduler field.
//! - Parent authority is actor-scoped, not campaign-global. The first live path
//!   may enforce one active parent as policy, but persisted leases and handoff
//!   records should be shaped so multiple parent actors can later advance
//!   different lineages in the same campaign graph.
//! - Evaluation records must name the evaluated Runtime or Artifact, the oracle
//!   or external task set, and the policy used to interpret the result. A score
//!   without evaluator and policy identity is not comparable evidence.
//! - Dirty worktrees are provisional candidates. Promotion, selection,
//!   successor handoff, and cross-lineage reuse should refer to durable
//!   Artifacts, not uncommitted filesystem state.
//! - A Runtime may begin acting as `Parent<Checked>` only after the active
//!   checkout, artifact-carried parent identity, and scheduler node agree. For
//!   git, gen0 checking means a fresh branch whose HEAD is exactly the
//!   `parent_identity.json` initialization commit, and later generations must
//!   also have an identity commit at HEAD for the Parent being started.
//! - Temporary child worktrees are cleanup targets after selection and handoff.
//!   The selected Artifact should be moved into the stable active checkout
//!   before the successor becomes Parent; the child worktree should not become
//!   the next Parent's long-lived home.
//! - Runtime provenance may be degraded but should not be erased. If a Runtime
//!   exists but its source Artifact is missing, records should say that
//!   directly instead of inventing or implying a recoverable source node.
//! - Append observations and decisions rather than overwriting them. Later
//!   graph traversal, recovery, and cross-lineage selection need the historical
//!   sequence of generation attempts, evaluations, selections, handoffs, and
//!   failures.
//!
//! In practical terms, new Prototype 1 state should prefer explicit records
//! like `PatchAttempt`, `ArtifactNode`, `RuntimeDerivation`,
//! [`crate::loop_graph::Coordinate`], [`crate::loop_graph::OperationTarget`],
//! `EvaluationRecord`, `ParentLease`, and `SuccessorSelection` over
//! convenience DTOs that mirror the current CLI report. The live controller can
//! still use narrow views for display, but the persisted model should retain
//! provenance and authority.
//!
//! The first in-code carriers for this model live in [`crate::loop_graph`].
//! They are wired into the current prototype at these entry points:
//!
//! - [`crate::intervention::spec::InterventionSynthesisInput`] and
//!   [`crate::intervention::spec::InterventionCandidate`]: patch-generation
//!   inputs/candidates can carry an [`crate::loop_graph::OperationTarget`] and
//!   [`crate::loop_graph::PatchId`].
//! - [`crate::intervention::apply::execute_intervention_apply`]: apply records
//!   preserve base artifact, patch, and derived artifact ids when available.
//! - [`crate::intervention::branch_registry::record_synthesized_branches`] and
//!   [`crate::intervention::branch_registry::mark_treatment_branch_applied`]:
//!   the branch registry keeps graph provenance beside the legacy branch
//!   handles.
//! - [`crate::intervention::scheduler::register_treatment_evaluation_node`]:
//!   scheduled nodes and runner requests copy graph provenance already visible
//!   in the registry without minting artifact ids from branch names.
//!
//! Current caveats are also intentional: most live CLI synthesis paths still
//! pass `operation_target: None`; `generation_coordinate` is usually absent
//! because generator runtime identity is not yet carried into synthesis; and
//! text-file fallback ids are surface identities, not whole-worktree or
//! whole-repository artifact ids. Future work should replace those fallbacks
//! with durable git/tree/manifest identities as the backend stops being a
//! single `include_str!` text target.
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
//!     - update: changes the active parent checkout from the previous Artifact
//!     to the selected next Artifact. This involves checking out that Artifact
//!     from the Tree in the stable active root, not moving the Parent into the
//!     child worktree.
//!     - build: hydrates a Successor Runtime from the selected Artifact in the
//!     active checkout and launches the Successor Runtime that may become the
//!     next Parent Runtime.
//!     - cleanup: cleans up temporary child worktrees and build products after
//!     they are no longer needed by evaluation or handoff policy.
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
//!     - fresh runtime hydrated from the selected Artifact in the active checkout
//!     - acknowledges handoff with Parent Runtime
//!     - becomes the next Parent only after policy and bootstrap validation
//!
//! The persisted invocation contract for the live Prototype 1 seam is
//! narrower than the full long-term role vocabulary but already has two
//! executable runtime roles:
//!
//! - `Child`: leaf evaluator that records one self-evaluation and exits.
//! - `Successor`: selected-continuation bootstrap that acknowledges handoff
//!   and runs one bounded rehydrated controller generation. It is not yet an
//!   unbounded autonomous trampoline, but it must be hydrated from the selected
//!   Artifact after that Artifact has been installed in the active checkout.
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
//! - hydrate/build child runtime in temporary child worktree?
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
//! - clean up temporary child worktree/build products as per Policy
//!     - on fail, follow policy
//!
//! ### select
//! 1. reads Journal to load Child self-evaluations
//!     - on fail, follow policy
//! 2. executes a Policy to choose the Successor
//!
//! ### update
//! 1. checkout Successor Artifact branch/node from Tree into the stable active
//!    parent root
//!
//! ## build
//! 1. use cargo check
//!     - on fail, follow Policy
//! 2. use cargo build
//!     - on fail, follow Policy
//! 3. execute Successor Runtime from the stable active parent root
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
//!   Parent/child/successor process seam: stage one temporary node workspace,
//!   hydrate one child runtime, spawn one runner, read back persisted results,
//!   update the active checkout, bootstrap the selected successor runtime, and
//!   clean up temporary child workspaces/build products after policy permits it.
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
//! - `nodes/<node-id>/successor-completion/<runtime-id>.json`
//!   Terminal record for the successor's bounded rehydrated controller turn.
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
pub(crate) mod child;
pub(crate) mod cli_facing;
pub(crate) mod event;
pub(crate) mod identity;
pub(crate) mod inner;
pub(crate) mod invocation;
pub(crate) mod journal;
pub(crate) mod parent;
pub(crate) mod record;
pub(crate) mod successor;
pub(crate) mod workspace;
