# Prototype 1 turn-live replay companion

Status: draft companion after `5a7dbe83` (`Emit turn-live replay records`)
Related walkthrough: [`README.md`](README.md)
Related config guide: [`campaign-configs.md`](campaign-configs.md)
Scope: broad headless-TUI candidate-generation artifacts written during `ploke-eval loop prototype1-state`, plus the `ploke-eval run replay turn-live` consumer path.

## Purpose

This document explains the turn-live replay surface separately from the central loop walkthrough. The central document describes the whole `prototype1-state` parent turn; this companion focuses on one observability/replay subsystem:

1. During broad headless-TUI candidate generation, the loop now writes replayable agent-turn artifacts beside the broad-harness submitted result.
2. Those artifacts let `ploke-eval run replay turn-live` replay a historical provider-response prefix through the current `ploke-tui` tool/session path.
3. The replay command can stop before a live call, continue live after the recorded prefix, or take one live provider step and save a replay branch tape for the next invocation.

The key distinction: turn-live replay replays provider/model responses, not historical tool results. Tool calls are re-executed against the current `--workspace`, so the probe is useful for checking whether current tool-surface fixes, indexing behavior, path policies, and post-apply barriers help a historical problematic turn.

## 1. Where turn-live artifacts are written in `prototype1-state`

The production write boundary is inside the broad headless-TUI path, not inside the generic C1-C4 child runner path.

Source path:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1532-1620`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2055-2088`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:152-173`

Execution sequence:

1. `run_broad_headless_tui_attempt_with_options` prepares the broad-harness workspace and reads the published prompt from `slot.published.prompt_path()`.
2. It computes the effective budget from the harness contract plus `BroadTuiAttemptOptions`.
3. It chooses the workspace used by the headless TUI attempt:
   - with stash-transfer enabled: the source repo root;
   - otherwise: `slot.published.workspace_path()`.
4. It computes a persisted model label:
   - `options.model_label()` when a `ModelSelection` is available;
   - otherwise the fallback string `unknown-headless-model`.
5. It calls `tui_adapter::run_headless_with_model_capture_responses(...)` rather than plain `run_headless_with_model(...)`.
6. After a terminal headless result is available, it writes the compact headless diagnostics JSON.
7. It then calls `write_broad_headless_tui_turn_live_bundle(slot, &run, &prompt, &selected_model)`.
8. Only after diagnostics and turn-live bundle writes does it finish/publish the broad headless-TUI attempt.

The path function is:

```rust
fn broad_headless_tui_turn_live_dir(submitted_result_path: &Path) -> PathBuf {
    submitted_result_path.with_extension("turn-live")
}
```

So if the submitted-result path is:

```text
.../prototype1/messages/edit-harness-result/<node>.json
```

then the turn-live directory is:

```text
.../prototype1/messages/edit-harness-result/<node>.turn-live/
```

The exact filename base depends on the submitted-result path extension, but the important rule is: the replay bundle sits beside the broad-harness submitted result, with extension replaced by `turn-live`.

## 2. Files written by the production bundle

`write_broad_headless_tui_turn_live_bundle` creates the bundle directory and writes three files.

| File | Writer | Originating in-memory data | Persisted type | Meaning |
| --- | --- | --- | --- | --- |
| `agent-turn-trace.json` | `write_json_file_pretty(&dir.join("agent-turn-trace.json"), &AgentTurnTraceRecord(...))` | `HeadlessRun::agent_turn_artifact_record(...)` | `AgentTurnTraceRecord(pub AgentTurnArtifactRecord)` | Replay/debug trace for one headless agent turn. Current code writes it at the end of the attempt; it is not streamed incrementally. |
| `agent-turn-summary.json` | `write_json_file_pretty(&dir.join("agent-turn-summary.json"), &AgentTurnSummaryRecord(...))` | same `AgentTurnArtifactRecord` as trace | `AgentTurnSummaryRecord(pub AgentTurnArtifactRecord)` | Final summary artifact. Current wire shape is the same as trace. |
| `llm-full-responses.jsonl` | loop over `run.full_response_records()` and `fs::write` | captured `RecordedResponse` values tagged with the assistant message id | `RawFullResponseRecord` JSON lines | Provider-response tape used by replay. Each line records one normalized provider response plus response index. |

Important caveats:

- `AgentTurnTraceRecord` and `AgentTurnSummaryRecord` currently share the same transparent wire shape. `crates/ploke-records/src/agent_turn.rs:1-6` explicitly says these records mirror persisted shape only and do not grant tool execution, replay, or runtime authority.
- The bundle is written only after the headless run reaches a terminal outcome. This is not a live-streaming trace file.
- `llm-full-responses.jsonl` can be empty if no provider response was captured, but the file is still written.
- The bundle is a replay/debug artifact. It is not a History block, not a Crown/lineage authority record, and not sufficient to admit a child by itself.

## 3. Originating and persisted data types

### 3.1 `tui_adapter::HeadlessRun`

Source: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:2031-2040`

