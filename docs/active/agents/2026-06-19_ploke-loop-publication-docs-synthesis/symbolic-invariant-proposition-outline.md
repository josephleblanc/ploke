# Symbolic invariant proposition outline for Ploke runtime authority

Date: 2026-06-19
Task: `t_a96b126b`
Status: proof-track outline, not a completed proof
Primary inputs: source inventory `README.md`, implementation invariants slice `implementation-invariants-source-slice.md`, GraphRAG/History adapter note `graphrag-history-adapter-note.md`, and current VM source anchors named below.

## Purpose

This outline gives the symbolic proof track a Euclid-like proposition sequence for Ploke's runtime, authority, History, parent/successor handoff, and artifact claims. It is intentionally not literal geometry. The intended product is a structured proof strategy that separates:

- implementation facts already visible in the pushed VM checkout;
- design intent stated in current evalnomicon/prototype1 docs;
- assumptions needed to reason across processes and runtimes;
- proof obligations that remain unproved or blocked.

Critical caveat: the parent corpus says typestate transitions may be complete on the user's home machine but are not pushed to this VM. This outline must not promote current VM typestate-transition scaffolding into a completed proof of the live controller. Any proposition that depends on the missing home-machine typestate work is marked `blocked: home-machine typestate`.

## Authority of sources

Use this hierarchy when turning propositions into final prose or proof artifacts:

1. Current implementation facts: current pushed code under `crates/ploke-eval/src/cli/prototype1_state/**`, `crates/ploke-records/src/proof_facts.rs`, `crates/ploke-db/src/proof_graph.rs`, and focused tests such as `crates/ploke-tree/src/tests.rs`.
2. Current/canonical design docs: `docs/workflow/evalnomicon/src/prototype1/{artifact-runtime-model,history-crown,invariant-ledger,persistence-and-observability,runtime-authority,runtime-loop,selection-and-evaluation}.md`.
3. Current source inventories: this directory's `README.md`, `implementation-invariants-source-slice.md`, and `graphrag-history-adapter-note.md`.
4. Formal target drafts: `docs/workflow/evalnomicon/drafts/formal/{detached-process-callgraph-proof-target,callgraph-implementation-design-for-detached-process-proof,rustc-macro-expansion-backend-plan}.md`.
5. Older runtime drafts and reviews: useful vocabulary and negative evidence only; revalidate before publication.

## Definitions

### D1. Artifact

An `Artifact` is a checkout/file-surface state capable of hydrating a runtime. A worktree path, git branch, or scheduler node id may locate or project an Artifact, but is not by itself the Artifact's semantic identity.

Source status:

- Implementation fact / design doc alignment: `prototype1_state/mod.rs` lines 40-44 define Artifact as checkout state and distinguish worktree path as handle, not identity.
- Design intent: `artifact-runtime-model.md` and `invariant-ledger.md` repeat this distinction.

### D2. Runtime

A `Runtime` is an executing process hydrated from an Artifact. Execution alone does not imply parent authority.

Source status:

- Implementation fact / design doc alignment: `prototype1_state/mod.rs` lines 46-56 define Runtime and say it may become Parent only if granted authority.
- Design intent: `runtime-authority.md` states a runtime does not gain parent authority merely because a `ploke-eval` binary is executing.

### D3. Parent

A `Parent` is a runtime in a lineage-scoped, state-scoped authority role that can synthesize children, observe evidence, select successors, and cross the handoff boundary when admitted.

Source status:

- Implementation fact: `parent.rs` defines private-field `Parent<S>` and state markers such as `Unchecked`, `Checked`, `Ready`, `Planned`, `Selectable`, and `Retired`.
- Caveat: the live controller is not fully replaced by the typed core; `prototype1_state/mod.rs` lines 1-9 state these typed states are only partially wired.

### D4. Child

A `Child` is a runtime assigned to evaluate a candidate artifact and emit child-shaped evidence. It may not self-promote.

Source status:

- Design intent: `runtime-loop.md` and `selection-and-evaluation.md` distinguish child evidence from parent selection.
- Implementation-grounded evidence: `c1.rs`-`c4.rs`, `successor.rs`, child-plan structures in `parent.rs`, and current live paths in `cli_facing.rs` / `run/**`.
- Caveat: current child transition scaffolding must be checked against missing home-machine typestate changes before making final enforcement claims.

