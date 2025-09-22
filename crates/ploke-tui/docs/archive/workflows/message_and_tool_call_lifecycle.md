# Message and Tool-Call Lifecycle (End-to-End)

This document describes the end-to-end workflow in ploke-tui from first run through indexing, external edits, code-context queries, database persistence, restart/load, and tool-driven code edits. It anchors the flow to concrete events, commands, and modules in the codebase so we can test and evolve reliably.

Sections
- Components at a glance
- Event/command glossary (subset)
- End-to-end scenario (10 steps) with event flow
- Notes on persistence and restart
- Tool-calling data path (request/response shapes)

## Components at a Glance
- `App` (UI runtime): accepts user input, renders, sends `StateCommand` to the state manager, listens on `EventBus`.
- `EventBus`: broadcast channels for AppEvent (realtime/background) and indexing status.
- `AppState` + `state_manager`: authoritative state + async command dispatcher.
- `rag` domain: request context, tool dispatcher, safe-edit pipeline (`apply_code_edit` + IoManager).
- `llm` domain: request/session loop, OpenRouter request/response types, tool-calls handling.
- `IoManager`: atomic write pipeline (temp file + fsync + rename) and verified reads.

## Glossary (subset)
- `AppEvent`: UI-wide events; includes `System(...), Llm(...), LlmTool(...)` and indexing progress.
- `SystemEvent`:
  - `ToolCallRequested/Completed/Failed`: typed tool-call lifecycle for system tools.
  - `ReIndex`: index workspace request (handled by state manager → indexing handler).
  - `LoadDb` and `BackupDb`: DB persistence/recovery.
- `StateCommand`:
  - `IndexWorkspace { workspace, needs_parse }`
  - `CreateAssistantMessage`, `UpdateMessage`, `AddMessageImmediate`
  - `SaveState`, `UpdateDatabase`
- `llm::Event`: internal LLM loop events (`Request`, `PromptConstructed`, `ToolCall`, ...).

## Scenario: First Run to Tool Edit (10 steps)

Assume `ploke-tui` starts with a fresh `App` + `AppState`; event loop and subsystems are running (`run_event_bus`, indexing, observability, etc.).

1) User enters `/index workspace tests/fixture_crates/fixture_nodes`
- UI parses the slash command and sends `StateCommand::IndexWorkspace { workspace, needs_parse: true }`.
- `state_manager` dispatches to `app_state::handlers::indexing::index_workspace`:
  - Sets `state.system.crate_focus` to the resolved workspace path.
  - Optionally runs `parser::run_parse(...)` to seed DB.
  - Spawns the indexer (`IndexerTask::index_workspace`), wiring progress through `event_bus.index_tx`.
- `run_event_bus` forwards `IndexingStatus` into realtime `AppEvent`: emits `IndexingStarted`, then `IndexingProgress` (many), then `IndexingCompleted` or `IndexingFailed`.
- UI shows progress and eventually completion.

2) Outside ploke-tui: user modifies a file under `fixture_nodes`
- Files are edited in an external editor; no immediate UI update occurs.
- Later calls that read or write will compute/verify tracking hashes through `IoManager`.

3) User enters normal message: `Hello, can you see any code snippets related to SimpleStruct?`
- On Enter in Insert mode, the App issues three StateCommands in sequence (ref: `app/mod.rs` Action::Submit):
  1) `StateCommand::AddUserMessage { content, new_msg_id, completion_tx }`
     - Handler: `app_state/handlers/chat.rs::add_msg_immediate` writes the user message into the chat tree, updates selection, emits `MessageUpdatedEvent`.
     - Critically: since kind == User, it also emits `AppEvent::Llm(llm::Event::Request { parent_id, new_msg_id, prompt: content, ... })` to the EventBus.
  2) `StateCommand::ScanForChange { scan_tx }`
     - Handler: `app_state/handlers/db.rs::scan_for_change` (invoked via dispatcher) checks for file hash changes and reparses if needed; its oneshot coordinates with EmbedMessage.
  3) `StateCommand::EmbedMessage { new_msg_id, completion_rx, scan_rx }`
     - Handler: `rag/context.rs::process_with_rag` waits for AddUserMessage completion (completion_rx) and the scan oneshot, then uses `RagService::get_context(...)` to assemble code context for the latest user message.
     - It constructs a mixed prompt (headers + code snippets + conversation path) and emits `AppEvent::Llm(llm::Event::PromptConstructed { parent_id, prompt })`.