`HeadlessRun` is the in-memory owner for one broad headless-TUI attempt. It stores:

- attempts and terminal result;
- observed tool/session events;
- cargo validation observations;
- debug relay messages;
- prompt diagnostics;
- captured full provider responses;
- terminal state.

It is the originating type for both the compact `.headless-tui.json` diagnostics and the turn-live replay bundle.

### 3.2 `AgentTurnArtifactRecord`

Source: `crates/ploke-records/src/agent_turn.rs:37-53`
Producer: `HeadlessRun::agent_turn_artifact_record(...)` at `tui_adapter.rs:2094-2189`

Semantic meaning: passive replay/debug evidence for one agent turn. It contains:

- `task_id`: in this path, the broad harness request id from `slot.published.request_id()`;
- `selected_model`: the model label chosen for the headless run;
- `issue_prompt`: the prompt submitted to the headless TUI runtime;
- `user_message_id`: recovered from observed events;
- `events`: typed observed turn events;
- optional terminal/final assistant-message records;
- patch outcome projection.

Current limitations in this projection:

- `patch_artifact.edit_proposals`, `create_proposals`, and expected file changes are currently empty in this broad turn-live conversion.
- `ToolCompletedRecord.ui_payload` and `ToolFailedRecord.ui_payload` are currently written as `None`.
- tool latency fields are written as `0` in the conversion helpers.

This is still useful because the replay path primarily needs event order, tool call ids, assistant message id, selected model label, and the original prompt.

### 3.3 `AgentTurnTraceRecord` and `AgentTurnSummaryRecord`

Source: `crates/ploke-records/src/agent_turn.rs:17-35`

These are transparent wrappers over `AgentTurnArtifactRecord` with different record families/schemas:

- `agent-turn-trace.v1`
- `agent-turn-summary.v1`

Semantic meaning:

- trace: replay/debug trace family;
- summary: final summary family.

Current implementation writes the same `AgentTurnArtifactRecord` into both. The distinction is useful for future streaming/finalization semantics even though the current broad bundle writes both at terminal time.

### 3.4 `ObservedTurnEventRecord`

Source: `crates/ploke-records/src/agent_turn.rs:55-65`
Producer: `HeadlessRun::agent_turn_artifact_record(...)`

Variants:

- `DebugCommand`
- `LlmEvent`
- `LlmResponse`
- `ToolRequested`
- `ToolCompleted`
- `ToolFailed`
- `MessageUpdated`
- `TurnFinished`

The broad conversion currently maps the observed `tui_adapter::Event` stream mainly into tool request/completion/failure events, assistant message snapshots, and turn-finished records. Proposal/internal outcome events are not directly persisted as `ObservedTurnEventRecord` variants in this conversion.

### 3.5 `RawFullResponseRecord`

Source: `crates/ploke-records/src/llm_response.rs:14-53`
Producer: `drain_response_records` at `tui_adapter.rs:869-889`

Semantic meaning: one line in `llm-full-responses.jsonl`. It tags a captured `RecordedResponse` with the assistant message id being updated by the response. Replay uses this as the provider-response tape.

Fields:

- `assistant_message_id`: UUID of the assistant message node;
- flattened `RecordedResponse`: includes `response_index` and normalized provider response payload.

Replay admission requires a contiguous response tape. `LoadedResponseTape::load` rejects missing response indices; inspection mode can still report gaps.

### 3.6 `AgentTurnRecordSet`

Source: `crates/ploke-tree/src/store/evidence.rs:325-341`
Consumer: `FsRunStore::load_agent_turn_records()` via replay/inspection modules.

Semantic meaning: read-side grouping of trace and summary artifacts loaded from a run directory. It is not a new persisted file; it is the in-memory set used by playback/replay consumers.

