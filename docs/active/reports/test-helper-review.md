# Test Helper Review

Date: 2026-03-20
Task title: backup DB fixture cleanup
Task description: review backup-fixture setup helpers for duplication, inconsistency, and likely deprecation candidates
Related planning files:
- [docs/active/todo/2026-03-19_backup-dbs.md](/home/brasides/code/ploke/docs/active/todo/2026-03-19_backup-dbs.md)
- [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md)

## Summary

The ingestion-oriented helpers in `ploke-test-utils` are still useful, but backup
fixture setup is fragmented across `ploke-db`, `ploke-rag`, and `ploke-tui`.
Most duplication is not in parsing or schema creation; it is in locating backup
files, importing them, choosing the right import mode, rebuilding indices, and
sharing immutable fixture DBs.

## Existing helpers that still make sense

- [crates/test-utils/src/lib.rs](/home/brasides/code/ploke/crates/test-utils/src/lib.rs)
  - `setup_db_full`
  - `setup_db_full_multi_embedding`
  - `setup_db_create_multi_embeddings_with_hnsw`
  - These are still the right helpers when a test wants a fresh DB created from
    source crates rather than a committed backup.
- [crates/test-utils/src/nodes.rs](/home/brasides/code/ploke/crates/test-utils/src/nodes.rs)
  - Node-id regeneration and parsed-graph validation helpers are unrelated to
    backup-fixture duplication and should remain.

## Duplicated or inconsistent backup-fixture helpers

- [crates/ploke-db/src/utils/test_utils.rs](/home/brasides/code/ploke/crates/ploke-db/src/utils/test_utils.rs)
  - `TEST_DB_NODES` hard-codes the canonical fixture path and import flow.
- [crates/ploke-db/src/bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs)
  - a second `TEST_DB_NODES` lazy static repeats the same fixture import flow.
- [crates/ploke-rag/src/core/unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs)
  - `load_local_fixture_db`
  - `default_test_db_setup`
  - all of these duplicate registry and import-mode concerns.
- [crates/ploke-rag/tests/integration_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/tests/integration_tests.rs)
  - previously had another local fixture constant and shared loader
- [crates/ploke-tui/src/test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_harness.rs)
  - direct fallback path selection for `PLOKE_TEST_DB_BACKUP`
- [crates/ploke-tui/src/test_utils/new_test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/new_test_harness.rs)
  - another shared `TEST_DB_NODES` backed by a different fixture

## Recommended deprecations after the shared helper lands

- Deprecate crate-local `TEST_DB_NODES` statics whose only job is:
  - choose a backup path
  - import it
  - create a primary index
- Replace those statics with the shared helpers now exposed from
  `ploke-test-utils`:
  - `shared_backup_fixture_db(&FIXTURE_...)` for immutable shared callers
  - `fresh_backup_fixture_db(&FIXTURE_...)` for isolated immutable callers
- Exception:
  - `ploke-db` lib-unit tests cannot consume `shared_backup_fixture_db(...)`
    directly because `ploke-test-utils` depends on `ploke-db`, which produces a
    duplicate `Database` crate type inside the crate’s own unit-test build.
    Those tests should still consume the shared registry constants and keep a
    small crate-local loader unless they are moved to integration tests.
- Once the shared helper is adopted, remove or collapse:
  - `crates/ploke-db/src/utils/test_utils.rs::TEST_DB_NODES`
  - `crates/ploke-rag/src/core/unit_tests.rs::{load_local_fixture_db, default_test_db_setup}`
  - `crates/ploke-tui/src/test_utils/new_test_harness.rs::TEST_DB_NODES`

## Helpers that should stay crate-local

- The app-harness builders in `ploke-tui` should stay local because they do much
  more than restore a backup DB.
- Tests that need a fresh mutable DB tailored to one scenario can keep a local
  wrapper if it delegates the fixture path/import choice to the shared helper.

## Next step

The shared backup fixture helper now exists in
[crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs).
The remaining cleanup is to migrate mutable/local fixture loaders only where
they genuinely benefit from the registry, rather than forcing every test setup
path through one abstraction.
