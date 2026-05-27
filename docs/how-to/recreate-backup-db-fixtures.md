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

- automated checkout-local fixtures: writes a root-scoped backup under
  `tests/backup_dbs/local/`
- automated shared-snapshot fixtures: writes a new dated backup under the
  shared DB snapshot fixture directory
- automated legacy registered-path fixtures: writes a new dated backup under
  `tests/backup_dbs/`
- manual: prints exact fixture-specific steps when the fixture is not
  hermetically reproducible yet

Checkout-local outputs use names like:

```text
tests/backup_dbs/local/<stem>__root-<workspace-root-hash>.sqlite
```

Checkout-local files are local to the checkout/worktree because the DB rows may
contain absolute crate or workspace roots. Registry loads prefer the
checkout-local file when it exists, otherwise they fall back to the committed
registered path and `verify-backup-dbs` reports root-mismatch failures with a
regeneration hint. Path-only consumers such as `setup-rag-fixtures` use
`FixtureDb::checked_path()` so that the fallback path is still imported and
validated before it is copied elsewhere.

If `XDG_CONFIG_HOME` is unset, the shared directory is
`~/.config/ploke/db_snapshot_fixtures`. Set `PLOKE_DB_SNAPSHOT_FIXTURE_DIR` to
override the path for tests or unusual local setups.

Shared-snapshot outputs use dated names like:

```text
$XDG_CONFIG_HOME/ploke/db_snapshot_fixtures/<stem>_2026-05-17.sqlite
```

Use shared snapshots for source-pinned corpus fixtures and other fixtures whose
contract is not tied to the current checkout root. Do not use shared snapshots
for checkout-local fixtures whose DB rows encode absolute paths from the
current worktree.

To ensure current active and typed graph fixtures are available without
sharing checkout-local DB rows:

```bash
cargo xtask fixtures ensure --snapshots
```

This command runs active fixture validation in the normal profile and then
invokes a typed-only xtask pass for typed graph fixtures. Checkout-local
fixtures are created or repaired under `tests/backup_dbs/local/`; only
shared-snapshot fixtures are staged into the shared DB snapshot directory.

To regenerate every automated registered fixture in its scope-specific output
location:

```bash
cargo xtask fixtures regenerate --all
```

Use `--active` or `--typed` instead of `--all` to regenerate only that subset.
The command skips manual legacy/orphaned snapshots. Active checkout-local
fixtures are written under `tests/backup_dbs/local/`; shared typed corpus
fixtures are written under the shared DB snapshot directory. `--all` runs
active fixtures in the normal profile and then invokes a typed-only xtask pass
for the typed graph fixtures.

GitHub corpus fixtures prepare their source checkouts through the shared
fixture cache before generating the DB. Set `PLOKE_FIXTURE_HOME` to override
the source cache location; otherwise xtask stores corpus mirrors and checkouts
under `<db_snapshot_fixtures>/_source_cache`.

To prepare registered typed corpus sources without generating new backup DBs:

```bash
cargo xtask fixtures ensure --typed
```

After generating a new registered-path or shared-snapshot dated backup that
should become the registered fixture, update:

- [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
- [docs/testing/BACKUP_DB_FIXTURES.md](../testing/BACKUP_DB_FIXTURES.md)

For checkout-local outputs, do not update the committed registry path just to
make the current worktree pass. The root-scoped local filename is the active
path for that checkout.

For shared-snapshot outputs, update the registry and docs when the new dated
backup should become the registered fixture. Copy the reviewed snapshot into
`tests/backup_dbs/` only when you want to commit it as a seed artifact for
other machines or CI.

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
- A path-bound fixture can be structurally valid but still fail validation if
  its stored roots point at another checkout. Regenerate that fixture in the
  current worktree instead of copying a backup from another path.
- `setup-rag-fixtures` is needed after recreating the local-embedding fixture so
  the config-dir copy used by some RAG/TUI paths is refreshed.
- Re-run `cargo xtask verify-backup-dbs` after recreation to confirm the active
  fixture set is healthy.
