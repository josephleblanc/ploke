# 2026-05-08 Bounded Edit Surface Handoff

## Purpose

Restart spine for the current Prototype 1 bounded edit-surface planning thread.
Use this after context compaction before launching more exploration agents.

The active design problem is no longer just "wire `ploke-tui` as an editor."
The design object is:

```text
History evidence
  -> Diagnosis
  -> SurfaceChoice
  -> EditObjective
  -> SurfaceGrant
  -> harness proposal
  -> SurfaceCheck / CheckedProposal
  -> ArtifactDelta
  -> child Runtime evaluation
  -> History-backed selection
```

The harness may propose and mechanically apply. `ploke-eval` grants, checks,
admits, records, hydrates, and selects.

## Current Focus

Build the planning and adapter shape that lets Prototype 1 choose useful
bounded edit surfaces from History evidence, then use `ploke-tui` as a
multi-turn repair harness over those surfaces.

The two underspecified areas we identified are:

1. How the parent identifies what hurt the score or blocked improvement.
2. How that diagnosis maps to a causally upstream graph surface and edit
   objective.

The immediate planning focus is item 2:

```text
failure kind
  -> failing procedure/tool
  -> graph anchors
  -> relation-expanded R/W/F subgraphs
  -> grant
  -> objective for the multi-turn harness
```

## Authoritative Planning Docs

Read these first, in this order:

- [`docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md`](2026-05-08_bounded-edit-surface-implementation-orientation.md)
  Short operational packet for sub-agents: core docs, invariants, task-flow
  diagram, task-stack ids, and prompt pattern.
- [`docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md`](../../workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md)
  Current implementation plan. Includes formal surface algebra, current blocker
  status, stable vocabulary, validation loops, first worked route, and
  milestone/slice plan.
- [`docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md`](../../workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md)
  Test-to-formal-proof index. Maps the current tests to `A' -> B* -> C'`
  splices and records formal gaps such as rejected-attempt evidence and
  apply-outcome classification.
- [`docs/workflow/evalnomicon/drafts/formal-edit-surface.md`](../../workflow/evalnomicon/drafts/formal-edit-surface.md)
  Formal core copied from the discussion: `Γ_a = (V_a, E_a, μ_a)`, grants
  `(R,W,F)`, proposal resolution `(Q_r,Q_w)`, containment, validity, and apply.
- [`docs/workflow/evalnomicon/drafts/2026-05-07-prototype1-edit-surface-model.md`](../../workflow/evalnomicon/drafts/2026-05-07-prototype1-edit-surface-model.md)
  Conceptual model for `SurfaceGrant`, proposal/check/apply, and trait adapter
  shape. Recently moved from the May 2026 archive into drafts.
- [`docs/workflow/evalnomicon/drafts/prototype-1-intervention-loop-v2.md`](../../workflow/evalnomicon/drafts/prototype-1-intervention-loop-v2.md)
  Runtime loop model: parent creates descendants, child self-evaluates after
  rebuild/spawn, successor receives authority.
- [`docs/workflow/evalnomicon/drafts/runtime/authority.md`](../../workflow/evalnomicon/drafts/runtime/authority.md)
  Role/state authority model for Parent/Child/Successor surfaces.

Useful supporting docs:

- [`docs/active/todo/2026-05-05_long-horizon.md`](../todo/2026-05-05_long-horizon.md)
  North star for long-horizon self-improvement, History-backed selection, and
  bounded surfaces.
- [`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md`](../../workflow/evalnomicon/chat-history/history-blocks-v2.md)
  Current History/Crown authority framing.
- [`README.md`](../../../README.md)
  Good overview of `ploke-tui` as a multi-turn Rust coding harness: parsed code
  graph, semantic search, exact code lookup, semantic/non-semantic edits,
  staged approvals, and managed cargo test tools.

## Useful But Secondary

