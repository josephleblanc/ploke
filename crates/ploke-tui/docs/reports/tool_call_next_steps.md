# Tool Call Next Steps and File Reference

This document enumerates prioritized steps to mature the tool-call system and provides a concise file/function reference to reduce context switching during implementation.

## Next steps (actionable plan)

1) Typed IO round-trip tests
- Goal: Ensure inputs/outputs for tools serialize/deserialize cleanly and remain backward compatible.
- Files to touch:
  - crates/ploke-core/src/rag_types.rs (add optional version fields if desired)
  - crates/ploke-tui/src/llm/session.rs (ensure RequestMessage::new_tool only receives serialized typed results)
  - New tests under crates/ploke-tui (unit tests for serde round-trips)

2) Configurable 404 “tool unsupported” fallback
- Goal: Make the retry-without-tools policy driven by config.
- Files to touch:
  - crates/ploke-tui/src/llm/session.rs (toggle retry behavior and system note based on config)
  - crates/ploke-tui/src/llm/mod.rs (ensure provider registry flags are respected)
  - user config code (ploke-tui user_config) to add a boolean knob (tools_only or allow_no_tool_fallback)

3) Parameterized DB helper for apply_code_edit
- Goal: Remove inlined CozoScript; add a safe helper in ploke-db to resolve canon/path to EmbeddingData-like records at NOW.
- Files to touch:
  - ploke-db (new module or function to fetch nodes by (node_type, canon, file_path) with NOW)
  - crates/ploke-tui/src/rag/tools.rs (call the helper; simplify assembly of WriteSnippetData)
  - crates/ploke-core/src/io_types.rs (verify the shape still matches what the DB returns)

4) Token budgeting improvements for history
- Goal: Use a TokenCounter abstraction for history budgeting; retain char fallback.
- Files to touch:
  - crates/ploke-tui/src/llm/mod.rs (cap_messages_by_tokens to optionally take a token counter)
  - crates/ploke-tui/src/llm/session.rs (inject/use token counter; keep char fallback)

5) Observability enhancements
- Goal: Persist model, provider_slug, error categories; confirm latency calc path is robust.
- Files to touch:
  - crates/ploke-tui/src/observability.rs (extend ToolRequestPersistParams/ToolDonePersistParams and DB adapters)
  - ploke-db observability store (persist new fields)

6) E2E tool-cycle tests (networked)
- Goal: Validate full loop against a real OpenRouter endpoint with forced tool calls (success, failure, timeout).
- Files to reference:
  - crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs (pattern for two-leg tool tests)
  - crates/ploke-tui/src/test_harness.rs (App initialization without UI)
  - crates/ploke-tui/src/tracing_setup.rs (turn console logging on/off for CI and local runs)

7) Apply_code_edit validation and UX
- Goal: Add stricter file path validation, namespace handling, and idempotency checks; ensure preview and auto-approval paths are robust.
- Files to touch:
  - crates/ploke-tui/src/rag/tools.rs (validation, path normalization, improved preview diffs)
  - crates/ploke-io/src/handle.rs (consider exposing more granular errors, if needed)

8) Documentation refresh
- Goal: Keep docs accurate as behavior evolves (tools fallback, typed IO, budgeting).
- Files to update:
  - crates/ploke-tui/docs/reports/tool_call_flow.md
  - crates/ploke-tui/docs/reports/tool_call_reflection.md (this doc)
  - crates/ploke-tui/docs/reports/tool_call_next_steps.md (this doc)

## Quick file reference

- crates/ploke-tui/src/llm/mod.rs
  - get_file_metadata_tool_def, request_code_context_tool_def, apply_code_edit_tool_def
    - JSON schemas for tools in request payloads.
  - OpenAiRequest, RequestMessage
    - Types for building provider requests.
  - llm_manager, process_llm_request, prepare_and_run_llm_call
    - Core event loop, spawning per-request sessions and wiring messages/tools.

- crates/ploke-tui/src/llm/session.rs
  - RequestSession::run
    - The per-request loop: builds payloads, dispatches tool calls, appends tool results, enforces history budgets.
  - build_openai_request
    - Stable request payload construction; includes provider.order when tools are active.
  - await_tool_result
    - Correlates tool outcomes on broadcast channel by (request_id, call_id) with timeout.

- crates/ploke-tui/src/llm/tool_call.rs
  - ToolCallSpec, dispatch_and_wait, execute_tool_calls
    - Concurrent tool dispatch and deterministic outcome ordering.

- crates/ploke-tui/src/rag/dispatcher.rs
  - handle_tool_call_requested
    - Routes tool requests by name to rag::tools handlers; logs unsupported tools.

- crates/ploke-tui/src/rag/tools.rs
  - handle_request_context
    - Parses RequestCodeContextArgs, calls RagService::get_context, returns typed result JSON.
  - get_file_metadata_tool
    - Computes file hash/metadata and returns typed GetFileMetadataResult JSON.
  - apply_code_edit_tool
    - Stages edits as WriteSnippetData, builds preview, persists proposal, emits typed ApplyCodeEditResult; supports auto-approval.

- crates/ploke-tui/src/rag/editing.rs
  - approve_edits, deny_edits
    - Applies or denies staged edit proposals; bridges outcomes back to ToolEvent/SystemEvent.

- crates/ploke-tui/src/observability.rs
  - run_observability, handle_event
    - Persists conversation turns and tool lifecycle; computes latency and stores args digests.

- crates/ploke-io/src/handle.rs
  - IoManagerHandle API
    - get_snippets_batch, read_full_verified, scan_changes_batch, write_snippets_batch.
    - Actor-backed, atomic writes (tempfile + fsync + rename).

- crates/ploke-rag/src/core/mod.rs
  - RagService, RetrievalStrategy
    - search_bm25, search, hybrid_search, get_context; configurable with RagConfig.

- crates/ploke-rag/src/context/mod.rs
  - assemble_context
    - Budgeting, ordering, trimming, dedup; returns AssembledContext.
  - TokenBudget, ApproxCharTokenizer, AssemblyPolicy
    - Controls for context assembly and token approximation.

- crates/ploke-core/src/io_types.rs
  - EmbeddingData, WriteSnippetData, WriteResult
    - IO-related types shared across crates.

- crates/ploke-core/src/rag_types.rs
  - ContextPart, AssembledContext, RequestCodeContextArgs/Result, GetFileMetadataResult, ApplyCodeEditResult
    - Typed payloads for RAG tool IO.

- crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs
  - Live smoke tests for tool round-trips against OpenRouter endpoints; useful reference for staged calls.

- crates/ploke-tui/src/test_harness.rs
  - TEST_APP lazy static
    - Builds a realistic App state (DB, IO, embedder, event bus) without spawning UI loops.

- crates/ploke-tui/src/tracing_setup.rs
  - init_tracing
    - File + console log setup; useful for test diagnostics with rolling logs.

When to reference:
- While wiring new tool behavior: see llm/session.rs (loop), llm/tool_call.rs (dispatch), rag/tools.rs (handlers).
- For IO and previews: ploke-io::IoManagerHandle, rag/tools.rs, rag/editing.rs.
- For typed payload shapes: ploke-core::rag_types.
- For end-to-end tests or live round-trips: exec_real_tools_live_tests.rs and test_harness.rs.
