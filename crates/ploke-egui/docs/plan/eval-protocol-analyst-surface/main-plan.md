# Eval & Protocol Analyst Surface Plan

Status: active planning note.

This plan tracks the next direction for the `ploke-egui` Eval & Protocol UI. It
should preserve useful drilldowns while moving the primary experience toward a
data-analyst workflow: overview first, problem slices second, typed evidence
third, raw payloads last.

## Current Read

The current pane is useful, but it still behaves mostly like a nested artifact
browser. It exposes much more evidence than before, but the user has to scroll
through long accordion forests to answer basic questions about run quality,
protocol coverage, and model behavior.

Things that are working:

- `Eval & Protocol` is the right home for run-level protocol and evaluation
  evidence.
- `Analyst Snapshot` is the right direction because it gives shape to a run
  before asking the user to inspect individual records.
- Copyable identifiers, path tails, raw/fields toggles, and explicit unavailable
  states make the evidence inspectable.
- The scroll-tail behavior helps reduce jarring expansion/collapse near the end
  of long sections.

Issues to solve:

- `Reviewed Calls` and `Usable Segment Reviews` are still primarily long
  accordion lists.
- The right inspector wastes space when no graph node is selected, even when a
  run-level protocol item is inspectable.
- Raw previews can still look like clipped one-line payloads.
- Charts show counts, but do not yet guide the next investigation step.
- Repeated verdicts such as `FocusedProgress`, `Distinct`, `High`, and
  `NoRecoveryNeeded` need stronger visual roles than plain repeated text.

## Target Interaction Model

The center `Eval & Protocol` pane should become a run dashboard plus selectable
evidence lists.

- Top: run health, lifecycle, coverage, failure, and patch-production summary.
- Middle: problem slices and sortable/filterable call or segment lists.
- Bottom/detail affordance: raw evidence and deep typed drilldowns.

The right inspector should become the detail pane for the selected evidence
object. When no graph node is selected, it should still show the selected run,
protocol call, protocol segment, artifact, or dashboard slice instead of a set
of `not_applicable` rows.

## Backlog

### 1. Run Dashboard Header

Goal: answer the first-pass analyst questions without opening a drilldown.

- Show lifecycle status: closure, eval, protocol, registry, model, instance
  count, and evidence availability.
- Show coverage status: reviewed calls, reviewed segments, missing reviews,
  truncated previews, failed calls, and patch-production counts.
- Preserve missing-vs-zero states in every summary number.
- Keep raw paths and records copyable but visually secondary.

Acceptance check:

- A protocol-only run can be understood at a glance without expanding
  `Reviewed Calls`.

### 2. Problem Slices

Goal: make the UI point the analyst toward the most interesting evidence.

- Add quick slices for failed tools, mixed outcomes, recoverable detours,
  redundant/thrashing behavior, low-confidence reviews, truncated previews, and
  missing fields.
- Each slice should show a count and select/filter the matching evidence.
- Empty slices should be visible as zero or missing, not silently absent.

Acceptance check:

- If there are mixed or failed calls, the user can jump to them directly from the
  dashboard.

### 3. Reviewed Calls List

Goal: replace the primary long accordion forest with a scannable table/list.

Suggested columns:

- call index
- tool name
- outcome/verdict
- confidence
- failed status
- latency
- segment or neighborhood reference

Behavior:

- Clicking a row selects the call and shows full typed detail in the right
  inspector.
- Expanding inline remains possible, but should not be the primary way to scan
  all calls.
- Copyable raw evidence remains available from the detail view.

Acceptance check:

- The user can scan 60+ calls without scrolling through 60 expanded-capable
  detail blocks.
- Clicking a mixed, failed, or low-confidence row keeps the table position
  stable and moves the corresponding typed detail into the right inspector.

### 4. Segment Review List

Goal: make segment-level interpretation comparable to call-level interpretation.

- Show segment index/range, label, status, confidence, and outcome.
- Allow filtering by label, status, confidence, and review outcome.
- Keep segment rationale and coverage as typed drilldowns.

Acceptance check:

- The user can compare segments without opening each segment review.

### 5. Right Inspector Detail Mode

Goal: stop treating `selection not_applicable` as an empty state when run-level
evidence exists.

- Add a selected Eval & Protocol evidence target for run, call, segment, chart
  slice, or artifact detail.
