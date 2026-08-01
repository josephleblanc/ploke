Native benchmark suite verified the live egui right-panel allocation surface.

## Change Summary

- Added `InspectorRenderCache` galley caches for stable right-panel labels, ids, and numeric text.
- Routed Inspector id rendering through cached `Arc<egui::Galley>` instead of per-frame `String`/`RichText` conversion.
- Changed `PatchDiffCache` to probe cached entries with borrowed patch fields and store final `Arc<egui::Galley>` render artifacts.
- Replaced hot right-panel `format!`/`to_string` rows with cached text or stack `itoa` buffers.

## Verification

- `cargo check -p ploke-egui --features "dev native-benchmark"`
- `cargo test -p ploke-egui inspector -- --nocapture 2>&1 | tail -n 120`
- `cargo test -p ploke-egui patch_diff_cache -- --nocapture 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark -- --nocapture 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard`

Current report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260517-c7a9bab87e76-standard/report.json`

Baseline comparison uses the same generated report path from the immediately
preceding run in this session before rerunning the benchmark with these edits.
The generated benchmark directory name is stable for this dirty commit hash, so
the final run overwrote the prior `report.json`.

## Allocation Results

Top targeted named groups, aggregate allocated object bytes per frame:

- `inspector_patch_debug`: `50,672 -> 6,770 B/frame`
- `inspector_run_records`: `20,916 -> 5,250 B/frame`
- `selection_inspector`: `17,140 -> 12,387 B/frame`
- `inspector_graph_edges`: `14,397 -> 2,835 B/frame`
- `inspector_artifact_edges`: `14,300 -> 2,736 B/frame`

Target scenario median allocated object bytes per frame:

- `inspector_patch_debug_expanded_300`: `1,210,205 -> 1,129,110`
- `inspector_source_refs_expanded_300`: `1,214,309 -> 1,131,662`
- `inspector_artifact_ids_expanded_300`: `1,220,211 -> 1,135,715`
- `inspector_run_records_expanded_300`: `1,106,909 -> 1,085,153`
- `inspector_graph_edges_expanded_300`: `1,132,224 -> 1,101,882`
- `inspector_artifact_edges_expanded_300`: `1,154,613 -> 1,117,698`

Current heap slopes are `plateau` for every standard scenario. Standard mode did
not capture callsite backtraces.

## Remaining Debt

- Overall per-frame allocations remain far above the 64 KiB tripwire because the
  root bucket is still dominant.
- `selection_inspector` remains visible at `12,387 B/frame`; remaining work is
  to continue moving right-panel text/layout construction behind explicit cache
  keys and then revisit any graph-side projection churn with focused attribution.