### 3.7 `TurnCursor` and `TurnEventStepRef`

Source: `crates/ploke-tree/src/playback/turn.rs:41-78`
Consumer: `crates/ploke-eval/src/replay/turn.rs`

Semantic meaning:

- `TurnCursor`: stable location inside an agent-turn artifact: artifact family, artifact path, event index.
- `TurnEventStepRef`: borrowed playback step for one event, including task id, selected model, issue prompt, event kind, and assistant-message/tape reference.

These are read-side projection types. They do not execute anything; they give replay code enough coordinates to choose the response tape and prompt.

### 3.8 `TurnAnchor`

Source: `crates/ploke-eval/src/replay/turn.rs:26-48`
Producer: `resolve_turn_anchor`

Semantic meaning: owned replay anchor derived from `TurnEventStepRef`. It crosses from borrowed playback into async probe execution and carries:

- cursor;
- task id;
- selected model label from the historical artifact;
- original issue prompt;
- event kind/tool/call id;
- assistant message id used to load `llm-full-responses.jsonl`.

### 3.9 `ResolvedReplayPrefix` and `LoadedResponseTape`

Sources:

- `crates/ploke-eval/src/replay/turn.rs:79-88`
- `crates/ploke-eval/src/replay/llm.rs:21-26`

Semantic meaning:

- `LoadedResponseTape` is the validated sidecar loaded from disk for one assistant message.
- `ResolvedReplayPrefix` records how much of that tape was installed and why: full tape, response-index prefix, or event-derived prefix.

### 3.10 `ReplayBranchTape`

Source: `crates/ploke-eval/src/replay/probe.rs:290-360`

Semantic meaning: optional operator artifact for continuing a live-step replay branch. It is pretty JSON with schema `ploke-eval-replay-branch.v1` and a list of `RawFullResponseRecord`s captured from live provider steps. It is a replay input for `--branch-in`, not a History/campaign authority record.

## 4. Live observer emissions

The broad headless-TUI path also has live stderr emissions. These are separate from the persisted replay bundle.

