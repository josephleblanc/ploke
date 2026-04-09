# Recreate Backup DB Fixtures

Use the shared registry in
[crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
as the source of truth for fixture ids, paths, validation rules, and
recreation mode.

## Validate current fixtures

Run this before changing fixture-dependent tests or after schema changes:

```bash
cargo xtask verify-backup-dbs
```

To validate one fixture only:

```bash
cargo xtask verify-backup-dbs --fixture fixture_nodes_canonical
```

Validation is strict. It checks:

- the registered backup exists
- it imports using the registry-configured import mode
- registry contract checks still pass after import
- the backup can be saved and re-imported in a roundtrip
- the roundtrip reload still satisfies the fixture contract

If validation fails specifically with:

```text
Cannot find requested stored relation 'workspace_metadata'
```

the backup predates the current schema relation set. Use the explicit repair
path:

```bash
cargo xtask repair-backup-db-schema --fixture <id>
```

This command is intentionally narrow. It restores the existing backup, adds
only the missing `workspace_metadata` relation with the real schema code, and
writes the repaired backup back to the registered fixture path.

## Recreate a fixture

Use:

```bash
cargo xtask recreate-backup-db --fixture <id>
```

The command has two modes:

- automated: writes a new dated backup under `tests/backup_dbs/`
- manual: prints exact fixture-specific steps when the fixture is not
  hermetically reproducible yet

New outputs use dated names like:

```text
tests/backup_dbs/<stem>_2026-03-20.sqlite
```

After generating a new dated backup, update:

- [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
- [docs/testing/BACKUP_DB_FIXTURES.md](../testing/BACKUP_DB_FIXTURES.md)

## Repair a stale legacy backup in place

Use:

```bash
cargo xtask repair-backup-db-schema --fixture <id>
```

Use this only when the failure mode is the missing `workspace_metadata`
relation on an otherwise valid backup. This is an explicit migration for the
current schema addition, not a general-purpose backup upgrader.

## Current fixture modes

### `fixture_nodes_canonical`

```bash
cargo xtask recreate-backup-db --fixture fixture_nodes_canonical
```

This is automated.

Why the automation uses `setup_db_full_multi_embedding("fixture_nodes")`:

- the fixture is still imported by tests as a plain backup
- but the canonical snapshot is meant to reflect the current schema shape
- regenerating it through `setup_db_full_multi_embedding(...)` preserves the
  current multi-embedding schema relations expected by downstream tests without
  trying to seed non-hermetic local vectors

### `fixture_nodes_local_embeddings`

```bash
cargo xtask recreate-backup-db --fixture fixture_nodes_local_embeddings
```

This is automated.

Current behavior:

- recreates `fixture_nodes` from source
- runs the local embedder/indexer to populate vectors
- forces CPU device selection during regeneration
- rejects the output if any non-file nodes remain unembedded

Residual limitation:

- the local embedder still uses the default model revision, which is not pinned
  in repo configuration yet

### `ploke_db_primary`

```bash
cargo xtask recreate-backup-db --fixture ploke_db_primary
```

This now recreates the active `ploke_db_primary` backup directly from the real
`crates/ploke-db` source graph using the shared
`setup_db_full_crate("ploke-db")` test helper, then validates it as a plain
backup fixture.

### `ws_fixture_01_canonical`

```bash
cargo xtask recreate-backup-db --fixture ws_fixture_01_canonical
```

This is automated.

Current behavior:

- parses the committed workspace fixture at `tests/fixture_workspace/ws_fixture_01`
- transforms its `workspace_metadata` row plus all member crate graphs into a
  fresh DB
- writes a plain backup snapshot that preserves the current workspace schema
  relations without requiring embeddings

## Review rule

Before changing fixture consumers or adding a new backup, update the inventory
in [docs/testing/BACKUP_DB_FIXTURES.md](../testing/BACKUP_DB_FIXTURES.md)
and keep the registry in sync.

## After Deleting Backups

If you intentionally remove the committed backup DB files and want to restore
the active fixture set from scratch, use this sequence:

```bash
cargo xtask verify-backup-dbs
cargo xtask recreate-backup-db --fixture fixture_nodes_canonical
cargo xtask recreate-backup-db --fixture fixture_nodes_local_embeddings
cargo xtask recreate-backup-db --fixture ploke_db_primary
cargo xtask recreate-backup-db --fixture ws_fixture_01_canonical
cargo xtask setup-rag-fixtures
```

Notes:

- `verify-backup-dbs` shows which registered fixtures are missing or invalid.
- `setup-rag-fixtures` is needed after recreating the local-embedding fixture so
  the config-dir copy used by some RAG/TUI paths is refreshed.
- Re-run `cargo xtask verify-backup-dbs` after recreation to confirm the active
  fixture set is healthy.
