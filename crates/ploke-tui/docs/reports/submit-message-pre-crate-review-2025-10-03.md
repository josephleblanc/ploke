# Submit Message Flow Before Crate Load — Review and Answers

Scope
- Validate and complete the overview at `crates/ploke-tui/docs/active/overviews/submit-message.md`.
- Confirm actual behavior when submitting a message before any crate/workspace is loaded.
- Answer your questions and highlight edge cases and likely breakages.

Summary
- Current behavior stalls the request if no crate is loaded: the user sees a SysInfo message "Embedding User Message" and no assistant reply.
- Root cause: the RAG path doesn’t emit a `PromptConstructed` event when RAG is unavailable or fails; the LLM manager pairs a `Request` only when a matching `PromptConstructed` arrives (and currently requires `Request` to arrive first).
- The SysInfo placeholder is not updated or removed; it remains in the conversation history as a sibling under the user message.
- Your suggestion to proceed without code context (no RAG) can work; the most coherent place to do it is in RAG construction (always emit a `PromptConstructed` with history-only messages) or conditionally in the LLM manager with tools disabled when no crate is loaded.

End‑to‑End Flow (as implemented)
1) Entry point: `Action::Submit` (file: `crates/ploke-tui/src/app/mod.rs`)
   - Enqueues four state commands when input is non-empty:
     - `StateCommand::AddUserMessage { content, new_user_msg_id, completion_tx }`
     - `StateCommand::ScanForChange { scan_tx }`
     - `StateCommand::EmbedMessage { new_msg_id: new_user_msg_id, completion_rx, scan_rx }`
     - `StateCommand::AddMessage { kind: SysInfo, content: "Embedding User Message", parent_id: new_user_msg_id, child_id: next_llm_msg_id }`

2) State dispatch (file: `crates/ploke-tui/src/app_state/dispatcher.rs`)
   - `AddUserMessage` → `handlers::chat::add_user_message`
     - Immediately adds the user message to chat history and completes `completion_tx`.
     - Emits `AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Request{ parent_id: <user_msg_id>, request_msg_id: <new uuid> }))` from inside `add_msg_immediate` when kind is User.
   - `ScanForChange` → `handlers::db::scan_for_change` → `app_state/database.rs::scan_for_change`
     - If `state.system.crate_focus` is `None` or invalid, returns an error (emits warning logs), and the oneshot `scan_tx` will never be observed by the downstream handler in (3) because it isn’t awaited there.
     - If no file changes are found, sends `None` via `scan_tx`; on changes, it performs parsing work and sends file paths.
   - `EmbedMessage` → `rag::context::process_with_rag(state, event_bus, scan_rx, new_msg_id, completion_rx)`
     - Waits for the user-message `completion_rx` only.
     - Does not currently wait on or use `scan_rx` (the oneshot is ignored here).
     - If `state.rag` is Some, calls `rag.get_context(...)` and, on success, builds `ChatEvt::PromptConstructed { parent_id, formatted_prompt }` using:
       - RAG-assembled context as system messages, then
       - Conversation history from root→current excluding SysInfo (and using Tool with tool_call_id where present).
     - If `state.rag` is None, emits a SysInfo message "No RAG configured" and returns without sending `PromptConstructed`.

3) LLM manager orchestration (file: `crates/ploke-tui/src/llm/manager/mod.rs`)
   - Maintains two maps keyed by `{ parent_id }`:
     - `pending_requests: { EvtKey -> ChatEvt::Request }`
     - `ready_contexts: { EvtKey -> ChatEvt::PromptConstructed }`
   - Pairing behavior:
     - On `ChatEvt::Request`: inserts into `pending_requests` and does not attempt to match any pre-existing `ready_contexts` entry.
     - On `ChatEvt::PromptConstructed`: if a matching request exists, removes it and spawns `process_llm_request`; otherwise caches the prompt in `ready_contexts`.
     - Consequence: if `PromptConstructed` arrives before `Request`, the pair will not be formed later because the `Request` branch does not check `ready_contexts`. This effectively requires the `Request` to arrive first.
   - `process_llm_request`:
     - Creates a placeholder assistant message via `StateCommand::CreateAssistantMessage { new_assistant_msg_id: request_msg_id, content: "Pending...", status: Generating }`.
     - Builds an OpenRouter request with tools enabled (RequestCodeContext, CodeEdit, GetFileMetadata, CreateFile) and `ToolChoice::Auto`.
     - On tool-calls: updates the user message by mistake (`UpdateMessage { id: parent_id, ... }`) with a content preview; dispatches tool workflows and then continues.
     - On success: sends `StateCommand::AddMessageImmediate { kind: Assistant, msg: <content> }` creating a new assistant message; it does not update the initial "Pending..." assistant placeholder.
     - On error: adds SysInfo error messages.

What the user sees before a crate is loaded
- The SysInfo placeholder "Embedding User Message" is added under the user message and remains in history (it is not removed/updated by current code).
- Because `process_with_rag` does not send `PromptConstructed` when RAG is missing/fails, the `ChatEvt::Request` is never paired; no assistant reply is produced.
- `ScanForChange` error (no `crate_focus`) is not surfaced to the chat; only logs are emitted.

Accuracy review of your overview
- 1. `Action::Submit`: Accurate. It enqueues the four commands listed.
- 1.1 `AddUserMessage`: Accurate. The completion oneshot gates RAG; the `Request` event is emitted here.
- 1.2 `ScanForChange`:
  - Accurate on error shape and lack of user-facing propagation.
  - Correction: The scan oneshot is not consumed by `process_with_rag` (current code ignores `scan_rx`). Therefore scan success/failure has no effect on the RAG step today.
