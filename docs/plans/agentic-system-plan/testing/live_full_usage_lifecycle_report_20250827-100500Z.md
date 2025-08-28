Live Tool Lifecycle — Full Usage Test Report

Summary
- Goal: Validate the end-to-end, production-ready tool-using workflow on a live tools-capable endpoint.
- Model: `moonshotai/kimi-k2` via OpenRouter, provider pinned to a tools-capable slug discovered at runtime.
- Gating: Requires `OPENROUTER_API_KEY` and `PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1`.
- Tests: Two live tests implemented under `crates/ploke-tui/tests/`.
  - `openrouter_live_fixture_code_edit.rs` — single-call apply_code_edit on fixture file with explicit approval.
  - `openrouter_live_full_usage_lifecycle.rs` — user message → request_code_context → apply_code_edit → approval → applied.

Files And Characteristics Under Test
- LLM lifecycle and tools
  - `crates/ploke-tui/src/llm/session.rs` — request loop, tool_calls detection, OpenAI schema handling, retry policy.
  - `crates/ploke-tui/src/llm/tool_call.rs` — Typed `ToolEvent::Requested`, dispatch, correlation, timeout.
  - `crates/ploke-tui/src/llm/openrouter_catalog.rs` — Endpoints fetch, supported_parameters("tools") detection.
- Tool routing and handlers
  - `crates/ploke-tui/src/rag/dispatcher.rs` — routes tool name to handler, logs structured `handle_tool_call_requested` entry.
  - `crates/ploke-tui/src/rag/tools.rs` — `apply_code_edit` staging; proposal registry updates; preview metadata; auto-approve off by config.
- Approvals and applying edits
  - `crates/ploke-tui/src/rag/editing.rs` — `approve_edits` applies via IoManager, emits ToolCallCompleted/Failed, schedules rescan.
  - `crates/ploke-io` — `IoManagerHandle::write_snippets_batch` atomic writes + hash validation (indirectly exercised).
  - `crates/ploke-tui/src/app_state/core.rs` — `EditProposal` staging and status transitions.

Properties Validated
- Tools-capable endpoint selection
  - Picks a provider slug for `moonshotai/kimi-k2` with `supported_parameters` containing `"tools"` before sending chat/completions.
- Typed tool lifecycle
  - Observes `AppEvent::LlmTool(ToolEvent::Requested { name, request_id, call_id, .. })` for both `request_code_context` and `apply_code_edit` (full test) and for `apply_code_edit` (single test).
  - Correlates request IDs to staged `EditProposal` entries (staged before approval).
- Proposal staging and approval
  - Ensures a Pending proposal exists with correct `files` list matching the target path.
  - Calls `approve_edits` and asserts `ToolCallCompleted` emission and file content change post-apply.
- Hash safety
  - Supplies `expected_file_hash` computed via `TrackingHash::generate(PROJECT_NAMESPACE_UUID, path, tokens)`; edits only apply when hash matches.
- End-to-end integrity
  - No reliance on deprecated `SystemEvent::ToolCallRequested` path; tests require actual provider `tool_calls`.
  - Post-apply: implicit rescan scheduled (observability is present in code; the test focuses on correctness and file-level effects).

Why This Validates Production Readiness
- Verifies real OpenRouter tool-calling with a tools-capable endpoint (no mocks/fakes).
- Exercises strong typing across request/response, tool dispatch, and approval paths—invalid states are rejected.
- Confirms staged-then-approve workflow, aligned with safety-first editing; detects any break in proposal registry or IoManager writes.
- Tests drift protection via file hash; accidental/misaligned edits are prevented by design.
- Ensures the LLM → tools → approvals loop functions as designed without bypasses.

How To Run
- Environment:
  - `export OPENROUTER_API_KEY=...`
  - `export PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1`
- Command:
  - `cargo test -p ploke-tui --test openrouter_live_fixture_code_edit -- --nocapture`
  - `cargo test -p ploke-tui --test openrouter_live_full_usage_lifecycle -- --nocapture`

