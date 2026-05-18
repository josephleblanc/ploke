# Nine-Phase Selection Run Questions

Verification surface: not tested; code inspection only. This note answers the
follow-up questions from the nine-phase selection runs in
`benchmarks/20260518-inspector-section-sequence-allocation.md` using source
inspection only. I did not run allocation tests, inspect heap artifacts, inspect
`report.json`, or read allocation logging output for this pass.

## Source Evidence Read

Line references below use paths relative to `crates/ploke-egui` unless they
name another crate.

- `src/benchmark.rs`: nine-phase scenario sequencing, primary/alternate
  selection rules, and per-phase action timing.
- `src/ui/app/mod.rs`: benchmark action application and right-panel render
  entrypoint.
- `src/ui/app/shell.rs`: inspector render body, section render functions,
  inspector render cache, run-record rendering, edge rendering, patch rendering,
  and tool payload render paths.
- `src/ui/diff/mod.rs`: patch diff cache, diff text construction, syntax
  highlighting, and galley layout.
- `src/ui/inspector.rs`: inspector-section cache, artifact inspection,
  run-record references, and section slot construction.
- `crates/ploke-tree/src/store/evidence.rs` and
  `crates/ploke-tree/src/graph/types.rs`: run-record evidence and branch-ref
  access shape.

## Questions And Best Guesses

### 1. Are the 30-frame phase medians stable across repeated runs?

Best guess: selected-vs-unselected deltas should be directionally stable, but
first-hit section costs will probably move across repeated runs. The inspector
sections are cached by graph revision plus selected reference, so selection
rebuild work should only happen on selection or graph revision changes
(`src/ui/inspector.rs:31-58`). However, render-boundary caches for text/id
galleys, decoded tool arguments/results, and patch diffs live on the app and
grow as new unique rows are encountered (`src/ui/app/shell.rs:31-155`,
`src/ui/diff/mod.rs:12-77`). That means repeated runs inside the same process or
after different scenario order can be warmer than a fresh process.

### 2. Do p95/p99 frame times correlate with allocation spikes?

Best guess: Patch Debug is the most likely to produce visible frame-time spikes
because a cache miss builds a unified diff, syntax-highlights it, and lays out
an egui galley before storing it (`src/ui/diff/mod.rs:52-76`). Run Records is
less obviously a single-frame spike source from code alone; its expanded section
renders many small rows from cached/built text and id galleys
(`src/ui/app/shell.rs:922-999`). It may create steady pressure rather than a
single obvious p99 event unless the first frame creates many new cached galleys.

### 3. Why do Graph edges and Artifact edges show the same primary delta?

Best guess: for artifact selections, they are currently the same data rendered
through the same row renderer. `InspectorSections::from_artifact` builds
`incoming` and `outgoing`, then assigns clones of those same vectors to both the
graph-edge slots and artifact-edge slots (`src/ui/inspector.rs:1555-1658` and
`src/ui/inspector.rs:1682-1689`). Both sections call `render_edges` over those
slots (`src/ui/app/shell.rs:1040-1088`), and `render_edges` creates the same row
shape for every edge (`src/ui/app/shell.rs:2994-3025`).

### 4. What explains the primary-vs-alternate gap for edge sections?

Best guess: the selected artifacts are not normalized by edge count, relation
kind, source count, or patch content. The primary selector picks the first
visible artifact with patches, and the alternate picks another artifact with
patches if available (`src/ui/app/mod.rs:722-759`). Artifact inspection scans
four edge families in both directions and collects per-edge patch ids for some
relations (`src/ui/inspector.rs:1555-1658`). A node with more incoming/outgoing
relations will render proportionally more edge rows, so edge-section deltas are
probably selection-shape differences rather than a distinct primary-only code
path.

### 5. Why is Patch Debug much larger for primary than alternate with similar counts?

Best guess: the count can stay close while the diff payload size changes. Patch
Debug renders one patch row per patch and always routes through
`render_diff` (`src/ui/app/shell.rs:3096-3133`). The diff cache key is based on
patch id, target path, source hash, proposed hash, and theme, while the cache
miss work is proportional to source/proposed content and generated diff text
(`src/ui/diff/mod.rs:24-76`, `src/ui/diff/mod.rs:91-115`). If primary and
alternate have a similar number of patch widgets but different source/proposed
content size, bytes can diverge while allocation counts remain similar.

