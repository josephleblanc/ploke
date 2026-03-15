# `ploke-rag` Fixture Configuration

## Summary

`ploke-rag` test failures after moving or recloning the repo were caused by the tests loading
`fixture_nodes` from two different places:

- The canonical repo backup used by older test helpers:
  `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- The user config-dir load path used by `ploke_db::multi_embedding::db_ext::load_db`:
  `$XDG_CONFIG_HOME/ploke/data` or `~/.config/ploke/data`

The failing library tests were using the config-dir path indirectly, not the repo fixture path.

## What Went Wrong

`ploke-rag/src/core/unit_tests.rs` previously created a fresh in-memory schema via
`ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")` and then called:

```rust
ploke_db::multi_embedding::db_ext::load_db(&db, "fixture_nodes".to_string()).await?;
```

`load_db` does not read from `tests/backup_dbs`. It scans the config dir for the first filename
with the `fixture_nodes_` prefix. After the repo moved, the config dir still contained multiple
matching backups:

- `fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- `fixture_nodes_3b3551b2-a061-5bee-96e4-b24e5a4361c9`

Those backups were created with different embedding setups. The `ploke-rag` tests instantiate the
default local embedder:

- provider: `local`
- model: `sentence-transformers/all-MiniLM-L6-v2`
- dimensions: `384`

When `load_db` selected a backup whose active/populated embedding set did not match that shape, the
dense search path failed with:

```text
Database error: query vector dimension mismatch
```

That then broke dense search directly and also hybrid/BM25 fallback paths that depend on dense
search as a fallback.

## Why The Repo Fixture Was Not Enough

The repo already had the expected local backup in `tests/backup_dbs`, but the unit tests were not
bound to it. They relied on ambient state in `~/.config/ploke/data`, so a stale backup outside the
repo could silently change test behavior.

This is why the failures showed up after a clone/location change even though the repository fixture
files themselves were still present.

## Fix Applied

`ploke-rag` tests now:

1. Import the canonical repo backup directly from
   `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`.
2. Verify that the default local embedding set is populated in that backup.
3. Force the active embedding set to the local 384-dimension set used by the test embedder.
4. Rebuild the matching HNSW index before search assertions run.

This keeps the tests hermetic and preserves the original behavior they were intended to cover.

## Implications For `xtask`

If a workflow still depends on `load_db("fixture_nodes")`, the config dir must not contain
multiple ambiguous `fixture_nodes_*` backups.

Use:

```bash
cargo xtask setup-rag-fixtures
```

That command:

- copies the canonical local backup into the config-dir load path, and
- moves conflicting `fixture_nodes_*` backups into a quarantine folder so prefix matching stops
  selecting the wrong database.

## Recommendation

For tests, prefer explicit repo-relative fixture paths over config-dir discovery.

For interactive workflows that must use config-dir discovery, ensure setup automation stages one
canonical `fixture_nodes_*` backup per intended embedding model or make `load_db` select by exact
embedding-set metadata instead of prefix alone.
