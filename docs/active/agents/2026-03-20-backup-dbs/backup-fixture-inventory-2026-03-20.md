# Backup Fixture Inventory 2026-03-20

## Current fixture files

- `tests/backup_dbs/fixture_nodes_3b3551b2-a061-5bee-96e4-b24e5a4361c9`
- `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- `tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- `tests/backup_dbs/ploke-db_642a4b75-2527-51f3-9c79-b00672588eb4`
- `tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`

## Observed test usage

### `fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`

- `crates/ploke-db/src/utils/test_utils.rs:9`
  - `TEST_DB_NODES` lazy static imports the backup into an in-memory DB and wraps it in `Arc<Mutex<Database>>`.
  - Shared mutable access pattern.
- `crates/ploke-db/src/bm25_index/mod.rs:784`
  - `TEST_DB_NODES` lazy static imports the backup into an in-memory DB and shares `Arc<Database>`.
  - Intended as immutable shared fixture for BM25 tests.
- `crates/ploke-db/src/index/hnsw.rs:265`
  - `test_hnsw_init_from_backup`
  - `test_hnsw_init_from_backup_error`
  - Fresh DB per test; mutable setup because indices are created/dropped.
- `crates/ploke-db/src/multi_embedding/hnsw_ext.rs:1035`
  - `test_load_db`
  - `helper_load_db`
  - `test_something`
  - Fresh DB per test/helper; mutable setup because indices/embedding relations are ensured.
- `crates/ploke-rag/tests/integration_tests.rs:16`
  - Shared `TEST_DB_NODES` lazy static via `import_backup_with_embeddings`.
  - Effectively immutable shared fixture for integration tests.
- `crates/ploke-tui/src/test_harness.rs:22`
  - Global `TEST_APP` imports this backup into the app test DB if present.
  - Shared mutable app-level state.
- `crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs:17`
  - Doc comment / dependency note only, not an import site.
- `crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs:347`
  - Real-tools test copies/uses this fixture path directly.
  - Mutable test flow.

### `fixture_nodes_3b3551b2-a061-5bee-96e4-b24e5a4361c9`

- `crates/ploke-rag/src/core/unit_tests.rs:28`
  - `load_local_fixture_db`, `default_test_db_setup`, `TEST_DB_NODES`
  - Shared immutable fixture for local embedding / RAG tests.
- `crates/ploke-rag/src/core/unit_tests.rs:126`
  - `dense_context_uses_multi_embedding_relations`
  - Fresh DB import; immutable query/assertion use.
- `crates/ploke-tui/src/test_utils/new_test_harness.rs:34`
  - Shared `TEST_DB_NODES` lazy static for headless app harness.
  - Shared immutable fixture.
- `crates/ploke-tui/tests/get_code_edges_regression.rs:26`
  - Uses `new_test_harness::TEST_DB_NODES`.
  - Immutable fixture use for resolver/tool queries.
- `crates/ploke-tui/tests/tool_ui_payload_fixture.rs:24`
  - Uses `new_test_harness::TEST_DB_NODES`.
  - Immutable fixture use for tool payload rendering.

### `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`

- No active import sites found.
- Only a commented-out path remains in `crates/ploke-rag/src/core/unit_tests.rs:126`.

### `ploke-db_642a4b75-2527-51f3-9c79-b00672588eb4`

- `crates/ploke-tui/tests/get_code_edges_regression.rs:149`
  - `code_item_edges_returns_edges_for_ploke_db_primary_node`
- `crates/ploke-tui/tests/get_code_edges_regression.rs:295`
  - second direct import site in same file
- `crates/ploke-tui/tests/get_code_edges_regression.rs:397`
  - third direct import site in same file
- All observed uses create a fresh DB and import the backup directly.
- Mutable local DB setup, but the imported backup is used read-only after indexing.

### `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`

- No active references found under `crates/`, `tests/`, `xtask/`, or `docs/active`.
- Looks orphaned until proven otherwise.

## Shared helpers and infrastructure

### `ploke-test-utils`

- `crates/test-utils/src/lib.rs:96`
  - `setup_db_full(fixture)` parses a fixture crate and transforms it into a fresh in-memory DB.
  - This is ingestion-driven setup, not backup-import setup.
- `crates/test-utils/src/lib.rs:204`
  - `setup_db_full_multi_embedding(fixture)` creates a fresh DB with multi-embedding schema/relations after parsing.
- `crates/test-utils/src/lib.rs:179`
  - `setup_db_create_multi_embeddings_with_hnsw(fixture)` layers HNSW setup on top.
- `crates/test-utils/src/lib.rs:26`
  - `test_run_phases_and_collect(fixture)` and related parser helpers.
- No shared helper currently exists for:
  - locating a named backup fixture
  - asserting expected fixture metadata/config
  - sharing immutable imported backup DBs across crates

### Ad hoc backup-loading helpers

- `crates/ploke-db/src/utils/test_utils.rs:9`
- `crates/ploke-db/src/bm25_index/mod.rs:784`
- `crates/ploke-rag/src/core/unit_tests.rs:39`
- `crates/ploke-rag/tests/integration_tests.rs:22`
- `crates/ploke-tui/src/test_harness.rs:22`
- `crates/ploke-tui/src/test_utils/new_test_harness.rs:34`

These duplicate the same pattern:
- `Database::init_with_schema()`
- build backup path from `workspace_root()`
- `relations_vec()` + `import_from_backup(...)` or `import_backup_with_embeddings(...)`
- optionally `create_index_primary(...)` / `create_index_for_set(...)`

## Existing xtask support

- `xtask/src/main.rs:22`
  - `verify-fixtures`
- `xtask/src/main.rs:23`
  - `setup-rag-fixtures`
- `xtask/src/main.rs:63`
  - `FIXTURE_CHECKS` currently validates only one backup DB fixture:
    `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- `xtask/src/main.rs:19`
  - `RAG_LOCAL_FIXTURE_BACKUP` hard-codes the canonical `fixture_nodes_bfc...` backup for staging into the config dir
- `xtask/README.md:7`
  - documents the same limited scope

## Gaps and duplication to address

- No single source of truth exists for backup DB fixtures, their intended crate/workspace source, or their required DB characteristics.
- `verify-fixtures` only checks one of the five backup DB files currently present.
- Backup DB paths are hard-coded across `ploke-db`, `ploke-rag`, and `ploke-tui`.
- There are at least two competing `fixture_nodes` backups in active use:
  - `fixture_nodes_bfc...`
  - `fixture_nodes_3b355...`
  - Their intended differences are not encoded in shared metadata.
- One backup file appears unused:
  - `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`
- One backup file appears stale/orphaned:
  - `fixture_nodes_multi_embedding_schema_v1_bfc...` is only referenced in a comment.
