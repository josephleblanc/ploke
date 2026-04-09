# Backup DBs Progress 2026-03-20: `ploke_db_primary` Refresh

- Switched `ploke_db_primary` from manual-only metadata to an automated
  workspace-crate recreation path backed by `setup_db_full_crate("ploke-db")`.
- `cargo xtask recreate-backup-db --fixture ploke_db_primary` now recreates:
  - `tests/backup_dbs/ploke_db_primary_2026-03-20.sqlite`
- `cargo xtask verify-backup-dbs --fixture ploke_db_primary` now passes for the
  regenerated backup.
- Updated the active fixture registry entry in
  [crates/test-utils/src/fixture_dbs.rs](../../../../crates/test-utils/src/fixture_dbs.rs)
  to point at the new dated file and mark its default access as shared
  immutable.
- Updated
  [crates/ploke-tui/tests/get_code_edges_regression.rs](../../../../crates/ploke-tui/tests/get_code_edges_regression.rs)
  to load `ploke_db_primary` via `shared_backup_fixture_db(&PLOKE_DB_PRIMARY)`
  instead of hand-importing a hard-coded backup path per test.
- Updated
  [docs/testing/BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md)
  to reflect the new active filename, source-backed recreation path, and shared
  immutable access pattern.

## Validation Outcome

- `cargo test -p ploke-tui --test get_code_edges_regression -- --nocapture`
  passed: `3 passed, 0 failed, 1 ignored`
- `cargo test -p ploke-tui --all-targets -- --nocapture` passed cleanly
