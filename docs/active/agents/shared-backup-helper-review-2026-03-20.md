# Shared Backup Helper Review 2026-03-20

- High: [crates/ploke-db/src/bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs) now imports
  `shared_backup_fixture_db(...)` from `ploke-test-utils`, but
  `ploke-test-utils` depends on `ploke-db` as an external crate. In `ploke-db`
  lib tests that produces a distinct `ploke_db::database::Database` type, which
  does not match the crate-local `database::Database`.
- Reproduced with `cargo check -p ploke-db --tests`.
- `ploke-rag` and `ploke-tui` test-target checks passed; the type duplication is
  specific to `ploke-db` lib-test compilation.

Resolution:

- `ploke-db` lib-unit tests now use the shared fixture registry constant
  (`FIXTURE_NODES_CANONICAL`) but keep a crate-local loader, avoiding the
  duplicate `Database` crate type while still removing the hard-coded backup
  path.
- The backup-dependent unit test in
  [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
  was replaced with a registry-only unit test.
- `shared_backup_fixture_db(...)` now checks the cache without holding the mutex
  across backup import / disk IO, then re-checks before insert.
