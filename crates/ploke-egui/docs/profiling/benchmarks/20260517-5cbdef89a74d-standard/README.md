# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `5cbdef89a74d`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/shell.rs`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 28769611312 ns
- `FsRunStore::load`: 932984688 ns
- `FsRunStore::load_history_blocks`: 23294222 ns
- `FsRunStore::load_transition_journal`: 4778547 ns
- `FsRunStore::load_record_set`: 961061355 ns
- `compressed_run_record_profile_probe`: 115354286 ns
- `Graph::from_records`: 5056058 ns
- `graph_load_total`: 1081473853 ns
- note: run_picker_discovery=28769611312 ns (kept in startup spans)
- note: warning: run_picker_discovery exceeded 250000000 ns
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=1681335 ns, p95=1881912 ns, p99=2375207 ns, max=61509584 ns, heap_slope=plateau
  - component `capture_overhead`: median=3116 ns, median_frame_share=0.18%
  - component `central_graph`: median=987162 ns, median_frame_share=58.71%
  - component `diagnostics`: median=69080 ns, median_frame_share=4.10%, nested_under=run_navigation
  - component `run_navigation`: median=276319 ns, median_frame_share=16.43%
  - component `selection_inspector`: median=224882 ns, median_frame_share=13.37%
  - component `timeline`: median=48982 ns, median_frame_share=2.91%
  - component `top_strip`: median=74499 ns, median_frame_share=4.43%
- `warm_idle_300`: frames=300, median=1679371 ns, p95=1696343 ns, p99=1852016 ns, max=2263117 ns, heap_slope=plateau
  - component `capture_overhead`: median=3116 ns, median_frame_share=0.18%
  - component `central_graph`: median=985710 ns, median_frame_share=58.69%
  - component `diagnostics`: median=68919 ns, median_frame_share=4.10%, nested_under=run_navigation
  - component `run_navigation`: median=275938 ns, median_frame_share=16.43%
  - component `selection_inspector`: median=224231 ns, median_frame_share=13.35%
  - component `timeline`: median=48942 ns, median_frame_share=2.91%
  - component `top_strip`: median=74430 ns, median_frame_share=4.43%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-5cbdef89a74d-standard/standard.puffin`: 1896934 bytes, sha256 `977aa5e81bef1ebbfcc94ff9e2e4cba0a155668a2f24a245819c27aab363ac2e`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-5cbdef89a74d-standard/startup_frames_300.heap.json`: 4132 bytes, sha256 `891f299e5970c16d73e43dafdcb5a49f820cd66ecdd84ad04ef95c3fba021adb`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-5cbdef89a74d-standard/warm_idle_300.heap.json`: 4084 bytes, sha256 `a75a2caceba81189f8f9da2508f8f2fdfd98068da13460f58d23261ff97563ee`

See `report.json` for typed timings and compact allocation summaries.

## Change Summary

Native benchmark over a tool-step inspector layout change in
`crates/ploke-egui/src/ui/app/shell.rs`: run-record tool steps now render as
individual collapsible sections with a step number, tool name, status badge,
summary, arguments block, and nested tool-call content block.

## Verification Surface

Native interactive window benchmark, plus cargo check and focused benchmark
unit tests.

Commands:

- `cargo check -p ploke-egui --features "dev native-benchmark"`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-scenario startup_frames_300 --benchmark-scenario warm_idle_300`

## Baseline Comparison

Nearest valid same-scenario baseline:
`crates/ploke-egui/docs/profiling/benchmarks/20260517-c7a9bab87e76-standard/report.json`.
The immediately prior standard report covered only
`inspector_run_records_expanded_300`, so it was not a direct comparison for the
two scenarios measured here.

- `startup_frames_300`: median frame improved from 1,720,397 ns to
  1,681,335 ns; p95 improved from 1,955,421 ns to 1,881,912 ns; p99 moved from
  2,362,539 ns to 2,375,207 ns.
- `warm_idle_300`: median frame improved from 1,719,505 ns to 1,679,371 ns;
  p95 improved from 1,825,695 ns to 1,696,343 ns; p99 improved from
  1,972,262 ns to 1,852,016 ns.

## Allocation Summary

- `startup_frames_300`: median 1,180 allocations/frame, 830,899 object
  bytes/frame, 842,272 wrapped bytes/frame, 1,208,883 live object bytes/frame;
  heap slope `plateau`; end live heap 1,770,342 object bytes and 1,820,672
  wrapped bytes.
- `warm_idle_300`: median 1,180 allocations/frame, 830,892 object bytes/frame,
  842,264 wrapped bytes/frame, 578,633 live object bytes/frame; heap slope
  `plateau`; end live heap 1,139,836 object bytes and 1,172,560 wrapped bytes.
- Top allocated-bytes group, allocation-count group, and retained/live group
  attribution all resolved to `root` or `none`; callsite attribution was not
  captured in this standard run.

## Regression Notes

No frame-time regression was observed against the nearest same-scenario
baseline. Median per-frame allocation count increased from 1,143 to 1,180, and
allocated object bytes increased by roughly 5.9 KiB/frame; this is small
relative to the existing root-attributed allocation debt, but it is still a
measured allocation increase.

## Remaining Allocation Debt

Both scenarios remain above the current allocation-debt tripwires: more than
100 allocations/frame and more than 64 KiB object bytes/frame. The standard
report does not include callsites, so the source of the remaining churn is
inconclusive from this run. A focused inspector-expanded scenario with callsite
attribution is the next useful measurement if this surface becomes a hot path.