### D5. Successor

A `Successor` is the incoming parent before handoff acknowledgement, not a separate live controller role with independent authority.

Source status:

- Implementation fact: `successor.rs` lines 1-6 say successor is not a separate controller role or live `Successor<State>` authority carrier.
- Implementation fact: `successor.rs` records states including `Selected`, `Stopped`, `Spawned`, `Checkout`, `Ready`, `TimedOut`, `ExitedBeforeReady`, and `Completed`.
- Proof warning: successor records are evidence about handoff progress, not sufficient authority by themselves.

### D6. History

`History` is the durable authority surface for admitted lineage facts: sealed, lineage-local blocks, entries, ingress, regimes, and projections.

Source status:

- Implementation fact / module claim: `history/mod.rs` lines 52-65 distinguish History from scheduler snapshots, branch registries, CLI reports, dashboards, database side tables, and projections.
- Design intent: `history-crown.md` and `invariant-ledger.md` use the same authority/projection distinction.

### D7. Crown

`Crown` is lineage-scoped ruling authority. It is not a process id, branch, path, machine, or global singleton.

Source status:

- Design intent: `history-crown.md` lines 20-29.
- Implementation-grounded target: `history/mod.rs` lines 124-138 state the core invariant for at most one valid `Crown<Ruling>` carrier per lineage.
- Caveat: current code claims local lineage authority, not distributed consensus or OS-process uniqueness.

### D8. Projection and evidence

A projection is a read-side or operator convenience view derived from History or other evidence. Evidence may inform selection or later admission only after passing an explicit import/admission policy.

Source status:

- Implementation fact: `history/mod.rs` lines 6-11 and 52-65; `history/projection/mod.rs`; `crates/ploke-records/src/history.rs`.
- Design intent: `persistence-and-observability.md` and GraphRAG/History adapter note lines 156-176.

### D9. Policy-bearing surface

The policy-bearing surface is the part of the Artifact that defines parent creation, child/successor execution, History admission, Crown transitions, surface checks, handoff, and proof/admission policy. In current Prototype 1, it is `crates/ploke-eval`.

Source status:

- Implementation fact / module claim: `prototype1_state/mod.rs` lines 111-127.
- Formal target: detached-process proof target lines 92-119.
- Caveat: ordinary succession preserves this surface; future protocol-upgrade transitions are not yet modeled as ordinary edits.

## Common notions / axioms for this outline

A1. Authority is narrower than execution. A process can execute without holding Parent/Crown authority.

A2. History authority is narrower than evidence. A record, JSON file, scheduler row, graph projection, ready file, pid, branch, or report is not authority unless admitted into the History/Crown transition path.

A3. Artifact identity is narrower than filesystem location. Worktree paths and branch names are handles/projections, not semantic artifact identity.

A4. Local lineage authority is narrower than distributed consensus. Current History/Crown claims are local, lineage-scoped, and transition-checked; they do not prove global process uniqueness or consensus across machines.

A5. Proof must fail closed. Missing call edges, missing typestate wiring, stale source anchors, or unpushed home-machine changes are blockers, not evidence of safety.

A6. Ordinary descendant admission preserves the policy-bearing surface digest, unless a future explicit protocol-upgrade or fork transition is modeled and admitted.

A7. Child self-report is evidence. Parent-side policy plus History/Crown admission are required before promotion.

## Postulates / assumptions to make explicit

P0. Pushed-VM boundary. The only implementation facts this outline can treat as directly observed are facts in the VM checkout at task time. Home-machine changes are assumed to exist only as blocked/missing source until pushed or supplied.

P1. Local single-ruler domain. The main proof domain is one configured local lineage and History store, not global absence of competing authorities.

P2. Shared protocol domain. The predecessor and successor are assumed to be compiled from Artifacts preserving the same policy-bearing surface digest `D`, unless an explicit protocol-upgrade transition is in scope.

P3. Clean artifact domain. Successor admission claims require a clean current Artifact tree or equivalent content-addressed artifact identity checked against sealed History.

P4. Build-domain domain. Detached-process and call/effect claims are meaningful only for a named build domain: Cargo metadata, lockfile, target, features, cfgs, rustc/toolchain, environment policy, proof-policy version, and immutable surface digest.

