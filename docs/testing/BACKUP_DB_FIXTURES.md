# Backup DB Fixtures

Last reviewed: 2026-04-03
Last updated: 2026-04-03

This document is the current inventory for backup database fixtures under
`tests/backup_dbs/`. It records which source targets produced each fixture,
which tests consume it, whether those tests expect mutable or immutable access,
and the DB-level assumptions that make those tests valid.

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
  - recreates automated fixtures to a new dated filename under
    `tests/backup_dbs/`
  - prints exact manual recreation steps for fixtures that are not hermetic yet
- `cargo xtask repair-backup-db-schema --fixture <id>`
  - repairs a stale legacy backup in place when it is missing the current
    `workspace_metadata` relation
  - use this for the specific schema-drift failure surfaced by
    `verify-backup-dbs`, not as a substitute for full regeneration

Operator workflow details live in
[docs/how-to/recreate-backup-db-fixtures.md](/home/brasides/code/ploke/docs/how-to/recreate-backup-db-fixtures.md).

## Shared helper API

Immutable backup consumers should load fixtures through the registry-backed
helpers in
[crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs):

- `shared_backup_fixture_db(&FIXTURE_...)`
  - loads, validates, and caches an immutable `Arc<Database>` for reuse
- `fresh_backup_fixture_db(&FIXTURE_...)`
  - creates a fresh in-memory `Database` from a registered fixture while still
    enforcing the registry’s import mode, embedding expectations, and index
    setup

The registry file is the source of truth for fixture metadata. Test code should
reference fixture constants there instead of hard-coding backup paths.

One exception currently remains for `ploke-db` lib-unit tests: because
`ploke-test-utils` depends on `ploke-db`, those unit-test modules cannot consume
`shared_backup_fixture_db(...)` directly without hitting a duplicate-crate type
split for `Database`. In that case, use the shared registry constant (for
example `FIXTURE_NODES_CANONICAL.path()`) with a crate-local loader.

## Fixture Summary

| Fixture | Parsed target(s) | Primary usage | Last update |
| --- | --- | --- | --- |
| `fixture_nodes_canonical_2026-04-01.sqlite` | `tests/fixture_crates/fixture_nodes` | canonical parsed `fixture_nodes` backup | 2026-04-01 |
| `fixture_nodes_local_embeddings_2026-04-01.sqlite` | `tests/fixture_crates/fixture_nodes` | local-embedding `fixture_nodes` backup | 2026-04-01 |
| `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` | `tests/fixture_crates/fixture_nodes` | legacy multi-embedding schema snapshot | 2026-03-20 |
| `ploke_db_primary_2026-03-21.sqlite` | `crates/ploke-db` | current-schema `ploke-db` graph backup | 2026-03-21 |
| `ws_fixture_01_canonical_2026-03-21.sqlite` | `tests/fixture_workspace/ws_fixture_01` | canonical plain backup of committed multi-member workspace fixture | 2026-03-21 |
| `ws_fixture_01_member_single_2026-04-03.sqlite` | `tests/fixture_workspace/ws_fixture_01/member_root` | single-member slice of workspace fixture | 2026-04-03 |
| `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45` | `crates/ploke-db` | currently orphaned snapshot | 2026-03-20 |

## `fixture_nodes_canonical_2026-04-01.sqlite`

