# Implementation log 013 — M1 start: staged edit proposals + approve/deny commands (2025-08-20)

Summary
- Added command surface and routing for human-in-the-loop editing:
  - New commands: "edit approve <request_id>", "edit deny <request_id>".
  - StateCommand variants ApproveEdits/DenyEdits with dispatcher wiring.
- Introduced in-memory proposal registry on AppState to stage code edits from apply_code_edit tool calls.
- Updated apply_code_edit handling to STAGE proposals instead of writing immediately; emits a SysInfo summary with instructions to approve/deny.
- Implemented approval/denial flows:
  - Approve: applies edits via IoManagerHandle::write_snippets_batch, updates proposal status, emits ToolCallCompleted and a SysInfo summary.
  - Deny: marks proposal as Denied, emits ToolCallFailed and a SysInfo notice.
- Help text updated with new commands.

Notes
- Diff preview currently produces per-file before/after stubs. Unified diff (via similar crate) will be added in a follow-up.
- Observability DB will see "requested" at tool-call time, and "completed"/"failed" only after approval/denial.

Files touched
- src/app_state/commands.rs: added ApproveEdits/DenyEdits and discriminants.
- src/app_state/core.rs: added proposal types and AppState.proposals store.
- src/app_state/dispatcher.rs: wired approve/deny to handlers.
- src/app_state/handlers/rag.rs: staged proposals, added approve_edits/deny_edits handlers.
- src/app/commands/parser.rs, src/app/commands/exec.rs: parse/execute new commands; updated help.

Follow-ups planned (next PRs)
- Implement full code-block previews and optional unified diff generation.
- Add basic unit tests for command parsing and proposal staging (pure).
- Consider config flags editing.auto_confirm_edits and editing.preview_mode (default codeblock).

Requested access (to continue M1)
- crates/ploke-tui/src/app_state/handlers/chat.rs (for any UX tweaks to SysInfo messages, optional)
- crates/ploke-tui/src/app_state/mod.rs (only if new modules become necessary; currently not required)

No breaking changes
- Existing tool-call routing remains intact; staging path is additive.
