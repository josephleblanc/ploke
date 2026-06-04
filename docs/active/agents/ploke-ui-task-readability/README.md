# ploke UI Task Readability

Coordination area for the next UI readability wave across the current
`ploke-egui` surface and the longer-horizon typed drilldown work expected to
feed future browser/gui fronts.

This directory supersedes `ploke-egui-task-readability/` as the live planning
area for new orchestration waves. The older directory remains useful as prior
wave evidence and accepted cleanup history.

Start here:

- [`plan.md`](plan.md)
  Current objective, semantic boundary, dependency graph, and phase plan.
- [`lanes.md`](lanes.md)
  Six-slot posture, lane ownership, blocker rules, and worker refresh policy.
- [`orchestration.md`](orchestration.md)
  `target/debug/xtask orchestrate` workflow, board-reset policy, and packet
  generation steps.
- [`inventory/`](inventory/README.md)
  Refresh scope for the stale graph-ingestion inventory and how to classify UI
  blockers as graph, loader, projection, or drilldown gaps.
- [`packets/`](packets/README.md)
  Shared worker brief and lane prompt templates.
- [`reviews/`](reviews/README.md)
  Readability and boundary review checklists.
- [`handoffs/`](handoffs/README.md)
  Successor-worker handoff template and durable turnover notes.
- [`artifact-tree-default/`](artifact-tree-default/README.md)
  Source-of-truth note for the default `ploke-egui` graph: Artifact nodes,
  patch/derivation edges, History as reveal/highlight state, and debug/drilldown
  detail kept out of the default canvas.
- [`failure-ledger/`](failure-ledger/README.md)
  Concrete failure records from this graph-view task, used to prevent repeat
  mistakes in follow-up patches and sub-agent packets.

Related context:

- [`../../../crates/ploke-egui/docs/style/operator-ui-policy.md`](../../../crates/ploke-egui/docs/style/operator-ui-policy.md)
  Canonical operator UI policy — cite in chat for implementation style.
- [`../ploke-egui-task-readability/README.md`](../ploke-egui-task-readability/README.md)
  Prior artifact-view readability wave and accepted phase-1/2 work.
- [`../2026-05-12_ploke-egui-artifact-view-handoff.md`](../2026-05-12_ploke-egui-artifact-view-handoff.md)
  Current artifact-first egui graph handoff.
- [`../2026-05-11_ploke-tree-graph-ingestion-inventory.md`](../2026-05-11_ploke-tree-graph-ingestion-inventory.md)
  Stale-but-useful source-family tracker that must be refreshed before new
  coverage claims.
- [`../../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`](../../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md)
  Typed answer-contract spine for the eventual front-facing drilldown UI.

Core rule:

`ploke-tree::graph::Graph` owns meaning. UI crates own presentation,
interaction state, layout geometry, inspector state, and diagnostics. Any UI
path that allocates or clones semantic ids, refs, records, partial records, or
wrapper carriers out of `Graph` for later semantic use is a hard blocker. There
are no workaround exceptions for mirror types, report wrappers, or convenience
payloads that merely reconstruct the same meaning under a new name. Snapshot
and app-summary text must stay reference-derived and render-only.
