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

The replay-shaped parent-input splice is now covered on the History side:
typed rejected surface-attempt evidence routes through `Diagnosis ->
EditObjective -> SurfaceRequest` and admits to `EditableSurface`. Backend/TUI
proposal-touch lowering is also covered at the local/proof boundary; the real
live adapter remains a future slice.

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

## Current Implementation Status: After Narrow Slice 7.5 Local/Proof Coverage

Task-stack group:

```text
bounded-edit-surface-evidence-route
```

Closed tasks:

```text
bounded-edit-surface-attempt-evidence (7.1)
bounded-edit-surface-diagnosis-splice (7.2)
bounded-edit-surface-choice-objective (7.3)
```

Closed slice tasks:

```text
bounded-edit-surface-mock-candidate (7.5 narrow splice)
```

Open follow-up tasks:

```text
bounded-edit-surface-request-policy-receipt (7.5.1)
bounded-edit-surface-generator-provenance (7.5.2)
bounded-edit-surface-tui-adapter (7.6)
```

Implemented inside 7.3:

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
30 edit_surface tests passed
```

Important: these tests prove only the local primitive:

```text
explicit EditObjective + explicit ProtectedCore + explicit Γ_a
  -> EditableSurface::broad
  -> Grant::check
```

`EditObjective` now carries a structured machine-readable `ObjectiveSpec`
alongside evidence refs.

7.4 extends this with local/proof coverage for the parent-side route and
proposal-touch lowering:

- `SurfaceRequest::broad(...).admit()`
- `route::semantic_resolution(...)` from typed replay-shaped History evidence
- backend proposal-touch splices through `tui::MaterialSpan` /
  `tui::Bounds::touch` into `surface::Touch`
- `Grant::check` accepting the checked touch shape

These remain local/proof-level slices. Real History extraction, live
Router-backed request-policy receipt admission, live TUI adapter execution, and
T6 selection/outcome selectability remain future work.

7.5 narrow splice is now implemented and reviewed:

- `CheckedSurfaceEdit::surface_evidence(...)`
- `child_files_from_checked_edit(...)`
- `validate_requested_tui_surface_child(...)`
- `checked_edit_surface_candidate_is_accepted_by_tui_child_plan_consumer`

What this proves:

```text
EditProposal
  -> CheckedSurfaceEdit / ArtifactDelta
  -> ChildFiles / SurfaceEvidence
  -> requested TUI-surface child-plan consumer acceptance
```

This does not prove live generation, live Router-backed request-policy receipt
admission, the real ploke-tui adapter, or parent selection of the resulting
outcome.

7.5.1 is partially implemented:

- `edit_surface::request_policy` defines the typed request-policy receipt
  carrier and canonical `client_policy_hash`.
- `request_policy_receipt_hash_is_stable_for_equivalent_effective_provider_policy`
  proves equivalent effective client policies hash the same way.
- `request_policy_receipt_hash_changes_when_effective_policy_changes` proves a
  material effective policy change changes the hash.
- `requested_tui_surface_child_rejects_router_backed_proposal_producer` proves
  Router-backed provenance is not silently accepted on the deterministic
  non-router child path.

7.5.1 is not closed: the live proposal-producing Router request builder and
end-to-end Router-backed proposal admission splice still need to be added.

7.5.2 is implemented in bounded deterministic/non-router scope:

- `tui::GeneratorSurfaceVersion` records the generator surface version derived
  from typed TUI bounds/projection material.
- `EditProposal -> CheckedSurfaceEdit -> SurfaceEvidence` now carries that
  generator-surface provenance.
- `edit_surface_bridge_rejects_mutated_generator_surface_provenance` proves
  backend admission rejects forged generator provenance.
- `requested_tui_surface_child_rejects_missing_generator_surface_provenance`
  and `requested_tui_surface_child_rejects_mismatched_generator_surface_provenance`
  prove child-plan consumption fails closed when provenance is absent or does
  not match the requested TUI surface.

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
bounded-edit-surface-request-policy-receipt (7.5.1)
```

Corrected status:

```text
7.4 local/proof coverage is closed at the local boundary.

7.3 implemented:
  explicit EditObjective + explicit ProtectedCore + explicit Γ_a
    -> EditableSurface::broad
    -> Grant::check

7.4 added the local/proof route:
  typed replay-shaped History evidence + graph projection + protected-core
  policy
    -> SurfaceRequest::broad(...).admit()
    -> route::semantic_resolution(...)
    -> proposal-touch lowering into surface::Touch
    -> Grant::check accepts the checked touch shape

7.5 narrow splice is implemented/proven locally:
  SurfaceRequest + checked proposal/touches
    -> CheckedSurfaceEdit / ArtifactDelta
    -> ChildFiles / SurfaceEvidence
    -> requested TUI-surface child-plan consumer accepts it

7.5.1 partial:
  request-policy receipt carrier + stable client_policy_hash local tests exist,
  but live Router-backed proposal request/admission is still open

7.5.2 implemented/proven in bounded deterministic/non-router scope:
  GeneratorSurfaceVersion
    -> EditProposal
    -> CheckedSurfaceEdit
    -> SurfaceEvidence
    -> backend and child-plan provenance checks

Remaining:
  live Router-backed request-policy receipt admission, live TUI adapter, and
  T6 selection/outcome selectability.
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
  - route::semantic_resolution and proposal-touch splice coverage

docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md
  - indexes those tests as local primitive proofs only
  - explicitly separates the completed local/proof 7.4 route and narrow 7.5
    downstream candidate splice from later live Router/TUI/selection work
```

Important correction:

```text
Do not reopen 7.3 or 7.5 merely because live generation or parent selection is
missing. The current tests prove the local surface/objective primitive, the 7.4
local/proof route, and the narrow 7.5 checked-candidate-to-child-plan splice.
The bounded 7.5.2 generator-surface provenance splice is also now closed for
deterministic/non-router proposal evidence. Live Router-backed request-policy
receipt admission, live TUI adapter execution, and T6 selection/outcome
selectability remain separate open tasks.
```

The plan direction also changed. The current preferred first pass is no longer
a narrow `Diagnosis -> one semantic resolver SurfaceChoice` route. It is the
broad protected-core route, with 7.4 proving the route/proposal-touch splice
and 7.5 proving the candidate acceptance splice:

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

Implemented narrow 7.5 splice:

```text
bounded-edit-surface-mock-candidate (7.5)

A':
  SurfaceRequest + checked proposal/touches

B*:
  CheckedSurfaceEdit / ArtifactDelta / candidate artifact evidence

C':
  requested TUI-surface child-plan consumer accepts it
```

Partially implemented inside 7.4:

- `SurfaceRequest::broad(...)`
- `SurfaceRequest::admit()`
- `surface_request_admits_parent_context_into_broad_surface`

This proves the first local route/admission carrier:

```text
synthetic parent evidence/context refs
  + explicit graph bounds Γ_a
  + explicit ProtectedCore F
    -> SurfaceRequest
    -> EditObjective + EditableSurface
    -> ordinary write passes / protected-core write fails
```

It does not yet prove extraction from real History records, `ploke-tui`
proposal events, or backend `ProposedTouch` receipts.

Do not treat this as one undifferentiated route test. The route should be
closed by transition proofs:

```text
T1 EvidenceAdmitted:
  replay-shaped History/context evidence
    -> parent evidence/context admission
    -> admitted refs for EditObjective

T2 ObjectiveConstructed:
  admitted refs + broad ruling policy
    -> EditObjective
    -> intent/evidence/constraints/success criteria recorded

T3 SurfaceBounded:
  EditObjective + Γ_a + ProtectedCore
    -> EditableSurface::broad
    -> W = Γ_a \ F

T4 ProposalChecked:
  SurfaceGrant + proposed touches
    -> SurfaceCheck / Grant::check
    -> ordinary writes pass, protected-core writes fail

T5 CandidateProduced:
  checked proposal + target Artifact
    -> checked apply / ArtifactDelta
    -> candidate Artifact evidence accepted downstream

T6 OutcomeSelectable:
  evaluated child candidate with provenance
    -> parent selection fold
    -> History-backed selection and successor handoff
```

Current 7.3 coverage is local T2/T3/T4 primitive behavior. The 7.4 coverage is
local/proof route/admission plus proposal-touch lowering from typed
replay-shaped evidence into the 7.3 primitive. The narrow 7.5 coverage proves
checked edit evidence lowers into `ChildFiles` and is accepted by the requested
TUI-surface child-plan consumer.

7.5.2 is now closed in bounded deterministic/non-router scope. Proposal
evidence cites a `tui::GeneratorSurfaceVersion`, and that provenance is checked
at both backend admission and requested TUI child-plan validation:

```text
typed TUI bounds/projection material
  -> GeneratorSurfaceVersion
  -> EditProposal
  -> CheckedSurfaceEdit
  -> SurfaceEvidence
  -> requested TUI child-plan validation
```

Tests added for this closure:

```text
edit_surface_bridge_rejects_mutated_generator_surface_provenance
requested_tui_surface_child_rejects_missing_generator_surface_provenance
requested_tui_surface_child_rejects_mismatched_generator_surface_provenance
```

