# Retainer Report: Headless `ploke-tui` Harness Map

Worker: `ta-retainer-worker`
Task: `ta-retainer-map-v1`

## Summary

The vanilla `ploke-tui` path should be reused as-is. The important split is:

- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs` is the actor/runtime harness. It constructs `AppState`, `EventBus`, command channels, proposal maps, and type-state actor spawns.
- `crates/ploke-tui/src/test_utils/new_test_harness.rs` is the current helper that actually runs the full `App` headlessly with `ratatui::backend::TestBackend` and synthetic input.

`ploke-eval` already enables `ploke-tui`'s `test_harness` feature, so the harness module is available through that feature boundary: `crates/ploke-eval/Cargo.toml:25`, `crates/ploke-tui/Cargo.toml:117-126`.

## Harness Availability

- `app::commands::harness` is compiled when `test` or feature `test_harness` is enabled: `crates/ploke-tui/src/app/commands/mod.rs:1-9`.
- `ploke-eval` depends on `ploke-tui` with `features = ["test_harness"]`: `crates/ploke-eval/Cargo.toml:20-25`.
- `test_harness` is a declared `ploke-tui` feature: `crates/ploke-tui/Cargo.toml:117-126`.

## Core Runtime Constructors

Use `TestRuntime` when the adapter wants actor-level control without running the terminal loop.

- `TestRuntime` is a type-state builder over file manager, state manager, event bus, LLM manager, and observability actors: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:877-893`.
- `TestRuntime::new(&Arc<Database>)` builds a lightweight runtime with no actors spawned yet: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1035-1043`.
- `TestRuntime::new_with_embedding_processor(...)` lets callers avoid implicit dependence on default local embedding configuration: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1045-1052`.
- Runtime construction initializes `EventBus`, `AppState`, `proposals`, `create_proposals`, command channel, cancel watch channel, and RAG event channel: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1066-1125`.
- `into_app(pwd)` returns an `App` handle without requiring spawned actors: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:921-924`.
- `into_app_with_state_pwd(pwd)` also seeds `SystemState.pwd`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:926-930`.
- `state_arc()` exposes the shared `Arc<AppState>` for proposal inspection: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:947-949`.
- `setup_loaded_workspace(...)` and `setup_loaded_standalone_crate(...)` seed loaded workspace state without going through `/load`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:956-997`.

## Spawned Actors

The full actor stack from `harness.rs` is:

- `spawn_file_manager()` creates `FileManager` with IO handle, background event subscription, RAG event sender, realtime sender, and `pwd`, then spawns `fm.run()`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1140-1152`.
- `spawn_state_manager()` consumes the command receiver, inserts the relay, records `debug_string_rx`, spawns the relay, and spawns `state_manager(...)`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1155-1185`.
- `spawn_validation_probe()` is an alternative to `spawn_state_manager()` because it also consumes `cmd_rx`; use it only when validating commands without running the real state manager: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1188-1219`.
- `spawn_event_bus()` sets the global event bus and runs `run_event_bus(...)`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1222-1228`.
- `spawn_llm_manager()` subscribes to realtime/background events and spawns `llm_manager(...)`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1231-1241`.
- `spawn_observability()` spawns observability over the same event bus and state: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1244-1250`.
- The back-compat full actor stack is `setup_test_app_from_db(...)`: file manager, state manager, event bus, LLM manager, observability, then `into_app_arc`: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:1257-1266`.

For the proof-of-concept adapter, prefer the real `spawn_state_manager()` path, not `spawn_validation_probe()`, because the adapter needs normal chat/tool behavior.

## Event Builders And Receivers

`TestRuntime::events_builder()` is the typed subscription entry point:

- It consumes the optional `debug_string_rx` and `validation_rx`, subscribes app receivers, IO file events, and event bus receivers: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:999-1032`.
- App-side receivers include realtime `event_rx`, background `bg_event_rx`, cancel watch receiver, optional context-plan history, optional debug command receiver, and optional validation receiver: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:515-535`.
- Event-bus receivers include realtime `AppEvent`, background `AppEvent`, `ErrorEvent`, and indexing status: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:580-636`.
- `TestInAppActorBuilder::from_app(...)` subscribes realtime/background and cancel receivers directly from `EventBus` and cancel channel: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:675-689`.
- Builder outputs are `build_app_only`, `build_event_bus_only`, `build_app_event_bus`, `build_all`, etc.: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:757-859`.

The underlying bus uses priority-based channels:

- `EventBus::send` routes events according to `AppEvent::priority()`: `crates/ploke-tui/src/event_bus/mod.rs:160-179`.
- `EventBus::subscribe(EventPriority::{Realtime, Background})` returns `broadcast::Receiver<AppEvent>`: `crates/ploke-tui/src/event_bus/mod.rs:185-190`.
- `error_subscriber()` and `index_subscriber()` expose error/index streams: `crates/ploke-tui/src/event_bus/mod.rs:192-198`.
- Tool call requested/completed/failed and chat turn finished are realtime events: `crates/ploke-tui/src/lib.rs:390-393`.

## Full Headless App Harness

The helper that currently runs the full app headlessly is `AppHarness`:

- `AppHarness` exposes `state`, `event_bus`, `cmd_tx`, synthetic `input_tx`, and an app task: `crates/ploke-tui/src/test_utils/new_test_harness.rs:42-49`.
- `AppHarness::spawn()` constructs config, DB, IO, event bus, embedder, `AppState`, command channel, cancel channel, state manager, LLM manager, `TestBackend`, synthetic input stream, and then spawns `App::run_with(...)`: `crates/ploke-tui/src/test_utils/new_test_harness.rs:51-187`.
- `App::run_with(...)` is the actual full app loop over a terminal backend and input stream, with `RunOptions { setup_terminal_modes: false }` suitable for headless tests: `crates/ploke-tui/src/app/mod.rs:118-123`, `crates/ploke-tui/src/app/mod.rs:319-365`.
- The app loop receives realtime app events and background app events and routes both through `events::handle_event`: `crates/ploke-tui/src/app/mod.rs:537-545`.
- `AppHarness::shutdown()` sends `AppEvent::Quit` and awaits the app task: `crates/ploke-tui/src/test_utils/new_test_harness.rs:242-246`.

If the eval adapter must literally "run the full app headlessly", start from `AppHarness::spawn()` or mirror its construction. If the adapter only needs the vanilla actor graph, use `TestRuntime` from `harness.rs`.

## Sending A User Prompt

There are two equivalent paths.

Actor-level path:

- `TestAppAccessor::state_cmd_tx(&App)` exposes a clone of the app command sender: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:505-512`.
- Send `StateCommand::AddUserMessage { content, new_user_msg_id, completion_tx }`: variant fields are defined at `crates/ploke-tui/src/app_state/commands.rs:780-784`.
- Also send `StateCommand::ScanForChange { scan_tx }` and `StateCommand::EmbedMessage { new_msg_id, completion_rx, scan_rx }` to match app submit behavior and kick context construction: `crates/ploke-tui/src/app/mod.rs:1296-1316`.

Full headless helper path:

- `AppHarness::add_user_msg(content)` does exactly that triplet and returns the new user message id: `crates/ploke-tui/src/test_utils/new_test_harness.rs:190-216`.

Why all three commands matter:

- `AddUserMessage` adds the user chat message and fires the initial `LlmEvent::ChatCompletion(ChatEvt::Request { ... })`: `crates/ploke-tui/src/app_state/handlers/chat.rs:156-164`, `crates/ploke-tui/src/app_state/handlers/chat.rs:321-335`.
- `EmbedMessage` calls the RAG/context path, which emits `ChatEvt::PromptConstructed`: `crates/ploke-tui/src/app_state/dispatcher.rs:243-249`, `crates/ploke-tui/src/rag/context.rs:173-231`.
- The LLM manager waits until it has both `Request` and `PromptConstructed`, then spawns `process_llm_request(...)`: `crates/ploke-tui/src/llm/manager/mod.rs:276-303`.

