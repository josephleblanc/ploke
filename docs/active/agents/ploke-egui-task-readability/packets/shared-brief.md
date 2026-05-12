# Shared Worker Brief

You are working on `ploke-egui` task readability.

## Objective

Make the first artifact-first graph view readable by adding geometry-level
diagnostics and tuning layout from those diagnostics.

The UI should help an operator recover:

- current/promoted artifact lineage,
- local sibling alternatives,
- patch/derivation structure,
- evidence strength and drilldown availability later.

## Hard Rule

`ploke-tree::graph::Graph` owns semantic meaning. `ploke-egui` owns
presentation.

Allowed in `ploke-egui`:

- layout geometry,
- render-only labels that cannot be used as semantic identity,
- colors,
- interaction state,
- diagnostic counters/findings,
- snapshot projections of view diagnostics.

Forbidden in `ploke-egui`:

- duplicate semantic graph,
- egui-owned authoritative artifact/runtime/patch/candidate/history types,
- allocated semantic values cloned out of `ploke_tree::Graph`,
- copied ids, refs, records, or partial records used for later semantic work,
- wrapper report/status/info types that merely repackage graph facts,
- mirror types that reconstruct graph meaning to avoid references,
- direct parsing of run records or JSON,
- screenshot-first tuning as the main source of truth.

If you need a semantic fact that is absent from `ploke_tree::Graph`, stop and
report a graph gap.

Derived values are allowed only when they are genuinely computed view or
analysis facts, such as positions, bounds, collision counts, durations, token
averages, visibility scores, and render-only text. A cloned graph value wrapped
in another type is not derived.

Any violation of this reference-only rule invalidates the phase. Report it as a
hard blocker immediately.

## Reference Docs

Read only the needed sections:

- `docs/active/agents/ploke-egui-task-readability/plan.md`
- `docs/active/agents/ploke-egui-task-readability/lanes.md`
- `docs/active/agents/2026-05-12_ploke-egui-artifact-view-handoff.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`
- `docs/active/plans/self-improvement-loop/frontend-questions.md`

## Report Format

Final report must include:

- files changed,
- line ranges touched,
- tests or checks run,
- new diagnostics or behavior added,
- boundary-risk assessment,
- blockers and exact next action if blocked.

Do not update `.orchestrator` yourself.
