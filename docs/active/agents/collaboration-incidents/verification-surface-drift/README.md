# Verification Surface Drift

Incidents where the agent reported or implied verification on one surface while
the real feature lived on another, such as treating CLI snapshot output,
focused renderer tests, and live UI behavior as interchangeable.

## Entries

- [`2026-05-15-artifact-id-ui-verification-drift.md`](2026-05-15-artifact-id-ui-verification-drift.md)
  While validating the new `Artifact Ids` inspector section, the agent talked
  as though CLI inspection and focused renderer tests were equivalent to the
  live egui feature instead of naming the exact verified surface each time.
- [`2026-05-15-graph-edge-visibility-surface-mismatch.md`](2026-05-15-graph-edge-visibility-surface-mismatch.md)
  While validating `P_O` edge rendering, the agent treated projected edge
  counts as though they proved the edges were drawable, and missed that the
  readability/geometry path was still dropping self-loops.
- [`2026-05-18-selection-inspector-allocation-regression.md`](2026-05-18-selection-inspector-allocation-regression.md)
  While implementing Selection metric witness drilldown, the agent treated
  borrowed graph records as enough allocation discipline, accepted the change
  before native allocation gating, and regressed the live right-panel path.