### 6. Does collapse return the section to the previous idle baseline?

Best guess: per-frame rendering should return close to selected-collapsed
behavior because the collapse phase forces exclusive inspector mode without an
open section (`src/ui/app/mod.rs:689-701`). It will not return live heap to the
previous state. The text/id galley cache and patch diff cache retain entries
after collapse (`src/ui/app/shell.rs:31-110`, `src/ui/diff/mod.rs:68-76`).
Patch Debug is only explicitly reset at the start of its phase sequence, not on
collapse or unselect (`src/ui/app/mod.rs:672-676`, `src/ui/app/mod.rs:703-707`).

### 7. Does repeated expand/collapse plateau, improve after warmup, or keep growing?

Best guess: repeated expand/collapse of the same selection should improve after
warmup for Patch Debug because repeated patch keys hit the diff cache
(`src/ui/diff/mod.rs:43-50`). The broader render cache should also avoid
rebuilding already-seen text/id galleys, but it has no eviction and uses linear
search through `Vec` caches (`src/ui/app/shell.rs:61-110`). It likely plateaus
for identical selection/theme/input, grows when new records, ids, patches, tool
payloads, or themes are encountered, and may get slower to search as cache
cardinality rises.

### 8. What retained objects are accumulating in growing Run Records and Patch Debug slopes?

Best guess: Patch Debug retention is likely intentional-but-unbounded cached
diff galleys and owned cache keys (`src/ui/diff/mod.rs:12-15`,
`src/ui/diff/mod.rs:68-76`, `src/ui/diff/mod.rs:101-115`). Run Records retention
is more likely accumulated text/id galleys for record paths, instance ids,
manifest ids, repo roots, model/provider names, and status labels
(`src/ui/app/shell.rs:922-999`), plus render cache entries that are retained for
the app lifetime (`src/ui/app/shell.rs:31-110`). That is not necessarily a leak,
but the cache boundary is not currently bounded or keyed by selected artifact.

### 9. How much selected-node idle cost is unavoidable egui/layout churn versus app-owned projection work?

Best guess: it is mixed. Even with all collapsible sections closed, a selected
node renders Summary, Identity, Roles, and Patch Generation on every frame
(`src/ui/app/shell.rs:395-425`). The selected `InspectorSections` carrier is
cached by revision and selection (`src/ui/inspector.rs:31-58`), but some selected
body work still resolves graph-backed data and summarizes run records each
frame. `render_parent_create_attempt` calls `run_record_turn_summary` or
`agent_turn_summary` before rendering the closed LLM/source-status drilldowns
(`src/ui/app/shell.rs:1777-1833`). So the baseline is not just egui overhead.

### 10. Are Run Records allocations caused by projection rebuilding, formatting, nested tool payloads, or widgets?

Best guess: for the top-level Run Records section in the nine-phase run, nested
tool payloads are probably not the main cause. That section calls
`render_run_records`, which only renders record-level summary rows and does not
open the deeper turn/tool tree (`src/ui/app/shell.rs:902-999`). The likely costs
are row/widget construction, cached galley lookup/clones, and first-hit cache
entries for many unique record strings. The deeper turn/tool paths are in the
Patch Generation `LLM calls` drilldown, not the Run Records section, and they
can decode tool arguments/results if opened (`src/ui/app/shell.rs:2025-2298`,
`src/ui/app/shell.rs:113-155`).

Measured answer: native interactive window benchmarking with focused Run Records
spans says this is mostly row rendering and text galley/cache churn in the
top-level Run Records body. It is not nested tool payload decoding, and it is not
material projection rebuilding. The focused report is
`benchmarks/20260518-run-records-focused-spans/report.json`.

