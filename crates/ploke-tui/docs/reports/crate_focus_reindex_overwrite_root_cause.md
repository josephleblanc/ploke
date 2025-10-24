Crate Focus Overwrite on ReIndex: Root Cause Analysis

Summary
- When a user submits a chat message after loading a crate (e.g., `fixture_nodes`), the app runs a scan and then emits a `ReIndex` event. Handling `ReIndex` calls `index_workspace`, which incorrectly recomputes `state.system.crate_focus` as `current_dir().join(workspace)`. If `workspace` is just the crate name (e.g., `fixture_nodes`) rather than the absolute crate root path, this overwrites the previously correct absolute crate root with an incorrect path like `/home/.../ploke/fixture_nodes`.

Observed Symptom
- `create_file_tool` receives a `crate_root` of `/home/brasides/code/openai-codex/ploke/fixture_nodes` instead of `/home/brasides/code/openai-codex/ploke/tests/fixture_crates/fixture_nodes`, causing proposed file creations to target the wrong directory and fail.

Reproduction Path
1) User runs "load crate fixture_nodes" → `StateCommand::LoadDb` → `handlers::db::load_db`.
2) `load_db` correctly sets `state.system.crate_focus` to the absolute path from the DB (e.g., `.../tests/fixture_crates/fixture_nodes`).
3) On message submit, `App::Submit` triggers `StateCommand::ScanForChange` → `handlers::db::scan_for_change`.
4) `scan_for_change` derives `crate_name = crate_focus.file_name()` and emits `SystemEvent::ReIndex { workspace: crate_name }` — only the crate name, not the absolute path.
5) In `app/events.rs`, `SystemEvent::ReIndex` dispatches `StateCommand::IndexWorkspace { workspace, needs_parse: false }`.
6) `handlers::indexing::index_workspace` sets `write_guard.crate_focus = Some(current_dir().join(workspace))`, overwriting the previously correct absolute path with an incorrect one.

Key Code References
- Correct assignment on load: `crates/ploke-tui/src/app_state/database.rs`
  - `load_db`: sets `system_guard.crate_focus = Some(root_path.clone())` where `root_path` is the absolute path pulled from the DB; also updates `IoManager` roots.
- Incorrect overwrite on reindex: `crates/ploke-tui/src/app_state/handlers/indexing.rs`
  - `index_workspace`: computes `crate_focus = current_dir().join(&workspace)` and assigns `write_guard.crate_focus = Some(crate_focus.clone())`.
- Event chain:
  - `scan_for_change` (database.rs) → emits `SystemEvent::ReIndex { workspace: crate_name }`.
  - `app/events.rs` → on `ReIndex`, sends `StateCommand::IndexWorkspace { workspace, needs_parse: false }`.

Why This Fails
- `workspace` is not guaranteed to be a path; it can be just a crate name (derived via `file_name()` from the absolute `crate_focus`). Joining it to `current_dir()` discards the true root and points at a non-existent or wrong path (`/.../ploke/fixture_nodes`).
- Subsequent tool path resolution (e.g., `create_file_tool`) uses `state.system.crate_focus`, so it resolves relative paths under the wrong base.

Impact
- File creation/edit proposals point outside the actual crate root and can fail (`create_parents` or IO checks), and previews list unexpected paths.

Fix Direction (proposed)
- Do not recompute `crate_focus` inside `index_workspace` when it’s already set; use the existing absolute path in `state.system.crate_focus` as the source of truth.
- Alternatively, change `ReIndex` to carry the absolute root path (or a typed `CrateRoot` newtype) and have `index_workspace` set/verify from that, not from `current_dir()`.
- Ensure tools always resolve relative paths via `resolve_in_crate_root(user_path, crate_focus)`; already implemented for `create_file` and `apply_code_edit`.

Tests Added
- `reindex_overwrites_crate_focus` (current behavior): demonstrates that `index_workspace` overwrites `crate_focus` to `current_dir()/fixture_nodes` when `workspace` is a crate name.
- `reindex_preserves_absolute_crate_focus_expected` (ignored pending fix): asserts `crate_focus` remains the absolute DB-derived root after `index_workspace`.

Evidence
- See tests under `crates/ploke-tui/tests/` and code references above. Running the first test shows the overwrite; the second is ignored and will pass once the overwrite logic is corrected.
