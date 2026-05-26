# 2026-05-25 Eval Protocol Row Hover Samply Note

Verification surface: native interactive `ploke-egui` window, manually driven on
the Eval & Protocol call-review table.

Profile artifact:

- `target/profiling/eval-protocol/row-hover.samply.json`

The profile was recorded with `samply` against the running native process. It is
not checked in; this note records the extracted findings so the result survives
local artifact cleanup.

## CPU Sample Summary

The saved profile contains 2,075 samples on the main `ploke-egui` thread across
roughly 54 seconds. Inclusive sample categories overlap because a single stack
can pass through multiple UI layers.

| Inclusive samples | Share | Category |
| ---: | ---: | --- |
| 1,283 | 61.8% | egui UI layout |
| 997 | 48.0% | egui collapsing headers |
| 691 | 33.3% | tile/pane dispatch |
| 592 | 28.5% | Eval & Protocol pane |
| 564 | 27.2% | protocol drilldowns |
| 431 | 20.8% | text galley/cache |
| 392 | 18.9% | call review scan |
| 390 | 18.8% | egui grid |
| 373 | 18.0% | Agent Trace LLM trace |
| 359 | 17.3% | Patch Generation trace |
| 357 | 17.2% | tool step list |
| 144 | 6.9% | right inspector |
| 88 | 4.2% | egui text layout |
| 51 | 2.5% | call review hover |

## Interpretation

The bounded-hover change appears to have moved row hover out of the main hot
path. `call_review_scan_reason_hover` and related hover preview rendering showed
only 51 inclusive samples, about 2.5% of the main-thread profile.

The remaining sampled cost is broader structural rendering. The hot path is
dominated by immediate-mode layout across nested collapsible sections,
drilldowns, and run-record tool lists. The call-review table is visible in the
profile, but the heavier theme is that several long evidence lists are still
rendered as deep collapsible trees every frame.

Relevant sampled app frames included:

- `render_eval_protocol_for_graph`
- `render_protocol_artifact_drilldowns`
- `render_call_review_scan`
- `render_run_record_turn_llm_trace`
- `render_patch_generation_record`
- `render_run_record_tool_steps`
- `render_right_inspector`
- `InspectorRenderCache::text_galley`

## Allocation Evidence

This `samply` capture does not contain allocation counters or heap callsites.
It answers "where was CPU time sampled while interacting with the UI"; it does
not answer "where were allocations created per frame."

The allocation question is still open. The next allocation pass should use the
repo-native native benchmark allocator rather than ad hoc heap instrumentation.
The Eval & Protocol render paths now have coarse benchmark-only profile scopes
via `#[ploke_egui_macros::profile_scope(...)]`, which expands to
an entered `tracing::trace_span!(...)` only for native benchmark builds.

The first useful run is a focused benchmark callsite sample using existing
allocator machinery:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard \
  --benchmark-scenario warm_idle_300 \
  --benchmark-callsite-sample-every 32
```

The new coarse scopes added for this surface are:

- `eval_protocol`
- `eval_protocol_visual_summary`
- `eval_protocol_protocol_drilldowns`
- `eval_protocol_call_review_scan`
- `eval_protocol_call_review_spotlight`
- `eval_protocol_call_review_row`
- `inspector_agent_trace_llm_trace`
- `inspector_patch_generation`
- `inspector_patch_generation_record`
- `inspector_run_record_tool_steps`

Heaptrack attach mode exists but its own help warns that runtime attach is
unstable and can crash the target process after detach. For this UI, a launched
heaptrack run or a focused benchmark scenario is the safer next allocation
measurement.

## 2026-05-25 Allocation Handoff

The allocation pass was stopped before producing a useful allocation diagnosis.
Do not treat the generated call-review-scan probe directories as final evidence
about per-row allocation behavior.

What was changed before stopping:

- Added `proc_macros/ploke-egui-macros` with
  `#[ploke_egui_macros::profile_scope("...")]` so benchmark-only tracing spans
  can be attached to render functions without inline profiling branches.
- Added an `eval_protocol_call_review_scan_300` native benchmark scenario.
- Added an `EvalProtocolRenderMode` so benchmark code can request the
  call-review-scan surface through a named shell render mode instead of
  branching inside the pane renderer.
- Added benchmark scope names for Eval & Protocol and related run-record
  drilldowns in the native allocation tracker.

What was verified:

- `cargo check -p ploke-egui --features dev,native-benchmark`
- `cargo test -p ploke-egui-macros`

What was inconclusive:

- `20260525-eval-protocol-call-review-scan-probe-r4` was not a valid allocation
  answer for the Eval & Protocol table. The action changed the render mode, but
  the benchmark still rendered the default graph pane.
- `20260525-eval-protocol-call-review-scan-probe-r5` corrected the action to
  put `Eval & Protocol` in the primary tile, but the heap groups still reported
  only broad egui/layout scopes such as `inspector_collapsing_header_layout`.
  This likely reflects exclusive allocation grouping under nested egui spans,
  not proof that the call-review scan has no allocation cost.
- A launched `heaptrack` run was interrupted by the operator. No heaptrack data
  file was produced; `target/profiling/eval-protocol/heaptrack/` was empty when
  checked afterward.

Recommended next step:

- Stop adding more inline probes. If allocation work resumes, first make the
  native allocation tracker report the active span stack or nearest app-owned
  parent scope so nested egui layout spans do not hide Eval & Protocol render
  functions. After that, rerun a single focused call-review-scan benchmark and
  compare against this note.

## 2026-05-26 Instrumentation Catalog Follow-Up

The first cleanup step was to stop treating allocation scope names as duplicated
string conventions. `proc_macros/ploke-egui-macros` now provides a
`profile_scope_catalog!` macro that generates the `crate::allocation::scope`
catalog, index constants, the ordered `ALL` registry, and name lookup from a
single declaration. The same crate's `#[profile_scope(...)]` attribute now
accepts catalog constant paths instead of raw string literals.

Tracked allocation spans in `ploke-egui` render/runtime code were switched from
raw `tracing::trace_span!("...")` names to `crate::allocation::scope::*`
constants. This does not fix the attribution issue by itself, but it makes the
next tracker change durable: adding scope metadata, parent-scope fallback, or
benchmark inclusion flags can be done at the catalog layer instead of by
sprinkling more hand-maintained strings through the UI code.

## Next Performance Direction

Prioritize structural UI changes before further hover micro-optimization:

- Convert long protocol/run-record lists to table-plus-detail or master-detail
  views.
- Avoid rendering dozens of nested collapsed evidence entries when a summary row
  plus selected detail would answer the operator question.
- Keep selected rows and pinned detail panes borrowed/cached, and preserve the
  existing missing/truncated/not-applicable distinctions.
