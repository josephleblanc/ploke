# 2026-05-21 Runtime Playback Implementation Handoff

Status: current implementation restart packet for the runtime-playback /
agent-turn observability work.

Use this after reading
[`2026-05-21_runtime-playback-observability-handoff.md`](2026-05-21_runtime-playback-observability-handoff.md).
That file is the design contract. This file is the code-facing route.

For Prototype 1 replay terminology, read
[`2026-05-21_prototype1-self-edit-vs-eval-replay-orientation.md`](2026-05-21_prototype1-self-edit-vs-eval-replay-orientation.md)
before interpreting "self-edit patch", "eval patch", or replay
`--workspace` paths. That note records the current ripgrep replay target, the
dirty shared target cache to avoid for clean probes, and the clean disposable
workspace to use for continued CLI grounding.

## Implementation Goal

Build the first graph-backed `RuntimePlayback` implementation without turning
CLI output, egui rendering, replay sidecars, or run-directory layout into source
truth.

The first useful implementation target is:

```text
loaded typed records
  -> ploke_tree::Graph
  -> RuntimePlaybackRef<'g, G>
  -> typed iterators / cursor / step refs
  -> agent-turn drilldown and replay/debug renderers
```

The larger object is a read-only operator over `ploke_tree::Graph`. The tempting
but weaker reduction is another replay-specific iterator over files under
`~/.ploke-eval`, or another CLI report type that quietly owns ordering.

## Reviewed Current Surfaces

Current code already has these useful pieces:

- [`2026-05-21_prototype1-self-edit-vs-eval-replay-orientation.md`](2026-05-21_prototype1-self-edit-vs-eval-replay-orientation.md)
  Operational orientation for the current CLI replay target and the boundary
  between self-edit patches and eval patches.
- `crates/ploke-tree/src/graph/mod.rs`
  Defines `Graph` as the read-side semantic boundary over loaded Prototype 1
  records. It keeps sealed History as primary lineage authority and attaches
  scheduler, branch, turn, provider, protocol, metric, and log records as
  evidence.
- `crates/ploke-tree/src/graph/types.rs`
  `Graph` already carries `history`, `artifacts`, `runtimes`, `operations`,
  `candidates`, `selections`, `metrics`, `child_plans`, `evidence`, and
  `agent_turn_records`.
- `crates/ploke-tree/src/store/record_set.rs`
  `RunRecordSet` is the typed loaded record carrier. It includes
  `history_blocks`, `transition_journal`, `forest_input`, and
  `agent_turn_records`.
- `crates/ploke-tree/src/store/graph_snapshot.rs`
  `GraphSnapshot` already gives a git-portable typed input snapshot:
  `RunRecordSet -> Graph::from_records`.
- `crates/ploke-tree/src/playback/{coarse,fine}`
  Existing sealed-History playback projections. These prove the current
  `RunPlayback` vocabulary and evidence classes, but they are still
  History-spine projections rather than the full graph-backed runtime operator.
- `crates/ploke-tree/src/playback/turn.rs`
  Current borrowed event-level playback over `AgentTurnRecordSet`:
  `TurnCursor`, `TurnEventStepRef`, `TurnEventPlaybackRefSteps`, and
  `ResponseTapeRef`.
- `crates/ploke-eval/src/replay/probe.rs`
  Current executable live replay probe behind `ploke-eval run replay
  turn-live`. It resolves a `TurnCursor`, installs a recorded provider prefix,
  runs the normal headless TUI session against the requested workspace, captures
  provider requests/responses, and can branch live-step tapes.
- `crates/ploke-tui/src/llm/manager/session.rs`
  Current session-level replay source: `RecordedPrefixThenLive` and
  `RecordedPrefixThenLiveSteps`. The request tap still captures only
  `Vec<RequestMessage>`.

Current design docs:

- [`../plans/self-improvement-loop/handoffs.md`](../plans/self-improvement-loop/handoffs.md)
  Active route and stale-reference filter.
- [`../../workflow/evalnomicon/drafts/observability/runtime-playback/README.md`](../../workflow/evalnomicon/drafts/observability/runtime-playback/README.md)
  Runtime playback design contract.
- [`../../workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md`](../../workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md)
  Agent-turn timeline and record map.
- [`2026-05-12_agent-turn-record-projection-handoff.md`](2026-05-12_agent-turn-record-projection-handoff.md)
  Current persisted `agent-turn` ownership boundary.

## Current Gaps

- There is no code-level `RuntimePlaybackRef<'g, G>` yet.
- There is no code-level `PlaybackScope` yet.
- There is no `RuntimePlaybackIndex`, `PlaybackCursor`,
  `RuntimePlaybackStepRef`, `RuntimePlaybackFrameRef`, or
  `RuntimePlaybackDeltaRef`.
- Existing `coarse` and `fine` playback builders are History-spine projections.
  They should remain working, but they are not the full graph-backed operator.
- Existing turn playback is artifact/path ordered and event-local. It is not yet
  joined to a runtime playback step.
