# Backup DBs Progress 2026-03-20 Post Local Fixture Validation

- `fixture_nodes_local_embeddings` is now recreated in-repo via
  `cargo xtask recreate-backup-db --fixture fixture_nodes_local_embeddings`.
- The regenerated dated backup was adopted as the active registered fixture at
  `tests/backup_dbs/fixture_nodes_local_embeddings_2026-03-20.sqlite`.
- Strict validation passed:
  - `cargo xtask verify-backup-dbs --fixture fixture_nodes_local_embeddings`
  - result: `relations=43 | roundtrip=ok`
- Validation outcome by crate:
  - `ploke-db`: full `--all-targets` suite passed
  - `ploke-rag`: full `--all-targets` suite passed
  - `ploke-tui`: the fixture-backed RAG/apply-edit paths now pass; remaining failures are isolated
    to `tests/get_code_edges_regression.rs` and the stale `ploke_db_primary` backup fixture

## Remaining Narrow Gap

- `ploke_db_primary` is still an active stale backup fixture.
- Current failing tests:
  - `code_item_edges_returns_edges_for_ploke_db_primary_node`
  - `code_item_edges_returns_edges_for_database_struct_in_ploke_db`
- Failure mode:
  - `Cannot find requested stored relation 'workspace_metadata'`

## Recommended Next Step

- Recreate `ploke_db_primary` at a new dated path, update the registry/docs, and rerun the
  `ploke-tui` `get_code_edges_regression` tests.

## Reproducibility Note

- `fixture_nodes_local_embeddings` is materially better now:
  - forced CPU during recreation
  - explicit failure if any non-file nodes remain unembedded
- It is still not fully hermetic because the local embedder revision is not
  pinned yet. The recreate command uses the default local model id but does not
  lock to a specific upstream model revision.
