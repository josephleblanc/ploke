# Plan: Scope File Edits to Loaded Crate Root

Context: The TUI app loads a code graph DB for a target crate via `/load crate <name>`. The LLM proposes edits/creations using paths that are relative to that crate’s root (e.g., `src/lib.rs`). When the application is run from a different current working directory, edits can be resolved incorrectly. We want to guarantee that all file edits and creations are applied within the root directory of the crate that has been loaded.

This document contains: codebase research, root cause analysis, proposed changes (with small new types), and a Test-Driven Design (TDD) plan including unit and end-to-end tests. No code changes have been made yet.

---

## Research Summary

Primary components involved:
- `crates/ploke-tui/src/app_state/database.rs` → `load_db(...)`
  - Restores a DB backup and then sets `system.crate_focus` based on the database’s `crate_context`.
  - Current logic:
    - Queries: `?[root_path] := *crate_context {name: crate_name, root_path @ 'NOW' }, crate_name = "{crate_name}"`
    - Sets `system_guard.crate_focus = current_dir().join(crate_root_path)`
- `crates/ingest/ploke-transform/src/schema/crate_node.rs` and `crates/ingest/ploke-transform/src/transform/crate_context.rs`
  - Schema has `root_path: String`, and `transform_crate_context` stores the crate’s absolute `root_path`.
- `crates/ploke-tui/src/rag/tools.rs`
  - `apply_code_edit_tool`: resolves edit paths using `crate_focus` if present; falls back to `current_dir()` if not.
  - `create_file_tool`: resolves creation path using `crate_focus` if present; falls back to `current_dir()` if not.
- `crates/ploke-io` (IO manager):
  - By default, no roots are configured; absolute paths are required if not using roots. If roots are configured, strict normalization/containment checks are enforced.
  - TUI currently constructs IO requests with absolute paths (after joining with `crate_focus` or `current_dir`).

Other relevant artifacts:
- `AppState.system.crate_focus: Option<PathBuf>` (the in-memory “workspace root”)
- Existing tests use a harness that sets `crate_focus` to fixture paths; this masks the bug during tests that don’t use `/load`.

---

## Root Cause

- `load_db` sets `crate_focus` incorrectly:
  - It retrieves `crate_root_path` from the database (which is an absolute path) and then sets `crate_focus = current_dir().join(crate_root_path)`.
  - Joining the current working directory with an already-absolute `crate_root_path` produces an invalid path, and when relative paths are later resolved, they are resolved incorrectly.

Consequence:
- When the LLM proposes `src/...` and the app is running from a different directory, edits are applied relative to the wrong place.

---

## Goals

- Always resolve relative file paths against the loaded crate’s root directory.
- Ensure `system.crate_focus` is the canonical absolute path to the loaded crate’s root.
- Minimize incidental behavior changes; preserve current semantics when no crate is loaded.

Secondary (optional follow-up):
- Enforce root containment at the IO layer by configuring `ploke-io` with the loaded root(s).

---

## Proposed Changes (No code yet — this is the plan)

1) Fix `load_db` crate root assignment
- Replace `current_dir().join(crate_root_path)` with: 
  - Parse `crate_root_path` into a `PathBuf`, ensure it is absolute; if not, canonicalize it, or error if canonicalization fails.
  - Set `system.crate_focus` to this absolute path.
- Rationale: The DB stores the absolute crate root captured during ingestion; we should use it directly.

