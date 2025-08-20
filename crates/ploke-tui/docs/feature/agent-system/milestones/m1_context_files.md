# Milestone 1 — Context files checklist (Human-in-the-loop editing)

Core TUI integration
- crates/ploke-tui/src/llm/mod.rs
  - Tool schema for apply_code_edit already defined; keep active.
- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Extend handle_tool_call_requested: diff previews, approval gate reading Config.editing.*; emit pending/approved/denied outcomes.
- crates/ploke-tui/src/app_state/commands.rs
  - Add ApproveEdits { request_id }, DenyEdits { request_id }.
- crates/ploke-tui/src/app_state/dispatcher.rs
  - Route ApproveEdits/DenyEdits to rag handler.
- crates/ploke-tui/src/app/commands/parser.rs, src/app/commands/exec.rs
  - Parse "edit approve <request_id>", "edit deny <request_id>" and dispatch StateCommands.

IO and types
- crates/ploke-io (external crate): IoManagerHandle::write_snippets_batch; absolute-path policy enforced.
- crates/ploke-core/src/io_types.rs: WriteSnippetData, TrackingHash.

DB side (tracked separately)
- ploke-db: code_edit_proposal relation with time-travel; persistence of proposals and outcomes.
- See: crates/ploke-tui/docs/feature/agent-system/ploke_db_requests.md

Tests to add
- Mapping JSON → WriteSnippetData (pure function).
- End-to-end: temp file, apply_code_edit, expect ToolCallCompleted ok:true; validate file content and new hash.
