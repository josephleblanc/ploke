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

- [`docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md`](../../workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md)
  Current implementation plan. Includes formal surface algebra, current blocker
  status, parent planning layer, phase plan, and sub-agent brief.
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

## Suggested Sub-Agent Pattern

Use one scout at a time unless there are independent questions.

Good first scout prompt:

```text
Inspect current ploke-eval History/scoring/selection records and report what
typed evidence is available to a parent before candidate generation. Do not
design new code. Return file paths, line ranges, and which fields can support
Diagnosis -> SurfaceChoice -> EditObjective.
```

Second scout only after that:

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