Source:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1716-1806`
- emission sites throughout `run_headless_with_model_inner`, `run_attempt`, proposal application, scan/index barriers, and debug-drain helpers.

Environment controls:

- `PLOKE_EVAL_HEADLESS_TUI_LIVE`: enables stderr live lines unless set to an off value (`0`, `false`, `off`, `no`, or empty).
- `PLOKE_EVAL_HEADLESS_TUI_LIVE_RESOURCES`: adds RSS and workspace/target size emissions.

Prefix format:

```text
[headless-tui elapsed_ms=<ms>] <message>
[headless-tui elapsed_ms=<ms> rss_kb=<kb>] <message>
```

Representative emission groups:

| Phase | Emission examples | Meaning |
| --- | --- | --- |
| attempt start/end | `start workspace=...`, `attempt N start`, `terminal ...`, `done ...` | lifecycle heartbeat for the headless TUI attempt |
| resource diagnostics | `workspace_start`, `workspace_done` with workspace/target byte counts | optional size/RSS monitoring |
| prompt construction | `attempt N prompt parent=... messages=... estimated_tokens=... rag_parts=... bm25=...` | context-building evidence before provider call |
| context failure | `context_unavailable ...` | terminal path before meaningful provider/tool progress |
| tool request | `tool_request call_id=... tool=... args=...` | model asked for a tool call |
| tool result | `tool_completed ...`, `tool_failed ...` | current tool surface result |
| cargo validation | `cargo_validation ... ok=... status=... command=...` | special observation of cargo tool output |
| assistant/message error | `assistant_error ...`, `message_error ...` | provider/session message failure evidence |
| turn completion | `turn_finished outcome=... attempts=... summary=...` | assistant turn closed; this is also where response records are drained |
| policy repair | `policy_repair_prompt ...`, `repair_event ...` | retry/repair path after invalid tool arguments or protected edits |
| proposal staging | `proposal ...`, `creation ...` | proposal/create-file surfaced from a tool UI payload |
| proposal admission | `proposal_rejected`, `proposal_approve`, `proposal_applied`, `proposal_partially_applied`, `proposal_apply_failed`, `proposal_denied`, `proposal_post_approval_indeterminate` | current apply/admission state |
| post-apply refresh | `scan_barrier`, `sparse_refresh ...`, `reindex_scheduled`, `index_barrier ...` | current workspace/index refresh barrier after an applied edit |
| retry/exhaustion | `retry attempt=...`, `terminal exhausted ...`, `terminal completed_without_edit ...` | loop attempt control |

Important distinction:

- Live observer emissions are human/operator stderr output.
- Turn-live bundle files are durable replay/debug evidence.
- If the parent process redirects child stdout/stderr, live lines may appear in process stream files, but the `LiveObserver` itself is not a persisted record family.

## 5. Model selection and live API boundaries

### 5.1 Production broad headless-TUI generation

Model-setting path:

1. `BroadTuiAttemptOptions::from_cli` or `BroadTuiAttemptOptions::for_parent_patcher_defaults` creates an optional `tui_adapter::ModelSelection`.
2. If the CLI supplies a model, `headless_model_selection` parses `ModelId`, validates/normalizes provider routing, and chooses OpenRouter or direct Google.
3. Without an explicit broad-TUI model, the code loads the parent-patcher model selection, falling back to active model selection if the parent-patcher selection is missing.
4. `run_broad_headless_tui_attempt_with_options` passes `options.model().cloned()` into `run_headless_with_model_capture_responses`.
5. `start_attempt_runtime` applies that selection by setting `RuntimeConfig.active_model`, `RuntimeConfig.active_router`, and model-provider selection in the runtime registry.
6. The persisted turn-live artifact stores only the selected model label string in `AgentTurnArtifactRecord.selected_model`; route/provider details are not stored in this record family.

Live API boundary:

- The actual provider call occurs inside the normal `ploke-tui` chat/session path after the headless prompt is submitted.
- `run_headless_with_model_capture_responses` installs a response tap before the run starts, so provider responses are captured as `RawFullResponseRecord`s and later written to `llm-full-responses.jsonl`.

### 5.2 Replay command live tail

Model-setting path:

1. `ReplayTurnLiveCommand::run` computes `ReplayTail` from `--tail`.
2. If `--tail stop` and neither `--model-id` nor `--provider` is passed, it sets `model = None`. In this mode replay should stop before any live provider request.
3. Otherwise, `resolve_replay_probe_model_selection` chooses the live-tail model:
   - with `--model-id`: parse that id and optional provider;
   - with `--provider` alone: error, because a provider pin requires a model id;
   - with neither: load the parent-patcher model selection.
4. `ProbeRequest` passes that model selection into `tui_adapter::run_headless_with_model`.
5. `start_attempt_runtime` sets the runtime config the same way as production broad generation.

Live API boundary:

- The recorded prefix is not a live API call; it is served from installed `RecordedResponseTape`.
- `--tail live` switches to the normal live provider path once the recorded prefix is exhausted.
- `--tail live-step` permits one live provider response, executes its tool calls, then stops at the next provider boundary.

## 6. Replay command execution path

CLI command:

```text
ploke-eval run replay turn-live \
  --run-dir <turn-live-dir> \
  --workspace <workspace> \
  --artifact-kind trace \
  --artifact-path agent-turn-trace.json \
  --event-index <N> \
  [--through-response-index <N> | --through-event] \
  [--tail stop|live|live-step] \
  [--branch-in <file>] [--branch-out <file>] \
  [--model-id <model>] [--provider <provider>] \
  [--max-attempts <N>] [--timeout-secs <secs>] \
  [--format table|json]
