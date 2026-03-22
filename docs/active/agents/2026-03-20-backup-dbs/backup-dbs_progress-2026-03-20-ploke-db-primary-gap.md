# Backup DBs Progress 2026-03-20: `ploke_db_primary` Gap

- `ploke_db_primary` remains registered as a manual backup fixture in
  [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
  with the current path
  `tests/backup_dbs/ploke-db_642a4b75-2527-51f3-9c79-b00672588eb4`.
- `cargo xtask recreate-backup-db --fixture ploke_db_primary` does not recreate
  it from repo code today; it prints the manual TUI workflow captured in the
  registry metadata and operator guide.
- The active in-repo consumer is still the direct-path `ploke-tui`
  `get_code_edges_regression` tests in
  [crates/ploke-tui/tests/get_code_edges_regression.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/get_code_edges_regression.rs).
- If a new dated backup is adopted, the minimum follow-up is to update the
  fixture registry entry, the fixture inventory doc, and the hard-coded backup
  path in `get_code_edges_regression.rs`.