7.5.1 is now closed for the current live-path proof shape. The request-policy
receipt carrier binds explicit client-policy/proposal evidence to the live 7.6
proposal shape without claiming deterministic replay or provider-side
completeness:

```text
request_policy_receipt_hash_is_stable_for_equivalent_effective_provider_policy
request_policy_receipt_hash_changes_when_effective_policy_changes
requested_tui_surface_child_rejects_router_backed_proposal_producer
router_receipt_rejects_unset_payload_hashes_for_admission
router_receipt_rejects_mismatched_run_binding_for_admission
router_proposal_producer_rejects_mismatched_base_artifact_id
live_tui_router_staged_proposal_lowers_to_checked_artifact_delta
```

The current receipt proves:

```text
parent Artifact + Router config/defaults + EditObjective
  -> live 7.6 proposal id/run id shape
  -> explicit client-policy receipt
  -> explicit PayloadHash::{Known, Unknown(reason)}
  -> Router-backed proposal admission rejects missing/incomplete/mismatched
     proposal/run/base-artifact binding
```

What remains is no longer the current 7.5.1 proof. It is deferred hardening
tracked as `bounded-edit-surface-outbound-request-capture`:

```text
actual serialized outbound request digest
normalized response digest
tool schema digest
provider route/metadata where available, or explicit unknowns
```

Do not add a mock Router detour for this hardening. Live API tests are
explicitly allowed and expected, gated by the repo's live-test controls.

7.6 first live adapter slice is closed:

```text
live ploke-tui TestRuntime + OpenRouter x-ai/grok-4-fast / xai
  -> real model/tool loop stages apply_code_edit proposal
  -> staged WriteSnippetData lowers into eval touches
  -> eval Grant::check
  -> checked ArtifactDelta evidence
```

The live test intentionally records explicit current client-policy/proposal
binding evidence. It must not be described as complete outbound request,
tool-schema, provider-route, or deterministic replay coverage.

## Suggested Sub-Agent Pattern

Use one scout at a time unless there are independent questions.

Good next scout prompt:

```text
Read docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md
and docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-surface-proof-index.md.

Task: bounded-edit-surface-mini-run (7.7).

Inspect how to connect the now-proven live TUI adapter evidence into the
smallest Prototype 1 trampoline proof:
  live bounded objective
  checked TUI proposal / ArtifactDelta evidence
  child materialization
  child evaluation
  History-backed selection visibility

Do not implement mock Router. Do not claim complete 7.5.1 replay receipts.
Return exact files, line ranges, test names, and the smallest A -> B* -> C'
splice test or command to add.
```

Useful sidecar scout, if needed:

```text
Inspect the live request-policy path and identify where the partial client
policy receipt could be bound to the actual outbound Router request without
mocking Router. Return exact files and a deferred 7.5.1 patch plan only.
```

Avoid asking agents to reread all evalnomicon docs. Point them to this handoff
and the current adapter plan.

## Verification Notes

Known useful targeted command from prior audit:

```bash
cargo test -p ploke-eval edit_surface::tests -- --nocapture 2>&1 | tail -n 30
```

Recent targeted commands from the 7.5.1/7.5.2 pass:

```bash
cargo test -p ploke-eval real_tui_resolver_touch_is_checked_before_adapter_apply 2>&1 | tail -n 80
cargo test -p ploke-eval tui_bounds_touches_requires_one_target_per_write 2>&1 | tail -n 60
cargo test -p ploke-eval live_tui_router_staged_proposal_lowers_to_checked_artifact_delta 2>&1 | tail -n 100
PLOKE_RUN_LIVE_TESTS=1 cargo test -p ploke-eval live_tui_router_staged_proposal_lowers_to_checked_artifact_delta -- --nocapture 2>&1 | tail -n 100
cargo test -p ploke-eval request_policy_receipt_hash 2>&1 | tail -n 20
cargo test -p ploke-eval edit_surface_bridge_rejects_mutated_generator_surface_provenance 2>&1 | tail -n 20
cargo test -p ploke-eval requested_tui_surface_child_rejects_router_backed_proposal_producer 2>&1 | tail -n 20
cargo test -p ploke-eval requested_tui_surface_child_rejects_mismatched_generator_surface_provenance 2>&1 | tail -n 20
cargo test -p ploke-eval checked_edit_surface_candidate_is_accepted_by_tui_child_plan_consumer 2>&1 | tail -n 60
```

Use bounded output for cargo tests per `AGENTS.md`.