```

High-level path:

1. `ReplayTurnLiveCommand::run` parses flags into:
   - `TurnCursor`;
   - `ReplayPrefixSelector`;
   - `ReplayTail`;
   - optional model selection;
   - `ProbeRequest`.
2. `ProbeRequest::run` calls `run_prefix_then_live_probe`.
3. `run_prefix_then_live_probe` validates `run_dir` and `workspace` exist.
4. It resolves the historical prefix with `resolve_replay_prefix_at`.
5. It optionally appends `--branch-in` records to the historical prefix.
6. It installs the tape into `ploke-tui` using one of:
   - `install_recorded_response_tape` for `stop`;
   - `install_recorded_response_prefix_then_live` for `live`;
   - `install_recorded_response_prefix_then_live_steps(..., 1)` for `live-step`.
7. It installs request and response taps.
8. It submits the historical `issue_prompt` to a fresh headless TUI runtime in the requested workspace.
9. The normal TUI tool/session path runs. Tool calls execute against current workspace state.
10. It drains captured provider requests/responses.
11. It records workspace git status before/after as operator signal.
12. If `--branch-out` is present, it writes a `ReplayBranchTape` containing incoming branch records plus newly captured live response records.
13. It returns `ProbeRun`, which the CLI prints as table or JSON.

## 7. Prefix selection semantics

`ReplayPrefixSelector` has three modes:

| Selector | Meaning | Use case |
| --- | --- | --- |
| `FullTape` | install every recorded response for the resolved assistant message | replay all historical provider output, then stop/live-tail depending on `--tail` |
| `ThroughResponseIndex { response_index }` | install records through a chosen provider response index | step by provider-response boundary |
| `ThroughEvent { cursor }` | derive enough provider responses to replay through the selected event | step by agent-turn event such as a tool request or turn finished |

`ThroughEvent` works by mapping the chosen event's call id back to the provider response that emitted that call id. `TurnFinished` maps to the last recorded response. If a tool event references a call id that no response contains, replay admission errors instead of guessing.

## 8. Replay tail semantics

`ReplayTail` has three modes:

| Tail | Provider behavior | Tool behavior | Common use |
| --- | --- | --- | --- |
| `stop` | recorded tape only; stop before live provider call | current tools execute for recorded tool calls until the tape boundary | verify whether current tool path can reproduce a historical prefix without spending live tokens |
| `live` | after recorded prefix, continue normal live provider path | current tools execute throughout | see whether the current loop can recover naturally after historical prefix |
| `live-step` | after recorded prefix, allow one live response, execute its tool calls, then stop before the next provider request | current tools execute; newly live response can be captured into branch tape | step-by-step debugging with `--branch-out` / `--branch-in` |

`ProbeRun::tail_reached` and `ProbeRun::live_step_boundary_reached` are the authority helpers for table rendering. The CLI does not re-derive those semantics in the print function.

## 9. Disk writes during replay

Read-only inputs:

- `agent-turn-trace.json` or `agent-turn-summary.json` from `--run-dir`;
- `llm-full-responses.jsonl` from `--run-dir`;
- optional branch tape from `--branch-in`.

Replay command writes:

| Write | Condition | Writer | Meaning |
| --- | --- | --- | --- |
| target workspace file changes | only if replayed/live tool calls approve/apply edits | normal current `ploke-tui` tool path | this is the point of live replay: current tools operate on current workspace |
| replay branch tape | only when `--branch-out <file>` is passed | `ReplayBranchTape::write` | pretty JSON continuation tape for later `--branch-in` |
| stdout table/json | always | CLI print path | operator output, not a durable repo artifact unless redirected |

Replay command does not rewrite `agent-turn-trace.json`, `agent-turn-summary.json`, or the historical `llm-full-responses.jsonl`.

## 10. Parallelism notes

The replay probe is intentionally serial at the session level:

1. submit prompt;
2. consume one provider response from tape/live path;
3. execute emitted tool calls through current TUI state;
4. settle proposals / scan / refresh index;
5. continue or stop at the next provider boundary.

Parallelism opportunities that are plausible but need care:

| Area | Possible parallelism | Hazard |
| --- | --- | --- |
| replay inspection over many artifacts | load/decode artifacts and response tapes in bounded parallel, then sort deterministically | diagnostic order must remain stable; filesystem reads can still be cheap relative to comprehension |
| multiple independent replay probes | run separate cursors in parallel if each uses an isolated workspace/process | recorded tape/tap installation is process-global inside a running process; shared workspaces would race on proposal/worktree state |
| branch-step sweeps | fan out multiple `live-step` branches from the same historical prefix into separate workspaces | token/provider quota and workspace mutation isolation |
| production bundle writes | write trace/summary/jsonl concurrently | low value; three small files, and sequential writes are simpler for crash reasoning |
| turn event projection | build `AgentTurnArtifactRecord` event vectors in parallel for very large turns | not worth it until turns become huge; event order must be preserved |

Do not parallelize live tool execution within one replay probe unless the TUI/session state model explicitly supports that. Provider responses, tool results, proposal application, and index refresh are causally ordered.

## 11. Practical operator workflow

### Inspect first

```text
ploke-eval run replay inspect \
  --run-dir <turn-live-dir> \
  --workspace <workspace> \
  --format table
