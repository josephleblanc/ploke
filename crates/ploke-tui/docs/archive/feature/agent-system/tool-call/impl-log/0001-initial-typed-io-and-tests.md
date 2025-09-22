# Impl Log 0001 — Initial typed IO and tests

Date: 2025-08-24

Summary
- Switched LLM history token budgeting to use a real TokenCounter (ApproxCharTokenizer) from ploke-rag instead of an ad-hoc ceil(chars/4).
- Added integration tests for typed tool IO serde round-trips: RequestCodeContextArgs/Result, GetFileMetadataResult, ApplyCodeEditResult.
- Set up an implementation log location to track decisions, next steps, and blockers.
- No external payload shape changes (OpenAI chat/completions); tools remain serialized as JSON strings only at the provider boundary.

Rationale
- Eliminate stringly-typed patterns across crate boundaries. Maintain typed structs until the final serialization step for API calls and persisted observability.
- Align budgeting with the shared TokenCounter abstraction to make future tokenizer swaps trivial (plug a model-specific counter later).

Changes
- ploke-tui/src/llm/mod.rs
  - cap_messages_by_tokens now uses ploke_rag::context::ApproxCharTokenizer.
  - Added a minimal import of ApproxCharTokenizer.
- ploke-tui/tests/tool_io_roundtrip.rs
  - New integration tests covering serde round-trips for our typed tool IO.

Open questions (non-blocking)
- Should we expose a configurable TokenCounter at runtime (e.g., via LLMParameters) and/or wire it to model metadata? For now, ApproxCharTokenizer is sufficient.
- When we add a DB helper in ploke-db for apply_code_edit canonical lookups, which path signature do we stabilize on? Likely (node_type, canon, file_path, NOW) with clear error types.

Planned next steps
- Add a parameterized DB helper in ploke-db so apply_code_edit_tool can drop the inlined CozoScript (NOW snapshot, typed return).
- Extend observability to persist provider_slug/model on tool lifecycle events. Requires ploke-db updates.
- Add a small toggle in provider configuration for a configurable tool fallback policy (currently implements one-shot retry w/o tools).
- Add e2e tests that run a full tool cycle against a real endpoint with a pre-loaded DB.

Files included (bare file-path list)
- crates/ploke-tui/src/llm/mod.rs
- crates/ploke-tui/src/llm/session.rs
- crates/ploke-tui/src/llm/tool_call.rs
- crates/ploke-tui/src/rag/tools.rs
- crates/ploke-tui/src/rag/editing.rs
- crates/ploke-tui/src/rag/dispatcher.rs
- crates/ploke-tui/src/observability.rs
- crates/ploke-tui/src/tracing_setup.rs
- crates/ploke-tui/src/test_harness.rs
- crates/ploke-io/src/handle.rs
- crates/ploke-core/src/io_types.rs
- crates/ploke-core/src/rag_types.rs
- crates/ploke-rag/src/core/mod.rs
- crates/ploke-rag/src/context/mod.rs
- crates/ploke-tui/docs/reports/tool_call_flow.md
- crates/ploke-tui/docs/reports/tool_call_reflection.md
- crates/ploke-tui/docs/reports/tool_call_next_steps.md
- crates/ploke-tui/docs/reports/tool_io_implementation_notes.md
- crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs
- crates/ploke-tui/Cargo.toml

Candidate files removable from active context (keep summaries handy)
- crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs
  - Why: Focused on live smoke tests using OpenRouter with local tool execution; not required for current typed IO refactor tasks.
  - What: Fetches user models/endpoints, selects tools-capable endpoints, performs two-leg tool calls with local tool execution.
  - When to reference: Live regression checks for tool plumbing against real endpoints.
- crates/ploke-io/src/handle.rs
  - Why: API is stable and well-documented; current tasks only consume it indirectly via rag/tools. No immediate changes needed.
  - What: Actor-backed file IO interface (read snippets, write snippets, scan changes).
  - When to reference: When expanding editing behaviors or adding reranker IO-reads inside ploke-rag.

Files required to be added (to remove blockers)
- ploke-db helper for canonical lookups (please add to chat to proceed)
  - Suggested paths:
    - crates/ploke-db/src/query/helpers.rs (new) — parameterized helper to resolve (node_type, canon, file_path) at NOW into EmbeddingData-like rows.
    - crates/ploke-db/src/lib.rs (mod wiring) — expose the helper publicly.
  - Rationale: Replace inlined CozoScript in apply_code_edit_tool with a typed, parameterized function that returns stable shapes.

Reflections
- Typed IO across crate boundaries has improved ergonomics already; tests are straightforward.
- Using TokenCounter from ploke-rag avoids duplicating logic and aligns with context assembly behavior.
- The next complexity lies in DB/API integration; keeping helpers in ploke-db will reduce error-prone string queries.

Next log guidance
- Keep only the last two impl logs in this directory for day-to-day context. Older logs can be archived if needed.
