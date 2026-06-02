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
- [`2026-05-18-ploke-egui-allocation-cache-layout-regression.md`](2026-05-18-ploke-egui-allocation-cache-layout-regression.md)
  While trying to reduce `ploke-egui` allocation churn, the agent cached
  left-panel diagnostic text as no-wrap galleys and shrank the central graph
  viewport, treating allocation measurements as insufficiently coupled to
  visible UI layout acceptance.
- [`2026-05-21-live-google-canary-test-body-skipped.md`](2026-05-21-live-google-canary-test-body-skipped.md)
  While checking the live Google `ploke-tui` canary, the agent summarized the
  passing test before reading the test body and assertion chain.
- [`2026-05-21-live-google-harness-skip-and-timeout.md`](2026-05-21-live-google-harness-skip-and-timeout.md)
  While adding the live Google command-harness test, the agent substituted cheap
  wiring checks and skip behavior before proving the exact live
  `llm_manager -> list_dir -> final assistant response` surface the user named.
- [`2026-05-21-google-router-live-test-substitution.md`](2026-05-21-google-router-live-test-substitution.md)
  While checking Google `Router` usage in `ploke-eval`, the agent let a non-live
  selector test sit too close to the requested live API verification surface.
