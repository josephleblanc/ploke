# Analyst Representation

This note describes how `ploke-egui` should shape Prototype 1 run, protocol,
tool-call, and patch data for an analyst-facing UI. It starts from the current
rule used elsewhere in this directory: `ploke-egui` renders typed facts borrowed
from `ploke_tree::Graph`; it does not parse raw run JSON or protocol artifact
files directly.

## Larger Object

The richer object is not a list of records. It is an analyst view of a run:

- coverage: what evidence was expected, observed, reviewed, missing, or
  mismatched;
- effort: how many turns, tool calls, failed calls, and patch proposals the run
  spent;
- outcome mix: how protocol review classified the observed behavior;
- sequence: where the run moved from exploration to patch production or
  recovery;
- provenance: which individual call, segment, LLM request, response, artifact,
  and path supports each aggregate claim.

The current nested inspector lists are useful provenance surfaces. They are not
enough as the first analytical view because they force the operator to inspect
evidence one item at a time before seeing the shape of the run.

## Reduction To Avoid

Do not reduce this to prettier nested dropdowns. A data analyst usually wants a
first-pass answer to:

- How complete is the protocol evidence?
- Did the run mostly make focused progress, useful exploration, detours, or
  redundant loops?
- Is failure concentrated in a small number of tools or spread across the run?
- Did patch production happen, and was it proportional to the effort spent?
- Where should I drill next?

The drilldown list should answer "show me the record." The analytical surface
should answer "what pattern am I looking at?"

## Structural Carrier

The UI should carry this as a typed visual summary derived from the existing
dashboard boundary:

```text
ploke_records
  -> ploke_tree::Graph
  -> EvalProtocolDashboard
  -> EvalProtocolVisualSummary
  -> egui chart/drilldown renderers
```

The summary is a view projection, not new semantic authority. It may group,
count, and label facts, but it should remain reproducible from graph-owned
typed evidence.

Current first slice:

- run effort bars: records, turns, tool calls, failed tool calls;
- protocol coverage bars: reviewed calls, missing call reviews, usable
  segments, mismatched segments, missing segments;
- review outcome mix bars: focused progress, useful exploration, recoverable
  detour, redundant thrash, mixed, unclear;
- patch production bars: edit proposals, create proposals, expected file
  changes, applied patch artifacts.

These bars belong above the detailed protocol, LLM, and patch drilldowns. They
provide orientation before evidence inspection.

## Analyst Views To Add

### Coverage Funnel

Show the run as a funnel:

```text
observed tool calls -> reviewed calls -> reviewed segments -> usable segments
```

This answers whether protocol completeness is limited by missing call reviews,
segmentation gaps, mismatched segment anchors, or absence of reviewed segments.

### Outcome Mix

Show protocol assessments as a distribution, split by call reviews and segment
reviews. Useful variants:

- combined outcome bars for the run-level summary;
- stacked call-vs-segment bars for comparing granular and segment-level
  judgments;
- small multiples when multiple instances or branches are loaded.

### Tool Behavior Pareto

Group tool calls by tool name or kind, then show:

- total calls;
- failed calls;
- repeated calls;
- average or total latency when available;
- share of low-value or redundant reviews.

This is the first view that should tell us whether the model is searching too
much, rereading the same file, or failing on edit/application tools.

### Segment Strip

Render intent segments as a compact horizontal strip ordered by call index. Each
segment should show:

- label and status;
- call range;
- outcome verdict;
- confidence;
- missing or mismatched review marker.

Clicking a segment should drill into the segment review and its calls. This
preserves sequence, which flat counts lose.

### LLM Turn Timeline

Render each turn as a row with compact marks for:

- prompt/request;
- response;
- tool-call burst;
- tool failures;
- patch proposal or application;
- terminal outcome.

This should sit beside the raw LLM trace. The raw trace remains the evidence;
the timeline is the navigation surface.

### Patch Production Summary

Patch generation should be represented as a production object:

```text
proposal calls -> proposal snapshots -> expected file changes -> changed files
```

This answers whether the model produced patch material, whether the patch was
applied, and whether expected files changed.

## Current Implementation Slice

The first visualization should remain intentionally modest:

- add an `Analyst Snapshot` to `Eval & Protocol`;
- render compact bar charts for run effort, protocol coverage, outcome mix, and
  patch production;
- keep the existing nested protocol/LLM/patch drilldowns as provenance;
- keep all data borrowed from `EvalProtocolDashboard` and
  `EvalProtocolVisualSummary`.

This gives the operator a shape-of-run view without moving semantic ownership
out of `ploke-tree` or hiding the raw evidence needed for run review.

## Open Design Questions

- Should the analyst surface become its own tile, or stay as the first section
  inside `Eval & Protocol`?
- Should segment strips use call index, turn number, or elapsed time as the
  primary x-axis?
- Which verdicts should be considered "good", "neutral", or "concerning" for
  color purposes?
- Should patch production be shown per turn, per run record, or only as an
  aggregate until multiple records are loaded?
- What is the right threshold for switching from right-panel bars to a larger
  dashboard tile?
