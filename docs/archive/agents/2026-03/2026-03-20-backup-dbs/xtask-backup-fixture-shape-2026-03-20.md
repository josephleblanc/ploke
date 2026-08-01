# Xtask Backup Fixture Shape 2026-03-20

## Summary

The current `xtask` support is too narrow for backup DB lifecycle management:

- `cargo xtask verify-fixtures` only checks one backup DB path plus unrelated JSON fixtures.
- `cargo xtask setup-rag-fixtures` only stages one `fixture_nodes_*` backup into the config dir for
  `load_db(...)` prefix-based discovery.
- There is no existing non-interactive `xtask` command that creates a backup DB from a crate or
  workspace source target.

The feasible implementation path is to make `xtask` drive repo-local creation and validation
directly from Rust code, not through the interactive TUI `/index start` + `/save db` workflow.

## Existing code paths

### What exists today

- [xtask/src/main.rs](../../../../xtask/src/main.rs)
  - `verify-fixtures`
  - `setup-rag-fixtures`
  - no backup regeneration command
- [crates/ploke-tui/src/app_state/database.rs](../../../../crates/ploke-tui/src/app_state/database.rs)
  - `save_db(...)` writes to the config dir and derives the filename from `system.crate_focus`
  - `load_db(...)` restores from the config dir using filename-prefix search
- [crates/ploke-db/src/multi_embedding/db_ext.rs](../../../../crates/ploke-db/src/multi_embedding/db_ext.rs)
  - `load_db(...)` has the same config-dir + prefix-discovery behavior
- [crates/test-utils/src/lib.rs](../../../../crates/test-utils/src/lib.rs)
  - `setup_db_full(...)`
  - `setup_db_full_multi_embedding(...)`
  - these already create fresh in-memory DBs directly from source fixtures
- [crates/ingest/syn_parser/src/lib.rs](../../../../crates/ingest/syn_parser/src/lib.rs)
  - `parse_workspace(...)`
- [crates/ingest/ploke-transform/src/transform/workspace.rs](../../../../crates/ingest/ploke-transform/src/transform/workspace.rs)
  - `transform_parsed_workspace(...)`

### Why the TUI route is a poor primary regeneration path

- `/index start` is only available through the interactive TUI command loop.
- `/save db` writes to the user config dir, not directly to `tests/backup_dbs`.
- `save_db(...)` still depends on app state (`system.crate_focus`) and backup naming derived from
  the active DB contents.
- `load_db(...)` and `setup-rag-fixtures` still rely on prefix matching in the config dir, which is
  intentionally an ambient workflow rather than a hermetic test-fixture path.

That makes the TUI flow suitable as a manual fallback, but not the clean implementation base for
`xtask`.

## Recommended xtask commands

### `cargo xtask verify-backup-dbs`

Purpose:
- Check every registered backup fixture in the shared registry.
- Fail fast with fixture-specific remediation instructions.

Inputs:
- none by default
- optional `--fixture <id>` to scope validation to one fixture

Checks that are feasible from repo code:
- file exists at the expected path
- backup imports into a fresh `Database::init_with_schema()`
- for `BackupWithEmbeddings` fixtures, `import_backup_with_embeddings(...)` succeeds
- relation count after import is non-zero
- save roundtrip succeeds:
  - import into fresh DB
  - write a temp backup via Cozo backup API
  - import that temp backup into a second fresh DB
- expected embedding contract:
  - expected active set can be restored or set
  - expected vector relation exists
  - embedding count for expected set is non-zero when `vectors_present == true`
- expected index preconditions:
  - if `requires_primary_index`, `create_index_primary(...)` succeeds after import

Checks that are probably not worth baking in initially:
- exact node counts or relation counts
- exact HNSW persistence expectations, because HNSW is rebuilt post-import in current tests

### `cargo xtask recreate-backup-db --fixture <id>`

Purpose:
- Rebuild one registered fixture from repo source targets and write a new dated backup artifact.

Recommended behavior:
- read the fixture definition from the shared registry
- create a fresh DB from source, not from an existing backup
- write to `tests/backup_dbs/<base_name>_<yyyy-mm-dd>` or a similarly dated filename
- print explicit follow-up instructions to update:
  - the registry
  - `docs/testing/BACKUP_DB_FIXTURES.md`
  - tests that reference the old filename if the canonical name changes

Feasible build strategies:
- for crate fixtures like `tests/fixture_crates/fixture_nodes`:
  - reuse `ploke_test_utils::setup_db_full(...)`
  - or `setup_db_full_multi_embedding(...)` when vectors / embedding relations are required
- for workspace fixtures:
  - use `parse_workspace(...)` + `transform_parsed_workspace(...)`
- for live crate snapshots like `crates/ploke-db`:
  - direct source parse/transform is feasible if the intended fixture is “current crate graph”
  - if the fixture is intentionally a user-repro snapshot rather than a canonical current graph,
    this command should refuse and print manual regeneration steps instead

### `cargo xtask print-backup-db-help --fixture <id>`

Purpose:
- Print the exact manual recreation steps when automation is not appropriate yet.

This is useful for:
- repro fixtures that intentionally mirror a user session
- fixtures whose embedding model or provider setup is not fully reproducible offline

## Main blockers / decisions

### 1. Canonical vs repro fixtures

Not every backup should be recreated the same way.

- `fixture_nodes_*` is a canonical fixture target and should be recreated from repo source.
- `ploke-db_642a4b75-...` looks like a user-repro snapshot used by
  [crates/ploke-tui/tests/get_code_edges_regression.rs](../../../../crates/ploke-tui/tests/get_code_edges_regression.rs).
  That may need a documented manual path unless you explicitly want to redefine it as a
  canonical “current `crates/ploke-db` parse” fixture.

### 2. Dated filenames vs stable references

The task asks for dated fixture names. That is straightforward for generation, but it means either:

- the registry becomes the stable source of truth and tests stop hard-coding filenames, or
- every regeneration requires editing all consumer paths

The first option is cleaner and matches the shared-registry direction already underway.

### 3. Embedding fixture regeneration scope

Local embedding fixtures are feasible if the repo already has the necessary local embedder path and
the generation step can populate vectors deterministically enough for tests.

If that path is flaky or slow, a practical split is:
- automate plain parsed-graph backups first
- keep embedding-backed fixture regeneration manual but validated

## Recommendation

Implement `xtask` around the shared fixture registry, with this order:

1. `verify-backup-dbs` against all registered fixtures
2. `recreate-backup-db --fixture <id>` for canonical source-backed fixtures
3. `print-backup-db-help --fixture <id>` for repro/manual fixtures

Do not make the interactive TUI backup flow the primary regeneration mechanism. It is useful as a
manual escape hatch, but the repo already has enough direct parser/transform/setup code to build a
more hermetic `xtask` path.