P5. Projection import domain. GraphRAG/code-graph observations may become History-compatible payload/evidence only through an adapter that preserves provenance and does not mint continuation authority.

## Proposition sequence

### Proposition I. Runtime succession is not a flat rerun

Claim: A Ploke generation transition must distinguish an already-running parent runtime from descendant artifacts and descendant runtimes.

Status: design intent with implementation grounding.

Proof sketch:

1. `runtime-loop.md` states that changing an artifact does not change an already-running parent binary.
2. `prototype1_state/mod.rs` defines Artifact as checkout state and Runtime as executing process hydrated from an Artifact.
3. Therefore, evaluating descendant behavior requires hydrating a descendant runtime; parent-side observation of a candidate artifact is not enough to claim the candidate's runtime behavior.

Traceability:

- `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `implementation-invariants-source-slice.md`, runtime/authority inventory

Proof obligations:

- Record explicit Artifact identity for each child/successor runtime.
- Name the runtime identity and build domain in evidence records.
- Avoid collapsing graph nodes, scheduler nodes, branch names, and artifact ids.

Weak point / false-confidence risk:

- A successful build or spawned process can be mistaken for admitted successor authority. It is only runtime/evidence until History/Crown admission succeeds.

### Proposition II. Authority is role-, state-, and lineage-scoped

Claim: For an admitted runtime/path pair, available authority is bounded by role, state, and lineage; execution of a binary does not imply parent authority.

Status: design intent with implementation-grounded carriers.

Proof sketch:

1. `runtime-authority.md` states `A(Runtime) ⊆ A(role(path), state(path)) ⊆ A(role(path))`.
2. `parent.rs` uses `Parent<S>` with private fields and state markers to represent role/state carriers.
3. `Startup<Validated>` fields are private, preventing callers from converting transport evidence into Parent readiness by convention.
4. Therefore, authority can be modeled as a typed carrier rather than inferred from pid/path/binary existence.

Traceability:

- `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`

Proof obligations:

- Prove every live controller path constructs `Parent<Ready>` / `Parent<Ruling>` only through trusted transitions.
- Prove legacy `cli_facing.rs` paths cannot bypass the typed carrier boundary.
- Recheck after home-machine typestate changes are pushed.

Blocked: home-machine typestate. Current VM module docs say typed states are only partially wired into the live controller.

Critical review:

- Private Rust fields are useful but not a complete cross-process proof. The proof must include serialization, invocation files, ready files, and bootstrap loaders.

### Proposition III. History is the authority surface; projections are not authority

Claim: Scheduler snapshots, branch registries, reports, previews, dashboards, side tables, passive records, and GraphRAG projections are evidence or projections, not History authority.

Status: implemented claim / current local claim, with read-side proof obligations.

Proof sketch:

1. `history/mod.rs` defines History as authenticated store over sealed lineage-local blocks and explicitly excludes scheduler snapshots, branch registries, CLI reports, metrics dashboards, preview aggregates, and database side tables from authority.
2. `persistence-and-observability.md` and `invariant-ledger.md` repeat this boundary.
3. `crates/ploke-records/src/history.rs` treats passive records as mirrors/projections, not validators of History.
4. The GraphRAG adapter note proposes evidence-payload bridging, not authority minting.

Traceability:

- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`
- `docs/workflow/evalnomicon/src/prototype1/{history-crown,invariant-ledger,persistence-and-observability}.md`
- `crates/ploke-records/src/history.rs`
- `graphrag-history-adapter-note.md`

Proof obligations:

- Show import paths from projections into History are explicit and policy-checked.
- Show read-side consumers cannot advance lineage heads.
- Preserve blocker/provenance rows rather than erasing unresolved evidence.

Weak point / false-confidence risk:

- Rich projection graphs and useful RAG context can feel authoritative. They are only queryable evidence unless admitted by History/Crown policy.

### Proposition IV. Crown authority is lineage-local and not pid/path/branch identity

Claim: For one lineage, Crown authority is represented by lineage-scoped ruling state, not by a process id, branch name, path, or global singleton.

Status: current architecture claim; local proof target partially implemented.

Proof sketch:

1. `history-crown.md` defines Crown as one-at-a-time authority to mutate an active lineage and excludes pid, branch, filesystem path, and global singleton interpretations.
2. `history/mod.rs` states that multiple runtimes may execute around handoff; execution is not Crown authority.
3. `prototype1_state/mod.rs` distinguishes Tree/lineage and says the same branch/worktree/tree key is not by itself an authority conflict.
4. Therefore, proof obligations should target lineage/ruling-carrier overlap, not OS-process overlap as the primary authority claim.

Traceability:

- `docs/workflow/evalnomicon/src/prototype1/history-crown.md`
- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`

Proof obligations:

- Define exact lineage key and head projection semantics.
- Prove append rejects stale/different lineage-state roots for the local store.
- State clearly what remains outside scope: distributed consensus, OS-process uniqueness, global fork choice.

Critical review:

- Saying "not pid/path/branch" prevents overclaiming, but it also means operational duplicate-launch bugs remain possible unless separate process lease/lock/consensus mechanisms are added.

### Proposition V. The successor handoff is a cross-runtime contract

Claim: Successor handoff is proved by sealed predecessor evidence plus successor admission checks, not by passing one in-memory object across processes.

Status: partially implemented; typestate completion blocked for final proof.

Proof sketch:

1. `history/mod.rs` describes the outgoing parent locking handoff material and a later successor runtime verifying sealed material before becoming the next Parent.
2. It explicitly says the Crown is not locked because a single in-memory object survives both runtimes.
3. `successor.rs` records handoff states, but says successor is an incoming Parent before acknowledgement, not a separate authority carrier.
4. Therefore, proof must model a logical authority transition across serialized/sealed evidence, not a Rust object transfer.

Traceability:

- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs`
- `docs/workflow/evalnomicon/src/prototype1/history-crown.md`
- `detached-process-callgraph-proof-target.md`, successor handoff semantics

Proof obligations:

- Prove predecessor retirement or Crown lock happens before successor can obtain ruling authority.
- Prove successor validates active Artifact identity, sealed predecessor head, lineage, and policy-bearing surface commitment.
- Prove only one selected successor is admissible for one lineage transition unless a future fork/new-lineage rule exists.
- Prove ready files, pids, invocation JSON, and transport acknowledgements are evidence only.

Blocked: home-machine typestate. Current VM code has typed scaffolding and live handoff checks, but final typestate-transition claims require pushed/supplied home-machine changes.

Weak point / false-confidence risk:

- A ready acknowledgement can be misread as authority. It should be phrased as operational evidence whose authority effect depends on the sealed predecessor and successor admission checks.

### Proposition VI. At most one valid ruling carrier per lineage

Claim: For one lineage, at most one valid typestate carrier may hold `Crown<Ruling>` at a time; during handoff there may be zero rulers.

Status: target invariant, partially implemented locally.

Proof sketch:

1. `history/mod.rs` states the invariant directly for `Crown<Ruling>`.
2. The intended sequence moves Parent<Ruling> through lock/seal/retire before successor validation into the next Parent<Ruling>.
3. Surface-digest preservation makes the predecessor and successor share the same authority protocol.
4. Therefore, if all live authority paths are routed through the typed transition and the policy surface is preserved, overlap in valid ruling carrier states is excluded by construction.

Traceability:

- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`

Proof obligations:

- Lemma L6.1: all constructors for ruling authority are private/trusted and unreachable from legacy bypass paths.
- Lemma L6.2: sealed History append/lineage-root checks prevent reopening the same lineage epoch from stale state.
- Lemma L6.3: successor admission consumes exactly the sealed predecessor head for the intended lineage and artifact.
- Lemma L6.4: bootstrap/genesis is local-store absence only; it must reject unreadable, ambiguous, or inconsistent stores.

Blocked: home-machine typestate for complete live-controller transition coverage.

Critical review:

- The invariant is about valid authority carriers, not the absence of duplicate processes. A red-team review should try to launch duplicate binaries, replay old invocation files, mutate projections, or fork stale local stores and verify that none become admitted ruling authority.

### Proposition VII. Policy-surface digest preservation supplies the induction base

Claim: Ordinary admitted descendants preserve the policy-bearing surface digest `D`; therefore the rules that enforce runtime authority and History/Crown admission remain stable across generations.

Status: current Prototype 1 policy / proof target.

Proof sketch:

1. `prototype1_state/mod.rs` identifies the policy-bearing surface as `crates/ploke-eval` for current Prototype 1 and states an induction over admitted parent-created descendants.
2. `invariant-ledger.md` says ordinary self-improvement must not mutate the policy-bearing `ploke-eval` surface until an explicit protocol-upgrade transition exists.
3. The detached-process proof target makes immutable surface digest the base of its induction.
4. Therefore, ordinary child/successor execution can inherit the same authority protocol only if candidate Artifact admission checks preserve digest `D`.

Traceability:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`
- `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`