```

Use inspection to find replayable event cursors, missing tape indices, and prompt/workspace path mismatch signals.

### Replay to a historical breakpoint without live tokens

```text
ploke-eval run replay turn-live \
  --run-dir <turn-live-dir> \
  --workspace <workspace> \
  --artifact-kind trace \
  --artifact-path agent-turn-trace.json \
  --event-index <N> \
  --through-event \
  --tail stop \
  --format table
```

This checks current tool execution for the recorded prefix and stops before a live provider call.

### Take one live step and save branch tape

```text
ploke-eval run replay turn-live \
  --run-dir <turn-live-dir> \
  --workspace <workspace> \
  --event-index <N> \
  --through-event \
  --tail live-step \
  --branch-out <branch-1.json> \
  --format table
```

Then continue the same branch:

```text
ploke-eval run replay turn-live \
  --run-dir <turn-live-dir> \
  --workspace <workspace> \
  --event-index <N> \
  --through-event \
  --tail live-step \
  --branch-in <branch-1.json> \
  --branch-out <branch-2.json> \
  --format table
```

Use isolated workspaces if you want to compare multiple branches.

## 12. Stabilization checklist

When a turn-live replay looks wrong, check these in order:

1. Does `--run-dir` point at the `.turn-live` bundle directory, not the parent node output directory?
2. Does the selected artifact path exist inside that run dir?
3. Does `llm-full-responses.jsonl` contain contiguous `response_index` values for the relevant assistant message?
4. Does `run replay inspect` show prompt paths pointing at the intended current workspace?
5. Did `--tail stop` stop before the live provider boundary as expected?
6. If using `--tail live` or `live-step`, where did `resolve_replay_probe_model_selection` get the live model: explicit CLI, provider error, or parent-patcher fallback?
7. Did the current workspace become dirty during the probe? Compare `workspace_before` / `workspace_after` in JSON output.
8. Are live observer lines enabled with `PLOKE_EVAL_HEADLESS_TUI_LIVE=1` when you need phase-level progress?
9. If post-apply behavior is suspect, check scan/index/sparse-refresh emissions before blaming model output.

## 13. Source index

Production emission:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:646-715` - `BroadTuiAttemptOptions` and model label/provider helpers.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1532-1620` - broad headless-TUI attempt path and bundle call.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2055-2088` - bundle writer and `.turn-live` path rule.
- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:1639-1685` - test asserting bundle files and decoded trace fields.

Headless TUI observation/capture:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:152-173` - response-capturing wrapper.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:175-304` - main headless run loop and live observer lifecycle emissions.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:470-859` - event loop mapping prompt/tool/message/turn events.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:869-889` - captured response drain into `RawFullResponseRecord`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:906-1578` - proposal admission, post-apply, scan/index/sparse-refresh emissions.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1716-1806` - `LiveObserver` env and stderr formatting.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:2031-2354` - `HeadlessRun` and artifact-record conversion.

Record/playback types:

- `crates/ploke-records/src/agent_turn.rs:1-65` - passive agent-turn record families and event enum.
- `crates/ploke-records/src/agent_turn.rs:201-401` - tool/message/turn/patch record shapes.
- `crates/ploke-records/src/llm_response.rs:1-53` - `llm-full-responses.jsonl` record shape.
- `crates/ploke-tree/src/store/evidence.rs:325-415` - `AgentTurnRecordSet` and compact evidence projection.
- `crates/ploke-tree/src/playback/turn.rs:1-259` - turn cursor/event-step playback projection.

Replay command and probe:

- `crates/ploke-eval/src/cli.rs:3111-3180` - `ReplayTurnLiveCommand` flags.
- `crates/ploke-eval/src/cli.rs:4790-4835` - CLI run method to `ProbeRequest`.
- `crates/ploke-eval/src/cli.rs:4896-4921` - replay probe model selection.
- `crates/ploke-eval/src/replay/turn.rs:1-300` - anchor/prefix resolution and tape installation.
- `crates/ploke-eval/src/replay/llm.rs:1-260` - full response tape loading/slicing/admission.
- `crates/ploke-eval/src/replay/probe.rs:1-620` - executable live replay probe and branch tape.
- `crates/ploke-eval/src/replay/inspect.rs:1-240` - read-only replay inspection.
