Native interactive window allocation benchmarking was verified for the standard
`select_artifact_inspector_300` scenario; target-fixture Selection drilldown
native behavior was not measured because the standard suite rejects non-standard
run roots.

## Change Summary

- Added a borrowed `Selection` Inspector section backed by `ploke_tree::Graph`
  selection metric witnesses.
- The live egui path stores witness keys / lookup state in `InspectorSections`
  and resolves `SelectionMetricWitnessRef<'_>` from `Graph` while rendering.
- Snapshot/export output now includes selection procedure, traversal,
  candidate payload, candidate set, metric candidates, `imp@k`, compared runs,
  operational inputs, protocol counters, and missing-arm diagnostics.

## Verification

- `cargo test -p ploke-tree selection_metric_witness 2>&1 | tail -n 120`
- `cargo test -p ploke-egui selection_inspector 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- CLI fixture check against
  `/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1`
  with `--inspect-node git-commit:fded9bb00d2aa67ad57311393151fa402e158c3c`.
- Native benchmark attempted against the same target fixture:
  `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1 --benchmark-suite standard --benchmark-scenario select_artifact_inspector_300`.
  The harness rejected this because `standard` requires
  `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`.
- Native benchmark completed on the required standard root:
  `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-scenario select_artifact_inspector_300`.
  Report:
  `crates/ploke-egui/docs/profiling/benchmarks/20260518-c4a6acb3ab66-standard/report.json`.

## Baseline And Allocation Notes

- Nearest prior same-scenario allocation baseline:
  `crates/ploke-egui/docs/profiling/benchmarks/20260517-c7a9bab87e76-standard/report.json`.
- `select_artifact_inspector_300` current vs baseline:
  - median frame time: `2060035 ns` vs `1870260 ns`
  - p95 frame time: `2611098 ns` vs `1970939 ns`
  - p99 frame time: `5994996 ns` vs `2773193 ns`
  - max frame time: `102111512 ns` vs `32133000 ns`
  - heap slope: `plateau` vs `plateau`
  - median allocations/frame: `1393` vs `1216`
  - median object bytes/frame: `925682` vs `905954`
  - median wrapped bytes/frame: `938976` vs `917608`
  - median live object bytes/frame: `2325162` vs `778326`
  - heap live object bytes at scenario end: `2901404` vs `1352195`
  - heap live wrapped bytes at scenario end: `2952480` vs `1389792`
- Top current allocation groups:
  - by allocated bytes: `root` `264873724`, `selection_inspector`
    `8047488`, `central_graph` `6026043`
  - by allocation count: `root` `204214`, `central_graph` `96166`,
    `selection_inspector` `71899`
  - by retained/live bytes: `root` `2151807`, `selection_inspector`
    `290751`, `frame_update` `159613`
- Callsite attribution was not captured. Standard mode reported zero
  `top_callsites_by_allocated_bytes` entries.

## Risk

- This is a measured regression in the standard native window
  `select_artifact_inspector_300` scenario. The measured path includes
  `selection_inspector` component timing and allocation groups, but it is not
  the concrete target-fixture Selection metric drilldown because the standard
  suite currently hard-codes its accepted run root.
- The current steady-state allocation level is still allocation debt even
  aside from the regression: `1393` allocations/frame and `925682` object
  bytes/frame exceed the benchmark skill tripwires of `100` allocations/frame
  and `64 KiB` object bytes/frame.
- Next action: add or expose a benchmark scenario that forces the `Selection`
  inspector section on the selection-metrics fixture, then reduce the measured
  `selection_inspector` allocation group before treating the drilldown as
  performance-acceptable.