2) Introduce a small, explicit path resolution helper
- New utility: `resolve_in_crate_root(user_path: &str, crate_root: &Path) -> Result<PathBuf, Error>`
  - If `user_path` is absolute: require that it is within `crate_root`; otherwise return an error (prevents accidental edits outside the workspace).
  - If `user_path` is relative: join to `crate_root`, normalize (`..` and `.`), and ensure containment.`
  - Potentially support a convenience where `user_path` begins with the crate directory name (e.g., `crate_name/src/file.rs`) and strip it once for common LLM outputs.
- Use this helper in `apply_code_edit_tool` and `create_file_tool` instead of rolling ad-hoc `current_dir()` fallbacks. When `crate_focus` is `None`, keep the current behavior (fallback to `current_dir()`) to preserve legacy behavior until we decide to hard-require a loaded crate.

3) (Optional, staged later) Scope IO manager to crate root
- After successful `/load`, rebuild (or reconfigure) `IoManagerHandle` with roots `[crate_focus]` and symlink policy `DenyCrossRoot`.
  - This adds defense-in-depth so even absolute paths outside the root cannot be written.
  - Requires either:
    - Swapping the handle in `AppState` safely (add RwLock or provide a setter + shutdown old handle), or
    - Adding a new IoManager message to update roots dynamically.
- This is not strictly required for the immediate fix but recommended.

---

## New Types (minimal)

- `CrateRoot(PathBuf)` newtype (optional but recommended):
  - Invariant: absolute, canonical (best-effort), points to an existing directory.
  - Provides `join_relative(&self, rel: &Path) -> Result<PathBuf>` with containment checks.
  - Stored in `SystemStatus` instead of a plain `PathBuf` to make invariants explicit.

If we choose not to introduce a newtype initially, we will at least add the `resolve_in_crate_root` helper and unit-test it thoroughly.

---

## TDD Plan

We will write tests first; they should fail on current code, and pass after implementation.

### Unit Tests

1) Path resolution helper tests (`resolve_in_crate_root`)
- `relative_path_is_joined_to_root`: given root `/tmp/r`, input `src/lib.rs` → `/tmp/r/src/lib.rs`.
- `absolute_inside_root_ok`: root `/tmp/r`, input `/tmp/r/src/lib.rs` → same path.
- `absolute_outside_root_errors`: root `/tmp/r`, input `/etc/passwd` → error.
- `relative_escape_errors`: root `/tmp/r`, input `../outside.rs` → error.
- `leading_crate_dir_stripped`: root `/tmp/r` (name `r`), input `r/src/lib.rs` → `/tmp/r/src/lib.rs`.

2) Crate focus assignment logic (extracted helper or direct `load_db` unit target)
- `load_db_sets_absolute_crate_focus`: simulate DB returning an absolute `root_path` and assert `system.crate_focus == root_path` (not joined with CWD). This will fail before the fix.
  - If testing `load_db` directly is heavy, we create a small helper (to be called by `load_db`) that converts the DB `root_path` string into the `crate_focus` value; we unit test that helper here.

3) Tool resolution correctness (unit-level)
- `code_edit_canonical_uses_crate_root`: 
  - With `crate_focus` set and `current_dir` switched to a different folder, call `apply_code_edit_tool` with `file: "src/structs.rs"` (fixture exists). Assert the staged proposal file path is under `crate_focus`.
- `create_file_uses_crate_root`:
  - Same approach for `create_file_tool`, stage creation with `file_path: "src/generated.rs"` and assert the staged proposal path is under `crate_focus`.
  - For safety, do not actually approve/apply here (unit layer); that’s for e2e.

### End-to-End Tests

1) `load_db_scopes_paths_e2e`
- Arrange:
  - Create a temp config directory (set `XDG_CONFIG_HOME` on Unix; analogous env var on Windows if needed).
  - Copy an existing backup DB (e.g., `tests/backup_dbs/fixture_nodes_<uuid>`) into `${XDG_CONFIG_HOME}/ploke/data/`.
  - Start a headless app instance with default state (so `crate_focus` is None initially); set the process CWD to a different directory than the crate root.
- Act:
  - Send `StateCommand::LoadDb { crate_name: "fixture_nodes" }` and wait for `SystemEvent::LoadDb { is_success: true }`.
  - Assert `system.crate_focus` equals the absolute `root_path` pulled from the DB (not joined with the CWD).
  - Stage a `create_file_tool` request with a relative path (e.g., `src/generated_e2e.rs`). Approve it via dispatcher.
- Assert:
  - The file is created under `crate_focus/src/generated_e2e.rs` (not under the test CWD).
  - Proposal status becomes `Applied` and a "Scheduled rescan of workspace" SysInfo is emitted.

2) `reject_outside_root_paths_e2e` (depends on helper enforcement)
- Arrange: same as above (crate loaded and `crate_focus` set).
- Act: Call `apply_code_edit_tool` with an absolute path outside the crate root.
- Assert: Tool call is rejected (ToolCallFailed), and no proposal is staged.

Notes:
- If interacting with OS-specific config dirs is brittle, we’ll add a small, test-only override (env var `PLOKE_DB_BACKUP_DIR`) for `load_db` in code. Tests set this env var and place the backup there. This ensures deterministic e2e without touching real user config paths.

---

## Implementation Steps

1) Update `load_db` crate focus assignment
- Parse `root_path` from DB as `PathBuf`.
- If path is absolute, use it directly. Otherwise attempt `canonicalize` and fall back with a warning if it fails.
- Set `system.crate_focus = Some(crate_root_abs)`.

2) Add `resolve_in_crate_root`
- Location: new module `crates/ploke-tui/src/utils/path.rs` or within `rag/utils.rs`.
- Implement resolution rules and errors as described; unit-test thoroughly.
- Apply in `apply_code_edit_tool` and `create_file_tool` where relative/absolute user paths are consumed.

3) (Optional) Configure IO roots after load
- Add a method to rebuild/swap `IoManagerHandle` with `with_roots([crate_root])` and default `DenyCrossRoot` policy. Initiate on successful `/load`.
- Ensure safe replacement (shutdown old handle; store new one). This may require putting `io_handle` behind an `RwLock` in `AppState`.
- Add an e2e test only if we decide to include this in the first pass.

4) Tests
- Implement unit tests listed above.
- Implement e2e `load_db_scopes_paths_e2e` with temp config dir override and file creation approval path.

5) Docs
- Update inline comments for `load_db`, tools, and any new helpers to reflect the scoping behavior and invariants.

---

## Edge Cases and Considerations

- Missing or malformed `root_path` in the DB: emit a warning and do not change `crate_focus`; require the user to load a supported backup.
- Windows path separators: allow typical `src\\lib.rs` inputs by relying on `PathBuf` parsing. For unit tests, avoid Windows-specific semantics unless CI supports it.
- Symlinks: optional IO scoping will deny cross-root traversals by default (DenyCrossRoot). The helper will canonicalize/normalize to prevent `..` escapes.
- Backward compatibility: when no crate is loaded, keep fallback to `current_dir()` as today. Consider tightening this in a future release.

---

## Expected Outcomes

- Relative paths proposed by the LLM (e.g., `src/...`) always resolve to the loaded crate’s root directory, regardless of the current working directory.
- Tests will initially fail due to the incorrect `crate_focus` assignment in `load_db` and missing helper enforcement.
- After implementing the changes, unit and e2e tests pass; user experience becomes predictable when working across directories.

---

## References (files of interest)
- TUI load path and state: `crates/ploke-tui/src/app_state/database.rs`
- Tools path handling: `crates/ploke-tui/src/rag/tools.rs`
- IO manager and path policy: `crates/ploke-io/src/{handle.rs, actor.rs, write.rs, create.rs, path_policy.rs}`
- Harness and fixtures: `crates/ploke-tui/src/test_utils/new_test_harness.rs`, `tests/backup_dbs/...`