- [`docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`](../../archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md)
  Earlier phased implementation plan. Superseded in part by the current
  adapter plan, but still useful for historical intended stages.
- [`docs/archive/agents/2026-05/2026-05-07-edit-surface-implementation-handoff.md`](../../archive/agents/2026-05/2026-05-07-edit-surface-implementation-handoff.md)
  Handoff claiming several edit-surface implementation slices landed. Treat as
  a historical claim that must be checked against current code.
- [`docs/archive/reports/prototype1-hyperagents-design-2026-05-06/04-ploke-tui-semantic-edit-surface-status.md`](../../archive/reports/prototype1-hyperagents-design-2026-05-06/04-ploke-tui-semantic-edit-surface-status.md)
  Good summary of reusable TUI semantic edit machinery and why TUI proposal
  status is not authority.
- [`docs/archive/reports/prototype1-hyperagents-design-2026-05-06/65-bounded-semantic-edit-surface-ploke-tui-census.md`](../../archive/reports/prototype1-hyperagents-design-2026-05-06/65-bounded-semantic-edit-surface-ploke-tui-census.md)
  Inventory of reusable `ploke-tui`, `ploke-db`, `ploke-core`, and `ploke-io`
  pieces.
- [`docs/archive/agents/2026-05/2026-05-07-edit-surface-phase3-integration-review.md`](../../archive/agents/2026-05/2026-05-07-edit-surface-phase3-integration-review.md)
  Review of fail-closed CLI/config knobs. Useful for history, but current CLI
  was later culled and must be rechecked.

## Probably Skip For Now

- `docs/workflow/evalnomicon/drafts/prototype-1-intervention-loop.md`
  Historical v1 loop. Use v2 for runtime-succession semantics.
- `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
  Older History/Crown background. Prefer `chat-history/history-blocks-v2.md`.
- `docs/workflow/evalnomicon/chat-history/framework-ext-01.md` through
  `framework-ext-04.md`
  Broad background. Low signal for the current implementation pass.
- Transcript-style chat-history files unless a specific reference is needed.

## Current Design Commitments

### Bounded Edit Operation

The adapter encodes the operation interface:

```text
given Artifact + graph projection + grant + objective:
  propose edit and expose touches;

given CheckedProposal:
  mechanically apply and return evidence.
```

`ploke-tui` is one implementation behind this boundary. The same boundary should
also support deterministic producers, mocks, direct semantic resolvers, or later
non-TUI harnesses.

### Formal Surface Core

The surface is graph-slice based:

```text
Γ_a = (V_a, E_a, μ_a)
g = (R, W, F)
ρ_a(q) = (Q_r, Q_w)
g ⊢ q iff Q_r ⊆ R ∧ Q_w ⊆ W ∧ Q_w ∩ F = ∅
valid_a(q) checks expected hashes against Artifact a
CheckedProposal unlocks apply_checked
```

Path globs, canonical prefixes, node kinds, and manual allowlists are ways to
construct graph subsets. They are not the surface itself.

Observation and logging are also inside the model. In a self-modifying
evaluator, there are no authority-free "just logs"; there are records with
different admissibility for different decisions:

```text
Event
  -> Observation
  -> Record
  -> AdmissibleEvidence<D>
  -> DecisionInput<D>
```

Projection text, raw logs, monitor views, and TUI-local state may be records,
but they are not automatically admissible for `Diagnosis`, `Selection`,
`SurfaceCheck`, or `HistoryAdmission`.

Router/model calls that materially shape a harness proposal need an additional
receipt before the real `ploke-tui` adapter is admitted:

```text
AdmitProposal(q) =>
  ∀ call ∈ material_model_calls(q).
    ∃ receipt(call).
      complete_effective_policy(receipt)
