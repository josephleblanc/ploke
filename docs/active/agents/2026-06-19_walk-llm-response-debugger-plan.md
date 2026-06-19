# 2026-06-19 Walk-integrated LLM response debugger plan

Status: active plan / implementation design.

Short description: plan for turning the existing Prototype 1 `walk` typestate debugger plus turn-live replay machinery into a gdb-like nested debugger for harness-backed LLM/tool loops. The intended pause boundary is one provider/network response plus its full tool batch, not one individual parallel tool call.

Related planning and source files:

- `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/turn-live-replay.md`
- `docs/active/agents/2026-06-16_walk-server.md`
- `docs/active/agents/2026-06-17_walk-command-guide.md`
- `docs/active/agents/2026-06-17_typestate-loop-driver-plan.md`
- `crates/ploke-eval/src/replay/{probe.rs,turn.rs,llm.rs,inspect.rs,probe_text.rs}`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness/{mod.rs,tui.rs}`
- `crates/ploke-eval/src/cli/prototype1_state/walk/`
- `crates/ploke-tui/src/llm/manager/session.rs`

## Goal

Add a debugger surface that lets an operator start from `ploke-eval loop walk`, reach a typestate edge that invokes the headless TUI/harness LLM tool loop, step into that inner loop, and pause after every provider/network response once that response's tool batch has been executed and settled.

The mental model is gdb for Prototype 1:

```text
outer walk frame: Prototype 1 typestate Rn -> Rn+1
  inner harness frame: one broad/headless TUI attempt or planner call
    llm response step 0: provider response -> all requested tools execute -> settle -> pause
    llm response step 1: provider response -> all requested tools execute -> settle -> pause
    ...
  inner frame returns terminal harness result
outer walk frame resumes at the next typestate
```

The step boundary is deliberately **not** one individual tool call. A single model response may request a parallel batch of tools. The debugger should let that whole batch execute using current `ploke-tui` semantics, then pause before the next provider request.

## Desired pause boundary

One debugger `llm step` should do exactly this:

1. Use the current request/conversation state to reach a provider boundary.
2. Obtain exactly one provider response envelope from either:
   - a recorded tape;
   - a live provider call; or
   - a durable branch/checkpoint continuation.
3. Parse that provider response through the normal chat parser.
4. If the response contains tool calls, execute the entire tool-call batch through the normal `ploke-tui` event-bus path.
5. Wait for current gated edit/tool-loop semantics to settle:
   - pending edit payloads should not be treated as final results;
   - staged proposal admission should use the same harness policy as normal runs;
   - approved edits should pass the current post-apply scan/index/refresh barrier;
   - declared validation should run at the same point as normal harness execution.
6. Persist a durable checkpoint with the provider response, tool results, workspace/proposal effects, and next request/conversation state.
7. Pause before the next provider request.

At the pause, the operator should be able to inspect:

- the provider request that was sent;
- the raw provider response and normalized `ChatStepOutcome`;
- all tool calls requested by that response;
- all completed/failed tool results from the batch;
- proposal/admission/settle effects;
- workspace dirty status and changed paths;
- the next request state that would be sent if stepping continues;
- whether the inner harness frame is terminal and ready to return to outer `walk`.

## What already exists

### Provider-response step semantics

`ploke-tui` already has provider-response step machinery:

- `ChatStepSource::RecordedPrefixThenLiveSteps`
- `install_recorded_response_prefix_then_live_steps(tape, 1)`
- request and response taps
- replay boundary errors that are converted into clean completed reports

Current behavior already matches the desired response-level boundary in the replay probe case:

```text
recorded prefix -> one live provider response -> execute that response's tools -> stop before next provider request
```

Important file:

- `crates/ploke-tui/src/llm/manager/session.rs`

### Replay probe and branch tapes

`ploke-eval run replay turn-live --tail live-step` already exposes one-response stepping for historical turn-live bundles.

Existing pieces:

- `ReplayTail::LiveStep`
- `ProbeRequest` / `ProbeRun`
- `ReplayBranchTape`
- `ProbeRun::live_step_response_count`
- `ProbeRun::live_step_tool_calls`
- `ProbeRun::live_step_tool_events`
- `ProbeRun::live_step_boundary_reached`

Important files:

- `crates/ploke-eval/src/replay/probe.rs`
- `crates/ploke-eval/src/replay/probe_text.rs`
- `crates/ploke-eval/src/replay/turn.rs`
- `crates/ploke-eval/src/replay/llm.rs`

Limit: the replay branch tape stores provider responses only. It is useful for replay probes, but it is not enough for durable live checkpoints because replaying it later re-executes tools.

### Turn-live artifacts

Broad headless TUI attempts write replay/debug artifacts:

- `agent-turn-trace.json`
- `agent-turn-summary.json`
- `llm-full-responses.jsonl`

Important files:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness_io.rs`
- `crates/ploke-records/src/agent_turn.rs`
- `crates/ploke-records/src/llm_response.rs`