Proof obligations:

- Define the exact digest computation and Artifact surface partition.
- Prove child hydration checks and successor startup recomputation compare candidate/current surfaces against sealed commitments.
- Prove mutable tool-description edits cannot affect policy code or build configuration indirectly.
- Define protocol-upgrade/fork transition before allowing `crates/ploke-eval`, `Cargo.toml`, build scripts, verifier code, or admission policy changes into ordinary mutation scope.

Weak point / false-confidence risk:

- A clean git tree is not the same as a policy-surface proof if build scripts, features, environment, dependencies, or generated code are unaccounted for.

### Proposition VIII. Child evidence is not promotion

Claim: Child self-evaluation and child-produced evidence may inform selection, but cannot by themselves promote the child to successor/Parent authority.

Status: current architecture claim; implementation-grounded by selection/History surfaces.

Proof sketch:

1. `runtime-loop.md` says child evaluates itself and records evidence, then parent observes evidence and applies selection policy.
2. `selection-and-evaluation.md` states child self-evaluation is evidence; promotion requires parent-side policy and History/Crown checks.
3. `prototype1_state/mod.rs` includes the design constraint not to let runtime self-report become promotion without independent verification.
4. Therefore, any proof of safe evolution must route promotion through parent selection and handoff, not child-local success status.

Traceability:

- `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`

Proof obligations:

- Prove child output paths are child-shaped mutable surfaces only.
- Prove parent selection consumes evidence through typed/provenance-bearing records.
- Prove evaluator identity and policy identity are preserved in records.
- Audit temporary short-circuits in selection or handoff that might accept completed child evaluation despite rejection.

Critical review:

- If LLM adjudication becomes both generator and authority, the loop becomes circular. The proof track must keep model judgments as evidence interpreted by explicit policy.

### Proposition IX. Parent/child/successor messages are typed transition obligations

Claim: Cross-runtime files and messages are not arbitrary JSON status writes; they should be treated as typed obligations coupling lock/unlock transitions with file schemas.

Status: design intent with partial implementation grounding.

Proof sketch:

1. `invariant-ledger.md` defines a message box as `(Lock transition, Unlock transition, File schema)`.
2. `parent.rs` defines child-plan message structures and parent state markers around planning and selectability.
3. `channel.rs` provides role-indexed parent/child runtime channel scaffolding.
4. Therefore, message files should be proved as transition records or evidence, not ambient mutable shared state.

Traceability:

- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
- `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`

Proof obligations:

- For each message box, name writer, readers, schema, lock transition, unlock transition, and admitted state effect.
- Prove stale/replayed message files do not unlock later authority states.
- Replace or contain legacy ad hoc file protocol paths.

Blocked: home-machine typestate for full transition coverage.

Weak point / false-confidence risk:

- A typed Rust struct for message contents does not prove the file was produced at the correct transition or read by the correct authority carrier.

### Proposition X. GraphRAG observations may enter only as provenance-bearing evidence

Claim: Code graph, retrieval, embeddings, and context assembly can inform History candidates only through an evidence adapter that preserves provenance and does not mint continuation authority.

Status: proposed adapter seam; current GraphRAG and History surfaces exist separately.

Proof sketch:

1. The GraphRAG adapter note identifies existing code graph/RAG observations: parsed graph facts, database query rows, embedding data, assembled context, scores, snippets, and tool/request provenance.
2. It proposes normalizing them into History-compatible subject/procedure/evidence/payload records.
3. `History::candidates(scope)` already operates as a read-side projection over verified sealed blocks.
4. Therefore, the safe bridge is evidence/payload import or read-side projection, not parser/TUI prompt code becoming authority.

Traceability:

