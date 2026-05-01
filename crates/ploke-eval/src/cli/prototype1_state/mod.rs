//! Typed prototype1 configuration states.
//!
//! These states are only partially wired into the live controller. They exist
//! so the Parent -> Child -> Completed child -> Selected successor -> incoming
//! Parent handoff seam can be modeled explicitly as move-only configuration
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
//! The types in this module are reflections of concepts for a complex
//! structure of self-propagating, self-evaluating configurations of
//! artifact/runtime pairs that exist in a branching tree structure with a
//! shared History store. Update recorded 2026-04-29 10:35 UTC: that store
//! should be understood as a global authenticated substrate over lineage-local
//! authority chains, not as one global linear chain whose height defines every
//! lineage.
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
//! History: The intended durable substrate behind the Journal. History is not
//! the scheduler snapshot, branch registry, CLI report, or any other mutable
//! projection. It is the provenance-preserving store of sealed authority blocks
//! for one or more lineages. A block is the record of one Crown epoch within a
//! lineage: entries are written while a runtime is `Parent<Ruling>`, the block
//! is sealed when the Crown is locked for the selected successor, and the
//! successor must verify that sealed block before it may become the next
//! `Parent<Ruling>`.
//!
//! A History entry should preserve the subject, transition/procedure/policy,
//! executor, operational environment, observer, recorder, proposer, ruling
//! authority, input/output references, timestamps, and payload hash. These
//! roles are not interchangeable: a late event submitted under one authority
//! and admitted by another must retain that chain of custody.
//!
//! `Crown<Locked>` does not make local execution globally trustworthy. It
//! gives a concrete boundary for one lineage: prior decisions must be in the
//! sealed block, while late/backchannel observations go to append-only ingress
//! and can affect later control flow only after an explicit import policy.
//! Current code should describe this as a tamper-evident, transition-checked
//! local History model, not as distributed consensus or proof of LLM judgment
//! correctness.
//!
//! Update recorded 2026-04-29 10:35 UTC: the intended startup admission check
//! is:
//!
//! ```text
//! ProducedBy(SelfRuntime, CurrentArtifact)
//! AdmittedBy(CurrentArtifact, Lineage, Policy, History)
//! ```
//!
//! Genesis admission is a local, configured-store absence claim: the runtime
//! may use bootstrap authority only if the configured History store has no
//! valid associated head for the lineage/artifact. Predecessor admission uses a
//! sealed History head that names the current clean artifact tree. Update
//! recorded 2026-04-30 10:13 PDT: the live successor handoff path now checks
//! the current clean Artifact tree against the sealed History head before the
//! next runtime enters the parent path; bootstrap startup is still handled by
//! local configured-store absence rather than a uniform admission carrier.
//! Update recorded 2026-04-30 11:54 PDT: successor handoff now computes a
//! partitioned surface commitment before successor execution and commits it
//! into the sealed History block. The current concrete partition is:
//! `Immutable = crates/ploke-eval`, `Mutated = all tool-description text
//! files`, and `Ambient = empty declared surface`.
//! Update recorded 2026-04-30 12:20 PDT: child evaluation now validates this
//! surface before hydrating a child Runtime and again after the child Artifact
//! is persisted; successor startup recomputes the current checkout surface and
//! compares it with the sealed History head before entering the parent path.
//!
//! Update recorded 2026-04-30 10:13 PDT: the policy that matters for
//! cross-runtime continuity is embodied by the runtime's policy-bearing code
//! surface. For the current Prototype 1 contract, this means the `ploke-eval`
//! surface that defines parent creation, child/successor execution, Crown
//! transitions, History admission, and handoff. If the first admitted Parent
//! has digest `D` for that surface, and every admitted parent may execute a
//! child/successor only after proving the candidate Artifact also has digest
//! `D`, then every runtime descendant produced by this transition system also
//! carries digest `D`. This is an inductive invariant over admitted
//! parent-created descendants. It does not claim that no external process can
//! run incompatible code; it says such a process is outside the admitted
//! transition system and may not enter the History/Crown mutation path.
//! Ordinary Prototype 1 self-improvement therefore keeps this policy-bearing
//! `ploke-eval` surface out of the bounded edit scope. We do plan to admit that
//! surface into scope later, but only through an explicit protocol-upgrade
//! transition. Until that transition exists, preserving the digest is a
//! precondition for ordinary descendant execution and History admission.
//!
//! Terminology status recorded 2026-04-29 10:35 UTC: terms such as
//! "transaction", "relation", "intervention", "policy", and "lineage
//! projection" are still underspecified relative to the larger state model.
//! Do not treat them as implemented APIs until their invariants are written
//! down and encoded. In particular, an `Intervention` is not automatically a
//! History transaction; the relationship still needs a formal definition.
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
//! ## Crown and succession
//!
//! A live Parent has exclusive authority over one active lineage. The working
//! name for that authority is the Crown. The Crown is not a process id, a git
//! branch, or a path. It is the capability to mutate the active checkout and
//! decide which child Artifact becomes the next active Parent for that lineage.
//!
//! The handoff model is:
//!
//! ```text
//! Parent<Ruling>
//!   -> installs selected Artifact into the active checkout
//!   -> locks succession authority for the selected next runtime
//!   -> launches the next runtime from that active checkout
//!   -> waits for the next runtime to unlock succession
//!   -> exits as Parent
//!
//! next runtime
//!   -> validates the active checkout and artifact-carried identity
//!   -> unlocks succession authority
//!   -> becomes Parent<Ruling>
//! ```
//!
//! In shorthand: the Parent dies as Parent at the same authority boundary where
//! the next Runtime becomes Parent. This is a cross-runtime invariant preserved
//! through a typed contract, not a claim that one in-memory Crown object crosses
//! the process boundary. There may be overlapping processes during handoff, but
//! there should not be overlapping mutable Parent authority for the same
//! lineage. The single-lineage prototype can enforce that directly. A later
//! multi-parent system must make the lineage parameter explicit so sibling
//! Parents do not share one Crown by accident.
//!
//! Sealing the History block at `Crown<Locked>` gives the successor a specific
//! validation target. Before satisfying the unlock transition, the successor
//! should be able to verify the previous block hash, the selected Artifact
//! installed in the active checkout, the selected successor identity, the
//! policy-bearing surface digest, and the required evidence references. If
//! required evidence arrives after the lock, it belongs to ingress and must be
//! imported by a later admitted Parent under an explicit rule rather than
//! silently rewriting the sealed block.
//!
//! The first code carriers for this shape are in [`inner`] and [`parent`].
//! [`inner::Crown`] and [`inner::LockBox`] name the intended authority-transfer
//! structure. The current live handoff now seals and appends a History block
//! carrying a surface commitment before successor launch, and the successor
//! handoff path verifies the current clean Artifact tree against the sealed
//! head before entering the next parent path. Concrete invocation and ready
//! files remain transport/debug evidence, not authority. The child-selection
//! path has the first concrete message box. Do not extend the old process seam
//! by adding another ad hoc acknowledgement file. Add the missing concrete
//! box/transition pair instead.
//!
//! ## Boxes, messages, and buffers
//!
//! A message is not merely "some record we persisted". In this module,
//! `Message` means a cross-runtime obligation:
//!
//! ```text
//! one Runtime in Role<StateA> writes one payload into one concrete buffer
//! another Runtime in Role<StateB> reads that payload from that same buffer
//! the write and read are preconditions for typed state transitions
//! ```
//!
//! For filesystem-backed communication, the buffer is not an arbitrary
//! `PathBuf`. It is a concrete file schema implementing [`inner::File`], such
//! as [`parent::ChildPlanFile`]. The concrete file schema plus the lock
//! transition plus the unlock transition define the box:
//!
//! ```text
//! Box = (Lock transition, Unlock transition, File schema)
//! ```
//!
//! The current concrete example is the child-plan message:
//!
//! ```text
//! File:
//!   prototype1/messages/child-plan/<parent-node-id>.json
//!
//! Lock:
//!   Parent<Ready> -> Parent<Planned>
//!
//! Unlock:
//!   Parent<Planned> -> Parent<Selectable>
//! ```
//!
//! The body of that message names the exact scheduler, branch registry, node,
//! and runner-request files that form the parent-owned candidate set for this
//! turn. The receiver checks that the message was read from the same concrete
//! box named by the body, that it is addressed to the same Parent identity, and
//! that the child generation is exactly `parent_identity.generation + 1`.
//!
//! This is intentionally stricter than "look in scheduler.json for something
//! runnable". The run10 failure came from that weaker shape: the producer wrote
//! candidate records for the wrong generation/parent coordinate, and the
//! consumer later tried to infer the next child from mutable scheduler state.
//! The child-plan box makes the producer publish the candidate set before the
//! parent can select from it.
//!
//! More boxes should not mean more random files. A box is a typed access rule.
//! Several boxes may later be implemented as slots in one authoritative
//! lineage file if the file format supports independent append-only or
//! authority-owned sections. The important property is that each mutable
//! buffer has an explicit owner, allowed readers, and a typed transition that
//! justifies each write and read.
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
//! Intended, not implemented as of 2026-04-29 10:35 UTC: durable Artifacts
//! should carry an artifact-local provenance manifest committed by the tree.
//! History should admit the Artifact by committing to the backend tree key plus
//! manifest digest. Larger evidence, such as self-evaluation records,
//! intervention details, build/runtime records, and later validator
//! attestations, may live in that manifest or be referenced from it by digest.
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
//! - A Runtime may begin acting as `Parent<Ready>` only after
//!   `Startup<Validated>` has checked either local genesis absence or the
//!   predecessor sealed History head, current checkout Artifact, and current
//!   surface commitment.
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
//! ## History block audit
//!
//! Prototype 1 History and Crown authority must be audited at least weekly by
//! combined human and LLM review while this architecture is active. The audit
//! should compare the actual implementation against the claims above and in
//! `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md`, using
//! `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
//! as older background where it has not yet been refreshed.
//! In particular, review private fields, constructor visibility, sealed or
//! module-private state markers, move-only transitions, and the durable records
//! emitted by those transitions. If the code permits forging `Parent<Ruling>`,
//! `Crown<Locked>`, `Block<Sealed>`, or successor admission outside the
//! intended transition path, the claim must be narrowed or the implementation
//! corrected before relying on the History model for longer runs.
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
//! - `messages/child-plan/<parent-node-id>.json`
//!   Typed parent-owned child-plan box. The Parent writes this after producing
//!   the candidate set for a turn and reads it back before selecting a child.
//!   It binds the candidate set to the Parent identity, expected child
//!   generation, scheduler file, branch registry file, node files, and runner
//!   request files.
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
//! - `.ploke/prototype1/parent_identity.json` in the active checkout
//!   Artifact-carried parent identity witness. This is currently committed
//!   into the Artifact tree and used by startup validation, but it is not yet a
//!   full artifact-local provenance manifest.
//!
//! Intended, not implemented as of 2026-04-29 10:35 UTC: each admitted Artifact
//! should carry or reference a provenance manifest whose digest is committed by
//! both the Artifact tree and the admitting History block. That manifest is the
//! natural home for reconstructive evidence such as production provenance,
//! self-evaluation refs, intervention refs, build/runtime refs, and later
//! consensus or validator attestations.
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
//! - a uniform bootstrap admission carrier; first-parent startup still depends
//!   on configured-store absence
//! - a distributed lineage-head map and fork-choice policy. Update recorded
//!   2026-05-01 10:57 PDT: the live filesystem store now computes
//!   `HistoryStateRoot` from a sparse Merkle map over its local lineage-head
//!   projection, and `LineageState` carries the sparse proof for the observed
//!   lineage key. That supports local present/absent head checks; it is not yet
//!   distributed consensus, process uniqueness, or global canonical state.
//! - artifact-local provenance manifests committed by the Artifact tree and
//!   admitted by History through digest/reference
//! - minimal explicit policy references and policy scopes defined through
//!   bounded `Surface` identities for History admission; current code still
//!   uses procedure references and runtime contract assumptions in places
//! - stochastic evidence commitments such as evaluation sample refs,
//!   uncertainty/risk refs, validator/reporter refs, and rejected/failure
//!   evidence refs where policy uses those data for admission
//! - first-class head-state concerns for rollback, fork/conflict, admission, and
//!   finality; current code only has local parent links and rebuildable head
//!   projections
//! - a first-class attempt record that unifies invocation, pid, workspace
//!   lease, process output streams, and terminal status
//! - a durable scheduler decision for selected successors distinct from
//!   per-node `keep` evaluations
//! - explicit bounded fan-out policy state such as concurrent-child budget,
//!   total-completed budget, and absolute deadline
//! - a single authoritative inventory of mutable buffers, their owning role,
//!   allowed readers, and the transition that permits each access
//!
//! The record sprawl is a design problem, not just a naming problem. Today the
//! scheduler, branch registry, node records, runner requests/results,
//! invocation files, transition journal, evaluation summaries, parent identity,
//! stream logs, and compressed run records all carry pieces of one protocol.
//! Cleanup should not add another parallel "status" document. It should move
//! each field into either:
//!
//! - an authority-owned buffer for the active Parent lineage,
//! - an append-only observation stream,
//! - an attempt-scoped child/successor result,
//! - or a durable Artifact/Runtime/operation provenance record.
//!
//! Fields duplicated across those buffers should be kept only where they are
//! needed for recovery or validation, and the validating transition should name
//! which copy is authoritative.
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
pub(crate) mod history;
pub(crate) mod history_preview;
pub(crate) mod identity;
pub(crate) mod inner;
pub(crate) mod invocation;
pub(crate) mod journal;
pub(crate) mod metrics;
pub(crate) mod parent;
pub(crate) mod record;
pub(crate) mod report;
pub(crate) mod successor;
pub(crate) mod workspace;
