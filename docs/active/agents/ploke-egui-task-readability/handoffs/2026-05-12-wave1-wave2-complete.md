# ploke-egui Task Readability Wave 1-2 Handoff

Date: 2026-05-12

## State

The first two task-readability waves are complete and reviewed on the
orchestrator board.

Completed reviewed egui tasks:

- `egui-retain-active-boundary`
- `egui-projection-borrowed-handles`
- `egui-diagnostics-task-readability-v1`
- `egui-snapshot-findings-v1`
- `egui-remove-legacy-semantic-graph`
- `egui-review-boundary-v1`
- `egui-review-blocker-repairs`
- `egui-projection-borrowed-passive-index`
- `egui-layout-after-diagnostics`
- `egui-review-phase2-layout-index`

The previous board was archived at
`.orchestrator.archive.2026-05-12-egui-task-readability`; the current board is
the clean egui wave board, though unrelated `bh-*` lanes appeared during the
run and should be treated as board noise unless that work is resumed.

## Boundary Result

The hard reference-only rule currently passes review:

- `ploke_tree::Graph` remains the semantic authority.
- `ploke-egui` no longer exports or compiles an egui-owned semantic graph.
- `crates/ploke-egui/src/graph/mod.rs` was deleted.
- `crates/ploke-egui/src/import/history.rs` was deleted.
- `crates/ploke-egui/src/history_playback.rs` was deleted as dead stale
  semantic bypass clutter.
- `GraphSummary` and `GraphCounts` were removed.
- `TreatmentBranchStatus` was removed from widget payloads and diagnostics.
- Widget payloads own only render data: labels, colors, edge style, and
  `ViewEdgeKind`.
- Artifact branch lookup uses a borrowed
  `HashMap<&ArtifactId, &ArtifactNode>`.

Current audit command:

```bash
rg -n "serde_json::Value|from_reader|read_to_string|load_record|RunRecordSet|struct .*Artifact|struct .*Runtime|struct .*Patch|struct .*Candidate|struct .*Selection|struct .*History|struct .*Report|struct .*Info|struct .*Status|struct .*Summary|struct .*Payload|struct .*Counts|TreatmentBranchStatus|pub mod graph|crate::graph|ploke_egui::graph|history_playback" crates/ploke-egui/src
```

Expected production hits are limited to render/projection struct names,
`StatusColors`, `GraphEdgePayload`, and typed import boundary references.

## Implemented

- Projection now resolves passive artifacts through borrowed references without
  cloning semantic artifact ids for lookup.
- Projection now converts semantic classification to render-only colors before
  widget graph conversion.
- Diagnostics now compute selected-path crossings from `ViewEdgeKind`
  `HistoryArtifact` only, avoiding copied status authority.
- Diagnostics now distinguish edge-label intersections from label-own-edge
  collisions and handle rank-spacing outliers via geometry.
- Snapshot diagnostics now include ranked human-readable findings.
- The app side panel renders graph facts directly from `&Graph`; it no longer
  stores count/summary wrappers.
- Layout uses leaf-span subtree placement rather than local sibling offsets.
- Layout/style defaults now use larger spacing, larger label gap, smaller edge
  label text, stronger selected color, and a larger curve handle minimum.

## Verification

Latest main-thread checks passed:

```bash
cargo check -p ploke-egui 2>&1 | tail -n 80
cargo test -p ploke-egui 2>&1 | tail -n 80
```

`cargo test -p ploke-egui` reported 12 passed, 1 ignored. The ignored test is
`import::tests::real_run_imports_execution_spine`.

## Residual Risks

- `ViewEdgeKind` is render/diagnostic classification. Do not let it grow into
  History authority or status reconstruction.
- Non-History primary lineage salience is intentionally deferred until it can be
  computed from references into `ploke_tree::Graph`.
- `GraphSignature` is count-based; same-count graph content changes can leave
  the widget cache stale.
- Layout tests cover nested subtree leaf spans, but not explicit shared-child
  DAG or cycle fixtures.
- Snapshot findings do not yet expose distinct rank spacing, subtree overlap,
  primary-path visibility, or straightness fields.

## Next Slices

Recommended order:

1. Fix `GraphSignature` so cache invalidation cannot miss same-count graph
   content changes. Preserve reference-only semantics.
2. Add shared-child/cycle layout tests around the new leaf-span layout.
3. Design primary-lineage salience diagnostics with references into `Graph`,
   not copied status flags.
4. Expose distinct diagnostic fields for rank spacing, subtree overlap, primary
   path visibility, and path straightness.
5. Only after those diagnostics exist, tune layout further against measured
   failures.