Limit: these artifacts are written at terminal attempt completion today. They are not a streamed or checkpointed authority surface.

### Harness step seam

The headless TUI adapter now has a real internal harness stepping boundary:

```rust
trait Harness {
    async fn next(&mut self, deadline: Instant) -> Result<Progress, Error>;
    async fn decide(&mut self, decision: Decision) -> Result<bool, Error>;
    async fn settle(&mut self, deadline: Instant) -> Result<Settled, Error>;
}
```

Useful progress values:

- `Progress::Prompt`
- `Progress::Tool`
- `Progress::PendingEdit`
- `Progress::TurnEnded`
- `Progress::ProviderUnavailable`
- `Progress::ContextUnavailable`

Important files:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness/tui.rs`

Limit: normal production code calls `drive_to_attempt_end`, so this seam is not yet exposed as a durable debugger frame.

### Outer `walk` typestate debugger

`loop walk` can reconstruct and step outer Prototype 1 typestates, including live gated edges. It is the correct operator entrypoint for this work.

Important files:

- `crates/ploke-eval/src/cli/prototype1_state/walk/`
- `crates/ploke-eval/src/cli/prototype1_state/driver/advance.rs`
- `crates/ploke-eval/src/cli/prototype1_state/driver/reconstruct.rs`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`

Limit: `walk` does not yet represent an inner harness/tool-loop frame. A live typestate edge either runs or blocks; it cannot currently suspend inside the harness.

## What must be implemented

### 1. Durable response-step checkpoints

Current `ReplayBranchTape` is provider-response-only. For a durable debugger, the checkpoint must include enough state to resume without re-running prior tools.

Add a new persisted shape, not as a replacement for `ReplayBranchTape`:

```text
.ploke/prototype1/debug/tool-loop/<session-id>/
  session.json
  steps/
    0000.json
    0001.json
    ...
  resume.json
```

Possible names:

- `ToolLoopDebugSession`
- `ToolLoopStepRecord`
- `ToolLoopResumeState`
- `ToolLoopCheckpointStore`

Minimum `session.json` fields:

- schema/version;
- session id;
- campaign id / parent node id / branch id / generation when known;
- outer walk phase and edge that created the frame;
- harness kind, e.g. broad TUI attempt, planner, future harness-backed edge;
- workspace path;
- surface policy/evidence roots or references to their authoritative source;
- model selection / route summary;
- status: `active | paused | terminal | abandoned`.

Minimum `steps/<n>.json` fields:

- monotonically increasing response step index;
- provider request snapshot or typed request-message record list;
- raw provider response record;
- parsed outcome summary;
- requested tool calls from that response;
- completed/failed tool batch results;
- pending/proposal/admission events produced by this response;
- settle/refresh/validation observations;
- workspace git status before/after;
- terminal/progress status after the step;
- hashes/paths for large payload sidecars if needed.

Minimum `resume.json` fields:

- next response index;
- assistant message id / parent id / request id coordinates;
- full current request message vector or a typed, lossless equivalent;
- pending retry/error repair state if present;
- commit phase and attempt counters needed by the chat loop;
- model/config knobs that influence the next request;
- references to already-executed tool results so they are not re-run.

The hard invariant: resuming from `resume.json` must not silently re-execute already completed tools or duplicate workspace mutation.

### 2. Step-capable chat/session API

`run_chat_session` is currently run-to-terminal. We need a lower-level step API that can do one provider response plus its full tool batch and return a pause record.

Conceptual API:

```rust
struct ChatStepFrame { /* current req, ids, policy, commit phase, attempts, etc. */ }

async fn run_one_provider_response(
    frame: ChatStepFrame,
    source: ChatStepSource,
) -> Result<ChatStepPause, ChatStepError>;
```

`ChatStepPause` should include:

- updated frame/resume state;
- provider request;
- provider response;
- parsed `ChatStepOutcome`;
- tool results appended into the request messages;
- whether this response ended the session.

