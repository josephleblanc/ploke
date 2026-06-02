# Lanes And Six-Slot Posture

This file is durable coordination state. Live assignment state belongs in
`.orchestrator/board.json` through `target/debug/xtask orchestrate`.

## Default Six-Slot Posture

Treat the six slots as a conveyor, not six unrelated ideas.

| Slot | Role | Default lane | Purpose | Write posture |
|---|---|---|---|---|
| 1 | Retainer | `ui-retainer` | Read-only locator for code/doc/inventory questions. | No writes. |
| 2 | Inventory or graph-gap worker | `ui-inventory` first, `ui-graph-gap` only if proven necessary | Refresh stale coverage claims or fix a specific missing typed join. | Docs only by default; tree/records writes only after a proven blocker. |
| 3 | Projection worker | `ui-projection` | Keep the UI projection borrowed and readability-oriented. | `ploke-egui` projection/import/test surfaces only. |
| 4 | Diagnostics or layout worker | `ui-diagnostics` first, `ui-layout` after metrics land | Measure readability, then tune geometry. | One lane at a time. |
| 5 | Inspector/docs worker | `ui-inspector-docs` | Snapshot fields, app summary, plan/inventory updates, first inspector glue, all render-only. | App/docs surfaces only. |
| 6 | Reviewer | `ui-review` | Independent boundary/readability review. | Read-only unless explicitly converted into a fixer. |

Keep slot 6 as reviewer once implementation begins. During very early setup it
may answer one bounded retainer question, but free it again before accepting
shared-structure changes.

## Lane Ownership

Recommended lane set:

| Lane | Owned edit surfaces | Purpose |
|---|---|---|
| `ui-retainer` | none | Read-only context, file-location, and stale-claim checks. |
| `ui-inventory` | `docs/active/agents/ploke-ui-task-readability/inventory`, `docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md` | Refresh first-wave coverage claims without broadening scope. |
| `ui-projection` | `crates/ploke-egui/src/ui/view/projection.rs`, `crates/ploke-egui/src/import/mod.rs`, `crates/ploke-egui/src/import/tests.rs` | Borrowed artifact/candidate/selection projection work. |
| `ui-diagnostics` | `crates/ploke-egui/src/ui/view/diagnostics.rs`, `crates/ploke-egui/src/ui/view/geometry.rs`, `crates/ploke-egui/src/ui/view/label.rs` | Readability metrics, edge/label interference, and ranked findings. |
| `ui-layout` | `crates/ploke-egui/src/ui/view/layout.rs`, `crates/ploke-egui/src/ui/view/style.rs`, `crates/ploke-egui/src/ui/view/edge.rs` | Geometry tuning once metrics identify real failures. |
| `ui-inspector-docs` | `crates/ploke-egui/src/diagnostics/mod.rs`, `crates/ploke-egui/src/ui/app/mod.rs`, `crates/ploke-egui/src/native.rs`, `docs/active/agents/ploke-ui-task-readability` | Snapshot persistence, reference-only shell/summary text, durable docs, narrow inspector glue. |
| `ui-review` | none | Boundary and readability review. |
| `ui-graph-gap` | closed by default; open only with a proven blocker | Exact `ploke-tree` / `ploke-records` files needed for one missing typed relation or loader. |

## Shared-Pressure Hotspots

Keep these serialized or main-thread-reviewed even if a worker touched adjacent
files:

- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-egui/src/ui/mod.rs`
- `crates/ploke-egui/src/lib.rs`
- `crates/ploke-egui/src/main.rs`
- broad `crates/ploke-tree/src/graph/mod.rs` and `crates/ploke-tree/src/store/mod.rs`

Additional serialization hotspots:

- `crates/ploke-egui/src/ui/view/projection.rs` when `WidgetGraph`,
  `GraphEdgePayload`, or `ViewEdgeKind` change.
- `crates/ploke-egui/src/ui/view/style.rs` whenever layout spacing, curve, or
  status-color fields change.
- `crates/ploke-egui/src/ui/view/mod.rs`,
  `crates/ploke-egui/src/diagnostics/mod.rs`, and
  `crates/ploke-egui/src/ui/app/mod.rs` together when diagnostics fields change.

These files define boundaries, exports, or module stitching. They are easy to
turn into merge-conflict or authority-drift hotspots.

## Immediate Task Sequence

1. `ui-retainer`
   - locate exact semantic-borrow boundaries and any remaining clone risks.
2. `ui-inventory`
   - refresh which graph/loader rows matter for the first UI drilldowns.
3. `ui-projection`
   - expose borrowed selected-lineage/candidate handles for diagnostics.
4. `ui-diagnostics`
   - measure rank spacing, overlap, and primary-lineage salience.
5. `ui-inspector-docs`
   - persist accepted render-only metrics and keep the docs/summary current.
6. `ui-review`
   - audit authority, naming, and readability claims before layout tuning.
7. `ui-layout`
   - only after diagnostics produce actionable failures.

## Blocker Policy

Reference-only violations outrank ordinary blockers.

If a worker allocates or clones semantic values out of `Graph`, or reconstructs
mirror carriers to avoid borrowing:

- stop the slice immediately,
- mark it as a hard blocker,
- repair it before any downstream lane continues,
- do not treat it as follow-up cleanup.

If a worker needs a file outside its lane:

- stop,
- report the blocker,
- let the main thread split or reassign the task.

If two workers need the same file:

- serialize them,
- or convert one worker into a reviewer.

If a worker proves a missing semantic fact:

- do not add a UI-local mirror,
- reclassify slot 2 from `ui-inventory` to `ui-graph-gap`,
- assign only the exact `ploke-tree` / `ploke-records` files needed,
- update the inventory in the same change.

If a lane blocks on downstream review:

- move an idle writer to another independent lane,
- or refresh the retainer/inventory slot instead of waiting.

## Refresh Policy

Close and respawn a worker when any of these becomes true:

- it has answered about five bounded questions,
- its report assumes stale board state,
- the phase changed from discovery to implementation or from implementation to
  review,
- it begins proposing UI-owned semantic mirrors,
- it crosses from doc refresh into graph or UI code edits,
- its write surface needs to change.

Before closing a useful worker, require a compact handoff using
[`handoffs/handoff-template.md`](handoffs/handoff-template.md).

## Long-Horizon Priorities

Every lane should optimize for:

- maintainable structure over patchy cosmetic wins,
- typed joins over stringly inference,
- reference-based UI semantics over copied ids or records,
- drilldown-ready answer objects over ad hoc widget payloads,
- narrow vertical slices with tests and review,
- reusable graph/readability foundations for future browser/gui fronts.