- `graphrag-history-adapter-note.md`
- `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs`
- `crates/ploke-db/src/database.rs`, `crates/ploke-rag/src/core/mod.rs`, `crates/ploke-tui/src/rag/context.rs`
- `crates/ploke-records/src/proof_facts.rs`, `crates/ploke-db/src/proof_graph.rs`

Proof obligations:

- Define content-addressed evidence references for snippets, query configs, scores, graph facts, embedding set/version, and artifact identity.
- Prove adapter output cannot advance lineage heads or grant parent authority.
- Preserve retrieval uncertainty and degraded type-context flags as evidence quality, not hidden success.

Critical review:

- A convincing answer from RAG is not a proof. Retrieval may be stale, incomplete, or from a different artifact/embedding set.

### Proposition XI. Detached-process safety requires a typed call/effect/lifetime graph

Claim: The strong long-horizon claim that no process outlives its runtime except through admitted successor handoff cannot be proved from a plain caller/callee graph.

Status: formal target, not implemented proof.

Proof sketch:

1. The detached-process proof target defines detached process as any OS process whose lifetime is not bounded by the runtime that created it.
2. The callgraph implementation design says the graph must include build-aware call resolution, process effects, command-builder dataflow, lifetime/containment policy, authority effects, and proof obligations.
3. It also says the verifier must fail closed on unresolved dangerous edges.
4. Therefore, current parser/GraphRAG surfaces are insufficient for the strong detached-process theorem until proof facts are extracted and checked.

Traceability:

- `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`
- `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`
- `docs/workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`
- `crates/ploke-records/src/proof_facts.rs`
- `crates/ploke-db/src/proof_graph.rs`

Proof obligations:

- Define `BuildDomain` for each admitted proof.
- Extract process effects, async/task effects, authority effects, evidence writes, and cleanup paths.
- Model macro expansion, proc macros, build scripts, dependencies, dynamic dispatch, panic/unwind, cancellation, and external commands.
- Treat unresolved dangerous edges as blockers.

Weak point / false-confidence risk:

- Grepping for `Command::spawn` or using an incomplete call graph will miss wrappers, macros, build scripts, external programs, async tasks, process groups, shells, and cleanup failures.

### Proposition XII. The final theorem is an induction over admitted descendants, not over all possible machine states

Claim: The defensible theorem should quantify over descendants admitted by the Ploke transition system, not arbitrary processes, arbitrary working trees, or arbitrary user actions on the host.

Status: proof strategy.

Proof sketch:

1. Proposition VII gives the base/step for policy-surface digest preservation.
2. Propositions II-VI constrain authority acquisition and handoff.
3. Propositions VIII-X constrain evidence and projections.
4. Proposition XI defines additional graph facts needed for process-lifetime claims.
5. Therefore, the theorem should be shaped as: every admitted descendant runtime produced from the genesis parent under the ordinary transition system preserves policy digest `D`, cannot obtain ruling authority except by validated History/Crown transition, cannot self-promote from child evidence, and cannot create unbounded surviving processes except through admitted successor handoff once the call/effect proof is discharged.

Traceability:

- `detached-process-callgraph-proof-target.md`, primary theorem target
- `prototype1_state/mod.rs`, policy-surface induction
- `history/mod.rs`, local authority sequence

Proof obligations:

- Formalize admitted transition relation.
- Formalize base/genesis admission and local absence limits.
- Formalize successor step and predecessor retirement.
- Add process-lifetime theorem only after call/effect/lifetime proof artifacts exist.

Critical review:

- This theorem is intentionally narrower than "Ploke makes the whole machine safe" or "Ploke has globally unique rulers". Publishing a broader claim would be false confidence.

## Inductive invariant families

### I. Surface-digest invariant

Form:

```text
AdmittedParent(g, D) ∧ OrdinaryStep(g -> g+1) => AdmittedParent(g+1, D)
```

Known evidence:

- `prototype1_state/mod.rs` lines 111-127.
- `invariant-ledger.md` policy-surface entry.

Missing lemmas:

- Exact digest/partition checker coverage.
- Build-domain/environment closure.
- Protocol-upgrade transition for policy-surface edits.

### II. Lineage authority invariant

Form:

```text
For each lineage L and time/epoch boundary E:
  count(valid Crown<Ruling> carriers for L at E) <= 1
```

Known evidence:

- `history/mod.rs` core invariant.
- Private typed carrier patterns in `parent.rs`.

Missing lemmas:

- Live controller transition coverage.
- Bootstrap/genesis rejection on ambiguous stores.
- Replay/stale invocation defense.

Blocked: home-machine typestate for final transition coverage.

### III. Evidence/authority separation invariant

Form:

```text
Projection(x) ∨ Evidence(x) does not imply Authority(x)
unless Admit(x, Policy, History, Crown) succeeds.
```

Known evidence:

- `history/mod.rs` projection distinction.
- passive record docs and GraphRAG adapter note.

Missing lemmas:

- Exhaustive import/admission choke points.
- Query/projection consumers cannot call authority transitions.
- All child evidence records preserve evaluator/policy identity.

### IV. Child non-promotion invariant

Form:

```text
ChildEvidence(c) => not ParentAuthority(c)
without ParentSelection ∧ History/CrownAdmission.
```

Known evidence:

- `runtime-loop.md`, `selection-and-evaluation.md`, `prototype1_state/mod.rs` design constraints.

Missing lemmas:

- No temporary selection short-circuit remains in live path.
- Rejected child dispositions cannot become successor-eligible.
- Current successor selection code aligns with sealed History evidence.

### V. Runtime-bounded process invariant

Form:

```text
ProcessCreatedBy(Runtime r) may outlive r only if it is part of AdmittedSuccessorHandoff(r, r').
```

Known evidence:

- formal detached-process target.

Missing lemmas:

- Complete call/effect/lifetime graph.
- Process-family containment policy.
- Cleanup/cancellation/panic/unwind path proof.
- External command and dependency summaries.

Status: future proof obligation, not current implementation claim.

## Missing lemmas and evidence register

| Lemma/evidence | Needed for | Current status |
| --- | --- | --- |
| Complete pushed home-machine typestate transition changes | Propositions II, V, VI, IX | Blocked: source absent from this VM; `prototype1_state/typestate/**` absent and module docs say partial live wiring. |
| Live-controller bypass audit | Propositions II, VI, VIII | Required because `cli_facing.rs` and `run/**` still contain large live paths. |
| Constructor/privacy audit for authority carriers | Propositions II, VI | Partially evidenced by private fields; needs systematic audit of trusted loaders/transitions. |
| History append/state-root proof refresh | Propositions III, IV, VI | Code docs say append rejects mismatched state root; needs refreshed line-specific proof and tests. |
| Genesis/bootstrap formalization | Propositions V, VI, XII | Current claim is local configured-store absence, not global absence. |
| Artifact identity and clean-tree proof | Propositions I, V, VII | Need exact identity/digest algorithm and negative tests around dirty/provisional worktrees. |
| Surface partition digest checker | Proposition VII | Current policy exists; exact verifier coverage must be audited before final claims. |
| Protocol-upgrade/fork transition | Proposition VII, XII | Not yet modeled; ordinary edits must not mutate policy-bearing surface. |
| Selection disposition proof | Proposition VIII | Need audit for temporary short-circuits that may promote completed/rejected children. |
| Message-box transition registry | Proposition IX | Need writer/reader/schema/lock/unlock matrix for every runtime message file. |
| GraphRAG evidence adapter schema | Proposition X | Proposed seam only; define payload/provenance/evidence refs before implementation claims. |
| BuildDomain extraction | Proposition XI | Formal target only; current `proof_facts` DTOs are vocabulary, not proof. |
| Process-effect/lifetime graph | Proposition XI | Formal target only; plain parser/GraphRAG insufficient. |
| Red-team duplicate launch/replay tests | Propositions IV, V, VI | Needed to validate non-claims and prevent operator-facing false confidence. |

## Critical reviews to perform before promoting this outline