```

The receipt proves reconstructability of the client-side request policy, not
deterministic replay of the model output. It should distinguish explicit
values, defaulted values, config source digests, prompt/schema/tool digests,
request/response payload digests, retry/fallback policy, and external provider
metadata or explicit `unknown` values.

Generation records also need generator-surface provenance. If a parent Runtime
uses code, tools, prompts, or configs from its own Artifact to generate a
proposal, the proposal record must cite the version of those generator surfaces
that actually participated:

```text
GeneratedProposal(q) =>
  ∃ parent_runtime, parent_artifact, generator_surface_version.
    hydrated_from(parent_runtime, parent_artifact)
    ∧ used_generator_surface(q, generator_surface_version)
    ∧ generator_surface_version ∈ parent_artifact
```

This is separate from the later statistical/fitness analysis. The near-term
requirement is provenance preservation; the later task is correlating
generator-surface deltas with descendant proposal/evaluation quality.

Do not add future-only Rust scaffolding silently. If a future-use type or helper
is necessary before its consumer exists, cite the relevant task-stack id with a
searchable marker such as
`#[allow(dead_code, reason = "task-stack:<task-id> <short reason>")]`. Only do
this for items explicitly tracked in the task stack; otherwise defer or delete
the code.

### Parent Planning Layer

Use existing History/scoring evidence to drive candidate generation:

```text
History stats -> failure classifier -> routing table -> surface choice
```

Prefer a mechanistic policy first. Bounded LLM-adjudicated protocol steps may
classify, rank, or summarize within known labels, but should not secretly choose
arbitrary writable surfaces.

### Ploke-TUI Role

`ploke-tui` is a multi-turn agentic code-workbench. It can use:

- indexed code graph;
- exact code lookup and edges tools where available;
- semantic/RAG/BM25 search;
- semantic and non-semantic edits;
- staged proposals;
- managed cargo test tooling.

It should receive an `EditObjective`, not a vague "improve the score" prompt.

## Current Blocker Status

Available on this branch for graph-slice construction:

- exact semantic item lookup;
- file path, module path, canonical path, namespace, node kind, byte span, and
  file/tracking hash evidence;
- functions, methods, impls, traits, modules, files, and related parser/index
  nodes where exposed;
- containment-style structure: same file, same module, containing impl/trait,
  explicit companion nodes;
- BM25/RAG/text search as weak read-only discovery;
- manually seeded anchors for known tools/procedures.

Not available on this branch:

- call graph: no reliable `callers(anchor)` / `callees(anchor)`;
- type-reference graph: mostly finished on another branch, not integrated here.

Do not write v1 tests or claims that depend on call/type-ref relations. Keep
those relation kinds in the long-term plan as future expansions.

First-slice surface construction should look like:

```text
A_f = exact anchor nodes from failure kind / tool name
R_f = A_f ∪ same_file(A_f) ∪ same_module(A_f)
      ∪ containing_impl_or_trait(A_f)
      ∪ explicit_read_companions(f)
      ∪ text_search_hits(anchor names, read_only)
W_f = A_f ∪ explicit_write_companions(f) ∪ selected tests
F_f = policy-bearing ploke-eval nodes ∪ unrelated crates
      ∪ lower-level substrates unless explicitly selected
```

Future expansion after call/type-reference graphs land:

```text
R_f += callers(A_f, depth = 1)
R_f += callees(A_f, depth = 1)
R_f += type_ref_neighbors(A_f)
W_f += selected local callees(A_f)
W_f += selected referenced helper/type nodes
```

## Code State Known From Sub-Agent Audits

Recent audits found `ploke-eval` already has a partial edit-surface skeleton:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs`
  has `graph::View` with `project`, `bounds`, `search`, `resolve`, `delta`;
  also has `Projection`, `Span`, and `Delta`-like carriers.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`
  has `harness::Harness` with `graph`, `propose`, and `apply_checked`; also has
  `ArtifactDelta`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
  has `surface::Artifact`, `Ref`, `Grant`, `Touch`, and `Check`.
