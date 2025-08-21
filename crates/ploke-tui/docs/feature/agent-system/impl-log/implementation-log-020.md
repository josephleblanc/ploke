# Implementation log 020 — Tool-aware prompt rewrite to enable live tool calls (2025-08-21)

Summary
- Rewrote PROMPT_CODE in handlers/rag.rs to explicitly instruct the LLM on:
  - When and how to call request_code_context to fetch additional code.
  - How to safely construct apply_code_edit payloads (non-overlapping ranges, descending order, expected_file_hash requirement).
  - Using file metadata (file hash) from snippet headers and deferring edits until metadata is available.
- Goal: Increase the likelihood of correct, provider-accepted tool calls and reduce failed edit attempts.

Changes
- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Deleted legacy snippet-tag guidance; added structured, tool-aware instructions and examples.

Rationale
- Previous prompt focused on how snippets were displayed but did not teach the model to use tools effectively.
- Providers reject malformed tool payloads; making instructions explicit reduces invalid calls and guides the model to request missing metadata before editing.

Files to keep vs. not needed for immediate next changes

Keep (actively used or needed next)
- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Contains PROMPT_CODE and tool-call handler; we changed PROMPT_CODE here.
- crates/ploke-tui/src/llm/mod.rs
  - Defines tool schemas (request_code_context, apply_code_edit), request flow, and event wiring.
- crates/ploke-tui/src/llm/session.rs
  - Executes requests and orchestrates tool-call cycles.
- crates/ploke-tui/src/app_state/dispatcher.rs
  - Applies state updates and routes tool events; needed to observe tool-call outcomes.
- crates/ploke-tui/src/app_state/commands.rs
  - StateCommand definitions (Approve/Deny edit commands included).
- crates/ploke-tui/src/observability.rs
  - Persists tool-call lifecycle; useful for debugging live calls.
- crates/ploke-tui/src/user_config.rs
  - Provider registry and defaults; relevant if we tweak model/provider selection flow next.
- crates/ploke-tui/src/app/commands/{parser.rs,exec.rs}
  - Command entry points and async model search UX; not changed here but used in normal operation.
- crates/ploke-tui/Cargo.toml
  - Build and dev-deps (e.g., insta); keep as-is.

Not needed for the immediate step (reference only)
- crates/ploke-tui/docs/feature/agent-system/agentic_system_plan.md
  - Long-term roadmap; not required to validate prompt/tool calls.
- crates/ploke-tui/docs/feature/agent-system/impl-log/implementation-log-018.md
  - Historical log; no action needed.
- crates/ploke-tui/docs/feature/agent-system/impl-log/implementation-log-019.md
  - Verification notes; informational only for this step.
- crates/ploke-tui/docs/feature/agent-system/observability_guide.md
  - DB queries and logs reference; useful later during in-depth validation.
- docs/bugs/tui_ux_issues.md
  - UX tracking log; keep for later UI work, not needed to validate prompt/tool calls.
- docs/openrouter/tool_ready_selection.md
  - Model/provider selection background; unrelated to prompt/tool-call text.
- docs/planning/model_provider_selection_mvp.md
  - Overlay planning; not part of this change.
- docs/testing/llm_request_snapshot_harness.md
  - Test harness doc; will matter when we add new snapshots for tool-body payloads.
- docs/testing/snapshot_tests_plan.md
  - Rendering tests plan; not required here.

Next steps
- Validate end-to-end tool-call loop against a provider that supports tools (e.g., OpenRouter with a tool-capable model).
- If apply_code_edit fails due to missing expected_file_hash, iterate:
  - Enhance context assembly to include file hash metadata for shown snippets, or
  - Add a lightweight tool to fetch file metadata by path.
- Add a minimal snapshot/test asserting tool-call JSON payload structure for both tools.

References
- Live tool-call flow: llm/session.rs (execute_tool_calls and await_tool_result).
- Tool schemas: llm/mod.rs (request_code_context, apply_code_edit).
