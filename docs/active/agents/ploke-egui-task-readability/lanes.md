# Lanes And Six-Slot Posture

This file is durable planning state. Live assignment state belongs in
`.orchestrator/board.json` through `target/debug/xtask orchestrate`.

## Slot Policy

Use the six available sub-agent slots as a conveyor, not as six independent
brainstorms.

Default posture:

| Slot | Role | Purpose | Write posture |
|---|---|---|---|
| 1 | Retainer | Read-only locator for code, docs, and stale inventory questions. | No writes. |
| 2 | Projection worker | Add borrowed projection structure needed for diagnostics. | `crates/ploke-egui/src/ui/view/projection.rs` and tests only. |
| 3 | Diagnostics worker | Add geometry metrics and ranked findings. | `crates/ploke-egui/src/ui/view/diagnostics.rs`, `label.rs`, `geometry.rs` only when needed. |
| 4 | Layout worker | Improve tree layout after metrics exist. | `crates/ploke-egui/src/ui/view/layout.rs`, `style.rs` only. |
| 5 | Snapshot/docs worker | Persist diagnostics, update UI summary, refresh docs/inventory. | `crates/ploke-egui/src/diagnostics/mod.rs`, `ui/app/mod.rs`, this doc dir. |
| 6 | Reviewer | Boundary/readability review of completed slices. | Read-only unless explicitly converted to fixer. |

Keep one slot as reviewer once implementation patches start landing. During
initial setup, slot 6 may be a second retainer for a bounded question, but it
must be freed before accepting code that touches shared structures.

## Lane Ownership

Proposed board lanes:

| Lane | Owned edit surfaces | Purpose |
|---|---|---|
| `egui-readability-retainer` | none | Read-only context, code location, stale inventory checks. |
| `egui-projection` | `crates/ploke-egui/src/ui/view/projection.rs` | View keys, artifact projection structure, selected lineage handles. |
| `egui-diagnostics` | `crates/ploke-egui/src/ui/view/diagnostics.rs`, `crates/ploke-egui/src/ui/view/label.rs`, `crates/ploke-egui/src/ui/view/geometry.rs` | Geometry-level metrics and finding generation. |
| `egui-layout` | `crates/ploke-egui/src/ui/view/layout.rs`, `crates/ploke-egui/src/ui/view/style.rs` | Layout tuning driven by diagnostics. |
| `egui-snapshot-docs` | `crates/ploke-egui/src/diagnostics/mod.rs`, `crates/ploke-egui/src/ui/app/mod.rs`, `docs/active/agents/ploke-egui-task-readability` | Snapshot persistence, app summary, durable docs. |
| `egui-graph-gap` | `crates/ploke-tree/src/graph`, `docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md` | Only if a missing semantic relation is proven. |
| `egui-review` | none | Boundary and task-readability review. |

Do not put `crates/ploke-egui/src/graph/mod.rs` in an implementation lane
without an explicit cleanup task. It is legacy/suspect until audited against
the active `ploke_tree::Graph` import path.

## Immediate Task Set

Recommended first wave:

1. Retainer: identify exact line ranges where view node/edge payloads are
   built, where labels/edge geometry are available, and whether the active path
   already clones semantic values out of `ploke_tree::Graph`.
2. Projection worker: add borrowed structure sufficient for diagnostics to
   group ranks, edges, and selected/promoted lineage without duplicating or
   cloning semantic facts.
3. Diagnostics worker: add rank-spacing and selected-path straightness metrics
   over existing widget geometry.
4. Snapshot/docs worker: extend snapshot schema and UI summary for new metrics.
5. Reviewer: audit projection and diagnostics for graph-boundary violations.

Layout tuning starts only after Phase 1 diagnostics produce failing or
thresholded signals.

## Blocker Handling

If a worker needs a file outside its lane:

- report the blocker,
- stop before editing,
- main thread either reassigns the file lane or splits the task.

If two workers need the same file:

- serialize those tasks,
- or convert one worker to read-only reviewer.

If a worker discovers a missing semantic relation:

- do not add an egui-local semantic field,
- do not clone ids/refs/records into egui as a workaround,
- write a graph-gap report,
- create a graph-lane task that names the source fact, join key, and ambiguity.

If board state is stale:

- run bounded status,
- either add isolated new lanes with unique ids,
- or archive/remove `.orchestrator` and start a clean wave at main-thread
  discretion.

## Long-Horizon Priorities

Every lane should optimize for:

- maintainable structure over screenshot-local tweaks,
- references into `ploke_tree::Graph` over copied semantic payloads,
- readable diagnostic failures over one scalar beauty score,
- typed joins and evidence strength over inferred meaning,
- small vertical slices with tests,
- no semantic authority drift from `ploke-tree` into `ploke-egui`.