1. Typestate review: after home-machine changes are pushed, diff the new typestate files and live-controller call paths against Propositions II, V, VI, and IX. Reject final prose if it still depends on stale VM scaffolding.
2. Authority bypass review: search for every path that constructs parent readiness, Crown/ruling state, History open/seal/append state, successor invocation, or continuation decision. Confirm each path is behind trusted transition functions.
3. Projection confusion review: test or inspect whether scheduler, branch registry, passive records, `ploke-tree` projections, GraphRAG rows, and TUI/RAG context can accidentally drive authority decisions.
4. Selection review: confirm child rejection and failed evidence cannot become successor-eligible through broad/fallback paths.
5. Build-domain review: enumerate features/cfg/target/build-script/proc-macro/dependency boundaries before making any callgraph proof claim.
6. Process-lifetime review: inspect direct and indirect process creation, async tasks, external commands, cleanup paths, timeouts, cancellations, panic/unwind, signal handling, and process groups.
7. Artifact identity review: verify the distinction between worktree path, branch, git commit/tree, artifact manifest, runtime id, scheduler node id, RunForest node id, and ArtifactTree id remains explicit.

## Known weak points

- Current VM typestate code is explicitly partial. Do not claim the live controller is typestate-complete until the home-machine changes are pushed and audited.
- `channel.rs` is staged and may be dead-code-scaffolded. It is proof vocabulary and implementation direction, not evidence that the live channel is fully migrated.
- `authority.rs` is marked stale by date in the source inventory; use it for vocabulary only.
- Large live paths remain in `cli_facing.rs` and `run/**`; they are current behavior anchors but not automatically clean proof boundaries.
- Bootstrap/genesis is local store absence, not global absence.
- History/Crown are local lineage authority, not distributed consensus.
- Surface-digest preservation does not by itself prove build hermeticity, dependency integrity, or process-lifetime safety.
- Passive records and graph projections are valuable observability, but can create false confidence if readers forget they are not authority.
- Formal proof DTOs and proof graph storage are vocabulary/transport until a proof checker discharges obligations.

## False-confidence risks to flag in final docs

1. "The successor wrote ready, therefore it is Parent." False: ready is operational evidence; authority requires admission.
2. "Only one process is running, therefore one Crown." False: process count and Crown authority are different domains.
3. "Two processes overlap, therefore authority invariant failed." False: overlap may be allowed around handoff if at most one valid ruling carrier exists.
4. "The scheduler says selected, therefore History selected." False: scheduler is projection/evidence unless admitted.
5. "RAG found the right code, therefore the claim is proved." False: retrieval is evidence, not proof or authority.
6. "The Rust type exists, therefore the live path uses it." False: current VM docs say typed states are only partially wired.
7. "The policy surface is clean, therefore process lifetime is safe." False: detached-process safety needs call/effect/lifetime proof.
8. "A branch/worktree/node id identifies the Artifact." False: those are handles/projections unless bound to explicit artifact identity.
9. "A local genesis absence claim proves no other authority exists." False: it is configured-store absence only.
10. "A proof over one build domain applies to all builds." False: build domain must be named and preserved.

## Candidate final theorem wording

Conservative theorem target:

```text
For the local configured lineage L and ordinary admitted descendants produced
from a genesis Parent under policy-surface digest D, if each transition preserves
D, if all ruling authority constructors are reachable only through the trusted
History/Crown transition relation, if successor admission validates predecessor
sealed head, current clean Artifact identity, lineage, and surface commitment,
and if child evidence and projections enter only through explicit admission
policies, then no child self-report, projection, ready file, pid, branch,
worktree path, or GraphRAG observation can by itself grant ruling Parent
authority for L. At every logical authority boundary, at most one valid ruling
carrier for L exists; during handoff there may be zero.
```

Deferred theorem extension:

```text
Additionally, no process created by an admitted runtime outlives that runtime
except through admitted successor handoff.
```

Do not state the deferred extension as proved until the BuildDomain, call/effect/lifetime graph, external-command summaries, cleanup paths, and unresolved-edge blockers are implemented and checked.

## Recommended next proof-track artifacts

1. `authority-transition-ledger.md`: exact transition table from startup through parent ready, child plan, child observation, successor selection, Crown lock/seal, successor validation, and next parent.
2. `proof-obligation-matrix.md`: proposition-to-lemma-to-source matrix with status `implemented`, `partial`, `design intent`, `blocked`, or `false claim`.
3. `message-box-registry.md`: every cross-runtime file/message with writer, reader, schema, lock transition, unlock transition, stale/replay rule, and authority effect.
4. `build-domain-proof-scope.md`: first BuildDomain definition for detached-process proof work.
5. `false-confidence-red-team.md`: tests/reviews that intentionally confuse projections with authority, pids with Crown, paths with Artifacts, and child evidence with promotion.
