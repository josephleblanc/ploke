# Path Scoping — IO Enforcement & LoadDb Test (Addendum)

Date: 2025-09-26

## Summary

This addendum completes two follow-ups:
- IO-level enforcement: allow switching IoManager roots at runtime and apply it after DB load.
- Deterministic test: assert crate_focus assignment uses the DB’s absolute `root_path`.

## Changes

- ploke-io (actor + handle):
  - New message `IoManagerMessage::UpdateRoots { roots, policy }` to update roots/symlink policy.
  - `IoManager::run` handles the message by updating in-actor state.
  - Handle API: `IoManagerHandle::update_roots(Option<Vec<PathBuf>>, Option<SymlinkPolicy>)` sends the message.
- ploke-tui (database):
  - After successfully resolving `crate_root_path`, set `system.crate_focus` and call:
    - `state.io_handle.update_roots(Some(vec![root_path]), Some(SymlinkPolicy::DenyCrossRoot)).await;`
- Test helper:
  - `app_state::database::test_set_crate_focus_from_db(...)` behind `test_harness` feature, used by integration tests.

## Tests

- New: `tests/load_db_crate_focus.rs`
  - Seeds `*crate_context` via schema `script_put` with an absolute `root_path`.
  - Calls `test_set_crate_focus_from_db` and asserts `SystemState.crate_focus == root_path`.
- Previously added tests remain valid and targeted-only:
  - `tests/path_scoping.rs`, `tests/path_scoping_apply.rs`.

### Commands

- Run the addendum test:
  - `cargo test -p ploke-tui --test load_db_crate_focus --quiet`
- Run all targeted path scoping tests:
  - `cargo test -p ploke-tui --test path_scoping --quiet`
  - `cargo test -p ploke-tui --test path_scoping_apply --quiet`

## Notes

- IO enforcement is now active once `load_db` (or the test helper) sets `crate_focus`: reads/writes/creates go through `IoManager` with roots and `DenyCrossRoot` policy, providing defense-in-depth.
- Tools still perform preflight path checks via `resolve_in_crate_root` to give clear, early errors.
- No field or API refactors were required in `AppState`; the runtime message allows updating the existing IO actor in place.

## Follow-ups

- Extend tests to exercise `IoManager` path enforcement end-to-end (e.g., attempt `read_full_verified` on an outside path post-load and assert policy error).
- Emit a SysInfo message when updating roots to aid observability.
- Consider persistence of last-used `crate_focus` to rehydrate IO roots on app start.

