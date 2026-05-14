# ploke UI Task-Readability Plan

Date: 2026-05-12

## Objective

Make the current UI surface readable enough to answer the first operator
question quickly:

> What artifact lineage is currently selected or promoted, what nearby
> alternatives exist, and where should I drill next?

The immediate execution surface is `ploke-egui`. The plan must also keep the
path open for `ploke-tree-egui`, `ploke-tree-browser`, and a future `ploke-gui`
without making today’s egui slice the semantic authority.

Out of scope for the first wave:

- `crates/ploke-egui/src/demo.rs`
- `crates/ploke-egui/src/web.rs`
- `crates/ploke-tree-egui/**`

Those are demo or older surface paths, not the active readability authority.

## Hard Boundary

- `ploke-tree::graph::Graph` is the semantic authority.
- Typed persisted records live behind `ploke-records` and `ploke-tree`
  loaders.
- UI code may compute layout, diagnostics, visibility scores, inspector state,
  and render-only strings.
- UI code may not own artifact/runtime/patch/candidate/selection/history facts
  as substitute semantic carriers.
- App summaries and snapshot text must stay reference-derived and render-only.
- If a needed fact is missing, fix the typed loader or graph boundary instead of
  inventing a UI-local mirror.

## Reference-Only Rule

This rule is strict, absolute, and phase-gating:

`ploke-egui` and adjacent UI work must use references into
`ploke_tree::graph::Graph` for semantic access.
Snapshot and app-summary surfaces may only project accepted diagnostics and
derived counts; they must not become semantic carriers.

Any phase that completes with allocated or cloned semantic values taken out of
`Graph` for later semantic use is immediately invalid. That is a hard failure
and must be repaired before any current work continues.

Blocked examples:

- copied semantic ids kept in UI state as join handles,
- copied refs or partial records stored for later semantic use,
- `Option<SomeReport>`, `Status`, `Info`, `Summary`, `Payload`, or similar
  wrappers that merely repackage graph meaning,
- mirror `Artifact`, `Runtime`, `Patch`, `Candidate`, `Selection`, or
  `History` types reconstructed in UI code to avoid borrowing from `Graph`.

Allowed derived values are only values with clearly derived meaning that cannot
act as substitute semantic authority, for example:

- layout positions,
- rectangles,
- edge curves,
- visibility or overlap scores,
- summed durations,
- average token counts,
- render-only strings that are not later used as semantic join keys.

When in doubt, treat a value as semantic until proven otherwise.

## Audit Rule

Reference-only audits are required for every lane completion that touches
projection, diagnostics, snapshot, app, layout, inventory, or governance docs.
They should also be folded into every review step.

The audit question is not only "was something cloned?" but also "did the patch
defeat the spirit of the rule by reconstructing mirror carriers or semantic
wrappers that avoid direct borrowing while preserving the same authority?"

Any violation is a hard blocker, not deferred cleanup. A clean earlier audit
does not cover later lane completions.

## Long-Horizon Questions

The readability wave is only the first front door. The resulting structure must
stay compatible with the typed drilldown contract, especially these answer
families:

- `ui.child.lineage.genesis`
- `ui.successor.candidate.frontier`
- `ui.selection.candidate.set`
- `ui.successor.selection`
- `ui.child.self.eval.actions`
- `ui.tool.calls`
- `ui.provider.attempts`
- `ui.patch.diff.view`

This means the plan should prefer typed joins and explicit ambiguity over
screenshot-local tweaks or egui-only convenience state. Any semantic answer
object belongs below the UI boundary in `ploke-tree`, `ploke-records`, or an
explicit domain projection crate; UI code may borrow or query it, but may not
mirror it.

## Current State

The live board has been reset to a clean `ui-*` wave.

- Previous mixed board state was archived under
  `.orchestrator-archive/2026-05-12-pre-ui-wave`.
- Current live state is `.orchestrator/board.json`.
- Active lanes are `ui-retainer`, `ui-inventory`, `ui-projection`,
  `ui-diagnostics`, `ui-inspector-docs`, `ui-review`, `ui-layout`, and
  `ui-governance-docs`.
- The board must validate cleanly before spawning or reassigning workers.

## The Larger Object

The system being built is:

```text
typed records
  -> RunRecordSet
  -> ploke-tree::Graph
  -> borrowed UI projection
  -> layout and readability diagnostics
  -> typed drilldown/inspector paths
  -> future browser/gui consumers
```

