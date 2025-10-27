# Path Scoping for Edits — Implementation Report

Date: 2025-09-26

## Summary

Scoped all LLM-driven code edits and file creations to the loaded crate root. When `crate_focus` is set, relative paths are resolved against that root and absolute paths are only accepted if they are within the root. Attempts to target files outside the root are rejected with a `ToolCallFailed` event. Also fixed the `/load` DB behavior to correctly set the crate root.

## Root Cause

- `load_db` set `system.crate_focus = current_dir().join(crate_root_path)`, even though the DB’s `root_path` is absolute. This produced incorrect resolution bases.
- `apply_code_edit_tool` and `create_file_tool` joined relative paths to `crate_focus` but did not prevent absolute or escaping paths from targeting outside the workspace.

## Changes

- Added helper `resolve_in_crate_root(user_path, crate_root) -> Result<PathBuf, String>`:
  - Normalizes paths component-wise and ensures the result stays within `crate_root`.
  - Accepts absolute paths only if they are contained by `crate_root`.
  - File: `crates/ploke-tui/src/utils/path_scoping.rs`
- Exposed module:
  - File: `crates/ploke-tui/src/utils/mod.rs` adds `pub mod path_scoping;`
- Enforced scoping in tools:
  - File: `crates/ploke-tui/src/rag/tools.rs`
    - `apply_code_edit_tool`: validates both `Splice.file_path` and `Canonical.file` with `resolve_in_crate_root` when `crate_focus` exists; emits `ToolCallFailed` on violation.
    - `create_file_tool`: validates `file_path` with `resolve_in_crate_root` under same rules.
- Fixed crate focus assignment on DB load:
  - File: `crates/ploke-tui/src/app_state/database.rs`
    - Now uses the absolute `crate_root_path` from DB: `system_guard.crate_focus = Some(PathBuf::from(crate_root_path));`

## Tests

- Unit and E2E (targeted-only due to unrelated failures elsewhere):
  - File: `crates/ploke-tui/tests/path_scoping.rs`
    - `resolve_in_crate_root_unit_cases`: relative-inside OK; absolute-inside OK; relative-escape ERR; absolute-outside ERR.
    - `create_file_rejects_outside_root_e2e`: verifies `ToolCallFailed` on outside-root file creation.
  - File: `crates/ploke-tui/tests/path_scoping_apply.rs`
    - `apply_code_edit_rejects_outside_root_e2e`: verifies `ToolCallFailed` on outside-root `apply_code_edit` (splice mode).

### How to run targeted tests

```
cargo test -p ploke-tui --test path_scoping --quiet
cargo test -p ploke-tui --test path_scoping_apply --quiet
```

## Behavior Details

- With `crate_focus` set:
  - Relative paths resolve to `crate_root / relative` (normalized).
  - Absolute paths must be contained by `crate_root`.
  - Violations produce a `ToolCallFailed` event and return early.
- Without `crate_focus`:
  - Legacy behavior retained: relative resolve to `current_dir()`, absolute accepted as-is.

## Follow-ups (Optional)

- IO-level enforcement: configure `IoManagerHandle` with roots and deny cross-root symlinks for defense in depth.
- Additional tests: canonical mode edge cases (e.g., LLMs redundantly including root prefix); load_db test with deterministic backup fixture.
- Observability: emit a clear SysInfo when a path is rejected for being outside the crate root.

## Files Touched

- `crates/ploke-tui/src/utils/path_scoping.rs` (new)
- `crates/ploke-tui/src/utils/mod.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/app_state/database.rs`
- `crates/ploke-tui/tests/path_scoping.rs` (new)
- `crates/ploke-tui/tests/path_scoping_apply.rs` (new)