- File: `tests/backup_dbs/fixture_nodes_canonical_2026-04-01.sqlite`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - plain backup import
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - used as the canonical parsed graph fixture for `fixture_nodes`
- Tests using this fixture:
  - `ploke-db`
    - [crates/ploke-db/src/utils/test_utils.rs](/home/brasides/code/ploke/crates/ploke-db/src/utils/test_utils.rs): shared mutable `Arc<Mutex<Database>>`
    - [crates/ploke-db/src/bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs): shared immutable `Arc<Database>` via crate-local loader keyed by `FIXTURE_NODES_CANONICAL`
    - [crates/ploke-db/src/index/hnsw.rs](/home/brasides/code/ploke/crates/ploke-db/src/index/hnsw.rs): fresh mutable DB per test
    - [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs): fresh mutable DB per test
    - [crates/ploke-db/benches/resolver_bench.rs](/home/brasides/code/ploke/crates/ploke-db/benches/resolver_bench.rs): immutable benchmark input
  - `ploke-rag`
    - legacy direct-path use removed; immutable consumers should use the local-embedding fixture helper instead
  - `ploke-tui`
    - [crates/ploke-tui/src/test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_harness.rs): shared mutable app harness DB
    - [crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs): shared mutable DB for live-tool scaffolding
    - [crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs): fixture dependency note, immutable expectations
- Notes:
  - older backups remain on disk (for example `fixture_nodes_canonical_2026-03-20.sqlite`) but the active registry entry points at the 2026-04-01 snapshot

## `fixture_nodes_local_embeddings_2026-04-01.sqlite`

- File: `tests/backup_dbs/fixture_nodes_local_embeddings_2026-04-01.sqlite`
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
    - [crates/ploke-rag/src/core/unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs): shared immutable DB plus fresh immutable imports via `fresh_backup_fixture_db`
    - [crates/ploke-rag/tests/integration_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/tests/integration_tests.rs): shared immutable DB via `shared_backup_fixture_db`
  - `ploke-tui`
    - [crates/ploke-tui/src/test_utils/new_test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/new_test_harness.rs): shared immutable headless harness DB via `shared_backup_fixture_db`
    - [crates/ploke-tui/tests/get_code_edges_regression.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/get_code_edges_regression.rs): shared immutable DB via harness
    - [crates/ploke-tui/tests/tool_ui_payload_fixture.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/tool_ui_payload_fixture.rs): shared immutable DB via harness
- Notes:
  - older backups remain on disk (for example `fixture_nodes_local_embeddings_2026-03-20.sqlite`) but the active registry entry points at the 2026-04-01 snapshot

## `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`

- File: `tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - legacy schema snapshot
  - no active consumers found during the 2026-03-20 review
- Tests using this fixture:
  - none
- Notes:
  - only a commented-out reference remains in [crates/ploke-rag/src/core/unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs)
  - keep under review until explicitly removed or reintroduced

## `ploke_db_primary_2026-03-21.sqlite`

- File: `tests/backup_dbs/ploke_db_primary_2026-03-21.sqlite`
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
    - [crates/ploke-tui/tests/get_code_edges_regression.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/get_code_edges_regression.rs): shared immutable DB via `shared_backup_fixture_db`

## `ws_fixture_01_canonical_2026-03-21.sqlite`

- File: `tests/backup_dbs/ws_fixture_01_canonical_2026-03-21.sqlite`
- Parsed target(s): `tests/fixture_workspace/ws_fixture_01`
- Expected DB config:
  - plain backup import
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - generated from the committed multi-member workspace fixture via the shared
    workspace-fixture recreation path
- Tests using this fixture:
  - `ploke-test-utils`
    - [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs): registry lookup and strict-load witness for the active workspace fixture
- Notes:
  - this fixture is the canonical plain workspace backup required by the
    workspace rollout readiness gate
  - the filename is dated `2026-03-21` because `cargo xtask recreate-backup-db`
    stamps outputs with UTC date

## `ws_fixture_01_member_single_2026-04-03.sqlite`

- File: `tests/backup_dbs/ws_fixture_01_member_single_2026-04-03.sqlite`
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
  - the filename is dated `2026-04-03` because `cargo xtask recreate-backup-db`
    stamps outputs with UTC date

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
  - [AGENTS.md](/home/brasides/code/ploke/AGENTS.md)
  - the shared registry in [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
- Run `cargo xtask verify-backup-dbs` after schema or fixture changes.
- Use `cargo xtask recreate-backup-db --fixture <id>` instead of ad hoc copying
  whenever the registry already defines a recreation path.
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
