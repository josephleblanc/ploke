# Profiling Setup

`ploke-egui` depends on `profiling = "=1.0.17"` with default features disabled,
matching egui 0.34's profiling dependency, and exposes the feature:

```toml
profile-with-puffin = ["profiling/profile-with-puffin"]
```

Normal builds keep `profiling` as a no-op shim. Profiling builds can enable the
backend feature:

```sh
cargo run -p ploke-egui --features "dev profile-with-puffin" -- --run-root <RUN_ROOT>
```

That only enables Puffin collection while the app is running. To persist a
Puffin capture and a text summary, use:

```sh
cargo run -p ploke-egui --features "dev profile-with-puffin" -- \
  --run-root <RUN_ROOT> \
  --puffin-capture-frames 300 \
  --puffin-capture-close
```

## Instrumented Surfaces

The current first-pass scopes cover:

- native startup graph loading
- contract diagnostics generation
- full egui frame
- top strip, run navigation, right inspector, timeline, and central graph panel
- graph-view show/sync/cache refresh
- artifact-tree projection and widget-graph conversion

The frame path calls `profiling::finish_frame!()` once per egui frame.

## Puffin Captures

`--puffin-capture-frames <N>` turns Puffin scopes on, attaches a
`puffin::GlobalFrameView`, waits until at least `N` frames are collected, then
writes a rolling five-slot capture set:

- `crates/ploke-egui/data/profiling/puffin/latest.puffin`
- `crates/ploke-egui/data/profiling/puffin/latest.txt`
- `crates/ploke-egui/data/profiling/puffin/runs/01.puffin` through `05.puffin`
- `crates/ploke-egui/data/profiling/puffin/runs/01.txt` through `05.txt`

The `.puffin` files are Puffin's own capture format. The `.txt` files are a
small comparison summary with frame count and min/median/p95/max frame times.

Current capture target:

```sh
cargo run -p ploke-egui --features "dev profile-with-puffin" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --puffin-capture-frames 300 \
  --puffin-capture-close
```

## Rolling Logs

The dev CLI can write a typed coarse baseline without opening the native
interactive window. Always pass an explicit run root; `--perf-log` rejects
sample-graph or run-picker fallback baselines.

```sh
cargo run -p ploke-egui --features dev -- --perf-log --run-root <RUN_ROOT>
```

Current baseline target:

```sh
cargo run -p ploke-egui --features dev -- --perf-log --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1
```

The log writer persists:

- `crates/ploke-egui/data/profiling/latest.json`
- `crates/ploke-egui/data/profiling/latest.txt`
- `crates/ploke-egui/data/profiling/runs/01.json` through `05.json`
- `crates/ploke-egui/data/profiling/runs/01.txt` through `05.txt`

The `runs/` files are a five-slot rolling set keyed by sequence number. The
entire `crates/ploke-egui/data/` directory is ignored by git, so these are local
measurement artifacts unless explicitly copied into a report.

The persisted JSON shape is owned by `ploke_egui::perf::PerformanceLog`; do not
read or write it through anonymous `serde_json::Value` field walking.