## Terminal Events To Watch

Primary terminal event:

- `SystemEvent::ChatTurnFinished { session_id, request_id, parent_id, assistant_message_id, outcome, error_id, summary, attempts }`: `crates/ploke-tui/src/app_state/events.rs:103-112`.
- It is emitted from the LLM manager using `SessionOutcome::{Completed, Aborted, Exhausted}` as string outcomes: `crates/ploke-tui/src/llm/manager/mod.rs:235-256`.

Tool lifecycle events:

- `SystemEvent::ToolCallRequested { tool_call, parent_id, request_id }`: `crates/ploke-tui/src/app_state/events.rs:77-86`.
- `SystemEvent::ToolCallCompleted { request_id, parent_id, call_id, content, ui_payload }`: `crates/ploke-tui/src/app_state/events.rs:87-94`.
- `SystemEvent::ToolCallFailed { request_id, parent_id, call_id, error, ui_payload }`: `crates/ploke-tui/src/app_state/events.rs:95-102`.
- The session emits tool requests only after the dispatcher is waiting, then waits for completed/failed events matching the step request id: `crates/ploke-tui/src/llm/manager/session.rs:1568-1637`.
- The LLM manager turns `ToolCallRequested` into `tools::process_tool(...)` with a `Ctx` carrying state, event bus, request id, parent id, and call id: `crates/ploke-tui/src/llm/manager/mod.rs:305-328`, `crates/ploke-tui/src/tools/mod.rs:91-110`.

Error/timeout signals:

- Per-tool timeout becomes a `ToolCallUiError` with a timeout wire string and retry hint: `crates/ploke-tui/src/llm/manager/session.rs:1538-1564`.
- Tool execution completion/error helpers emit realtime `ToolCallCompleted` or `ToolCallFailed`: `crates/ploke-tui/src/tools/mod.rs:576-600`.
- Legacy RAG tool helpers emit `ToolCallFailed` through `ToolCallParams`: `crates/ploke-tui/src/rag/utils.rs:222-258`.
- Event channel closure during tool wait is converted into remaining failed tool results: `crates/ploke-tui/src/llm/manager/session.rs:1608-1617`.

## Proposal Map Access

The proposal registry is shared state:

- `AppState.proposals: RwLock<HashMap<Uuid, EditProposal>>`: `crates/ploke-tui/src/app_state/core.rs:30-49`.
- `EditProposal` fields include `proposal_id`, `request_id`, `parent_id`, `call_id`, `edits`, `edits_ns`, `files`, `preview`, `status`, and `is_semantic`: `crates/ploke-tui/src/app_state/core.rs:359-374`.
- `derive_edit_proposal_id(request_id, call_id)` is deterministic: `crates/ploke-tui/src/app_state/core.rs:376-382`.
- Semantic tool staging inserts a pending `EditProposal` into `state.proposals` and emits `ToolCallCompleted` with a `ToolUiPayload` containing `proposal_id`: `crates/ploke-tui/src/rag/tools.rs:216-314`.

Adapter implication: after each `ToolCallCompleted`, inspect `state.proposals.read().await` and/or use `ui_payload.proposal_id` to find the staged candidate. Do not infer proposal existence from chat text alone.

## Approving, Denying, And Retrying

Use normal TUI edit commands:

