# 2026-05-18 Selection Inspector Allocation Regression

## Trigger

The user reacted to the native benchmark result for the Selection metric witness
drilldown:

> wow, that is one serious fuckup right there. how did this happen?

The follow-up clarified that the real violation was not only late benchmark
execution. The agent had ignored the allocation doctrine in `ploke-egui` docs.

## User-Visible Failure

The agent implemented a large new right-inspector `Selection` drilldown and
initially reported success based on CLI snapshot/export, focused renderer tests,
and cargo checks. Only afterward did a native allocation benchmark show a
regression in `select_artifact_inspector_300`:

- median allocations/frame: `1216 -> 1393`
- median object bytes/frame: `905954 -> 925682`
- `selection_inspector` allocated bytes: `4522076 -> 8047488`
- median live object bytes/frame: `778326 -> 2325162`

The reported implementation therefore made the live right-panel path more
allocation-heavy despite explicit documentation requiring allocation discipline.

The user also reported that the change removed or reformatted existing
`Run Records` work that had been deliberately shaped in the same right-inspector
surface. A follow-up diff check showed the currently visible
`render_run_records_for_inspector` / `render_run_records` functions are
text-identical to `HEAD`, so the exact overwritten surface was not yet
identified from the visible git diff. The incident still includes this as a
preservation failure: broad edits to the right-panel shell and inspector were
made without first protecting the existing Run Records rendering contract and
without proving that the surrounding panel layout still preserved that work.

## Touched Code Surface

- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/inspector.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-tree/src/graph/types/selection.rs`
- `crates/ploke-tree/src/graph/build/selection.rs`

The semantic graph side was mostly aligned with the borrowed-witness direction,
but the egui render path expanded into a large per-frame label/widget/galley
surface.

## What The Agent Did

The agent treated "the data is borrowed from `Graph`" as sufficient allocation
discipline. It added a broad render-time drilldown with many per-frame row
emissions and only ran the native allocation benchmark after reporting the
implementation as complete.

This confused two separate requirements:

- semantic correctness: borrowed typed witnesses from `ploke_tree::Graph`
- render performance: avoid per-frame text/layout/widget allocation churn at the
  egui boundary

The first does not imply the second.

## Skipped Docs / Skills / Instructions

Relevant docs and instructions that should have constrained the implementation:

- `crates/ploke-egui/README.md`: do not clone graph facts; let references and the
  borrow checker keep graph/UI identity honest.
- `crates/ploke-egui/docs/model/source-process-graph.md`: `ploke-egui` should
  borrow from `&Graph` or named borrowed projections, not widget-local strings or
  row-shaped inspector carriers.
- `crates/ploke-egui/docs/model/debugger-claim-workflow.md`: typed witnesses
  must remain typed until the final egui render boundary; rows and labels are not
  source facts.
- `crates/ploke-egui/docs/model/graph-pipeline.md`: renderer-owned text is only
  allowed at the renderer/widget boundary; inspector projections must expose
  typed facts or borrowed values.
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-inspector-right-panel-allocation.md`:
  prior right-panel work had already identified hot `format!`/`to_string` row
  churn and remaining `selection_inspector` allocation debt.
- `.codex/skills/ploke-egui-benchmarking/SKILL.md`: native allocation churn is a
  first-class performance surface and regressions must be reported before
  continuing implementation.

## Why This Was Risky

The change increased a known hot live UI path while making the output harder to
maintain:

- the inspector grew a broad list of render helpers instead of a smaller,
  measured render-boundary projection;
- broad edits landed in the same right-inspector files as the existing
  Run Records implementation, increasing the risk of overwriting or degrading
  a better hand-shaped path;
- the live UI path now does more work per frame even when the semantic data is
  immutable;
- benchmark coverage arrived after acceptance rather than gating the design;
- the target fixture path was not supported by the standard native suite, so the
  exact Selection drilldown remained unmeasured even after the standard benchmark
  found a regression.

## Concrete Prevention Rule

For any `ploke-egui` right-panel, inspector, graph-view, or drilldown change:

1. Identify both carriers before implementation:
   - borrowed semantic carrier from `Graph`;
   - render-boundary carrier/cache that prevents per-frame text/layout churn.
2. Before editing shared right-panel files, snapshot or inspect the existing
   sibling sections in the same file, especially `Run Records`, and state what
   must remain unchanged.
3. Do not accept "borrowed records" as proof of allocation discipline.
4. Run the nearest native allocation scenario before final completion.
5. If the exact scenario cannot be measured, say that before acceptance and do
   not describe the path as performance-acceptable.
6. If any measured `selection_inspector` or touched UI group regresses, stop and
   report the regression before further implementation.

## Memory Hypothesis

Memory and skills contained the relevant warning, but the agent followed the
feature plan mechanically and treated the performance requirement as a final
verification chore instead of a design constraint. Future agents should treat
the right-panel allocation docs as part of the implementation spec, not as
post-hoc reporting guidance.
