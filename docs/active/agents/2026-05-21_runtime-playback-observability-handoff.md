# 2026-05-21 Runtime Playback Observability Handoff

Status: current restart handoff for the runtime-playback and agent-turn
observability design lane.

Use this file as the restart packet before implementing `RuntimePlayback`,
`RunPlayback`, playback iterators, replay CLI changes, graph-backed projections,
or agent-turn trace cleanup.

For the code-facing route, read
[`2026-05-21_runtime-playback-implementation-handoff.md`](2026-05-21_runtime-playback-implementation-handoff.md)
after this file.

## Current Direction

The current design center is `RuntimePlayback`: a read-only operator over a
loaded `ploke_tree::Graph`.

`RuntimePlayback` should turn graph facts into an ordered sequence of graph
states. A single playback cursor should drive the graph canvas, selected-step
details, model request/response panes, tool-call tables, edit-lifecycle views,
protocol tables, replay breakpoints, and cost/latency charts.

The design is not "read a run directory and list events." It is a graph-backed
playback model over History, Artifacts, Runtimes, operations, attempts,
procedures, selection, evaluations, agent turns, model exchanges, tool calls,
edit lifecycles, and cost events.

The agent-turn timeline is a nested drilldown under runtime playback. The
runtime cursor selects the turn; the turn timeline exposes the sequence of model
exchanges, request snapshots, tool executions, edit/proposal events, adapter
observations, and terminal events for that selected runtime step.

## Active Design Sources

- [`runtime-playback/README.md`](../../workflow/evalnomicon/drafts/observability/runtime-playback/README.md)
  Current durable design plan for graph-backed playback, shared cursor state,
  typed scopes, graph coverage, frames, deltas, cost projections, egui
  consumers, and first implementation direction.
- [`runtime-playback/agent-turn.md`](../../workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md)
  Current turn-level design for model exchanges, request snapshots, tool
  execution, edit lifecycles, adapter observations, replay, and model-facing
  trace queries.
- [`historical-replay-probe-workflow.md`](../plans/self-improvement-loop/historical-replay-probe-workflow.md)
  Current operator workflow for stepping historical provider output through
  current tools and branching live from a suspicious state. Verify command
  details against current code before use.

## Current Boundary Claims

- `ploke-records` owns passive persisted schemas and record metadata.
- `ploke-eval` owns the current live writer boundary for agent-turn artifacts
  and other eval/runtime records.
- `ploke-tui` owns live session, tool, edit, and proposal behavior. It should
  emit enough structured observations or taps for eval to record, but it should
  not own persisted schema.
- `ploke-tree` owns read-side loading, graph construction, and borrowed/owned
  playback projections over loaded typed records.
- CLI, `ploke-egui`, replay probes, and future model-facing tools render or
  query projections. They do not infer source truth from filenames, raw logs,
  or rendered CLI text.

## Near-Term Implementation Shape

Start by preserving the current replay/debug value while moving the authority
and ordering model into `ploke-tree`.

1. Keep existing sealed-History and replay-probe behavior working.
2. Add `RuntimePlaybackRef<'g, G>` and `PlaybackScope` vocabulary in
   `ploke-tree` without filesystem access.
3. Add graph-backed scopes for `Lineage`, `ArtifactAncestry`, and eventually
   `GraphUniverse`.
4. Add typed iterators for `AgentTurns` and `CostEvents` over the graph-backed
   playback index.
5. Add one drilldown join from a runtime playback step to current agent-turn
   evidence.
6. Add records-owned request snapshot and provider exchange records for the
   agent-turn timeline.
7. Change the TUI request tap from message-only to a session-local request
   snapshot that preserves model, route, params, tools, tool choice, schemas,
   and messages.
8. Project that request snapshot into records-owned schema at the eval writer
   boundary.
9. Move replay-probe `next-provider-request` output to render the
   exchange/request projection rather than a replay-only snapshot.
10. Add a synthetic graph test proving a History-selected Artifact can play
    forward into operation, agent-turn, tool-call, and cost projections.
11. Add one bounded real-run smoke check that reports counts and evidence
    warnings only.

Do not jump straight into egui wiring or model-facing query tools until the
runtime playback and agent-turn timeline carriers are stable.

## Design Guardrails

- Playback is a read/projection layer over typed persisted records. It is not
  active-loop authority.
- Causal order comes from sealed History first, then admitted History entries,
  typed transition/protocol records, runtime-local order, and agent-turn event
  order. Timestamps are for profiling, correlation, and anomaly detection.
- Coarsening, filtering, rendering, or summarizing must preserve the weakest
  relevant evidence class.
- Do not make the CLI own playback semantics. CLI commands choose scope,
  choose granularity, and render `ploke-tree` projections.
- Do not add new one-off subset record types just because a reader needs a
  smaller view. Prefer the owner type, borrowed accessors, or an explicit
  extension of the canonical schema.
- Do not let `ploke-egui` parse raw run files, protocol JSON, or CLI report
  text. It should read graph-backed borrowed facts until the render boundary.
- If a graph fact cannot be associated with a runtime playback step, frame,
  derived projection, or evidence source, call that out instead of inventing a
  renderer-local ordering.

## Historical Inputs

These older docs remain useful, but they are not the current restart route.

- [`2026-05-09_run-playback-typed-observability-plan.md`](2026-05-09_run-playback-typed-observability-plan.md)
  Historical design contract for typed `RunPlayback` / `RunPlaybackRef`
  projections, evidence strength, causal order, and projection authority.
- [`2026-05-09_ploke-records-protocol-handoff.md`](2026-05-09_ploke-records-protocol-handoff.md)
  Historical shared `ploke-records` / `ploke-tree` passive schema and
  record-store handoff.
- [`2026-05-09_records-emission-clean-sweep-handoff.md`](2026-05-09_records-emission-clean-sweep-handoff.md)
  Historical producer-side record emission normalization handoff.
- [`2026-05-12_agent-turn-record-projection-handoff.md`](2026-05-12_agent-turn-record-projection-handoff.md)
  Still useful for the current agent-turn writer boundary:
  `ploke-eval::runner` writes `agent-turn-*`, while `ploke-records` owns the
  passive schema.

Use those docs for constraints and historical implementation facts. Do not copy
their queue status, "current" labels, or next-task claims without rechecking
the active design sources and current code.

## Restart Checklist

Before implementing:

1. Read this handoff.
2. Read the two runtime-playback design docs linked above.
3. Check the relevant current code in `ploke-records`, `ploke-tree`,
   `ploke-eval`, and `ploke-tui`.
4. Name the graph facts, joins, playback step family, and evidence class being
   added or changed.
5. Pick the smallest verification surface that proves the new projection works
   without making CLI output or egui rendering the source of truth.
