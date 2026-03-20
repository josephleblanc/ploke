# Local Embedding Fixture Feasibility 2026-03-20

## Recommendation

Keep the `fixture_nodes` local-embedding backup fixture manual for now.

Reason:
- The repo has a working local embedding pipeline, but it is not hermetic in its
  current form.
- `LocalEmbedder::new(EmbeddingConfig::default())` loads model artifacts through
  `hf_hub` with `revision: None`, so the workflow depends on external model
  availability and an unpinned upstream revision.
- `EmbeddingConfig::default()` also uses `DevicePreference::Auto`, which can
  select GPU when available. That is a poor default for committing regenerated
  fixture data because CPU/GPU differences can introduce drift.

In other words: automation is possible from repo code, but not yet hermetic
enough for a committed regeneration workflow without first pinning the model
revision and forcing a deterministic execution mode.

## What exists today

### Fresh DB creation from source

- [crates/test-utils/src/lib.rs](/home/brasides/code/ploke/crates/test-utils/src/lib.rs)
  - `setup_db_full_multi_embedding("fixture_nodes")`
  - This builds the parsed graph into a fresh DB and creates the
    multi-embedding relations / default embedding-set metadata.
  - It does **not** populate vector rows by itself.

### Local embedding generation path

- [crates/ingest/ploke-embed/src/local/mod.rs](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/local/mod.rs)
  - `LocalEmbedder::new`
  - Uses `hf_hub::api::sync::Api` and fetches:
    - `config.json`
    - `tokenizer.json` or `tokenizer.model`
    - `model.safetensors` or `pytorch_model.bin`
  - Default config is:
    - model id `sentence-transformers/all-MiniLM-L6-v2`
    - `revision: None`
    - `DevicePreference::Auto`

- [crates/ingest/ploke-embed/src/indexer/mod.rs](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs)
  - `EmbeddingProcessor`
  - `EmbeddingSource::Local`
  - `IndexerTask::new(...).run(...)`
  - This is the existing repo-local end-to-end path for reading snippets and
    writing embeddings back into the DB.

### Existing tests that prove the path works

- [crates/ingest/ploke-embed/src/indexer/unit_tests.rs](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/unit_tests.rs)
  - `test_local_model_config`
  - `test_local_model_embedding_processor`
  - `test_next_batch("fixture_nodes")`
  - `test_next_batch_ss("fixture_nodes")`

These tests show that `fixture_nodes` can be parsed, fed through the local
embedder, and written back into the DB offline *assuming the model artifacts are
available or can be fetched*.

## Narrowest existing automation path

If you were willing to accept the current non-hermetic dependency on `hf_hub`,
the narrowest existing API path is:

1. Create the source-derived DB:
   - `ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")`
2. Wrap it:
   - `ploke_db::Database::new(...)`
3. Create the embedder/runtime:
   - `LocalEmbedder::new(EmbeddingConfig { ... })`
   - `EmbeddingProcessor::new(EmbeddingSource::Local(...))`
   - `EmbeddingRuntime::from_shared_set(...)`
4. Run the existing indexing pipeline:
   - `IndexerTask::new(...).run(...)`
5. Persist the backup:
   - `db.backup_db(path)`

Code references:
- [crates/test-utils/src/lib.rs](/home/brasides/code/ploke/crates/test-utils/src/lib.rs)
- [crates/ingest/ploke-embed/src/indexer/unit_tests.rs](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/unit_tests.rs)
- [crates/ingest/ploke-embed/src/indexer/mod.rs](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs)
- [crates/ploke-db/src/database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)

## Why I would not automate it yet

The current local-embedding generation path is not stable enough for fixture
regeneration because:

- Model revision is not pinned:
  - [crates/ingest/ploke-embed/src/local/mod.rs](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/local/mod.rs)
- Model artifacts are not vendored in-repo:
  - the code fetches via `hf_hub`
- Device choice is not fixed:
  - default `DevicePreference::Auto`

That means a future regeneration could succeed but produce different vectors or
fail depending on host/cache/network state.

## Minimal changes needed before automating

If this fixture should become hermetically regenerable, the minimum bar looks
like:

1. Pin the local model revision used for committed fixtures.
2. Force CPU for regeneration.
3. Decide whether model artifacts must be vendored or whether a pre-populated
   cache is an acceptable repo policy.
4. Only then add an `xtask` command that runs the existing
   `setup_db_full_multi_embedding` + `IndexerTask::run` path and writes a backup.
