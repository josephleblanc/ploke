# Impl Log 0002 — DB helper for canonical lookups; tool refactor to typed path

Date: 2025-08-24

Summary
- Added ploke-db helpers module with resolve_nodes_by_canon_in_file to encapsulate NOW-snapshot CozoScript and return typed EmbeddingData.
- Refactored apply_code_edit_tool to use the helper, removing inline query assembly from the tool layer.
- Maintained strongly typed IO across tool boundaries; no stringly types cross crate boundaries except at provider API edges.
- Kept existing behavior for edit staging, previews, and auto-approval.

Rationale
- Centralize DB query logic and shape translation to reduce ad hoc scripts in tool handlers.
- Improve safety by JSON-escaping literals in one place and reusing ploke-db Result adapters.
- Progress toward parameterized queries while preserving current DB API.

Changes
- New: crates/ploke-db/src/helpers.rs with resolve_nodes_by_canon_in_file(&Database, relation, file_path, module_path, item_name) -> Result<Vec<EmbeddingData>, DbError>.
- Exported helpers via ploke-db/src/lib.rs.
- Updated crates/ploke-tui/src/rag/tools.rs to call the helper and handle 0/1/ambiguous cases as before.

Open questions
- Parameterization: Cozo supports parameters in prepared statements; evaluate feasibility to remove string formatting entirely.
- Error taxonomy: Map result::to_embedding_nodes errors into a richer DbError variant instead of DbError::Cozo(String).

Next steps
- Observability: persist provider_slug/model on tool lifecycle events (extend ToolRequest/Done params and ploke-db store).
- Configurable tools-only policy: Toggle strictness for 404 “tool unsupported” endpoints.
- E2E tests against OpenRouter with a pre-loaded DB; rely on test harness for realistic App state.

Files included (bare file-path list)
- crates/ploke-db/src/helpers.rs
- crates/ploke-db/src/lib.rs
- crates/ploke-tui/src/rag/tools.rs
- crates/ploke-tui/src/llm/session.rs
- crates/ploke-tui/src/llm/mod.rs
- crates/ploke-tui/src/observability.rs
- crates/ploke-tui/src/rag/dispatcher.rs
- crates/ploke-tui/src/rag/editing.rs
- crates/ploke-tui/docs/reports/tool_call_next_steps.md
- crates/ploke-tui/docs/reports/tool_call_reflection.md
- crates/ploke-tui/docs/reports/tool_call_flow.md
- crates/ploke-tui/tests/tool_io_roundtrip.rs
- crates/ploke-core/src/rag_types.rs
- crates/ploke-core/src/io_types.rs
- crates/ploke-rag/src/core/mod.rs
- crates/ploke-rag/src/context/mod.rs

Candidate files removable from active context (keep summaries handy)
- crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs
  - Why: Networked smoke tests; not required for the DB helper refactor.
  - What: Live two-leg tool round-trips against OpenRouter; local tool execution.
  - When to reference: During e2e validation or provider troubleshooting.
- crates/ploke-tui/src/tracing_setup.rs
  - Why: Stable tracing setup; no changes needed for current tasks.
  - What: Rolling file logger + console; env-based filters.
  - When to reference: When diagnosing test runs or toggling console logs in CI/local.

Files that must be added for edits to unblock next steps
- Observability store fields in ploke-db (additions to record model/provider_slug and error categories)
  - Please add to chat:
    - crates/ploke-db/src/observability/mod.rs (or relevant files) if you want me to wire new fields end-to-end.
- Provider config/registry toggles for tools-only policy
  - Please add to chat:
    - crates/ploke-tui/src/user_config.rs (or equivalent) to introduce a config knob and plumb through.

Reflections
- Moving query construction out of tools keeps the handler focused on assembly and UX, not DB plumbing.
- The helper returns typed EmbeddingData, aligning perfectly with IO write staging and context assembly flows.
- As we add more helpers (e.g., by canon only, by node id, by path template), we should document a small cookbook in ploke-db.

Rolling window note
- Keep only the latest two impl logs in this directory for day-to-day context; archive older logs as needed.
