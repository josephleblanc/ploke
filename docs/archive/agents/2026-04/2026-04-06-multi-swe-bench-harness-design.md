# 2026-04-06

## Task Title
Multi-SWE-Bench Harness Design

## Task Description
Grounded design for running multi-SWE-bench style prompts through the current agentic harness, observing the tool loop, and capturing the final patch/application result using the existing `ploke-tui` runtime.

## Related Planning Files
- [2026-04-06-multi-swe-bench-harness-plan.md](./2026-04-06-multi-swe-bench-harness-plan.md)

## Current Ground Truth
- The user-message entry path already exists and is the correct benchmark entrypoint: `AddUserMessage` -> `ScanForChange` -> `EmbedMessage`, as driven from [`app/mod.rs`](../../../crates/ploke-tui/src/app/mod.rs#L1296) and wrapped in the older headless harness by [`new_test_harness.rs`](../../../crates/ploke-tui/src/test_utils/new_test_harness.rs#L186).
- The prompt/context boundary already exists as `ChatEvt::PromptConstructed`, and `llm_manager` waits for a matching `Request` + `PromptConstructed` pair before starting the LLM turn, in [`llm/manager/events.rs`](../../../crates/ploke-tui/src/llm/manager/events.rs#L122) and [`llm/manager/mod.rs`](../../../crates/ploke-tui/src/llm/manager/mod.rs#L191).
- Tool execution is already event-driven: the chat loop emits `SystemEvent::ToolCallRequested`, the tool layer emits `ToolCallCompleted` or `ToolCallFailed`, and the session waits on those events by `step_request_id`, in [`llm/manager/session.rs`](../../../crates/ploke-tui/src/llm/manager/session.rs#L745), [`llm/manager/session.rs`](../../../crates/ploke-tui/src/llm/manager/session.rs#L1371), and [`tools/mod.rs`](../../../crates/ploke-tui/src/tools/mod.rs#L486).
- Patch-style edits already flow through model-facing tools. `NsPatch` is the closest fit for SWE-bench patch output, but it currently supports one file patch per call, in [`tools/ns_patch.rs`](../../../crates/ploke-tui/src/tools/ns_patch.rs#L216).
- Patch application is not synonymous with tool completion. The edit tools stage proposals first and only write files during approval/apply, in [`rag/tools.rs`](../../../crates/ploke-tui/src/rag/tools.rs#L689) and [`rag/editing.rs`](../../../crates/ploke-tui/src/rag/editing.rs#L73).
- Auto-application already exists and is controlled by `editing.auto_confirm_edits`, in [`user_config.rs`](../../../crates/ploke-tui/src/user_config.rs#L544), [`rag/tools.rs`](../../../crates/ploke-tui/src/rag/tools.rs#L680), and [`app_state/dispatcher.rs`](../../../crates/ploke-tui/src/app_state/dispatcher.rs#L283).
- There is no first-class event for turn completion today. The only authoritative completion object is `ChatSessionReport`, which is logged after the session returns, in [`llm/manager/mod.rs`](../../../crates/ploke-tui/src/llm/manager/mod.rs#L433).

## Requirements For The First Pass
1. Use the existing loaded-workspace runtime and live OpenRouter-backed model loop.
2. Inject the benchmark prompt through the real user-message path, not a special-case direct LLM call.
3. Observe the prompt construction boundary.
4. Observe every tool request and terminal tool result.
5. Capture the assistant-visible tool message payloads.
6. Capture the final assistant message content.
7. Capture the final staged or applied patch result.
8. Know deterministically when the turn is over.
9. Avoid deeper RAG or subsystem probing in this first pass unless needed to unblock the run.

## Required Runtime Settings
### Workspace and tool exposure
- The benchmark runtime must load a workspace or standalone crate before sending the prompt.
- This is mandatory because tool definitions are only attached when `has_loaded_crates()` is true in [`llm/manager/mod.rs`](../../../crates/ploke-tui/src/llm/manager/mod.rs#L500).

### Autonomous edit application
- Set `editing.auto_confirm_edits = true` before the turn starts.
- Without this, patch/edit tools only stage proposals and the harness would need a second approval action.
- For benchmark execution, staging-only is the wrong default because it breaks end-to-end patch production.

### Chat/tool loop policy
- Use explicit `ChatPolicy` values in the benchmark harness instead of relying on defaults.
- At minimum set:
  - `tool_call_timeout_secs`
  - `tool_call_chain_limit`
  - `timeout_strategy`
  - `timeout_base_secs`
- The current defaults are usable but too implicit for benchmark reproducibility.

### Tool verbosity
- Use `ToolVerbosity::Verbose` in the benchmark harness so the recorded tool messages preserve fields and details.
- This matters for offline analysis more than for the turn logic itself.

## Recommended Harness Shape
Introduce a benchmark-specific runtime layer on top of [`TestRuntime`](../../../crates/ploke-tui/src/app/commands/unit_tests/harness.rs).

### Proposed types
```rust
pub struct BenchmarkRunRequest {
    pub benchmark_id: String,
    pub workspace_root: PathBuf,
    pub prompt: String,
    pub model: Option<String>,
    pub auto_confirm_edits: bool,
    pub chat_policy_override: Option<ChatPolicy>,
}

pub struct BenchmarkRunResult {
    pub benchmark_id: String,
    pub user_message_id: Uuid,
    pub assistant_message_id: Option<Uuid>,
    pub final_assistant_text: Option<String>,
    pub session_outcome: BenchmarkSessionOutcome,
    pub prompt_record: Option<PromptRecord>,
    pub tool_events: Vec<ToolEventRecord>,
    pub patch_result: Option<PatchArtifact>,
}

pub enum BenchmarkSessionOutcome {
    Completed,
    Aborted,
    Exhausted,
    TimedOut,
}
```

### Proposed runner
```rust
pub struct BenchmarkHarness {
    runtime: TestRuntime<Spawned, Spawned, Spawned, Spawned, Spawned>,
}

impl BenchmarkHarness {
    pub async fn run_case(&self, req: BenchmarkRunRequest) -> Result<BenchmarkRunResult>;
}
```

## Execution Flow
1. Build `TestRuntime` with file manager, state manager, event bus, llm manager, and observability.
2. Load the benchmark workspace using the existing loaded-workspace helpers.
3. Update runtime config:
   - active model if overridden
   - `editing.auto_confirm_edits = true`
   - explicit chat policy
   - verbose tool verbosity
4. Subscribe to:
   - realtime event bus
   - background event bus
   - app-actor debug relay if needed
5. Start an event recorder task before prompt injection.
6. Inject the benchmark prompt through the real user-message path:
   - `AddUserMessage`
   - `ScanForChange`
   - `EmbedMessage`
7. Wait for a deterministic end-of-turn signal.
8. Read final chat/proposal state and produce `BenchmarkRunResult`.

## Prompt Injection
The harness should not invent a new entrypoint for the first pass.

Use the existing sequence already represented in the app and older harness:
- `AddUserMessage`
- `ScanForChange`
- `EmbedMessage`

Reason:
- It exercises the real pipeline already known to work with your parse/code-graph DB.
- It preserves the current `ChatEvt::Request` and `PromptConstructed` handshake.
- It avoids creating a benchmark-only prompt path that would diverge from production behavior.

## Observation Model
### What to record
Record a normalized event stream with these categories:

```rust
pub enum BenchmarkObservedEvent {
    PromptConstructed(PromptRecord),
    ToolRequested(ToolRequestedRecord),
    ToolCompleted(ToolCompletedRecord),
    ToolFailed(ToolFailedRecord),
    MessageUpdated(MessageRecord),
    SessionFinished(SessionFinishedRecord),
}
```

### Minimum event sources
- `AppEvent::Llm(ChatEvt::PromptConstructed { .. })` from the background subscription.
- `AppEvent::System(SystemEvent::ToolCallRequested { .. })` from realtime.
- `AppEvent::System(SystemEvent::ToolCallCompleted { .. })` from realtime.
- `AppEvent::System(SystemEvent::ToolCallFailed { .. })` from realtime.
- `AppEvent::MessageUpdated(..)` from realtime to capture assistant/tool/user message mutations.

### Why not rely only on chat history
- Chat history tells you what ended up visible.
- It does not preserve the exact tool-request boundaries, arguments, or completion ordering.
- The benchmark harness needs both the semantic outcome and the causal event trace.

## Prompt Record
Capture `PromptConstructed` into:

```rust
pub struct PromptRecord {
    pub parent_id: Uuid,
    pub messages: Vec<RequestMessage>,
    pub context_plan_summary: ContextPlanSummary,
}
```

This is the correct place to inspect what was actually sent to the model. It is better than reconstructing later from chat history because the event already carries the exact formatted prompt.

## Tool Call Recording
For each requested tool call, record:
- outer `step_request_id`
- `parent_id`
- `call_id`
- tool name
- raw arguments JSON
- completion/failure payload
- UI payload fields

The correlation key should be `(step_request_id, call_id)`, matching the current chat loop.

## Patch Capture
### Immediate requirement
At minimum, capture the final patch/tool output as seen by the tool system and whether it was actually applied.

### Recommended capture strategy
After the turn ends:
1. Inspect `state.proposals` and `state.create_proposals`.
2. Locate proposals associated with the observed request ids and tool call ids.
3. Extract:
   - proposal status
   - affected files
   - unified diff preview when present
   - before/after preview when present
4. If auto-confirm was enabled, confirm that the resulting status is `Applied`.

### Patch artifact shape
```rust
pub struct PatchArtifact {
    pub request_ids: Vec<Uuid>,
    pub statuses: Vec<String>,
    pub files: Vec<PathBuf>,
    pub unified_diff: Option<String>,
    pub applied: bool,
}
```

### Important current limitation
`NsPatch` currently allows one patch per tool call. Multi-file SWE-bench solutions therefore require either:
- multiple `ns_patch` tool calls in one turn, or
- continued reliance on `apply_code_edit`/`create_file` for some file changes.

Do not weaken this validation in the first pass. The current restriction is explicit in [`tools/ns_patch.rs`](../../../crates/ploke-tui/src/tools/ns_patch.rs#L216).

## Turn Completion
### Current state
There is no benchmark-safe explicit completion event.

Today the harness would have to infer completion from some combination of:
- final assistant `MessageUpdated`
- absence of outstanding tool waiters
- log output from `ChatSessionReport`

That is too indirect for benchmark automation.

### Recommended change
Add a first-class end-of-turn event emitted from `llm_manager` immediately after `prepare_and_run_llm_call` returns.

Example:
```rust
pub enum SystemEvent {
    // existing variants...
    ChatTurnFinished {
        session_id: Uuid,
        parent_id: Uuid,
        assistant_message_id: Uuid,
        outcome: SessionOutcome,
        summary: String,
    },
}
```

Emit it in [`llm/manager/mod.rs`](../../../crates/ploke-tui/src/llm/manager/mod.rs#L433), where the report already exists.

### Why this is the right seam
- The chat session is already fully resolved there.
- The session outcome is already normalized.
- No inference from logs or state timing is needed.
- The benchmark harness can block on exactly one well-scoped terminal event.

## Recommended Minimal Implementation Order
1. Add `SystemEvent::ChatTurnFinished`.
2. Emit it from `llm_manager` after `prepare_and_run_llm_call`.
3. Add a benchmark event recorder that subscribes to realtime and background channels.
4. Add `BenchmarkHarness::run_case` built on `TestRuntime`.
5. Wire config overrides for:
   - auto-confirm edits
   - active model
   - chat policy
   - tool verbosity
6. Build `BenchmarkRunResult` from the event recorder plus final state inspection.
7. Add one live benchmark-style test that runs a small prompt and asserts:
   - prompt constructed
   - at least one tool event observed
   - terminal `ChatTurnFinished`
   - patch artifact captured

## Non-Goals For This First Pass
- Full RAG introspection.
- Streaming partial response support.
- Reworking tool schemas.
- Relaxing any patch validation or approval invariants.
- Making fixture compatibility more permissive.

## Design Summary
- Use the existing user-message path as the benchmark input boundary.
- Use the existing event bus as the tool observation boundary.
- Turn on `editing.auto_confirm_edits` so staged edits become real filesystem changes during the run.
- Add one explicit terminal event for the chat turn.
- Build a benchmark-specific recorder/result layer around `TestRuntime`, not a parallel bespoke harness.
