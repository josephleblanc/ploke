# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `73e3d0c50680`

dirty_state: `dirty_unrelated`

unrelated dirty paths:
- `docs/workflow/evalnomicon/drafts/eval/hyperagents-gap-review-2026-05-17.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-661f758fcce2-standard/`
- `docs/workflow/evalnomicon/drafts/eval/ha-review-plan.md`

## Scenarios

- `startup_frames_300`: frames=300, median=1535519 ns, p95=1774348 ns, max=60561286 ns
- `warm_idle_300`: frames=300, median=1538274 ns, p95=1781482 ns, max=2235886 ns
- `select_artifact_inspector_300`: frames=300, median=1682505 ns, p95=1929981 ns, max=31804043 ns
- `patch_debug_cold_300`: frames=300, median=1692144 ns, p95=1892650 ns, max=2593098 ns
- `patch_debug_warm_300`: frames=300, median=1687976 ns, p95=1888242 ns, max=2608678 ns
- `mode_lineage_300`: frames=300, median=871860 ns, p95=1028204 ns, max=2701042 ns
- `mode_artifact_tree_300`: frames=300, median=1696662 ns, p95=1923018 ns, max=4826951 ns
- `toggle_hide_unconsidered_children_300`: frames=300, median=1546329 ns, p95=1761764 ns, max=5528821 ns

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-73e3d0c50680-standard/standard.puffin`: 7401085 bytes, sha256 `5dd9ec66c601979e083118c2b70d5fe31625d19e066a24fa49949e02bbc8d353`

See `report.json` for typed timings and allocation deltas.