- `ploke-tree` does not load `llm-full-responses.jsonl` into a unified
  agent-turn timeline.
- The replay request tap captures request messages only, not the full
  `ChatCompRequest` boundary: model key, provider route, common params, tools,
  tool choice, schemas, and messages.
- `record.json.gz` still has an eval-side writer model in
  `crates/ploke-eval/src/record.rs`; do not grow that compatibility surface for
  new canonical timeline facts.

## First Code Slice

Start in `ploke-tree`. Keep the first implementation read-only and filesystem
free.

1. Add a new playback module for the graph-backed runtime operator, likely
   under `crates/ploke-tree/src/playback/runtime.rs`, and export it from
   `crates/ploke-tree/src/playback/mod.rs`.
2. Define the structural vocabulary:
   - `PlaybackScope`
   - `PlaybackCursor`
   - `RuntimePlaybackIndex`
   - `RuntimePlaybackGranularity`
   - `RuntimePlaybackRef<'g, G>`
   - marker granularities for the first useful step families
3. Implement the borrowed constructor:
   `RuntimePlaybackRef::new(graph: &'g Graph, scope: PlaybackScope)`.
   It may build a small derived index, but it must not read files or parse CLI
   text.
4. Implement `PlaybackScope::Lineage(LineageId)` first, using
   `graph.history.lineages` and `graph.history.blocks`.
5. Add a `HistoryBlocks` or `RuntimeCoarse` iterator that yields borrowed step
   refs with explicit evidence strength.
6. Add `PlaybackScope::ArtifactAncestry(ArtifactId)` after `Lineage` works.
   This should use existing graph artifact/history relations; if a join is
   missing, surface a warning rather than inferring from paths.
7. Add one drilldown join from a runtime playback step to current
   `AgentTurnRecordSet` evidence. It is acceptable for this first join to be
   partial and warning-bearing.
8. Only after the borrowed operator exists, teach one CLI debug/replay surface
   to render it. CLI code should choose scope/granularity and render; it should
   not own playback semantics.

Stop after this slice before egui wiring or model-facing trace-query tools.

## Agent-Turn Follow-On

After the runtime operator exists, continue with the agent-turn timeline slice:

1. Add event header fields in `ploke-records::agent_turn`: event index, elapsed
   time, optional trace scope ids, source component, and observation class.
2. Add records-owned request snapshot and provider exchange records.
3. Change the TUI request tap from `Vec<RequestMessage>` to a session-local
   request snapshot.
4. Project that request snapshot into records-owned schema at the eval writer
   boundary, currently `crates/ploke-eval/src/runner.rs`.
5. Preserve `llm-full-responses.jsonl` for replay, but link it to exchange
   records by assistant message id and response index.
6. Move replay-probe `next-provider-request` output to render the exchange /
   request projection instead of a replay-only snapshot.

Do not solve this by adding smaller mirror DTOs for just the replay command.
Extend the records-owned schema or borrow from the owner types.

## Boundaries

- `ploke-records`
  Owns passive persisted schemas and record metadata.
- `ploke-eval`
  Owns the current live writer boundary for `agent-turn-*` artifacts and the
  replay CLI/library executable probe.
- `ploke-tui`
  Owns live session/tool/edit/proposal behavior and may expose structured taps,
  but should not own persisted schema.
- `ploke-tree`
  Owns read-side loading, graph construction, playback indexes, and borrowed /
  owned playback projections.
- CLI and egui
  Consume playback projections. They do not decide History authority, infer
  joins from filenames, or parse rendered output as source truth.

## Verification Shape

For the first runtime playback slice:

- Add synthetic `ploke-tree` tests that construct a `Graph` or `RunRecordSet`
  in memory and prove:
  - lineage scope orders sealed History blocks by lineage-local order;
  - missing joins become warnings, not fabricated facts;
  - the borrowed step refs point back into the graph;
  - existing coarse/fine sealed-History playback still works.
- Run a bounded check such as:
  `cargo test -p ploke-tree runtime_playback`.
- If the slice touches exported types used by consumers, also run:
  `cargo check -p ploke-tree -p ploke-eval -p ploke-egui`.

For the later replay/agent-turn slice:

- Keep recorded provider replay flowing through the live session/tool loop.
  Do not inject historical tool events directly.
- Add a synthetic test proving request snapshot, response envelope,
  tool request/result, and next-request tool feedback join into one ordered
  timeline.
- Add one bounded real-run smoke check that reports counts and warnings only.
  It should not parse CLI output as authority.

## Restart Checklist

1. Read this file and the observability handoff.
2. Recheck the current `ploke-tree/src/playback` and `ploke-tree/src/graph`
   modules before editing.
3. Name the graph facts, join, step family, scope, and evidence class being
   implemented.
4. Keep the first patch in `ploke-tree` unless the chosen slice explicitly
   reaches the eval/TUI request snapshot boundary.
5. Verify with a focused `ploke-tree` test before touching CLI or egui renderers.
