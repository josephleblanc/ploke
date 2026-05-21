# Self-Improvement Loop Handoffs

Use this as the shared restart list for the records/playback/frontend track.
Do not treat older dated handoffs as current merely because they are listed
here. This file is the routing table; older links are references only unless
they appear in the active route.

## Active Route

- [`../../agents/2026-05-21_runtime-playback-implementation-handoff.md`](../../agents/2026-05-21_runtime-playback-implementation-handoff.md)
  Current code-facing restart packet for implementing graph-backed runtime
  playback, typed scopes/iterators, and the first agent-turn drilldown join.
- [`../../agents/2026-05-21_runtime-playback-observability-handoff.md`](../../agents/2026-05-21_runtime-playback-observability-handoff.md)
  Current design restart handoff for graph-backed runtime playback, agent-turn
  timelines, shared playback cursor state, and implementation guardrails.

## Active Design Sources

- [`../../../workflow/evalnomicon/drafts/observability/runtime-playback/README.md`](../../../workflow/evalnomicon/drafts/observability/runtime-playback/README.md)
  Current durable design plan for graph-backed runtime playback, shared cursor
  state, typed scopes, typed iterators, graph frames, deltas, charts, egui
  consumers, and implementation direction.
- [`../../../workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md`](../../../workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md)
  Current turn-level drilldown plan for model exchanges, request snapshots,
  tool execution, edit lifecycles, adapter observations, replay, and
  model-facing trace queries.
- [`../../../workflow/evalnomicon/drafts/observability/runtime-playback/inventory/`](../../../workflow/evalnomicon/drafts/observability/runtime-playback/inventory/)
  Current survey lane for record owners, emitted files, graph coverage gaps,
  joins, playback sequencing, egui aggregates, benchmark evidence, and
  operational metrics.
- [`historical-replay-probe-workflow.md`](historical-replay-probe-workflow.md)
  Current operator workflow for replaying historical provider output through
  current tools, stepping to breakpoints, and branching live from a suspicious
  state. Verify command details against current code before use.

## Supporting Contracts

- [`../../agents/2026-05-12_agent-turn-record-projection-handoff.md`](../../agents/2026-05-12_agent-turn-record-projection-handoff.md)
  Current useful background on `agent-turn` persisted record ownership and the
  writer boundary. Check against the runtime-playback docs before implementing.
- [`baseline-evidence-authority-plan.md`](baseline-evidence-authority-plan.md)
  Baseline-evidence design note for parent-local evidence before child fanout.

## Historical Or Superseded References

- [`typed-persistence-spine/operating-console.md`](typed-persistence-spine/operating-console.md)
  Older typed-persistence lane console. Useful as inventory background, not as
  the active implementation route for runtime playback.
- [`typed-persistence-spine/implementation-slices.md`](typed-persistence-spine/implementation-slices.md)
  Older typed-persistence queue. Treat statuses and next slices as stale until
  revalidated against runtime-playback and current code.
- [`typed-persistence-spine/2026-05-10-survey-inventory-handoff.md`](typed-persistence-spine/2026-05-10-survey-inventory-handoff.md)
  Historical first-pass survey handoff for the typed-persistence inventory.
- [`../../agents/2026-05-09_run-playback-typed-observability-plan.md`](../../agents/2026-05-09_run-playback-typed-observability-plan.md)
  Historical typed playback contract for `RunPlayback` / `RunPlaybackRef`,
  evidence strength, causal order, and projection authority.
- [`../../agents/2026-05-09_ploke-records-protocol-handoff.md`](../../agents/2026-05-09_ploke-records-protocol-handoff.md)
  Historical shared `ploke-records` / `ploke-tree` passive schema and
  record-store handoff.
- [`../../agents/2026-05-09_records-emission-clean-sweep-handoff.md`](../../agents/2026-05-09_records-emission-clean-sweep-handoff.md)
  Historical producer-side record emission normalization handoff.
- [`../../agents/2026-05-09_run-playback-coarse-history-handoff.md`](../../agents/2026-05-09_run-playback-coarse-history-handoff.md)
  Older coarse/fine sealed-History playback handoff. Superseded by the
  graph-backed runtime-playback design route above.
- [`../../agents/2026-05-09_egui-wasm-observability-handoff.md`](../../agents/2026-05-09_egui-wasm-observability-handoff.md)
  Older frontend observability handoff. Use only for historical native/WASM
  benchmark context.
- [`../../agents/2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](../../agents/2026-05-11_ploke-egui-graph-import-boundary-handoff.md)
  Older graph-import boundary handoff. Check current `ploke-tree::Graph` and
  runtime-playback docs before reusing claims.
- [`../../agents/2026-05-11_mbe-oracle-calibration-handoff.md`](../../agents/2026-05-11_mbe-oracle-calibration-handoff.md)
  Older MBE/oracle calibration handoff. Use only as historical benchmark
  context unless a current run-review revives it.
- [`../../../workflow/evalnomicon/drafts/history/handoff-2026-04-29.md`](../../../workflow/evalnomicon/drafts/history/handoff-2026-04-29.md)
  Older History/Crown background. Use for conceptual continuity, not current task status.
- [`../../../workflow/evalnomicon/drafts/observability/timing-projection-handoff-2026-05-02.md`](../../../workflow/evalnomicon/drafts/observability/timing-projection-handoff-2026-05-02.md)
  Older timing/projection context.
