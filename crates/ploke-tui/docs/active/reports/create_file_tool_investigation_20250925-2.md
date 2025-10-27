# Create-file Tool: Why approved creations don’t produce files (investigation)

Date: 2025-09-25
Author: Codex CLI Assistant

## Summary
- Symptom: After staging a create-file proposal and approving it in the UI, the chat shows “Applied edits for request_id …” and a rescan occurs, but the new file does not appear on disk.
- Root cause: The approvals UI and command dispatcher only handle edit proposals (`state.proposals`) and route Enter to `ApproveEdits`. Create-file proposals live in `state.create_proposals` and require `approve_creations`, but there is no UI path or `StateCommand` wired to call it. Pressing Enter therefore approves (some) edit proposals, not the staged file creation.
- IO path is implemented and tested; the missing piece is UI/command integration for create approvals.

## Reproduction (as reported)
1) `/load crate fixture_nodes`
2) Ask the LLM to create a file with the tool-capable model (kimi-k2)
3) Observe SysInfo preview of the staged creation
4) Open proposals with `e`, press Enter to approve
5) Chat shows: “Applied edits for request_id …”; a rescan is scheduled
6) The new file is absent on disk

This exact text (“Applied edits…”) matches the edit-approval flow, not the creation flow.

## What staging does today
- Tool staging for create-file:
  - crates/ploke-tui/src/rag/tools.rs: `create_file_tool(...)` inserts a `CreateProposal` in `state.create_proposals`, emits a SysInfo preview, and a typed `CreateFileResult` (staged/applied counts).
  - When `auto_confirm_edits` is true, it spawns `rag::editing::approve_creations(...)`.
- Proposal type and store:
  - crates/ploke-tui/src/app_state/core.rs: `CreateProposal` type and `AppState::create_proposals` store.
  - crates/ploke-tui/src/app_state/handlers/proposals.rs: `save_create_proposals`/`load_create_proposals` implemented.

## What approval the UI actually triggers
- Approvals overlay renders only edit proposals (code edits):
  - crates/ploke-tui/src/app/view/components/approvals.rs
    - Reads `state.proposals` (not `create_proposals`) to list items
- Key handling in overlay:
  - crates/ploke-tui/src/app/mod.rs → `handle_overlay_key`
    - On Enter, sends `StateCommand::ApproveEdits { request_id }`
- State command dispatcher:
  - crates/ploke-tui/src/app_state/dispatcher.rs
    - Handles `ApproveEdits` by calling `rag::editing::approve_edits(...)`
- There is no `StateCommand::ApproveCreations`/`DenyCreations`, and no overlay or key handler for create proposals. CLI commands `create approve|deny` are also not implemented.

Result: approving in the overlay applies edits (if any), not file creations. The chat string that appears on approval (“Applied edits for request_id …”) is emitted by `approve_edits` (crates/ploke-tui/src/rag/editing.rs) confirming the wrong path was exercised.

## IO path health check
- ploke-io implementation:
  - crates/ploke-io/src/handle.rs: `IoManagerHandle::create_file(request)` sends `IoRequest::CreateFile`.
  - crates/ploke-io/src/actor.rs: handles `IoRequest::CreateFile` → `crate::create::create_file(...)` and (feature-gated) emits a Created event.
  - crates/ploke-io/src/create.rs: performs absolute/roots normalization, `.rs` enforcement, optional parent creation, atomic temp-write + fsync + rename, and returns `CreateFileResult { new_file_hash }`.
- Unit tests:
  - crates/ploke-io/src/write_tests.rs: `test_create_file_success_and_hash` and `test_create_file_exists_error_policy` pass, indicating IO path works in isolation.

Note: create.rs begins with a bare text header and a trailing `*/`. Despite this oddity, the module compiles (likely an overlooked comment formatting; worth cleaning up separately).

## Why the file doesn’t appear
- The UI never calls `approve_creations`, so no `IoManagerHandle::create_file` requests are sent for the staged proposal when you press Enter in the overlay. Any “Applied edits…” message corresponds to edit proposals, not the file creation.

## Recommended fixes
1) Add creation approval commands:
   - `StateCommand::ApproveCreations { request_id }` and `StateCommand::DenyCreations { request_id }`
   - Dispatcher: call `rag::editing::approve_creations` and a new `deny_creations` (simple status update + persistence + ToolCallFailed bridge)
   - CLI: parser support for `create approve <request_id>` and `create deny <request_id>`
2) Update approvals UI to include creates:
   - Option A (unified): show a combined list with type badges (Edit/Create) and route Enter to the appropriate command based on the selected item’s type.
   - Option B (separate tabs): add a toggle key to switch between Edit and Create proposals; keep behavior consistent with current overlay.
   - When a create is applied, the chat should show “Applied file creations for request_id …” (as implemented in `approve_creations`).
3) E2E test (offline):
   - Tool call → stage → manual approve → verify file exists and rescan scheduled.
4) Nice-to-have: add a small SysInfo clarifier after pressing Enter if the overlay view contains only edits (e.g., “No create proposals in this view; try ‘create approve <id>’”).

## References (key locations)
- Staging: crates/ploke-tui/src/rag/tools.rs → `create_file_tool`
- Proposal store: crates/ploke-tui/src/app_state/core.rs → `CreateProposal`, `AppState::create_proposals`
- Approval (missing integration):
  - UI list: crates/ploke-tui/src/app/view/components/approvals.rs (reads `state.proposals` only)
  - Overlay Enter handler: crates/ploke-tui/src/app/mod.rs → sends `ApproveEdits`
  - Dispatcher: crates/ploke-tui/src/app_state/dispatcher.rs → `ApproveEdits` → `rag::editing::approve_edits`
- Creation apply path (exists but unused by UI): crates/ploke-tui/src/rag/editing.rs → `approve_creations`
- IO wiring: crates/ploke-io/src/handle.rs, crates/ploke-io/src/actor.rs, crates/ploke-io/src/create.rs
- IO tests: crates/ploke-io/src/write_tests.rs

## Conclusion
The IO stack and tool staging are in place, but the UI/command layer lacks a route to apply create-file proposals. Enter in the approvals overlay approves edits, not creations, which explains seeing “Applied edits…” with no new file on disk. Implementing `ApproveCreations`/`DenyCreations` and exposing create proposals in the overlay (or via CLI) will resolve the issue.