- Adjacent candidate handoff carriers exist in `history.rs` and `parent.rs`.

Main gaps found:

- no explicit `Γ_a = (V_a, E_a, μ_a)` identity/binding carrier;
- no explicit grant split `R/W/F`;
- no explicit resolved touches split `Q_r/Q_w`;
- no dedicated `CheckedProposal`;
- material validity is spread across checks rather than first-class evidence;
- History admission of the full chain exists only partially / elsewhere.

Recent `ploke-tui`/graph audit found reusable backing pieces:

- `crates/ploke-tui/src/tools/code_edit.rs`
  model-facing canonical edit params and wrapper.
- `crates/ploke-tui/src/rag/tools.rs`
  canonical parsing, DB resolution, `WriteSnippetData` staging, preview/diff.
- `crates/ploke-tui/src/rag/editing.rs`
  approval/apply/status/rescan mechanics; useful internally but UI-coupled.
- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
  `TestRuntime` type-state harness for standing up TUI actors without terminal
  UI.
- `crates/ploke-db/src/helpers.rs`
  exact graph resolution, edges, relaxed canon helpers.
- `crates/ploke-core/src/io_types.rs`
  `EmbeddingData`, `ResolvedEdgeData`, `WriteSnippetData`.
- `crates/ploke-io`
  hash-checked IO/write machinery.

TUI internals that should stay hidden behind the adapter:

- `AppState.proposals` and `create_proposals`;
- `EditProposalStatus`;
- user config proposal persistence;
- TUI `edit approve`;
- chat/tool UI events and rendering payloads;
- `RetrievalScope::LoadedWorkspace` as if it were an edit grant;
- raw `io_handle` calls as authority.

## Next Questions

Continue from these, preferably without rereading broad docs:

1. Which History/scoring records are available at parent candidate-generation
   time for `Diagnosis`?
2. What first failure taxonomy is testable from current records?
3. What first mechanistic routing table should map failure kind to surface
   family?
4. For one failure kind, what exact anchor queries define `A_f`?
5. What current graph operations can construct `R_f`, `W_f`, and `F_f`?
6. Where should `Diagnosis`, `SurfaceChoice`, and `EditObjective` live in code?
7. How should these planning decisions be recorded back into History/candidate
   evidence?

The likely first implementation target is:

```text
invalid candidate generation / semantic edit resolution problem
  -> ploke-tui semantic resolver surface
  -> explicit anchor list for apply_code_edit path
  -> same-file/same-module/explicit companions for R/W/F
  -> EditObjective for reducing invalid edit-surface candidates
```

## Current Implementation Status: After Slice 7.2, During Slice 7.3

Task-stack group:

```text
bounded-edit-surface-evidence-route
```

Closed tasks:

```text
bounded-edit-surface-attempt-evidence (7.1)
bounded-edit-surface-diagnosis-splice (7.2)
```

Open task:

```text
bounded-edit-surface-choice-objective (7.3)
```

Partially implemented inside 7.3:

- `EditObjective`
- `ProtectedCore`
- `EditableSurface::broad(...)`
- `Grant::with_forbidden(...)`
- `Grant::check(...)` rejection for protected/forbidden write touches

Files changed by the 7.3 local primitive:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`
- `docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md`

Verification for the 7.3 local primitive:

```bash
cargo fmt --all
cargo test -p ploke-eval edit_surface:: -- --nocapture 2>&1 | tail -n 80
```

Result from this thread:

```text
28 edit_surface tests passed
```

Important: these tests prove only the local primitive:

```text
explicit EditObjective + explicit ProtectedCore + explicit Γ_a
  -> EditableSurface::broad
  -> Grant::check