Do not copy `run_chat_session` semantics into `ploke-eval`. The parser, tool execution, finish-reason handling, repair payloads, commit phase, and request-message mutation rules should remain owned by `ploke-tui`/`ploke-llm`.

### 3. Step-capable harness adapter

Expose a harness mode that owns a `TuiHarness` and can pause at response boundaries.

The current `Harness::next` API observes tool events after the session internals have executed them. For response-level stepping, either:

1. teach the underlying chat/session API to stop after one response and let `TuiHarness` observe/drain the resulting events; or
2. add a `TuiHarness` method that combines chat-step pause data with observed event draining.

The second option may be easier for `ploke-eval` presentation, but the actual one-response stop should still be controlled by the chat/session layer.

### 4. Inner frame in `walk`

Extend the walk controller state model to represent nested debugger frames.

Conceptual state shape:

```rust
enum WalkState {
    Outer(/* existing typestate states */),
    ToolLoopPaused {
        outer: OuterWalkAnchor,
        edge: HarnessEdge,
        session_id: ToolLoopSessionId,
        phase: ToolLoopPhase,
    },
}
```

Where `OuterWalkAnchor` captures enough of the current outer typestate to return to it when the inner harness frame finishes.

The `walk` controller must preserve current gates:

- live provider calls require `--watch`;
- checkout mutation/handoff paths require `--allow git-changes` where already required;
- tool-loop stepping that can mutate workspace must be explicit about mutation admission;
- historical replay/back/forward stay read-only and must not execute tools.

### 5. `walk` command surface

Add a nested command group under `loop walk`.

Candidate UX:

```bash
# Reach an outer phase that can start a harness-backed live edge.
ploke-eval loop walk step --until r10

# Step into the harness/tool-loop frame instead of running the edge to completion.
ploke-eval loop walk step --into llm --watch --allow git-changes

# Show the paused inner frame.
ploke-eval loop walk llm show

# Execute one provider response and the full tool batch, then pause.
ploke-eval loop walk llm step --watch --allow git-changes

# Inspect current and previous response steps.
ploke-eval loop walk llm show --step latest
ploke-eval loop walk llm show --step 3 --format json

# Run remaining provider responses/tool batches until the inner harness frame is terminal.
ploke-eval loop walk llm finish --watch --allow git-changes

# Return to outer typestate stepping.
ploke-eval loop walk show
ploke-eval loop walk step
```

Possible `llm show` output:

```text
outer: R10 selection strategy ready
edge: R10 -> R11 child fanout
inner: tool-loop paused
session: tool-loop-node-...-slot-02
step: 3
last_response: tool_calls
model: google/gemini-3.5-flash
request_messages: 14
requested_tools: 4
completed_tools: 4
failed_tools: 0
pending_edit: no
applied: yes
changed_paths: src/lib.rs
next: walk llm step | walk llm finish | walk llm abandon
```

### 6. Typestate edge integration points

This debugger should be available for any live typestate edge that invokes a harness-backed LLM/tool loop. The first target should be the existing broad/headless TUI child-generation path used during child fanout, because it already emits turn-live artifacts and uses `TuiHarness`.

Likely first outer edge:

```text
R10 -> R11* child fanout / broad harness execution
```

Future targets:

- broad child-plan/planner calls if they move onto the same harness abstraction;
- protocol or review loops if they become harness-backed and need response-level stepping;
- any new guided edit-surface typestate edge that uses the headless TUI adapter.

The design should not bake in one phase name. It should model a generic `HarnessEdge` with phase-specific metadata.

## Invariants and guardrails

### Do not weaken authority

The debugger must not make the system more permissive. Missing channel evidence, invalid child terminality, invalid branch evaluations, stale History, or invalid checkout state should remain hard blockers.

### Do not duplicate side effects on resume

Durable checkpoints must distinguish:

- provider responses already consumed;
- tool calls already executed;
- proposals already approved/rejected/applied;
- scan/index refresh already completed;
- validation already run.

Resume must continue from the next provider request state, not replay prior tool effects.

### Preserve parallel tool-batch semantics

If a provider response requests multiple tools, the debugger pauses after the batch completes. It should not split a provider response into individual tool-call stops unless a later design explicitly introduces a manual approval mode.

### Keep replay read-only unless explicitly live

Existing `walk replay/back/forward` remain read-only. The new debugger surface is live/effectful and should require explicit admission flags.

