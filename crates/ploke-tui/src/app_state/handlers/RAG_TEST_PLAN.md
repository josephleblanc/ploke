# RAG module test plan

Scope: tests cover the observable behavior of functions in `rag.rs` as seen here. We validate:
- Emitted AppEvents (System events, chat SysInfo messages).
- Returned results where applicable (pure helpers).
- State changes to AppState fields touched explicitly by these functions (e.g., proposals map).

Out of scope:
- Internal behavior of external subsystems (Database, RagService, IoManager, LLM manager).
- UI rendering; we only assert on events/messages produced.

## Harness considerations

- Construct a minimal `Arc<AppState>` with in-memory/mocked fields:
  - Use a real `EventBus` if available in tests; otherwise, a lightweight stub that records sent events on `realtime_tx` and `background_tx`.
  - Initialize `state.proposals` to an empty map; set `state.rag` to `None` or a mocked service depending on the test.
- Where file IO is needed (e.g., get_file_metadata_tool), create temp files in the test directory and clean up after.

## Unit tests

- utils::calc_top_k_for_budget
  - Cases: small budget (< 1000), large budget, ensure clamp to [5, 20].
  - Property: monotonic non-decreasing until clamp limit.

- tools::get_file_metadata_tool
  - Happy path: temp file exists -> ToolCallCompleted with ok:true, file_path, exists:true, byte_len>0, tracking_hash string.
  - Missing file: expect ToolCallFailed with informative error.
  - Argument validation: missing file_path -> ToolCallFailed.

- tools::apply_code_edit_tool
  - Validation:
    - Empty edits -> ToolCallFailed and SysInfo message.
    - Unsupported node_type -> ToolCallFailed.
    - Invalid canon (missing item) -> ToolCallFailed.
  - Idempotency:
    - Duplicate request_id in `state.proposals` -> ToolCallFailed and SysInfo message.
  - DB resolution:
    - No match / multiple matches -> ToolCallFailed with clear message.
  - Preview:
    - With a single edit, verify a proposal is inserted into `state.proposals` and a SysInfo preview message is emitted.
    - If `auto_confirm_edits` is enabled in config, verify `approve_edits` is spawned (can assert eventual ToolCallCompleted or rely on a mock IoManager to intercept calls).

- tools::handle_request_context
  - Validation: missing/zero token_budget -> ToolCallFailed.
  - No hint and no last user message -> ToolCallFailed.
  - With mocked RagService:
    - Success -> ToolCallCompleted with results array length <= top_k.
    - Failure -> ToolCallFailed with message.

- dispatcher::handle_tool_call_requested
  - Unsupported tool name -> ToolCallFailed.
  - Pass-through to each supported tool (apply_code_edit, get_file_metadata, request_code_context) -> verify expected subordinate behavior is triggered.

- editing::approve_edits
  - Pending proposal with multiple edits:
    - Mock IoManager::write_snippets_batch to return Ok for some edits and Err for others.
    - Verify ToolCallCompleted contains per-file results, proposal.status becomes Applied, and SysInfo message acknowledges application.
  - Already Applied / Denied -> SysInfo message and no changes.

- editing::deny_edits
  - Pending/Approved/Failed -> status becomes Denied, ToolCallFailed is emitted with denial message, SysInfo message confirms denial.
  - Already Denied -> SysInfo message only.

- context::process_with_rag
  - With a completion signal that triggers early break -> no work performed.
  - With mocked RagService::get_context returning an AssembledContext:
    - EventBus receives an Llm PromptConstructed event.
  - Missing rag service -> no panic; function returns after no-op.

- context::construct_context_from_rag
  - Ensure PROMPT_HEADER and PROMPT_CODE are present in the constructed prompt.
  - Verify only User and Assistant messages are forwarded from the conversation.

- search::{bm25_rebuild,bm25_status,bm25_save,bm25_load,bm25_search,hybrid_search,sparse_search,dense_search}
  - With rag service present:
    - Success paths -> SysInfo messages contain the expected headers and result formatting.
    - Failure paths -> SysInfo messages report the error.
  - Without rag service:
    - SysInfo message indicates unavailability; no panic.

## Integration smoke tests

- Set up an in-memory Database and initialize schema.
- Start a minimal EventBus and AppState with RagService None.
- Exercise dispatcher::handle_tool_call_requested with each tool name to ensure no panics and appropriate events are produced.

## Notes

- Prefer deterministic assertions (counts, presence of fields) over full string equality where content includes timestamps or UUIDs.
- Use timeouts on async tests that await events.
- Keep tests hermetic; avoid relying on external environment variables or file paths except for controlled temp directories.
