# Shared Backup Helper Fix 2026-03-20

- Fixed the `ploke-db` lib-test compile break by switching
  [crates/ploke-db/src/bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs)
  back to a crate-local loader that reads the shared registry constant
  `FIXTURE_NODES_CANONICAL` instead of calling `shared_backup_fixture_db(...)`.
- Replaced the real-backup-dependent unit test in
  [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
  with a registry-only lookup test.
- Narrowed the global cache lock in `shared_backup_fixture_db(...)` so fixture
  import and validation happen outside the mutex.
- Updated fixture docs/review notes to call out the `ploke-db` unit-test
  exception explicitly.
