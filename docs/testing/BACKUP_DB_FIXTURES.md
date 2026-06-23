# Backup DB Fixtures

Last reviewed: 2026-06-12
Last updated: 2026-06-23

This document is the current inventory for backup database fixtures under
the shared DB snapshot fixture directory. It records which source targets
produced each fixture, which tests consume it, whether those tests expect
mutable or immutable access, and the DB-level assumptions that make those tests
valid.

Runtime fixture loading uses `$XDG_CONFIG_HOME/ploke/db_snapshot_fixtures`, or
`~/.config/ploke/db_snapshot_fixtures` when `XDG_CONFIG_HOME` is unset. Set
`PLOKE_DB_SNAPSHOT_FIXTURE_DIR` to override this location for tests or unusual
local setups. The files under `tests/backup_dbs/` are committed seed artifacts;
they are not the normal runtime load location.

Backup DB fixture identity has two parts:

- the registry entry in
  [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
- the fixture path scope

Checkout-local fixtures may contain absolute source roots, so registry loads
prefer a generated root-scoped file under `tests/backup_dbs/local/` when one
exists. Their committed registered path is a fallback and compatibility anchor,
not proof that the DB is valid for every worktree. Operational path-only
consumers must call `FixtureDb::checked_path()` so the effective path is loaded
and validated before it is copied or staged.

## Review cadence

- Review this document whenever a new backup is added, removed, renamed, or its
  schema expectations change.
- If this document is more than 7 days old, agents should remind the user that a
  fixture review is due before making more backup-fixture changes.

## Lifecycle commands

Use the registry-backed `xtask` commands for fixture health checks and
recreation guidance:

- `cargo xtask verify-backup-dbs`
  - validates active registered backup fixtures using their configured import
    mode and contract checks
- `cargo xtask verify-backup-dbs --fixture <id>`
  - scopes validation to one fixture
- `cargo xtask recreate-backup-db --fixture <id>`
  - recreates automated checkout-local fixtures to a deterministic root-scoped
    filename under `tests/backup_dbs/local/`
  - recreates automated shared-snapshot fixtures to a new dated filename under
    the shared DB snapshot fixture directory
  - recreates automated legacy registered-path fixtures to a new dated filename
    under `tests/backup_dbs/`
  - prints exact manual recreation steps for fixtures that are not hermetic yet
- `cargo xtask fixtures ensure --snapshots`
  - ensures current active and typed graph fixtures are available without
    sharing checkout-local DB rows
  - runs active fixture validation, then validates typed graph fixtures in the same baseline profile
  - creates or repairs checkout-local active fixtures under
    `tests/backup_dbs/local/`
  - stages committed seed artifacts from `tests/backup_dbs/` only when a
    shared-snapshot fixture is missing
  - refuses to overwrite differing shared snapshots
  - validates staged snapshots strictly, so stale schema seeds fail loudly
- `cargo xtask fixtures ensure --typed`
  - prepares typed GitHub corpus source checkouts under
    `<db_snapshot_fixtures>/_source_cache` unless `PLOKE_FIXTURE_HOME` is set
  - stages typed graph seed snapshots into the shared DB snapshot fixture
    directory
- `cargo xtask fixtures regenerate --all`
  - regenerates every automated active and typed graph fixture into its
    scope-specific output location
  - writes checkout-local fixtures under `tests/backup_dbs/local/`
  - writes shared-snapshot fixtures under the shared DB snapshot fixture
    directory using the registered filenames
  - skips manual legacy/orphaned snapshots
  - runs active fixtures, then regenerates typed graph fixtures in the same baseline profile
  - use `--active` or `--typed` instead of `--all` for narrower regeneration
- `cargo xtask repair-backup-db-schema --fixture <id>`
  - repairs a stale legacy backup in place when it is missing the current
    `workspace_metadata` relation
  - use this for the specific schema-drift failure surfaced by
    `verify-backup-dbs`, not as a substitute for full regeneration

Operator workflow details live in
[docs/how-to/recreate-backup-db-fixtures.md](../how-to/recreate-backup-db-fixtures.md).

## Shared helper API

Immutable backup consumers should load fixtures through the registry-backed
helpers in
[crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs):

- `shared_backup_fixture_db(&FIXTURE_...)`
  - loads from the shared DB snapshot fixture directory, validates, and caches
    an immutable `Arc<Database>` for reuse
- `fresh_backup_fixture_db(&FIXTURE_...)`
  - creates a fresh in-memory `Database` from a registered fixture while still
    enforcing the registry’s import mode, embedding expectations, and index
    setup
- `load_backup_fixture_db(&FIXTURE_...)`
  - returns both the checked effective path and the loaded `Database`
- `FIXTURE_....checked_path()`
  - validates the effective backup path for path-only consumers such as fixture
    staging or test registry snapshots

The registry file is the source of truth for fixture metadata and filenames.
Test code should reference fixture constants there instead of hard-coding backup
paths.

Because the default shared snapshot directory is global across local worktrees,
the registry helper compares a same-named default shared snapshot with the
committed seed in the current worktree when that seed exists. If the bytes
differ, the helper loads the worktree seed instead of importing a stale snapshot
produced by another checkout. The same protection applies when
`PLOKE_DB_SNAPSHOT_FIXTURE_DIR` is explicitly pinned to that default home cache,
which some tests do while moving `XDG_CONFIG_HOME`. A non-default
`PLOKE_DB_SNAPSHOT_FIXTURE_DIR` remains authoritative.

Registry status note:

- `Active` fixtures are expected to exist on disk and are validated by
  `cargo xtask verify-backup-dbs`.
- `Planned` fixtures record a source-pinned, reproducible fixture contract
  before the dated DB backup is committed. They may be recreated explicitly with
  `cargo xtask recreate-backup-db --fixture <id>`, but they are not part of the
  default verification set until promoted to `Active`.
- `TypedTypeGraph` fixtures are current-schema typed type graph backups. They
  are intentionally excluded from default backup verification because plain
  fixture imports intentionally exclude typed graph relations. Verify them with
  `cargo xtask verify-backup-dbs --fixture <id>`.
- `Legacy` and `Orphaned` fixtures remain outside the default active validation
  set unless explicitly selected.

One exception currently remains for `ploke-db` lib-unit tests: because
`ploke-test-utils` depends on `ploke-db`, those unit-test modules cannot consume
`shared_backup_fixture_db(...)` directly without hitting a duplicate-crate type
split for `Database`. In that case, use the shared registry constant (for
example `FIXTURE_NODES_CANONICAL.checked_path()?`) with a crate-local loader.

Test isolation note:

- Tests that mutate DB state loaded from a backup fixture must use
  `fresh_backup_fixture_db(&FIXTURE_...)` or a harness built on top of it.
  Do not use `shared_backup_fixture_db(...)` for tests that approve edits,
  refresh file hashes, write embeddings, update indexes, or otherwise change
  fixture-backed DB relations. Shared fixtures are for read-only consumers.
- Tests that mutate checked-in source fixtures associated with a backup DB
  also need a restore guard around the source file or tree. The DB and source
  isolation are separate: a fresh DB prevents cross-test database state leaks,
  while the restore guard prevents concurrent source-file edits from racing.
  A module-local pattern that has worked is:

```rust
use std::sync::{Mutex, MutexGuard, OnceLock};

struct FixtureRestoreGuard {
    _lock: MutexGuard<'static, ()>,
}

fn fixture_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

impl FixtureRestoreGuard {
    fn new() -> Self {
        let lock = fixture_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        restore_fixture();
        Self { _lock: lock }
    }
}

impl Drop for FixtureRestoreGuard {
    fn drop(&mut self) {
        restore_fixture();
    }
}
```

  Keep this guard scoped to the test duration. If multiple modules or test
  binaries mutate the same checked-in fixture tree, move the lock helper to a
  shared test utility instead of duplicating per-module locks.
- Tests that temporarily override `XDG_CONFIG_HOME` to point
  `WorkspaceRegistry::default_registry_path()` at a temp directory must
  serialize that mutation across the full workspace test run.
- This is test-only env mutation and does not change normal app behavior.
- Prefer a shared test lock/helper over per-module mutexes when multiple test
  binaries need to write the workspace registry.

## Fixture Summary

| Fixture | Parsed target(s) | Primary usage | Last update |
| --- | --- | --- | --- |
| `fixture_nodes_canonical_2026-05-17.sqlite` | `tests/fixture_crates/fixture_nodes` | canonical parsed `fixture_nodes` backup | 2026-05-17 |
| `fixture_nodes_local_embeddings_2026-05-17.sqlite` | `tests/fixture_crates/fixture_nodes` | local-embedding `fixture_nodes` backup | 2026-05-17 |
| `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` | `tests/fixture_crates/fixture_nodes` | legacy multi-embedding schema snapshot | 2026-03-20 |
| `ploke_db_primary_2026-05-06.sqlite` | `crates/ploke-db` | current-schema `ploke-db` graph backup | 2026-05-06 |
| `ws_fixture_01_canonical_2026-05-17.sqlite` | `tests/fixture_workspace/ws_fixture_01` | canonical plain backup of committed multi-member workspace fixture | 2026-05-17 |
| `ws_fixture_01_member_single_2026-05-06.sqlite` | `tests/fixture_workspace/ws_fixture_01/member_root` | single-member slice of workspace fixture | 2026-05-06 |
| `corpus_semver_type_graph_2026-05-17.sqlite` | `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_semver_openrouter_embeddings_2026-05-17.sqlite` | `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_memchr_type_graph_2026-05-17.sqlite` | `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_memchr_openrouter_embeddings_2026-05-17.sqlite` | `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_generic_array_type_graph_2026-05-17.sqlite` | `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite` | `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_chrono_type_graph_2026-05-17.sqlite` | `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite` | `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_axum_type_graph_2026-05-17.sqlite` | `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1` | typed graphRAG workspace-member corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1` | OpenRouter-searchable workspace-member corpus backup for type-context matrix tests | 2026-05-17 |
| `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45` | `crates/ploke-db` | currently orphaned snapshot | 2026-03-20 |

## `fixture_nodes_canonical_2026-05-17.sqlite`

- File: `tests/backup_dbs/fixture_nodes_canonical_2026-05-17.sqlite`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - plain backup import
  - normal type-resolution profile fixture; under current typed graph baseline workspace
    builds, active fixture loaders import the same code graph while leaving
    typed graph relations empty rather than treating this as typed graph corpus
    coverage
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - used as the canonical parsed graph fixture for `fixture_nodes`
- Tests using this fixture:
  - `ploke-db`
    - [crates/ploke-db/src/utils/test_utils.rs](../../crates/ploke-db/src/utils/test_utils.rs): shared mutable `Arc<Mutex<Database>>`
    - [crates/ploke-db/src/bm25_index/mod.rs](../../crates/ploke-db/src/bm25_index/mod.rs): shared immutable `Arc<Database>` via crate-local loader keyed by `FIXTURE_NODES_CANONICAL`
    - [crates/ploke-db/src/index/hnsw.rs](../../crates/ploke-db/src/index/hnsw.rs): fresh mutable DB per test
    - [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs): fresh mutable DB per test
    - [crates/ploke-db/benches/resolver_bench.rs](../../crates/ploke-db/benches/resolver_bench.rs): immutable benchmark input
  - `ploke-rag`
    - legacy direct-path use removed; immutable consumers should use the local-embedding fixture helper instead
  - `ploke-tui`
    - [crates/ploke-tui/src/test_harness.rs](../../crates/ploke-tui/src/test_harness.rs): shared mutable app harness DB
    - [crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs](../../crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs): shared mutable DB for live-tool scaffolding
    - [crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs](../../crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs): fixture dependency note, immutable expectations
- Notes:
  - older backups remain on disk (for example `fixture_nodes_canonical_2026-05-06.sqlite`) but the active registry entry points at the 2026-05-17 snapshot

## `fixture_nodes_local_embeddings_2026-05-17.sqlite`

- File: `tests/backup_dbs/fixture_nodes_local_embeddings_2026-05-17.sqlite`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - import with `Database::import_backup_with_embeddings`
  - default local embedding set expected:
    - provider: `local`
    - model: `sentence-transformers/all-MiniLM-L6-v2`
    - dims: `384`
    - dtype: `f32`
  - vectors must be present for the default local set
  - callers generally rebuild the primary index after import
  - recreation currently forces CPU device selection but still uses the default
    local model revision
- Tests using this fixture:
  - `ploke-rag`
    - [crates/ploke-rag/src/core/unit_tests.rs](../../crates/ploke-rag/src/core/unit_tests.rs): shared immutable DB plus fresh immutable imports via `fresh_backup_fixture_db`
    - [crates/ploke-rag/tests/integration_tests.rs](../../crates/ploke-rag/tests/integration_tests.rs): shared immutable DB via `shared_backup_fixture_db`
  - `ploke-tui`
    - [crates/ploke-tui/src/test_utils/new_test_harness.rs](../../crates/ploke-tui/src/test_utils/new_test_harness.rs): shared immutable headless harness DB via `shared_backup_fixture_db`; mutating edit tests use a fresh harness DB via `fresh_backup_fixture_db`
    - [crates/ploke-tui/tests/get_code_edges_regression.rs](../../crates/ploke-tui/tests/get_code_edges_regression.rs): shared immutable DB via harness
    - [crates/ploke-tui/tests/tool_ui_payload_fixture.rs](../../crates/ploke-tui/tests/tool_ui_payload_fixture.rs): shared immutable DB via harness
- Notes:
  - older backups remain on disk (for example `fixture_nodes_local_embeddings_2026-05-06.sqlite`) but the active registry entry points at the 2026-05-17 snapshot
  - 2026-06-12 review: regenerated the checkout-local effective backup after
    validation reported a stale schema missing `type_contains`; strict
    validation passed with `relations=61` and `roundtrip=ok`

## `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`

- File: `tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - legacy schema snapshot
  - no active consumers found during the 2026-03-20 review
- Tests using this fixture:
  - none
- Notes:
  - only a commented-out reference remains in [crates/ploke-rag/src/core/unit_tests.rs](../../crates/ploke-rag/src/core/unit_tests.rs)
  - keep under review until explicitly removed or reintroduced

## `ploke_db_primary_2026-05-06.sqlite`

- File: `tests/backup_dbs/ploke_db_primary_2026-05-06.sqlite`
- Parsed target(s): `crates/ploke-db`
- Expected DB config:
  - plain backup import
  - primary index created after import
  - recreated from the real `crates/ploke-db` source graph via `setup_db_full_crate("ploke-db")`
- Important tradeoff:
  - this is now a current-schema source-backed fixture, not a frozen user-repro snapshot
  - its contents will move with `crates/ploke-db` as that crate changes over time
- Tests using this fixture:
  - `ploke-tui`
    - [crates/ploke-tui/tests/get_code_edges_regression.rs](../../crates/ploke-tui/tests/get_code_edges_regression.rs): shared immutable DB via `shared_backup_fixture_db`

## `ws_fixture_01_canonical_2026-05-17.sqlite`

- File: `tests/backup_dbs/ws_fixture_01_canonical_2026-05-17.sqlite`
- Parsed target(s): `tests/fixture_workspace/ws_fixture_01`
- Expected DB config:
  - plain backup import
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - generated from the committed multi-member workspace fixture via the shared
    workspace-fixture recreation path
- Tests using this fixture:
  - `ploke-test-utils`
    - [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs): registry lookup and strict-load witness for the active workspace fixture
- Notes:
  - this fixture is the canonical plain workspace backup required by the
    workspace rollout readiness gate
  - the filename is dated `2026-05-17` because `cargo xtask recreate-backup-db`
    stamps outputs with UTC date

## `ws_fixture_01_member_single_2026-05-06.sqlite`

- File: `tests/backup_dbs/ws_fixture_01_member_single_2026-05-06.sqlite`
- Parsed target(s): `tests/fixture_workspace/ws_fixture_01/member_root`
- Expected DB config:
  - plain backup import
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - generated from the workspace fixture, but only includes the `member_root` crate
    plus workspace metadata (excluding `nested/member_nested`)
- Tests using this fixture:
  - `ploke-tui` command decision tree tests:
    - Single workspace member scenarios (focused crate)
    - "db already loaded and is: single crate and workspace" paths
- Notes:
  - this fixture simulates a workspace where only one member has been indexed
  - used for testing focused-crate operations within a multi-member workspace context
  - the filename is dated `2026-05-06` because `cargo xtask recreate-backup-db`
    stamps outputs with UTC date

## Corpus Type Graph Fixtures

These fixtures are source-pinned targets for DB-backed graphRAG type traversal
contracts. The checkout identity, backup stem, and test intent live in one
registry-backed place instead of in local symlinks under
`tests/fixture_github_clones/corpus`.

Run `cargo xtask recreate-backup-db
--fixture <id>` to clone or reuse the pinned checkout, check out the recorded
commit, parse and transform the crate with typed type graph relations enabled,
write the dated backup under the fixture's configured shared-snapshot path, and
verify that the generated backup imports with the current schema. Copy a
reviewed snapshot into `tests/backup_dbs/` only when it should become a
committed seed artifact.

The `corpus_*_openrouter_embeddings` variants use the same source-pinned
checkouts, then run the OpenRouter embedding indexer before backup. They are
intended for RAG and TUI tests that must exercise production search fixtures
without parsing or embedding during normal test execution.

Expected searchable corpus embedding config:

- import with `Database::import_backup_with_embeddings`
- provider: `openrouter`
- model: `mistralai/codestral-embed-2505`
- dims: `1536`
- dtype: `f32`
- vectors must be present and the active embedding set metadata must match the
  registry entry
- regeneration requires `OPENROUTER_API_KEY`; ordinary tests load the saved
  backup and do not call the remote provider
- regeneration batches corpus snippets in groups of `16` to stay below
  OpenRouter request-level token limits for large real crates

### `corpus_semver_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite`
- Parsed target: `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90`
- Checkout slug: `tests/fixture_github_clones/corpus/dtolnay__semver`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - graphRAG traversal from `matches_req` to `VersionReq` and `Version`
  - traversal from `VersionReq.comparators: Vec<Comparator>` to `Comparator`

### `corpus_semver_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_semver_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90`
- Checkout slug: `tests/fixture_github_clones/corpus/dtolnay__semver`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for named paths, references,
    and semver model targets

### `corpus_memchr_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_memchr_type_graph_2026-05-17.sqlite`
- Parsed target: `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905`
- Checkout slug: `tests/fixture_github_clones/corpus/BurntSushi__memchr`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - traversal from `memchr_iter` return types to iterator structs such as
    `Memchr`
  - later traversal from iterator self types to `Iterator` and
    `DoubleEndedIterator` impl surfaces

### `corpus_memchr_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_memchr_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905`
- Checkout slug: `tests/fixture_github_clones/corpus/BurntSushi__memchr`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for references and function
    pointer alias targets

### `corpus_generic_array_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite`
- Parsed target: `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23`
- Checkout slug: `tests/fixture_github_clones/corpus/fizyk20__generic-array`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - traversal from const-generic aliases to `GenericArray`
  - `ArrayBuilder::extend(..., source: impl Iterator<Item = T>)` reaches the
    local generic parameter `T`
  - later traversal through const-generic bounds and associated impls

### `corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23`
- Checkout slug: `tests/fixture_github_clones/corpus/fizyk20__generic-array`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for const-generic aliases,
    raw pointers, trait bounds, trait supers, generic parameter bounds, and
    where-clause owners
  - generic-array `ArrayBuilder::extend` impl-trait parameter coverage is
    DB-only because its terminal target is a generic parameter, not a RAG/TUI
    materialized target selector

### `corpus_chrono_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite`
- Parsed target: `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be`
- Checkout slug: `tests/fixture_github_clones/corpus/chronotope__chrono`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - traversal from `MappedLocalTime<T>` aliases to `LocalResult<T>`
  - later traversal from timezone API owners into their generic result model

### `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_chrono_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be`
- Checkout slug: `tests/fixture_github_clones/corpus/chronotope__chrono`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for named generic arguments,
    slices, arrays, tuples, associated type bounds, and where-bound type
    contexts

### `corpus_axum_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_axum_type_graph_2026-05-17.sqlite`
- Parsed target: `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- Checkout slug: `tests/fixture_github_clones/corpus/tokio-rs__axum`
- Selected workspace members:
  - `axum`
  - `axum-core`
  - `axum-macros`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - shared `TypeShapeCase` DB matrix coverage for trait object, impl trait,
    nested paren no-target, and nested macro no-target structures
  - `BoxedIntoRoute<S, E>(Box<dyn ErasedIntoRoute<S, E>>)` reaches
    `ErasedIntoRoute`
  - `Map.layer: Box<dyn LayerFn<E, E2>>` reaches `LayerFn`
  - `MakeErasedHandler::clone_box() -> Box<dyn ErasedIntoRoute<S, Infallible>>`
    reaches `ErasedIntoRoute`
  - `zip_longest(...) -> impl Iterator<Item = Item<I::Item>>` reaches the
    local `Item` enum
  - `StripPrefix::layer(...) -> impl Layer<S, Service = Self> + Clone` reaches
    `StripPrefix` through the associated type bound
  - `Option<&(dyn StdError + 'static)>` retains its nested `paren_type` without
    fabricating a terminal target for the paren wrapper
  - `Token![,]` under `Punctuated<syn::Variant, Token![,]>` retains a nested
    macro type without fabricating a terminal target

### `corpus_axum_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_axum_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- Checkout slug: `tests/fixture_github_clones/corpus/tokio-rs__axum`
- Selected workspace members:
  - `axum`
  - `axum-core`
  - `axum-macros`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for axum trait-object and
    impl-trait type contexts, including `BoxedIntoRoute`, `Map.layer`,
    `MakeErasedHandler::clone_box`, `zip_longest`, and `StripPrefix::layer`
  - parenthesized no-target coverage is DB-only because the paren wrapper is a
    nested structural node rather than a materializable search result

## `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`

- File: `tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`
- Parsed target(s): `crates/ploke-db`
- Expected DB config:
  - no current test contract
- Tests using this fixture:
  - none found during the 2026-03-20 review
- Notes:
  - treat this as orphaned until a concrete consumer is identified
  - remove or regenerate only after confirming it is not used outside the repo

## Update instructions

- When a fixture changes, update:
  - this document
  - [AGENTS.md](../../AGENTS.md)
  - the shared registry in [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
- Run `cargo xtask verify-backup-dbs` after schema or fixture changes.
- Use `cargo xtask recreate-backup-db --fixture <id>` instead of ad hoc copying
  whenever the registry already defines a recreation path.
- `tests/backup_dbs/` is ignored by default. Reviewed seed snapshots that
  should travel with a branch must be added explicitly rather than relying on
  normal `git status` output.
- If validation fails on a legacy backup only because `workspace_metadata` is
  missing, use `cargo xtask repair-backup-db-schema --fixture <id>` as the
  explicit schema repair path.
- Prefer adding or updating tests to consume the shared fixture registry instead of hard-coded
  backup paths.
- Prefer `shared_backup_fixture_db` for immutable shared callers and
  `fresh_backup_fixture_db` for isolated immutable callers before introducing a
  new crate-local lazy static.
- Prefer regenerating fixtures from repo code paths instead of copying ad hoc backups from the
  config dir without documentation.
