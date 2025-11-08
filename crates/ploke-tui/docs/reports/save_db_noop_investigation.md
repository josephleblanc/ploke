### Investigation: `/save db` appears to do nothing after indexing

#### Summary
- Observed: After indexing a crate (e.g., `/index start tests/fixture_crates/fixture_nodes`), running `/save db` yields no conversation update and no trace logs.
- Root cause: `system.crate_focus` is not set during indexing. `save_db` early-exits when no crate focus is available and emits no UI/system events, so the UI shows nothing.
- Proposed fixes: (1) set `crate_focus` when starting indexing; (2) emit a user-visible error when `crate_focus` is missing; (3) optional fallback to derive crate name from DB if focus is missing.

#### Reproduction
1) Run ploke-tui and index a crate via `/index start <path>`.
2) After indexing completes successfully (and code retrieval via RAG works), execute `/save db`.
3) Expected: a SysInfo message in the conversation showing the backup location, or an error message.
4) Actual: no message and no trace specific to the save attempt.

#### Command → Handler Flow (relevant locations)
- Command parsing routes `"save db" | "sd"` to `StateCommand::SaveDb`:
  - File: `crates/ploke-tui/src/app/commands/exec.rs` (legacy command mapping)
- Dispatcher handles `StateCommand::SaveDb` by calling `database::save_db(&state, &event_bus)`:
  - File: `crates/ploke-tui/src/app_state/dispatcher.rs`
- `save_db` attempts to resolve a default backup directory and then requires `system.crate_focus` to determine the crate backup filename:
  - File: `crates/ploke-tui/src/app_state/database.rs` → `pub(super) async fn save_db(...)`
- The UI layer listens for `SystemEvent::BackupDb` and posts a `SysInfo` message on success or failure:
  - File: `crates/ploke-tui/src/app/events.rs` (`SystemEvent::BackupDb` match arm)

#### Key Behavior in `save_db`
- Resolves `dirs::config_local_dir()/ploke/data`; on failure, emits `SystemEvent::BackupDb { is_success: false, ... }`.
- Reads `state.system.read().await.crate_focus` and proceeds only if `Some(...)`:
  - Calls `state.db.get_crate_name_id(<crate_name>)` to produce a backup filename like `<crate>_<uuid>`.
  - Calls `state.db.backup_db(file_dir)` and emits `SystemEvent::BackupDb { is_success: true/false }` accordingly.
- If `crate_focus` is `None`, it does not emit any event or message and simply returns.

#### Why nothing appears in the UI
- The UI only surfaces messages when it receives `SystemEvent::BackupDb`.
- When `crate_focus` is missing, `save_db` never emits that event, so there is no conversation update and (beyond generic directory setup) no meaningful trace log.

#### Why `crate_focus` is missing
- Indexing path does not set `system.crate_focus`. In `handlers::indexing::index_workspace`, the target directory is derived and used, but `system.crate_focus` is never updated during or after indexing.
- `crate_focus` is set when loading from a DB backup (`LoadDb`) and in tests, but not on `/index start`.

#### Proposed Fixes
- Set `crate_focus` during indexing:
  - In `crates/ploke-tui/src/app_state/handlers/indexing.rs`, after resolving `target_dir`, set `state.system.write().await.crate_focus = Some(target_dir.clone())` before indexing begins.
  - Consider also updating `io_handle.update_roots(...)` similarly to the `LoadDb` code path for consistent IO sandboxing.
- Emit an explicit user-visible error when `crate_focus` is missing:
  - In `save_db`, if `crate_focus.is_none()`, add a `SysInfo` message such as: “No active crate selected; run `/index start <path>` or `/load crate <name>` before saving.” This way, `/save db` never silently no-ops.
- Optional: derive crate name from DB if focus is missing:
  - Query the DB for the active/only crate name, or ask the user to choose when multiple exist. This would reduce reliance on `crate_focus` but is a larger change.

#### Validation Plan
- Manual:
  - Index a fixture via `/index start tests/fixture_crates/fixture_nodes`.
  - Confirm `crate_focus` is set (debug log or temporary SysInfo on indexing start).
  - Run `/save db` and verify a success SysInfo message and file creation under the default config directory (plausibly `~/.config/ploke/data/<crate>_<uuid>` on Linux).
  - Remove/rename the backup directory and retry to observe error handling.
- Automated:
  - Add an integration test to simulate `/index start` then invoke `save_db` and assert that a `SystemEvent::BackupDb` (success) is observed and that the UI emits the corresponding SysInfo message.
  - Add a unit test for the “no crate focus” branch to assert a SysInfo error or event emission (post-fix).