- In `llm/mod.rs::llm_manager`, `Event::Request` is stored in `pending_requests`. When `Event::PromptConstructed` arrives for the same `parent_id`, the pair is resolved and `process_llm_request` is spawned with `context = Some(Event::PromptConstructed { ... })`.
- Inside `process_llm_request`, the manager first issues `StateCommand::CreateAssistantMessage` to create a pending assistant message. Then `prepare_and_run_llm_call` constructs the OpenRouter `CompReq` via `build_comp_req`, including tool definitions if the active model/provider is marked tool-capable. After the HTTP call, the response is parsed and either tool-calls are dispatched/awaited (see step 8 for tool flow) or a final assistant content is produced via `StateCommand::UpdateMessage`.

4) User enters `/save db`
- UI resolves to `StateCommand::SaveState` and persistence helpers under `app_state::handlers::session` are invoked (e.g., proposals, session state). Any open proposals are saved; chat and config are persisted.

5) User exits ploke-tui
- Normal shutdown; background tasks unwind. UI restores terminal state and exits.

6) User runs ploke-tui again
- Subsystems initialize; `EventBus` started; `AppState` is fresh but can load saved session state.

7) User enters `/load crate fixture_nodes`
- Leads to database restore workflow (`app_state::database::persist_conversation_turn` helpers, `LoadDb` events). The UI processes:
  - `SystemEvent::LoadDb { crate_name, file_dir, is_success, error }` to show results.
  - On success, the DB contents for `fixture_nodes` are active. `state.system.crate_focus` can be updated via `ReIndex` or direct load helpers.

8) User enters normal message asking to edit `SimpleStruct` via tool call
- As before, the LLM loop constructs a `CompReq` with tool definitions.
- Upon a tool-capable response containing tool_calls, the loop emits `AppEvent::LlmTool(ToolEvent::Requested { name, arguments, call_id })`.
- `llm_manager` forwards that to `rag::dispatcher::handle_tool_call_requested` → `tools::dispatch_gat_tool`.
  - For `get_file_metadata`, the GAT tool reads file, returns a typed JSON result; emits `SystemEvent::ToolCallCompleted`.
  - For `apply_code_edit`, the GAT/legacy bridge parses edits and stages a proposal:
    - Canonical mode: resolves node by `canon` path against DB (`resolve_nodes_by_canon_in_file`), builds `WriteSnippetData` with `expected_file_hash`.
    - Splice mode: uses byte-range replacement and `expected_file_hash` verification.
    - The `rag::tools::apply_code_edit_tool` places an `EditProposal` into `state.proposals` and emits `ToolCallCompleted` with `ApplyCodeEditResult { ok, staged, files, preview_mode, ... }`. A sysinfo summary is added (files, preview snippet) and auto-approval may apply depending on config.
- The LLM loop awaits each `ToolEvent::Completed` or `SystemEvent::ToolCallCompleted`, appends tool outputs as `system` messages in the conversation, and continues. Finally, an assistant message is finalized.

9) User enters `/save db`
- As before, persists proposals and session state. If edits were applied (approved), the workspace may be rescanned via post-apply hooks.

10) User exits program
- Normal termination.

## Persistence & Restart
- Proposals are kept under `state.proposals` and saved on demand (`/save db`) and/or at strategic points.
- `LoadDb` restores DB relations; `IndexWorkspace` re-indexes source into DB. Both are accessible via UI commands and events.
- `state.system.crate_focus` ties relative file resolution to a workspace root; set on index/load and respected by edit preview.

## Tool-Calling Data Path
- Outgoing `CompReq` includes `tools: [ToolDefinition]` with typed `ToolName` and JSON Schema parameters. We build it via `llm/session.rs::build_comp_req`.
- Incoming tool calls use normalized OpenAI-like payloads. We deserialize into `tools::ToolCall<'a>` (borrowing arguments as `&str`) for zero-copy review.
- Dispatcher `tools::dispatch_gat_tool` routes by name to GAT tool impls and emits `SystemEvent::ToolCallCompleted/Failed` deterministically.
- For edits, the pipeline is safe by design:
  - Edit staging computes diffs/preview and persists an `EditProposal` entry.
  - Approval (`approve_edits`) applies via IoManager (verified hash → temp write + fsync + rename) and triggers a rescan.
  - Denial (`deny_edits`) marks proposal and emits a failure event to the LLM loop for immediate feedback.

References
- `crates/ploke-tui/src/app_state/dispatcher.rs` (StateCommand handling)
- `crates/ploke-tui/src/app/events.rs` (AppEvent routing)
- `crates/ploke-tui/src/llm/session.rs` (request/session, CompReq, tool_calls)
- `crates/ploke-tui/src/tools/mod.rs` (GAT dispatch + ToolDefinition types)
- `crates/ploke-tui/src/rag/tools.rs` (apply_code_edit legacy handler + preview/proposal)
- `crates/ploke-tui/src/rag/editing.rs` (approve/deny APIs)
- `crates/ploke-tui/docs/openrouter/request_structure.md` (OpenRouter request/response spec)
