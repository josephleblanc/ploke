# ploke-rag Backup Loading Survey 2026-03-20

Scope: `crates/ploke-rag` only, compared against
[docs/active/todo/2026-03-19_backup-dbs.md](/home/brasides/code/ploke/docs/active/todo/2026-03-19_backup-dbs.md)
and the shared registry/helper model in
[crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs).

## Backup DB Consumers

- [crates/ploke-rag/src/core/unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs)
  - `load_local_fixture_db()` -> `shared_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)`
  - `db_test_setup()` -> `load_local_fixture_db()`
  - `default_test_db_setup()` -> `load_local_fixture_db()`
  - `TEST_DB_NODES` -> `default_test_db_setup()`
  - `test_fixture_embeddings_loaded_into_active_set()` uses `TEST_DB_NODES`
  - `dense_context_uses_multi_embedding_relations()` uses `fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)`
  - `test_db_nodes_setup()` uses `default_test_db_setup()`
- [crates/ploke-rag/tests/integration_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/tests/integration_tests.rs)
  - `TEST_DB_NODES` -> `shared_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)`
  - `setup_rag()` reads from that shared DB

## Conforming

- `crates/ploke-rag/tests/integration_tests.rs`
  - already follows the shared helper pattern for immutable fixture use.
- `crates/ploke-rag/src/core/unit_tests.rs`
  - `load_local_fixture_db()`, `db_test_setup()`, `default_test_db_setup()`, and the `TEST_DB_NODES` lazy static all route through the shared registry/helper path.

## Non-Conforming

- `crates/ploke-rag/src/core/unit_tests.rs`
  - `dense_context_uses_multi_embedding_relations()` uses `fresh_backup_fixture_db(...)` even though it only performs immutable reads/assertions.
  - This is the clearest current candidate to migrate to `shared_backup_fixture_db(...)` under the todo’s immutable-access guidance.

## Justified Exceptions

- A fresh DB helper is justified when a test needs isolation or must validate fresh-load behavior explicitly.
- In the current `ploke-rag` survey, no backup-backed test clearly requires mutable state.
- `crates/ploke-rag/docs/FIXTURE_CONFIGURATION.md` is a historical note about the old config-dir `load_db("fixture_nodes")` path and does not represent an active test consumer.