| Suspect | Record evidence | Code evidence | Conclusion |
| --- | --- | --- | --- |
| Nested tool payload decoding | No `inspector_tool_*` or `inspector_run_record_tool_step` groups appear in either focused Run Records heap profile. | The top-level `render_run_records` path only renders record summary rows (`src/ui/app/shell.rs:922-999`). Tool argument/result decoding is under the LLM/tool drilldown path (`src/ui/app/shell.rs:2616-2850`) and the render cache decode helpers (`src/ui/app/shell.rs:113-155`). | Ruled out for this measured top-level Run Records scenario. |
| Projection rebuilding | `graph_projection_cache_refresh` is small: primary `181339` object bytes, alternate `91224`. Run Records slot resolution is absent/negligible in top groups. | `InspectorCache` rebuilds sections only when graph revision or selection changes (`src/ui/inspector.rs:31-58`). `RunRecordSlot::resolve` is a borrowed lookup into graph evidence (`src/ui/inspector.rs:473-491`). | Not the measured cost. |
| Text galley/cache work | `inspector_run_records_text_galley` allocated primary `1250086` object bytes / `2154` allocs; alternate `235106` object bytes / `101` allocs. | Run Records labels and non-short ids route through `run_record_label` / `run_record_monospace_label`, which call `render_cache.text_galley` (`src/ui/app/shell.rs:1029-1089`). | Proven contributor inside the Run Records body. |
| Row widget construction | `inspector_run_records_widget_row` allocated `312000` object bytes / `3120` allocs in both primary and alternate. | Every key/value row goes through `run_record_kv_id`, which creates an egui horizontal row and label widgets (`src/ui/app/shell.rs:1006-1017`). | Proven contributor inside the Run Records body. |
| Whole-frame/root/layout overhead | Expanded idle vs selected-collapsed idle adds primary `+230` allocs/frame and `+179187` object bytes/frame; alternate adds `+233` allocs/frame and `+179963` object bytes/frame. Top total heap groups are still `root`, `central_graph`, `selection_inspector`, and navigation groups, with no callsites captured. | The focused spans only cover Run Records body subparts registered in `src/allocation.rs:666-685`; they do not split egui/root/layout internals. | Real remaining attribution gap. Needs phase-scoped root/layout or callsite sampling before assigning it to app code. |

Priority from this pass: cache or reduce Run Records text/row rendering first;
do not spend the next slice on nested tool payloads or projection rebuilding.

### 11. Would callsite attribution change the priority order?

Best guess: it might change the exact fix order but probably not the section
order. Source inspection still points to Patch Debug diff construction/layout
and Run Records row volume as the biggest code-owned suspects. Callsite
attribution could reveal that a large share is actually egui widget/layout
internals, `Arc` churn, persistent-id state, or linear cache search rather than
the obvious app helper. Without callsites, those remain inferred suspects, not
proven allocation sources.

### 12. Is scenario order biasing live-byte conclusions?

Best guess: yes. The app-level `InspectorRenderCache` is passed into every
right-panel render and persists across frames/scenarios
(`src/ui/app/mod.rs:351-359`). The patch diff cache also persists except where a
benchmark action explicitly resets it (`src/ui/app/mod.rs:672-676`). Scenario
order can therefore change retained live bytes and first-hit cache behavior.
Per-phase per-frame deltas should be more trustworthy than absolute live-byte
levels unless every scenario starts from a fresh app/cache state.

### 13. Do these findings hold on a larger real run?

Best guess: the direction should hold, but magnitude should scale with graph and
record shape. Run Records scales with the number of branch refs for the selected
branch and each record's rendered summary rows (`src/ui/inspector.rs:739-796`,
`src/ui/app/shell.rs:922-999`). Edge sections scale with incoming/outgoing
relations (`src/ui/inspector.rs:1555-1658`). Patch Debug scales with patch count
and diff text size (`src/ui/app/shell.rs:3096-3133`, `src/ui/diff/mod.rs:52-76`).
A larger run with more run records, more edge relations, or larger patches
should be worse unless the selection happens to be simpler.

### 14. What target should count as fixed?

Best guess: the useful code-level target is not only "lower than this report."
For a warmed same-selection phase, there should be no repeated app-owned cache
insertion and no repeated diff build/highlight/layout for unchanged patch keys.
The later measured target should keep selected-collapsed and one-section-open
steady windows below the current allocation tripwires, but source-level closure
should require explicit cache/invalidation boundaries: selected-artifact keyed
patch diff cache, bounded or keyed inspector render cache, and section renderers
that avoid rebuilding owned row data outside the existing `InspectorSections`
cache.

### 15. What is the highest-priority follow-up?

Best guess: the next measured pass should use callsite attribution for Patch
Debug, Run Records, and one edge section, with repeated expand/collapse cycles.
The code-only priority is Patch Debug first because it has explicit cold
construction work in the render path. Run Records is second because it is row
volume and cache-boundary debt rather than an obvious one-function cold build.
The edge sections should be inspected mainly to decide whether duplicate Graph
edges vs Artifact edges are semantically useful, because for artifact
selections they currently project the same relations.