- `StateCommand::ApproveEdits { proposal_id }`, `DenyEdits { proposal_id }`, `ApprovePendingEdits`, `DenyPendingEdits`: `crates/ploke-tui/src/app_state/commands.rs:989-1008`.
- Dispatcher routes these to `rag::editing::{approve_edits, deny_edits, approve_pending_edits, deny_pending_edits}`: `crates/ploke-tui/src/app_state/dispatcher.rs:388-398`.
- `approve_edits(...)` applies through the normal IO manager path and handles idempotent applied/denied states: `crates/ploke-tui/src/rag/editing.rs:19-79`.
- Semantic apply writes `proposal.edits` with `write_snippets_batch`, updates proposal status, emits `ToolCallCompleted` on success/no-op and `ToolCallFailed` on IO error: `crates/ploke-tui/src/rag/editing.rs:339-490`.
- Non-semantic apply requires all files to apply for `status=applied`; partial application becomes `Failed("Partially applied ...")` but still emits `ToolCallCompleted` with `ok=false` and `partial=true`: `crates/ploke-tui/src/rag/editing.rs:190-251`.
- `deny_edits(...)` marks the proposal denied and emits `ToolCallFailed` with status `denied`: `crates/ploke-tui/src/rag/editing.rs:550-620`.
- `approve_pending_edits(...)` applies newest non-overlapping proposals and marks overlapping older ones stale: `crates/ploke-tui/src/rag/editing.rs:633-706`, overlap detection at `crates/ploke-tui/src/rag/editing.rs:744-810`.

Adapter implication: for bounded-surface retries, inspect the staged proposal first. If paths violate policy, send `DenyEdits { proposal_id }`, then send a new user prompt describing the rejection. Do not terminate the TUI session just because a proposal was out of surface.

## Suggested Adapter Event Loop

1. Spawn the full headless harness using `AppHarness::spawn()` if the goal is to run the app loop, or spawn `TestRuntime::{file,state,event_bus,llm,observability}` if only actors are needed.
2. Subscribe to realtime events from the harness event bus before sending the prompt.
3. Send prompt via `AppHarness::add_user_msg(prompt)` or the `StateCommand` triplet.
4. Watch realtime events:
   - collect `ToolCallRequested`;
   - on `ToolCallCompleted`, inspect `ui_payload.proposal_id` and `state.proposals`;
   - on `ToolCallFailed`, record attempt failure but keep retry policy in eval;
   - on `ChatTurnFinished`, classify terminal turn outcome.
5. When a pending proposal appears, run eval boundary checks on `proposal.files`, `proposal.edits`, and `proposal.edits_ns`.
6. If rejected, send `DenyEdits` and then another `AddUserMessage`/`EmbedMessage` prompt with the bounded rejection reason.
7. If accepted, send `ApproveEdits` or `ApprovePendingEdits`.
8. After apply, inspect proposal statuses and workspace diff. Treat `ToolCallCompleted` content with `ok=false`, `partial=true`, or proposal `Failed/Stale/Denied` as failed attempt evidence, not success.
9. On retry exhaustion or `ChatTurnFinished` without any material proposal, return a failed/no-edit adapter outcome to eval.

## Blockers / Caveats For Implementers

- `harness.rs` itself does not run the full terminal app except the `demo`-gated `spawn_terminal_app`; the existing full headless run helper is `test_utils/new_test_harness.rs`.
- `spawn_validation_probe()` cannot be combined with `spawn_state_manager()` because both consume the same command receiver.
- Semantic apply currently treats `applied > 0` as `Applied`; eval should do its own bundle check after apply if it wants all-or-rejected semantics: `crates/ploke-tui/src/rag/editing.rs:381-390`.
- Non-semantic partial apply emits `ToolCallCompleted`, not `ToolCallFailed`; eval must inspect payload/content/status rather than trusting event kind alone: `crates/ploke-tui/src/rag/editing.rs:192-251`.
- `App::send_cmd` is private; external adapter code should use the exposed command sender from `TestAppAccessor` or `AppHarness.cmd_tx`: `crates/ploke-tui/src/app/mod.rs:304-309`, `crates/ploke-tui/src/app/commands/unit_tests/harness.rs:505-512`, `crates/ploke-tui/src/test_utils/new_test_harness.rs:42-49`.

## Verification

Changed files:

- `docs/active/agents/2026-05-12_tui-adapter-implementation-wave/reports/retainer.md`

Commands used:

- `wc -l` / `ls -lh` on the worker brief and relevant source files before bounded reads.
- `rg` to locate harness constructors, event receivers, state commands, terminal events, proposal access, and timeout paths.
- `nl -ba ... | sed -n ...` for cited line ranges.

No code was edited and no compile/test command was needed for this mapping report.