```

They do not prove the upstream route from real parent-time context/evidence
into that primitive.

What is now implemented from 7.1:

- Rejected and applied edit-surface attempts have typed payload evidence through
  `history::surface_attempt::{Evidence, Outcome}`.
- `EvaluationPayload.surface_attempt` carries parent-readable attempt evidence.
- `ChildPlanFiles` persists rejected edit-surface attempts so they survive
  resume.
- Below-min candidate generation writes a rejected-attempt-only child plan and
  still fails closed without fabricating child artifacts.
- Resumed rejected-only plans skip child fanout and project
  `EvaluationPayload.surface_attempt` with `artifact = None`.
- Applied success path still uses `SurfaceEvidence` / `CandidateArtifact.surface`
  for checked Artifact transitions.

What is now implemented from 7.2:

- `edit_surface::diagnosis::Diagnosis` is the first authority-side diagnosis
  carrier.
- `edit_surface::diagnosis::classify(&EvaluationPayload)` is a pure
  mechanistic classifier over typed parent-readable payload evidence.
- Rejected `surface_attempt` evidence with `artifact = None` classifies as:

```text
Diagnosis {
  limiter: invalid_candidate_generation,
  failure_kind: semantic_edit_resolution,
  ...
}
```

- Payloads without typed `surface_attempt` evidence do not classify as
  `semantic_edit_resolution`, even if they contain projection/log-like failure
  prose.

Files changed by the 7.1 implementation:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

Files changed by the 7.2 implementation:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/diagnosis.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/mod.rs`
- `docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md`

Key proof tests are indexed in:

- [`docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md`](../../workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md)

Especially important tests:

- `below_min_rejected_attempts_are_persisted_and_recoverable_from_existing_child_plan`
- `current_generation_candidates_include_rejected_edit_surface_attempt_payload`
- `payload_surface_attempt_rejected_is_parent_readable_without_artifact`
- `payload_without_surface_attempt_is_not_parent_readable_attempt_evidence`
- `classify_rejected_surface_attempt_without_artifact_as_semantic_edit_resolution`
- `payload_without_surface_attempt_is_not_semantic_edit_resolution_diagnosis`
- `tui_apply_evidence_is_all_applied_or_rejected`

Accepted reviewer conclusion:

```text
7.2 is accepted. The classifier stays on typed parent-readable evidence and
does not consult logs, CLI text, projection views, or TUI-local state.
```

Non-blocking risk to carry forward:

- Rejected-only turns produce payload evidence but no selection/handoff path
  (`selection = None`). Diagnosis now reads directly from
  `EvaluationPayload.surface_attempt`; later surface/objective routing must keep
  using that typed evidence path and must not assume a sealed successor
  selection entry exists for rejected-only turns.
- `classify` also checks `payload.artifact.is_none()`. That is still typed
  authority, but it means the 7.2 contract is specifically rejected attempt
  evidence that did not produce an Artifact.
- The 7.2 proof-index entries are lighter than the older sections. If the proof
  index becomes a stricter audit artifact, expand their formal meaning and
  "why non-trivial" prose.

Formal gaps exposed by 7.1 tests:

```text
AttemptOutcome_a(q) =
    Applied(a', δ)
  | Rejected(reason)

H' = H ⋅ Attempt(a, Γ_a, g, q, ρ_a(q), AttemptOutcome_a(q))

record(rec) does not imply admissible_D(rec)

projection(rec) ∨ log(rec) ∨ ui_state(rec)
  does not imply admissible_D(rec)

ApplyOutcome(q) = Applied(a', δ) | Rejected(reason)
```

These gaps are recorded in the proof index. They should be addressed or
acknowledged when implementing later carrier/surface/objective slices.

## Next Task

Start here after compaction:

```text
bounded-edit-surface-choice-objective (7.3)
```

Corrected status:

```text
7.3 is still open.

Implemented local primitive:
  explicit EditObjective + explicit ProtectedCore + explicit Γ_a
    -> EditableSurface::broad
    -> Grant::check

Missing upstream splice:
  real-ish parent-time History/context evidence + graph projection +
  protected-core policy
    -> parent-side route/admission constructor
    -> EditObjective + EditableSurface with evidence refs and broad Grant
```

