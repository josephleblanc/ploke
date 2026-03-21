# Backup DBs Progress 2026-03-20: ploke-db Fixture Loader

- Added a test-only helper in
  [crates/ploke-db/src/multi_embedding/db_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/db_ext.rs)
  to import the registered local-embedding fixture directly from
  `FIXTURE_NODES_LOCAL_EMBEDDINGS.path()` instead of relying on ambient
  config-dir prefix discovery.
- Switched these tests to that helper:
  - [crates/ploke-db/src/multi_embedding/db_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/db_ext.rs)
    `test_load_backup`
  - [crates/ploke-db/src/database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
    `test_retract_embedding_single`
  - [crates/ploke-db/src/database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
    `test_retract_embeddings_full`
  - [crates/ploke-db/src/database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
    `test_retract_embeddings_partial`
- Production `load_db(...)` semantics were left unchanged.