The reduction to refuse is:

```text
ugly screenshot -> tweak UI constants -> add UI-owned semantic helpers
```

The correct progression is:

```text
typed graph facts -> borrowed projection -> measured readability failures
-> layout/inspector improvements -> future drilldowns
```

## Phase Plan

### Phase 0: Board Hygiene And Shared Frame

- Establish this directory as the durable planning area.
- Keep live state in `target/debug/xtask orchestrate`, not in markdown.
- Run with one retainer and one reviewer reserved as soon as code patches land.
- Refresh the stale ingestion inventory only for surfaces needed by the current
  UI question set.
- Establish the reference-only rule as the first acceptance gate.

Exit criteria:

- lane ownership is defined,
- worker packet brief exists,
- board policy is clear about reset vs isolated `ui-*` lanes.

### Phase 1: Readability Floor In `ploke-egui`

- Keep semantic access borrowed from `&Graph`.
- Make lineage/candidate visibility failures measurable.
- Improve diagnostics before layout tuning.
- Keep snapshot and app summary surfaces strictly render-facing and limited to
  accepted diagnostics or reference-derived counts.
- Reject any semantic clone, mirror type, or report-wrapper workaround
  immediately.

Primary files:

- `crates/ploke-egui/src/ui/view/projection.rs`
- `crates/ploke-egui/src/ui/view/diagnostics.rs`
- `crates/ploke-egui/src/ui/view/geometry.rs`
- `crates/ploke-egui/src/ui/view/label.rs`
- `crates/ploke-egui/src/ui/view/layout.rs`
- `crates/ploke-egui/src/ui/view/style.rs`
- `crates/ploke-egui/src/diagnostics/mod.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/native.rs`

Exit criteria:

- selected-lineage and candidate readability are measured with ranked findings,
- no semantic ids, refs, records, partial records, or wrapper carriers are
  allocated out of `Graph` for later semantic use,
- tests or focused checks cover the new projection/diagnostic behavior.

### Phase 2: Graph-Readiness Refresh For Near-Term Drilldowns

- Re-check the stale graph-ingestion inventory against current code.
- Refresh only rows needed for artifact, candidate, selection, evaluation, and
  first inspector joins.
- Distinguish:
  - true graph semantic gaps,
  - typed loader gaps,
  - UI projection gaps,
  - deferred drilldowns.

Priority drilldown slices to keep in view:

- `runtime-artifact-lineage`
- `candidate-frontier.replay`
- `evaluation-oracle-targets.selection-replay`
- `run-execution-graph.browser-spine`

Exit criteria:

- the inventory no longer overstates or understates current coverage for the
  first UI wave,
- any blocker proves exactly which layer is missing authority.

### Phase 3: First Typed Inspector Paths

- Add one or two narrow inspector/drilldown paths only after the readability
  floor and graph-readiness refresh are stable.
- Prefer one parent/child/selection path and one evaluation/tool path over a
  broad inspector skeleton.
- Keep browser/gui consumers downstream of borrowed or queried domain answer
  objects, not bespoke UI state.

Good first candidates:

- `ui.successor.selection`
- `ui.selection.candidate.set`
- `ui.child.self.eval.actions`

## Dependency Rules

- Layout tuning depends on readability diagnostics.
- Snapshot/app summary updates depend on accepted render-only diagnostics.
- Graph/loader work is dormant until a worker proves the UI lacks a needed fact.
- Future browser/gui work depends on domain-owned answer objects, not on
  egui-only widget state.

When a lane blocks:

1. classify the blocker,
2. record it in the board,
3. either retask an idle slot to the missing layer or keep moving on an
   independent lane,
4. do not unblock by adding UI-local semantic mirrors.

If the block is a reference-only violation:

1. stop the current slice,
2. repair the violation before downstream work continues,
3. treat the phase as failed until the repair lands and is reviewed.

## Done Gates

This wave counts as structurally successful only when all are true:

- the live board and the durable docs agree on lane purpose,
- the reviewer can explain the selected-lineage/candidate readability failure
  without invoking UI-owned semantic carriers,
- the reference-only audit passes for the phase, with no semantic clone,
  wrapper, mirror-type workaround, or authority-bearing summary surface left in
  place,
- the refreshed inventory clearly marks what is covered vs. still missing for
  the first typed drilldowns,
- a successor worker can restart from this directory without reading unrelated
  historical docs first.
