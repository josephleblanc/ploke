# Runtime Playback Implementation Audit — 2026-05-25

Status: ongoing working notes.

Purpose: track what the observability/runtime-playback plan promises, what is already present in the codebase, and what still needs implementation. This is a scratch/audit document, not yet a final design decision record.

## Scope

Primary plan docs under review:

- `docs/workflow/evalnomicon/drafts/observability/README.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/README.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md`
- `docs/workflow/evalnomicon/drafts/observability/run-tree-browser-design.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/*.md`

Primary implementation areas checked so far:

- `crates/ploke-records/src/playback.rs`
- `crates/ploke-records/src/agent_turn.rs`
- `crates/ploke-records/src/llm_response.rs`
- `crates/ploke-tree/src/playback/runtime.rs`
- `crates/ploke-tree/src/playback/turn.rs`
- `crates/ploke-tree/src/store/{fs,record_set,graph_snapshot}.rs`
- `crates/ploke-tree/src/graph/{build,types}.rs`
- `crates/ploke-eval/src/replay/*`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-egui/src/{import,native,ui}`
- `crates/ploke-tree-browser/*`
- `crates/ploke-tree-egui/*`

## Commands run during this audit

- `cargo test -p ploke-tree runtime_playback -- --nocapture`
  - Result: passed, 4 tests.
  - Confirms current `RuntimePlaybackRef` lineage/artifact-ancestry tests are green.
- `cargo test -p ploke-tree agent_turn -- --nocapture`
  - Result: passed, 3 tests.
  - Confirms current agent-turn loading/evidence tests are green.

## High-level finding

The plan is partially implemented.

The lower record and graph layers exist and are useful:

- passive shared record crate exists (`ploke-records`);
- typed run loader exists (`FsRunStore`);
- `RunRecordSet` carries scheduler/history/journal/agent-turn records;
- `Graph::from_records` imports typed run records;
- `GraphSnapshot` can export/import typed snapshots;
- `RunPlayback` / `RunPlaybackRef` and coarse/fine History playback exist;
- `RuntimePlaybackRef` exists with `Lineage` and `ArtifactAncestry` scopes;
- agent-turn trace/summary records load into `ploke-tree` and have event cursors;
- replay tooling can inspect/run from `TurnCursor` plus response sidecar.

But the larger runtime-playback design is not yet complete:

- no broad `PlaybackScope::GraphUniverse`, `Runtime`, `Operation`, `Attempt`, or `AgentTurn` scopes;
- `RuntimePlaybackRef` currently plays History blocks, not the whole runtime/operation/attempt/procedure graph;
- frames/deltas exist only as thin wrappers around current History-block steps;
- agent-turn data is attached as an unscoped drilldown, not joined to a specific runtime playback step;
- no `AgentTurnTimelineRef` / `AgentTurnTimeline` abstraction yet;
- no records-owned full model request snapshot/provider exchange schema yet;
- request tap still captures only `Vec<RequestMessage>`, not full `ChatCompRequest` boundary;
- `llm-full-responses.jsonl` is replay-sidecar evidence, not part of a unified graph-backed timeline;
- no cost-event iterator/projection found;
- egui loads graph/snapshot evidence but does not yet use one runtime playback cursor/frame/delta as the central synchronized UI model.

## Runtime Playback README: first implementation direction

Plan source: `docs/workflow/evalnomicon/drafts/observability/runtime-playback/README.md`, especially the “First Implementation Direction” section.

| Planned slice | Current status | Evidence / notes |
| --- | --- | --- |
| 1. Keep sealed-History `RunPlayback` path working. | Implemented. | `crates/ploke-records/src/playback.rs`; `crates/ploke-tree/src/playback/coarse.rs`; `crates/ploke-tree/src/playback/fine/*`. |
| 2. Add `RuntimePlaybackRef<'g, G>` and `PlaybackScope` vocabulary in `ploke-tree`, without filesystem access. | Implemented, narrow. | `crates/ploke-tree/src/playback/runtime.rs` defines `RuntimePlaybackRef`, `RuntimePlaybackIndex`, `RuntimePlaybackGranularity`, `PlaybackScope`, `PlaybackCursor`. |
| 3. Add graph-backed `PlaybackScope::Lineage`. | Implemented. | `PlaybackScope::Lineage`; test `runtime_playback_lineage_scope_orders_blocks_from_graph_lineage`. |
| 4. Add graph-backed `PlaybackScope::ArtifactAncestry`. | Implemented, History-ref based. | `PlaybackScope::ArtifactAncestry`; test `runtime_playback_artifact_ancestry_uses_history_artifact_refs`. |
| 5. Add one drilldown join from a playback step to current agent-turn evidence. | Partial / weak. | `RuntimePlaybackFrameRef` includes `agent_turn_steps: self.agent_turn_steps()`, and `RuntimePlaybackWarning::AgentTurnDrilldownUnscoped` flags unscoped agent-turn data. This is a drilldown surface, but not a step-specific join. |
| 6. Add `AgentTurns` and `CostEvents` typed iterators. | Missing. | No `AgentTurns`, `CostEvents`, `AgentTurnIter`, or `CostEventIter` implementation found in code search. |
| 7. Teach one CLI debug command to render those projections without adding CLI semantics. | Missing for runtime playback. | Replay CLI renders agent-turn replay inspection/probes, but code search found no CLI use of `RuntimePlaybackRef` / `PlaybackScope`. |
| 8. Add synthetic graph test proving one History-selected Artifact can be played forward into operation, agent-turn, tool-call, and cost projections. | Missing / not at that depth. | Existing tests cover lineage, missing block warning, borrowed block ref, and artifact ancestry. No operation/tool/cost projection test found. |
| 9. Add one bounded real-run smoke check that reports counts and warnings only. | Not found. | No obvious real-run runtime-playback smoke check found during search. |

## Agent-turn plan: first implementation slice

Plan source: `docs/workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md`, especially “First Implementation Slice”.

| Planned slice | Current status | Evidence / notes |
| --- | --- | --- |
| 1. Add minimal event header fields: event index, elapsed time, optional trace scope ids, source component, observation class. | Mostly missing. | `AgentTurnArtifactRecord` stores `events: Vec<ObservedTurnEventRecord>`. Event index is derived from vector position in `ploke-tree`, not persisted in each event. No obvious observation class / source component / trace scope fields in the record. |
| 2. Add read-side `AgentTurnTimelineRef` / granularity contract in `ploke-tree`. | Missing. | Search found no `AgentTurnTimelineRef`, `AgentTurnTimeline`, or `AgentTurnGranularity`. |
| 3. Add records-owned request snapshot and provider exchange records. | Missing. | `ploke-records/src/agent_turn.rs` has `llm_prompt: Vec<RequestMessageRecord>` and `llm_response: Option<String>`. `ploke-records/src/llm_response.rs` has full response sidecar records. No full request snapshot/provider exchange record found. |
| 4. Change TUI request tap from `Vec<RequestMessage>` to full session-local request snapshot preserving model, params, route, tools, tool choice, messages. | Missing. | `crates/ploke-tui/src/llm/manager/session.rs` has `REQUEST_TAP: Sender<Vec<RequestMessage>>`; `capture_request_for_tap` sends `req.core.messages.clone()`. |
| 5. Preserve response sidecar and link it by assistant message id and response index. | Partial. | `RawFullResponseRecord` carries `assistant_message_id` and `RecordedResponse` / `response_index`. Replay can load sidecars. Missing exchange-level join because exchange records do not exist. |
| 6. Add `ploke-tree` borrowed projections for model exchanges and request messages over one agent turn. | Missing / limited. | `TurnEventStepRef` exposes event-level steps and response tape refs, but no model-exchange/request-message projection layer. |
| 7. Move replay-probe `next-provider-request` output to render exchange/request projection. | Missing / current output is smaller. | `crates/ploke-eval/src/replay/probe_text.rs` prints `last_request_messages: <count>`, not a typed request projection. Search did not find `next-provider-request` surface. |
| 8. Add synthetic test proving request snapshot, response envelope, tool request/result, and next-request tool feedback join into one ordered timeline. | Missing. | Existing agent-turn tests verify loading/cursor/tool-event basics, not full model exchange/request feedback joins. |
| 9. Add bounded real-run smoke check printing counts/warnings only. | Not found. | No obvious smoke check found during search. |

## Run-tree browser design implementation sequence

Plan source: `docs/workflow/evalnomicon/drafts/observability/run-tree-browser-design.md`.

| Planned item | Current status | Evidence / notes |
| --- | --- | --- |
| 1. Add `ploke-records` passive structs for scheduler nodes, parent identity, journal entries, channel envelopes, invocation/result records, passive History shapes. | Largely implemented. | `ploke-records` exists and `ploke-tree` imports scheduler, identity, invocation, channel, history, evaluation, protocol, run-record, selection, etc. |
| 2. Refactor `ploke-eval` authority types to contain/convert from `ploke-records` structs without exposing authority constructors. | Partial / ongoing. | Some conversion exists, but active writer shapes still exist in `ploke-eval`; `agent-turn.md` explicitly notes `record.json.gz` write boundary still uses eval-side `RunRecord` mirror. |
| 3. Add `ploke-tree` with `RunForest` DTOs and `FsRunStore` over `ploke-records`. | Implemented. | `crates/ploke-tree/src/store/fs.rs`, `record_set.rs`, graph build/types. |
| 4. Add snapshot export JSON and small golden fixture from synthetic records. | Partial. | `GraphSnapshot` exists and round-trip tests exist. `ploke-egui` can export snapshots. I have not found a checked-in golden fixture yet. |
| 5. Add native egui prototype that loads one exported snapshot. | Implemented. | `crates/ploke-egui/src/native.rs`; `crates/ploke-egui/src/ui/app/mod.rs` snapshot load/export controls. |
| 6. Add wasm build using `eframe` and HTTP snapshot loading. | Partial / demo-level. | `crates/ploke-tree-egui` has wasm startup and paste/drop JSON loading. I have not found HTTP snapshot loading. Current memory says `ploke-tree-egui` is throwaway/demo prior art and `ploke-egui` is the main frontend direction. |
| 7. Add optional live polling/SSE after static snapshot contract is stable. | Missing / not found. | No live polling/SSE surface found in the inspected implementation areas. |

## Current code surfaces worth preserving

- `crates/ploke-tree/src/playback/runtime.rs`
  - Already contains the right vocabulary start: graph-borrowed `RuntimePlaybackRef`, explicit `PlaybackScope`, derived index, warnings, frame/delta placeholders.
  - Good next step is probably extending this rather than making CLI/replay own semantics.

- `crates/ploke-tree/src/playback/turn.rs`
  - Provides `TurnCursor`, `TurnEventStepRef`, `TurnEventPlaybackRefSteps`, and event-level cursor resolution.
  - Useful base for `AgentTurnTimelineRef`, but currently lower-level than the planned timeline model.

- `crates/ploke-tree/src/store/fs.rs`
  - Loads typed agent-turn trace/summary records into `RunRecordSet` and `Graph`.
  - Channel envelopes are counted/parsed into evidence summaries, not carried as ordered playback records.

- `crates/ploke-records/src/agent_turn.rs`
  - Current passive owner for `agent-turn-trace.json` / `agent-turn-summary.json` shape.
  - Needs schema evolution for event headers, model exchange, request snapshot, and observation class if the plan is pursued.

- `crates/ploke-records/src/llm_response.rs`
  - Current full provider-response sidecar owner.
  - Good preserved piece, but should be joined through exchange records rather than remaining replay-only sidecar knowledge.

- `crates/ploke-egui/src/import/mod.rs`
  - Correct boundary: egui imports typed `RunRecordSet`/`GraphSnapshot`, not raw run files directly.

## Second-pass code findings

### Coarse/fine History playback reuse

`crates/ploke-tree/src/playback/coarse.rs` and `crates/ploke-tree/src/playback/fine/*` are useful precedent for the next playback layer, but they are History-only today.

- Coarse playback builds `CoarseHistoryStep` rows from sealed blocks: block height/hash, parent hashes, ruling parent, selected successor, selected node/branch/candidate, occurrence/membership ids, and considered-candidate count.
- Fine playback adds deterministic History-local phase ranks for candidate considered, successor selected, History entry admitted, and History block sealed.
- The useful pattern is: build owned and borrowed projections from already-loaded typed records, keep ids stable, and keep source/evidence strength explicit.
- The missing part is not another History projection. Runtime/channel/agent-turn/provider/cost phases are still outside this step family.

### Runtime and operation graph indexes exist, but playback does not use them yet

`Graph` already has `runtimes` and `operations` indexes, and the graph builder attaches transition journal, invocation, runner request/result, successor, and passive attempt evidence to runtime/operation/branch objects:

- `crates/ploke-tree/src/graph/types.rs` carries `Graph.runtimes`, `Graph.operations`, `Graph.evidence`, and `Graph.agent_turn_records`.
- `crates/ploke-tree/src/graph/build.rs` observes `RuntimeId` values and `Coordinate` operation targets.
- `crates/ploke-tree/src/graph/build/journal.rs` attaches transition journal entries to runtimes and branches by line number.
- `crates/ploke-tree/src/graph/build/run_attempts.rs` attaches invocation and runner request/result evidence to runtimes, operations, artifacts, and branches.

That means `PlaybackScope::Runtime`, `PlaybackScope::Operation`, and `PlaybackScope::Attempt` have plausible graph material already. They are missing as playback indexes/cursors, not as raw records.

### Agent-turn replay consumers are useful but lower-level than the planned timeline

`crates/ploke-tree/src/playback/turn.rs` is close to the planned `TurnFine` granularity:

- it defines `TurnCursor`, `TurnEventKind`, `TurnEventStepRef`, `TurnEventPlaybackRefSteps`, and `turn_event_step_at`;
- it derives event index from vector position inside `AgentTurnArtifactRecord.events`;
- it exposes tool name/call id and a response-tape ref by assistant message id.

Current replay code consumes this directly:

- `crates/ploke-eval/src/replay/inspect.rs` loads agent-turn records with `FsRunStore::load_agent_turn_records`, renders `TurnCursor` rows, and separately inspects `llm-full-responses.jsonl` by assistant message id.
- `crates/ploke-eval/src/replay/probe.rs` resolves a `TurnCursor`, installs a response tape, runs the headless TUI, and captures requests/responses through test-harness taps.
- `crates/ploke-eval/src/replay/probe_text.rs` currently renders only `captured_requests`, `captured_responses`, and `last_request_messages`, not a typed model-exchange/request projection.

So replay is an important early consumer, but it is still working around the missing model-exchange object rather than consuming the planned `AgentTurnTimelineRef` / `ModelExchange` projection.

### Request/response and cost evidence is fragmented

Cost and latency facts do exist, but not as a graph/playback ledger:

- `ploke_records::agent_turn::LlmResponseRecord` carries model, usage, finish reason, and optional metadata.
- `LlmMetadataRecord` carries processing time, cost, tokens-per-second, time-to-first-token, and queue time.
- tool completed/failed records carry `latency_ms`.
- eval run records can aggregate total token usage/cost.
- `RawFullResponseRecord` owns the full response sidecar for replay.

Missing pieces:

- no `CostEvents` or `CostAndLatencyLedger` iterator was found;
- `llm-full-responses.jsonl` is not loaded through `RunRecordSet` / `Graph` as an agent-turn side payload;
- full provider request snapshots are not persisted; the TUI request tap still sends only `Vec<RequestMessage>` from `req.core.messages.clone()`;
- provider response sidecars join to turns by assistant message id/response index in replay code, not in a shared graph-backed model exchange.

### Egui/browser surfaces are graph-aware but not playback-cursor driven

There are two relevant UI lines:

- `crates/ploke-egui` is the current main operator graph UI. It imports through `ploke-tree` (`graph_from_run_root`, `graph_from_snapshot`), can export/read `GraphSnapshot`, has a run picker, graph view, inspectors, and an `EvalProtocolDashboard`.
- `crates/ploke-tree-egui` / `ploke_tree::browser` are older/browser-model surfaces over `PlaybackBrowserModel`. They can load JSON natively or paste/drop JSON in wasm, but I did not find HTTP snapshot loading, polling, or SSE. They are useful prior art, not the frontend direction to extend first.

`ploke-egui` currently has aggregates that overlap future playback panels:

- `EvalProtocolDashboard` exposes evidence availability, run-record counts, patch/projection counts, and protocol aggregate counts.
- dashboard panes expose artifact distribution and stats summaries; `RecentActivity` is still a placeholder.
- inspector sections can render run records, LLM calls, patches, candidate comparison, lineage authority, artifact/source refs, etc.

The important gap remains the one in the design docs: these panes do not share one `RuntimePlaybackRef` cursor/frame/delta. They query graph/evidence directly per pane or per inspector selection.

### Run-tree browser design status update

The browser-design implementation sequence is farther along in native typed records than in web delivery:

- Implemented: shared passive record crate, `FsRunStore`, `RunRecordSet`, `Graph`, snapshots, native egui loading/export, run picker, many inspector surfaces.
- Partial: renderer-neutral `PlaybackBrowserModel` and `ploke-tree-egui` exist, but they model coarse/fine History playback rather than the current graph-wide runtime playback plan.
- Missing/not found: `HttpSnapshotStore`, `/campaigns/<id>/forest` polling, `/events` SSE, and a browser/runtime playback cursor shared across multiple panes.

## Open gaps / likely next implementation slice

A conservative next slice, matching the docs, would be:

1. Add the missing turn-level read-side vocabulary in `ploke-tree`:
   - `AgentTurnTimelineRef<'a, G>`
   - `AgentTurnGranularity`
   - initial `TurnFine` / `ModelExchange` / `ToolLifecycle` markers.

2. Keep it read-only and backed by existing `AgentTurnArtifactRecord` first.
   - Do not change persistence yet.
   - Derive event indices from vector positions and mark observation/evidence limitations explicitly.

3. Add records-owned request/provider exchange structs only after the read-side shape is clear.
   - The current request tap proves the gap: it only has messages.
   - Avoid adding another eval-only DTO that later must be migrated.

4. Upgrade `REQUEST_TAP` after the records-owned shape exists.
   - Preserve model id, provider/route, params, tools, tool choice, and messages.
   - Redact secrets and avoid auth headers/env-derived values.

5. Make `RuntimePlaybackRef` drill into agent-turn timelines by explicit join keys.
   - Replace or refine the current unscoped `AgentTurnDrilldownUnscoped` warning path.

6. Add tests before UI work:
   - synthetic timeline test: request snapshot + response sidecar + tool request/result + next-request tool feedback;
   - synthetic runtime graph test: History-selected Artifact -> operation -> agent turn -> tool-call -> cost/event projection;
   - bounded real-run smoke check that prints counts/warnings only.

## Recommended implementation order

I would keep the next step deliberately read-side and test-first:

1. Introduce `AgentTurnTimelineRef<'a, G = TurnFine>` as a thin wrapper over existing `TurnEventStepRef`/`TurnEventPlaybackRefSteps`.
   - No persistence change.
   - Verify: unit test that `AgentTurnTimelineRef<TurnFine>` preserves artifact path, task id, event index, event kind, call id, and assistant-message response-tape ref.

2. Add derived `ToolLifecycle` projection over one agent-turn timeline.
   - Group requested/completed/failed by `call_id` using existing records.
   - Verify: synthetic record with one completed call and one failed call produces stable grouped rows and warnings for orphan result/request cases.

3. Add an explicit side-payload path for `llm-full-responses.jsonl` in the read layer.
   - Prefer `AgentTurnRecordSet` or a sibling typed payload in `RunRecordSet`; do not make replay load it through ad hoc filesystem logic forever.
   - Verify: synthetic/full-response JSONL loads and joins by `(assistant_message_id, response_index)`.

4. Add a minimal `ModelExchange` borrowed projection that joins existing `llm_prompt` messages, response-tape records, and tool-call ids where available.
   - Mark request completeness as partial until full provider request snapshots exist.
   - Verify: replay inspect/probe can render exchange/request counts from the projection rather than direct tap internals.

5. Only then add records-owned full request snapshot/provider exchange structs and upgrade `REQUEST_TAP`.
   - Preserve model id, provider/route/endpoint, params, tools, tool choice, and messages.
   - Redact secrets and avoid auth headers/env-derived values.

6. Extend `RuntimePlaybackRef` with explicit runtime/operation/attempt scopes and scoped agent-turn drilldowns.
   - Use existing `Graph.runtimes`, `Graph.operations`, transition journal evidence, invocations, runner requests/results, and path-derived node ids.
   - Replace the current unscoped `AgentTurnDrilldownUnscoped` path with warnings that name the missing join key.

7. Put egui behind the shared cursor only after the read-side contract is tested.
   - First UI target: one read-only pane that renders the selected playback cursor, warnings, agent-turn timeline summary, model-exchange count, tool-lifecycle count, and cost/latency totals.
   - Do not move raw run-file parsing into egui.

## Follow-up notes

- Closed in this pass: inspected `coarse`/`fine`, replay `inspect`/`probe`, egui dashboard/diagnostic overlap, and old `ploke-tree-egui` browser-model surfaces.
- Remaining: decide whether to promote this audit into a formal implementation plan under the docs workflow, or keep it as an audit note until the next coding slice is accepted.
