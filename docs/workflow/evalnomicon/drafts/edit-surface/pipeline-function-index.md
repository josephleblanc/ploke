# Edit Pipeline Function Index

Status: search index for current source. Re-run the commands before relying on
line numbers.

## Fast Discovery Command

```bash
rg -n "ToolLoopMode|should_wait_for_settled_edit|execute_tools_via_event_bus|ToolCallCompleted|ToolCallFailed|apply_ns_code_edit_tool|stage_semantic_edit_proposal|approve_edits|approve_pending_edits|deny_edits|rescan_for_changes|scan_for_change|settle_staged_batch|wait_for_selected|wait_for_refresh|proposal_from_resolved_writes|Apply::from_results|validate_edit_surface_candidate" crates/ploke-tui/src crates/ploke-eval/src
```

## TUI Tool Loop

- `crates/ploke-tui/src/llm/manager/session.rs`
  - `execute_tools_via_event_bus`
  - `should_wait_for_settled_edit`
  - `is_pending_edit_payload`
- `crates/ploke-tui/src/user_config.rs`
  - `ToolLoopMode`
  - `ChatPolicy`

These functions decide whether a staged edit tool result may advance the model
loop. Prototype 1 should use `ToolLoopMode::Gated`.

## TUI Tool Staging

- `crates/ploke-tui/src/tools/mod.rs`
  - `process_tool`
- `crates/ploke-tui/src/tools/ns_patch.rs`
  - `NsPatch::execute`
  - `NsPatch::adapt_error`
- `crates/ploke-tui/src/rag/tools.rs`
  - `stage_semantic_edit_proposal`
  - `apply_ns_code_edit_tool`
  - same-file fuzzy stale-anchor guard near `file_has_*proposal`
- `crates/ploke-tui/src/tools/insert_rust_item.rs`
  - `InsertRustItem::execute`

These functions validate tool arguments, create pending edit proposals, emit
staging results, and optionally auto-confirm.

## TUI Approval And Refresh

- `crates/ploke-tui/src/rag/editing.rs`
  - `spawn_auto_confirm_edits`
  - `approve_edits`
  - `apply_ns_edit`
  - `apply_semantic_edit`
  - `approve_pending_edits`
  - `deny_edits`
  - `rescan_for_changes`
- `crates/ploke-tui/src/app_state/handlers/db.rs`
  - `scan_for_change`
- `crates/ploke-tui/src/app_state/database.rs`
  - database-side `scan_for_change`

These functions settle proposals and refresh workspace state after mutation.

## Prototype 1 Headless Admission

- `crates/ploke-eval/src/runner.rs`
  - `configure_sparse_strict_rag`
  - `setup_workspace_tui_runtime`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  - `run_attempt`
  - `settle_staged_batch`
  - `select_disjoint`
  - `reject_item`
  - `approve_selected`
  - `wait_for_selected`
  - `wait_for_refresh`
  - `HeadlessRun`
  - `HeadlessAttemptResult`
  - `HeadlessTerminal`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`
  - `Apply::from_results`
  - `Apply::validate`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
  - `proposal_from_resolved_writes`
  - `validate_edit_surface_candidate`

These functions bridge TUI proposal settlement into Prototype 1 edit-surface
admission and candidate evidence.

## Durable Design References

- [`model.md`](model.md)
  Edit-surface grants, proposals, and authority boundaries.
- [`harness-adapter-plan.md`](harness-adapter-plan.md)
  Bounded edit harness adapter plan.
- [`tui-approve-deny-pipeline.md`](tui-approve-deny-pipeline.md)
  Intended approve/deny, refresh, and admission flow.