- Prefer existing graph/run/protocol carriers and cached borrowed projections;
  do not introduce a parallel mirror data model for convenience.
- Keep graph-node selection and protocol-item selection behavior explicit so
  existing inspector sections are not damaged.

Acceptance check:

- With no graph node selected, the right inspector can still show the selected
  run or selected protocol item.

### 6. Drilldown-Aware Charts

Goal: make charts operational rather than decorative.

- Clicking a chart bar should select or filter the corresponding evidence.
- Bars should preserve missing-vs-zero semantics.
- Chart labels should remain compact and not become another wall of text.

Acceptance check:

- Clicking `mixed`, `failed`, or `missing` evidence in a chart leads to the
  underlying calls or segments.

### 7. Raw Evidence Placement

Goal: keep proof available without making raw payloads the main UI.

- Primary view: typed fields.
- Secondary view: rationale/prose.
- Last view: raw persisted payload or raw preview.
- If only a truncated preview exists, label it as a truncated preview and say
  where the full-record fix belongs.

Acceptance check:

- Raw text is always copyable, but the default read path is typed and scannable.

## Constraints

- Use existing typed deserializers and graph-backed witnesses. Do not add generic
  JSON walkers in `ploke-egui`.
- Borrow or cache display projections. Do not parse, format, or rebuild large
  row vectors every frame.
- Preserve existing inspector siblings and popout behavior while adding new
  surfaces.
- Keep missing, unavailable, truncated, parse-failed, not-applicable,
  not-recorded, and zero distinct.
- Validate visible UI changes with focused `ploke-egui` checks, and use native
  allocation/performance checks for large inspector surfaces.

## Implementation Progress

- 2026-05-25: Started the center-pane analyst surface with a `Call Review Scan`
  section ahead of the detailed `Reviewed Calls` drilldowns. The first pass adds
  compact slice filters and a dense call-review index while preserving the
  existing deep evidence accordions.
- 2026-05-25: Cleaned up the call-review scan labels and affordances. The
  latency column is currently the protocol `NeighborhoodCall.latency_ms` value;
  a later pass should enrich latency with local/tool time versus model-wait time
  if that breakdown is available from run records or protocol artifacts.
- 2026-05-25: Next slice is row-selection navigation for `Call Review Scan`.
  Selected rows should remain stable across sorting/filtering when still
  visible, get a subtle highlight, and populate the right inspector with typed
  call-review detail when no graph node is selected.
- 2026-05-25: Polished selected call-review navigation. The selected row now
  has a stronger visual rail, the right-inspector detail starts with a compact
  selected-call header and clear action, provenance/packet metadata is secondary
  behind collapsibles, and scan-cell hovers expose cached LLM rationale or
  field-adjacent reasoning.
- 2026-05-25: Reined in dense-table hover cost after row hover/click stutter.
  `Call Review Scan` cells now show bounded rationale previews on hover and
  rely on row selection for full LLM reasoning in the right inspector and a
  visible `LLM Reasoning Spotlight` above the table.
- 2026-05-25: Fixed the `Call Review Scan` hover affordance so the row
  highlight responds to the row bounds, including whitespace between cells,
  while preserving per-cell hover text. Before another substantial UI pass,
  run native performance/allocation testing against row hover and row
  selection, because this dense table has already exposed hover-path stutter.
- 2026-05-25: Paused the allocation investigation after the benchmark
  attribution became unreliable. The current handoff is in
  `docs/profiling/benchmarks/20260525-eval-protocol-row-hover-samply-note.md`.
  If this resumes, fix the native allocation tracker attribution first so
  nested egui layout spans do not hide app-owned Eval & Protocol render scopes.

## User Notes

The top-level view could also use the description of the top-level llm review of all the calls together. That would be a really nice usability feature, and really differentiating. It gives you a per-protocol description of the overall take-away, which is just kind of amazing, when you think about it. That should be pushed up to the forefront as well as being available in the drilldown details.

A lot of the fields seem fairly opaque. Like it's hard to know exactly what they mean. It would be good to have some descriptions of them when you hover over a field name. Accurate descriptions, so grounded in their emitting source. The emitting source record files shouldn't be in the hover text, but maybe a short description, and then if you right click on it or something, it would pin the hover box and show the original file + function name + line number of the things in our codebase that emit that field. it would be really nice for me and debugging, and since I'm opensourcing this it would be a strong transparency feature.
