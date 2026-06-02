# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `661f758fcce2`

dirty_state: `dirty_unrelated`

unrelated dirty paths:
- `ocs/workflow/evalnomicon/drafts/eval/hyperagents-gap-review-2026-05-17.md`
- `docs/workflow/evalnomicon/drafts/eval/ha-review-plan.md`

## Scenarios

- `startup_frames_300`: frames=300, median=1530640 ns, p95=1690852 ns, max=59796286 ns
- `warm_idle_300`: frames=300, median=1541962 ns, p95=1943488 ns, max=2583221 ns
- `select_artifact_inspector_300`: frames=300, median=1684740 ns, p95=1990245 ns, max=32307502 ns
- `patch_debug_cold_300`: frames=300, median=1680723 ns, p95=1956632 ns, max=2542515 ns
- `patch_debug_warm_300`: frames=300, median=1694770 ns, p95=1948367 ns, max=2521235 ns
- `mode_lineage_300`: frames=300, median=867202 ns, p95=1179310 ns, max=2735447 ns
- `mode_artifact_tree_300`: frames=300, median=1690412 ns, p95=2057682 ns, max=5118733 ns
- `toggle_hide_unconsidered_children_300`: frames=300, median=1553503 ns, p95=1841605 ns, max=5553972 ns

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-661f758fcce2-standard/standard.puffin`: 7404868 bytes, sha256 `76d7ca335ccf54043dd94e2d5700c4179c7216e0366c4a9f93d28929d37c0f4f`

See `report.json` for typed timings and allocation deltas.