### Separate debugging artifacts from History authority

Tool-loop checkpoints are debugger/session artifacts. They are not Crown, History, child admission, branch evaluation, or successor handoff authority unless a later typed evidence design explicitly promotes a subset.

### Use typed records where possible

Avoid new ad-hoc `serde_json::Value` parsing in `walk`. Prefer the existing typed shapes:

- `RawFullResponseRecord`
- `AgentTurnArtifactRecord`
- `ObservedTurnEventRecord`
- `ToolRequestRecord`
- `ToolCompletedRecord`
- `ToolFailedRecord`
- `RequestMessageRecord`

Add new typed records only when existing shapes cannot represent checkpoint/resume authority.

### Include live-Google verification where applicable

Every implementation slice that touches a live provider boundary, model routing, provider-response stepping, harness execution, or `walk` admission into an LLM/tool-loop frame should include a live API verification path in addition to deterministic taped/unit tests.

Use the existing live-Google endpoint/route rather than OpenRouter for these checks. Live tests must remain gated behind the existing `live_api_tests` feature or ignored-test pattern and should document required environment variables in the test or nearby test helper. Deterministic tests remain the required default CI proof; live-Google tests are an explicit operator/provider smoke for the same slice.

Live-Google tests should be small and bounded:

- one prompt that requests a simple, safe tool call when testing tool-loop stepping;
- one provider response boundary per assertion when testing response stepping;
- tight `max_attempts` and timeout budgets;
- isolated temporary or fixture workspace;
- no History/handoff authority changes unless the tested slice explicitly owns that edge.

## Proposed slices

### Slice 0 — document and test current response-step behavior

Goal: lock down existing semantics before refactor.

Tasks:

- Add or update tests around `RecordedPrefixThenLiveSteps` to assert:
  - one live provider response is allowed;
  - tools from that response execute;
  - stop occurs before the next provider request;
  - request/response taps capture the boundary.
- Add eval-side tests around `ProbeRun::live_step_boundary_reached` and branch response indexing if coverage is thin.

Verify:

```bash
cargo test -p ploke-tui run_chat_session_can_replay_prefix_then_take_one_live_step --features test_harness
cargo test -p ploke-eval replay --all-targets
```

Live-Google verification, because this slice pins the provider-response boundary:

```bash
cargo test -p ploke-tui live_google_chat_session_executes_list_dir_tool_call_success_or_quota --features live_api_tests -- --ignored
```

If the existing live-Google test name changes, use the nearest live-Google session/tool-loop smoke that sends one provider request, observes a tool call or a quota/auth classified outcome, and does not require OpenRouter.

### Slice 1 — define durable checkpoint records

Goal: introduce typed record shapes and local filesystem store without changing live execution.

Tasks:

- Add `ToolLoopDebugSession`, `ToolLoopStepRecord`, `ToolLoopResumeState` records in the owning crate/module chosen for debugger persistence.
- Add `ToolLoopCheckpointStore` / `FsToolLoopCheckpointStore`.
- Add serde round-trip tests and schema-version rejection tests.

Verify:

```bash
cargo test -p ploke-eval tool_loop_checkpoint --all-targets
```

No live-Google test is required for pure checkpoint serde/store work unless the slice also captures a live provider response into the new record shape. If it does, add a tiny ignored `live_api_tests` capture smoke using the live-Google route.

### Slice 2 — factor chat/session one-response step API

Goal: extract reusable one-response stepping from `run_chat_session` while preserving existing run-to-terminal behavior.

Tasks:

- Run GitNexus impact on `run_chat_session`, `ChatStepSource`, and `execute_tools_via_event_bus` before edits.
- Extract a typed frame for mutable chat-loop state.
- Add a one-response step function that returns updated frame plus response/tool-batch evidence.
- Keep `run_chat_session` as a wrapper loop over the same step function.

Risk: high. `run_chat_session` is central and heavily tested.

Verify:

```bash
cargo test -p ploke-tui llm::manager::session --features test_harness
cargo test -p ploke-tui run_chat_session --features test_harness
cargo check -p ploke-tui --all-targets
```

Live-Google verification is required for this slice because it touches the session provider boundary. Add or reuse a gated live-Google test that proves the refactored one-response step API still reaches the direct Google provider, captures exactly the intended response boundary, and either executes the requested simple tool batch or returns a recognized quota/auth/provider-unavailable classification.

### Slice 3 — step-capable harness execution

