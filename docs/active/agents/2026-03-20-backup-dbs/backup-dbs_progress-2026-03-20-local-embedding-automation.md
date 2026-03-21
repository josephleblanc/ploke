# Backup DBs Progress 2026-03-20: Local Embedding Automation

- Switched `fixture_nodes_local_embeddings` in
  [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
  from manual recreation metadata to an automated fixture strategy.
- Added a bounded `xtask` automation path in
  [xtask/src/main.rs](/home/brasides/code/ploke/xtask/src/main.rs) that:
  - seeds the `fixture_nodes` database via `setup_db_full_multi_embedding(...)`
  - runs the local embedder/indexer on forced CPU to populate vectors
  - rejects the output if any non-file nodes remain unembedded
  - writes the dated backup and validates it through the existing fixture contract
- Updated `setup-rag-fixtures` in
  [xtask/src/main.rs](/home/brasides/code/ploke/xtask/src/main.rs)
  to source the local-embedding backup from the fixture registry instead of a stale hard-coded path.
- The regenerated dated backup was adopted as the active registered local fixture:
  - `tests/backup_dbs/fixture_nodes_local_embeddings_2026-03-20.sqlite`
- Validation after adoption:
  - `cargo xtask verify-backup-dbs --fixture fixture_nodes_local_embeddings` passed
  - `cargo test -p ploke-db --all-targets -- --nocapture` passed
  - `cargo test -p ploke-rag --all-targets -- --nocapture` passed
  - `ploke-tui` fixture-backed RAG/apply-edit tests now pass; only the separate
    `ploke_db_primary` repro fixture remains stale

## Residual Limitation

- The recreation path is still not fully hermetic because the local embedder
  uses the default model id with `revision: None`; device selection is pinned to
  CPU now, but the exact upstream model revision is not yet pinned.