What changed since the previous handoff:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
  - added EditObjective
  - added ProtectedCore
  - added EditableSurface::broad(...)
  - added Grant::with_forbidden(...)
  - Grant::check now rejects writes intersecting forbidden/protected spans

crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs
  - broad_surface_admits_writable_touch_outside_protected_core
  - broad_surface_rejects_touch_inside_protected_core
  - broad_surface_objective_records_context_without_diagnosis_specificity

docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md
  - indexes those tests as local primitive proofs only
  - explicitly says they are not a completed 7.3 route proof
```

Important correction:

```text
Do not describe 7.3 as closed.
The current tests prove B* -> C' for the local grant primitive.
They do not prove A -> B* from real parent context/evidence.
```

The plan direction also changed. The current preferred first pass is no longer a
narrow `Diagnosis -> one semantic resolver SurfaceChoice` route. It is the
broad protected-core route:

```text
History/context evidence
  -> EditObjective
  -> EditableSurface = Γ_a \ ProtectedCore
  -> checked proposal must preserve Φ
```

Where:

```text
Φ(a) = framework form / protected contract
Ω(a) = mutable object-level implementation and capabilities
```

Admitted children may improve `Ω`; ordinary child-producing transitions must
preserve `Φ`. This avoids the wrong rule:

```text
capabilities(child) ⊆ capabilities(parent)
```

The desired region is:

```text
Useful Transformations
  ∩ Statically Checkable Properties
  ∩ Sandbox-Enforceable Properties
  ∩ Form-Preserving Transitions
```

Concrete next implementation target:

```text
bounded-edit-surface-choice-objective (7.3)

A':
  real-ish parent-time context/evidence fixture
  plus graph projection
  plus protected-core policy

B*:
  parent-side route/admission constructor

C':
  EditObjective carries context/evidence refs
  EditableSurface::broad builds a broad Grant
  ordinary writes remain allowed
  protected-core writes remain forbidden
```

Do not implement the real `ploke-tui` adapter yet. Do not move authority into
`Harness`; it remains executor-only.

Later blocker before `bounded-edit-surface-tui-adapter (7.6)`:

```text
bounded-edit-surface-request-policy-receipt (7.5.1)
bounded-edit-surface-generator-provenance (7.5.2)
```

Use a fake Router or deterministic harness to prove that proposal-producing
Router calls return complete effective request-policy receipts, and that
proposal admission rejects missing or incomplete receipts.

Also prove that proposal records cite generator-surface versions from the
parent Artifact before the real TUI adapter depends on mutable generator
machinery.

## Suggested Sub-Agent Pattern

Use one scout at a time unless there are independent questions.

Good next scout prompt:

```text
Read docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md
and docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md.

Task: bounded-edit-surface-choice-objective (7.3).

Inspect the new edit_surface surface primitives:
  EditObjective
  ProtectedCore
  EditableSurface::broad
  Grant::with_forbidden

Find the smallest parent-side boundary where real-ish parent-time
History/context evidence can be converted into an EditObjective and
EditableSurface. The current tests hand-construct A; the missing splice is
A -> B*.

Return exact files, line ranges, test names, and the smallest A -> B* splice
test to add. Keep Harness executor-only.
```

Useful sidecar scout, if needed:

```text
Inspect edit_surface graph/surface/harness modules and propose the smallest
type-level patch to represent R/W/F, Qr/Qw, and CheckedProposal without broad
refactors. Return exact files and tests.
```

Avoid asking agents to reread all evalnomicon docs. Point them to this handoff
and the current adapter plan.

## Verification Notes

Known useful targeted command from prior audit:

```bash
cargo test -p ploke-eval edit_surface::tests -- --nocapture 2>&1 | tail -n 30
```

Use bounded output for cargo tests per `AGENTS.md`.