Goal: use the one-response chat step inside the headless TUI harness and produce durable step checkpoints.

Tasks:

- Add an execution mode beside `drive_to_attempt_end` that runs one provider response and drains harness events until the post-tool-batch boundary.
- Persist one `ToolLoopStepRecord` per response.
- Persist updated `ToolLoopResumeState` after each pause.
- Keep normal broad harness behavior unchanged.

Verify:

- test recorded response with one tool batch;
- test edit proposal settling before pause;
- test no duplicate tool execution on resume within the same process;
- test resume from persisted checkpoint if the slice includes durable resume;
- add a gated live-Google harness smoke that steps one provider response through the headless TUI adapter and pauses after the resulting tool batch or classified provider/quota/auth outcome.

### Slice 4 — add walk inner frame model

Goal: let `walk` represent a suspended harness/tool-loop frame.

Tasks:

- Extend `WalkState`/controller protocol with an inner `ToolLoopPaused` frame.
- Record outer phase/edge anchor.
- Add `walk show` rendering for inner frames.
- Ensure read-only replay/back/forward do not enter or mutate this state.

Verify:

```bash
cargo test -p ploke-eval loop_walk --all-targets
```

No live-Google test is required for a purely structural `WalkState` inner-frame slice. If the slice starts or resumes a live inner frame, add a gated live-Google smoke through `loop walk` using the direct Google route.

### Slice 5 — add CLI commands

Goal: operator can step into and through the LLM response loop from `loop walk`.

Tasks:

- Add `walk step --into llm` or equivalent.
- Add `walk llm show`.
- Add `walk llm step`.
- Add `walk llm finish`.
- Add `walk llm abandon` if needed for cleanup/provenance.
- Help text must clearly state mutation/live-provider gates.

Verify:

```bash
cargo test -p ploke-eval loop_walk_llm_command_parses --all-targets
cargo build -p ploke-eval
./target/debug/ploke-eval loop walk llm --help
```

CLI parsing/help does not require live-Google. Any command execution test that crosses the provider boundary must be gated behind `live_api_tests` and use the live-Google route.

### Slice 6 — wire first harness-backed typestate edge

Goal: first useful end-to-end debugger through a real Prototype 1 edge.

Recommended first edge: broad/headless TUI child execution during the R10/R11 fanout path.

Tasks:

- Identify the precise live edge call path that enters the broad harness.
- Add a debug admission branch that starts a `ToolLoopDebugSession` instead of running to terminal.
- When `walk llm finish` reaches terminal harness result, project/commit the same evidence the normal edge would have produced and return to the outer typestate.

Verify:

- smoke with a tiny fixture/model tape if possible;
- live ignored test behind `live_api_tests` using the live-Google route for the first real harness-backed typestate edge;
- compare normal run-to-terminal artifacts against stepped-to-terminal artifacts.

### Slice 7 — inspection polish

Goal: make the debugger useful in practice.

Tasks:

- Add concise table rendering of each response step.
- Add JSON output for all step records.
- Add request diff or request-message summary between steps.
- Decode known tool result payloads like `request_code_context`, `cargo`, edit proposal results.
- Show workspace dirty/change summary per step.

Verify:

- golden-ish table tests for renderer functions;
- no direct JSON field walking in CLI when typed shapes exist;
- no live-Google test is required for renderer-only work, but any example capture used to update golden fixtures should come from the live-Google route when a live provider is intentionally involved.

## Open questions

1. Should tool-loop checkpoints live under `.ploke/prototype1/debug/` in the worktree, under the campaign `prototype1/` tree, or both with one being a provenance pointer?
2. Should `walk llm step` require `--allow git-changes` for any harness surface whose tools may apply edits, or should it use a narrower `--allow workspace-mutation` gate?
3. Which fields are sufficient to resume without re-running tools: full request messages only, or a richer chat-loop frame including commit phase/error repair state?
4. Should `walk branch-live` interact with tool-loop checkpoints, or remain provenance-only and separate?
5. Should a terminal inner frame automatically project back into the outer typestate on `walk llm finish`, or require a separate `walk step` after finish?
6. How much raw provider/tool content should be inline vs sidecar files to avoid huge checkpoint JSON?

## Recommended next action

Start with Slice 0 and Slice 1. They preserve current behavior while pinning the exact response-step semantics and checkpoint schema. Do not refactor `run_chat_session` until the checkpoint shape and one-response invariants are explicit in tests.