- 1.3 `EmbedMessage`: Details filled above — currently implemented by `rag::context::process_with_rag`, which:
  - Waits for the user-message completion only.
  - Builds/forwards `PromptConstructed` when RAG succeeds; otherwise emits "No RAG configured" and stops.
- 1.4 `AddMessage` (SysInfo): It is not updated later. It remains in the message tree; likely you are seeing focus move to the assistant placeholder/response which makes it seem to "disappear".
- 2. LLM manager pairing: Your observation is correct — the design effectively requires the `Request` to arrive before `PromptConstructed`. If context arrives first, it is cached, but a subsequent `Request` does not check for a waiting context. This is a real pairing asymmetry.

Answers to your questions
1) Will adding logic in the LLM manager to proceed without crate_focus work?
- Yes, it can work, but there are two viable places to implement the behavior:
  - Preferred: in `process_with_rag`, always send a `PromptConstructed` even when RAG is unavailable or fails — construct a prompt from conversation-only messages. Also add a SysInfo message with instructions for indexing/loading a workspace. This keeps pairing logic unchanged and avoids duplicating "prompt construction" logic in the manager.
  - Alternative: in the LLM manager’s `ChatEvt::Request` handler, detect `crate_focus.is_none()` and synthesize a `PromptConstructed` using `chat_history.current_path_as_llm_request_messages()`, then spawn the request immediately. Also emit a SysInfo guidance message. This reduces dependence on RAG but spreads prompt construction across modules.
- Tools: In both approaches, disable tools when no crate is loaded (set `req.tools=None` and `tool_choice=None`). Otherwise tool calls will likely fail (no DB/Io roots) and create confusing SysInfo/tool-failure messages.

2) If that change is made, what else might break?
- Tooling:
  - RequestCodeContext/GatCodeEdit/GetFileMetadata/CreateFile may fail without DB/IoManager roots. Disabling tools when `crate_focus=None` prevents confusing failures and reduces API/tool churn.
- Messaging consistency:
  - The current assistant placeholder flow is inconsistent: a "Pending..." assistant is created but later a separate assistant message is appended with final content. If you add history-only requests, this inconsistency remains. A follow-up fix should update the placeholder instead of adding a new message.
- Pairing order dependency:
  - The manager requires `Request` to arrive before `PromptConstructed`. If you choose the manager-side synthesis path, this is OK because you are acting in the `Request` branch. If you fix `process_with_rag` to always emit a `PromptConstructed`, you still depend on `Request` arriving first (which it does today), but pairing remains asymmetric and could cause future surprises.
- UX copy:
  - The SysInfo placeholder text "Embedding User Message" is misleading when skipping RAG. Consider conditional copy like "Preparing request (no workspace loaded)" when `crate_focus=None`.

Suggested, low‑risk fixes (no code changes made; for future consideration)
- Pairing robustness (manager): On `ChatEvt::Request`, check `ready_contexts` for a cached `PromptConstructed` and pair immediately; on `PromptConstructed`, keep the existing pairing. This removes the order dependency.
- RAG fallback (preferred): In `process_with_rag`, when RAG is None or errors, send a conversation‑only `PromptConstructed`. Emit a SysInfo with instructions to index or load a workspace.
- Tools gating: In `prepare_and_run_llm_call`/`RequestSession::run`, set `tools=None` and `tool_choice=None` when `crate_focus=None`.
- Message lifecycle: Replace/update the assistant placeholder instead of creating a second assistant message on completion.
- Surface scan errors: When `ScanForChange` detects `crate_focus=None` or invalid path, surface a SysInfo warning with quick actions (short command hints) rather than logging only.

Evidence (key references)
- Submit pipeline: `crates/ploke-tui/src/app/mod.rs` (Submit → four StateCommands)
- State dispatch: `crates/ploke-tui/src/app_state/dispatcher.rs`
- DB scan: `crates/ploke-tui/src/app_state/database.rs::scan_for_change`
- RAG flow: `crates/ploke-tui/src/rag/context.rs::process_with_rag` and `construct_context_from_rag`
- Chat events and pairing: `crates/ploke-tui/src/llm/manager/events.rs`, `crates/ploke-tui/src/llm/manager/mod.rs::llm_manager`
- Request session and tools: `crates/ploke-tui/src/llm/manager/session.rs`
- Chat history and filtering rules: `crates/ploke-tui/src/chat_history.rs`
- SysInfo placeholder insertion: `crates/ploke-tui/src/app/mod.rs` under `Action::Submit`

Open questions / decisions
- Where to centralize prompt construction when skipping RAG? (RAG module vs Manager)
- Exact UX copy and flow for no‑workspace case; whether to keep or change the SysInfo placeholder text.
- Policy for tools when no crate loaded (disable vs allow and surface structured failures).

Appendix: Corrections to the overview doc
- Note under 1.2: add that the scan oneshot is currently not awaited/used by `process_with_rag`.
- Section 1.3: fill in description of `process_with_rag` responsibilities and its current early‑return behavior when `state.rag` is None or errors occur.
- Section 1.4: clarify that the SysInfo message is not updated/removed; it remains in history.
- Section 2: confirm pairing asymmetry (requires `Request` before `PromptConstructed`).
