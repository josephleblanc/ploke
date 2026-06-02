# Backup DBs Progress 2026-03-20: `ploke-db` Loader Survey

Scope: backup DB loading in `crates/ploke-db` only, compared against the
desired backup-fixture approach in
[2026-03-19_backup-dbs.md](../../todo/2026-03-19_backup-dbs.md)
and the `FixtureDb` / shared-helper guidance in
[BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md).

## Backup loaders found

- [crates/ploke-db/src/utils/test_utils.rs](../../../../crates/ploke-db/src/utils/test_utils.rs)
  - `lazy_static! TEST_DB_NODES`
  - hard-coded `tests/backup_dbs/fixture_nodes_canonical_2026-03-20.sqlite`
  - mutable shared `Arc<Mutex<Database>>`
- [crates/ploke-db/src/bm25_index/mod.rs](../../../../crates/ploke-db/src/bm25_index/mod.rs)
  - `lazy_static! TEST_DB_NODES`
  - uses registry constant `FIXTURE_NODES_CANONICAL.path()`
  - immutable shared `Arc<Database>`
- [crates/ploke-db/src/index/hnsw.rs](../../../../crates/ploke-db/src/index/hnsw.rs)
  - `test_hnsw_init_from_backup`
  - `test_hnsw_init_from_backup_error`
  - hard-coded canonical backup path
  - fresh mutable DB per test
- [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](../../../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs)
  - `test_load_db`
  - `helper_load_db`
  - hard-coded canonical backup path
  - fresh mutable DB per test/helper
- [crates/ploke-db/src/multi_embedding/db_ext.rs](../../../../crates/ploke-db/src/multi_embedding/db_ext.rs)
  - `load_registered_fixture_nodes_local_embeddings`
  - uses registry constant `FIXTURE_NODES_LOCAL_EMBEDDINGS.path()`
  - fresh mutable DB setup for tests that need backup-with-embeddings semantics
- [crates/ploke-db/benches/resolver_bench.rs](../../../../crates/ploke-db/benches/resolver_bench.rs)
  - hard-coded canonical backup path
  - immutable benchmark input

## Conforms

- [crates/ploke-db/src/bm25_index/mod.rs](../../../../crates/ploke-db/src/bm25_index/mod.rs)
  - Conforms to the registry-as-source-of-truth requirement by using
    `FIXTURE_NODES_CANONICAL.path()`.
  - This is the right `ploke-db`-local shape for an immutable shared loader
    under the documented unit-test exception in
    [BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md).
- [crates/ploke-db/src/multi_embedding/db_ext.rs](../../../../crates/ploke-db/src/multi_embedding/db_ext.rs)
  - Conforms as a justified crate-local test helper for mutable tests that need
    `import_backup_with_embeddings(...)` plus active-set restoration.
  - Uses the registry constant instead of a hard-coded path.

## Does not conform

- [crates/ploke-db/src/utils/test_utils.rs](../../../../crates/ploke-db/src/utils/test_utils.rs)
  - Hard-coded backup path instead of registry metadata.
- [crates/ploke-db/src/index/hnsw.rs](../../../../crates/ploke-db/src/index/hnsw.rs)
  - Hard-coded backup path in both backup-loading tests.
- [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](../../../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs)
  - Hard-coded backup path in both the direct test and helper.
- [crates/ploke-db/benches/resolver_bench.rs](../../../../crates/ploke-db/benches/resolver_bench.rs)
  - Hard-coded backup path in benchmark setup.

## Immutable-only strong candidates to migrate now

- [crates/ploke-db/src/bm25_index/mod.rs](../../../../crates/ploke-db/src/bm25_index/mod.rs)
  - Already registry-backed and immutable; no immediate migration needed.
- [crates/ploke-db/benches/resolver_bench.rs](../../../../crates/ploke-db/benches/resolver_bench.rs)
  - Immutable-only and a strong candidate to migrate off the hard-coded path.
  - Best immediate step: use the registry constant instead of the literal path.
  - Using `shared_backup_fixture_db(...)` directly is likely not appropriate in
    bench code without first checking for the same `ploke-db` / `ploke-test-utils`
    crate-type constraint that applies to lib-unit tests.

## Justified exceptions

- `ploke-db` lib-unit tests cannot directly use
  `shared_backup_fixture_db(...)` from `ploke-test-utils` because
  `ploke-test-utils` depends on `ploke-db`, causing a duplicate-crate-type split
  for `Database`. This exception is already documented in
  [BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md#L58).
- Under that exception, crate-local loaders are still acceptable if they source
  the path from the registry constant rather than hard-coding a backup path.

## Best near-term migrations inside `ploke-db`

- Switch hard-coded canonical paths to registry constants in:
  - [crates/ploke-db/src/utils/test_utils.rs](../../../../crates/ploke-db/src/utils/test_utils.rs)
  - [crates/ploke-db/src/index/hnsw.rs](../../../../crates/ploke-db/src/index/hnsw.rs)
  - [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](../../../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs)
  - [crates/ploke-db/benches/resolver_bench.rs](../../../../crates/ploke-db/benches/resolver_bench.rs)
- Keep mutable per-test fresh loads as fresh loads; they are not candidates for
  the shared immutable helper.
