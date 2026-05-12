# ploke-egui Task Readability

Coordination area for making the first `ploke-egui` artifact graph readable as
a task surface, while preserving `ploke-tree::graph::Graph` as the semantic
authority.

Start here:

- [`plan.md`](plan.md)
  Current plan of action, semantic boundary, phases, dependencies, and done
  criteria for the task-readability pass.
- [`lanes.md`](lanes.md)
  Six-slot sub-agent posture, lane ownership, allowed edit surfaces, blockers,
  and handoff behavior.
- [`orchestration.md`](orchestration.md)
  `target/debug/xtask orchestrate` workflow for this thread, including board
  setup, packet generation, status checks, and worker refresh policy.
- [`inventory/`](inventory/README.md)
  Inventory and refresh policy for graph/data coverage relevant to the artifact
  view. This folds forward stale ingestion notes instead of treating them as
  current truth.
- [`packets/`](packets/README.md)
  Shared worker brief and packet templates used when dispatching sub-agents.
- [`reviews/`](reviews/README.md)
  Review checklists for graph-boundary discipline and readability diagnostics.
- [`handoffs/`](handoffs/README.md)
  Handoff template and successor-agent context for refreshed workers.

Related context:

- [`../2026-05-12_ploke-egui-artifact-view-handoff.md`](../2026-05-12_ploke-egui-artifact-view-handoff.md)
  Current artifact-first graph handoff.
- [`../2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](../2026-05-11_ploke-egui-graph-import-boundary-handoff.md)
  Prior graph import boundary and `ploke-tree::Graph` consolidation handoff.
- [`../2026-05-11_ploke-tree-graph-ingestion-inventory.md`](../2026-05-11_ploke-tree-graph-ingestion-inventory.md)
  Stale-but-useful source-family tracker to refresh before relying on coverage
  claims.
- [`../../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`](../../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md)
  Typed UI drilldown contract and longer-horizon answer rows.
- [`../../plans/self-improvement-loop/frontend-questions.md`](../../plans/self-improvement-loop/frontend-questions.md)
  Front-facing questions the observability UI should eventually answer.

Core rule:

`ploke-tree::graph::Graph` owns meaning. `ploke-egui` owns presentation,
interaction state, layout geometry, and diagnostics. Semantic access from egui
must be reference-based. Any allocated value cloned out of
`ploke_tree::Graph` for later semantic use is a hard blocker, not a style issue.
