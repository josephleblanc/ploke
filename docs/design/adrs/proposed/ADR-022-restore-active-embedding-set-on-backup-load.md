# ADR-022: Restore Active Embedding Set on Backup Load

## Status
PROPOSED

## Context
- Backups taken after indexing with a runtime-selected embedding set (provider/model/dims) are restored while `Database.active_embedding_set` is still the default local MiniLM (384). The load path (`/load <crate>`) calls `create_index_primary_with_index`, which always uses the current active set, so it creates/validates indices for the default relation instead of the set stored in the backup.
- The embedder runtime is likewise not rehydrated; searches and any reindexing after restore target the wrong relation (usually empty), making it appear that the load failed.
- The backup contains `embedding_set` rows and vector relations, but there is no persisted “active set” marker and no logic to pick the right set on load.

## Decision
- Persist the chosen embedding set alongside the backup (hash/provider/model/dims/rel_name), e.g., in a DB meta row or crate-context-adjacent record, so load can deterministically select it.
- After `import_from_backup`, enumerate available embedding sets and their data (vector rows, HNSW presence). If exactly one populated set exists, set `db.active_embedding_set` to it and rebuild/verify indices for that set only. If multiple populated sets exist, require explicit user selection (CLI flag or prompt) before indexing/searching.
- Change index helpers to accept an explicit `EmbeddingSet` rather than implicitly reading the current active set; the load path must pass the restored set to index creation/validation.
- Rehydrate the embedder runtime to match the restored set when the provider/model is available; otherwise, warn and keep the DB active set so searches over restored vectors still work, while marking embedding generation as unavailable until a compatible provider is configured.
- Avoid creating default-set indices during load unless the default set is actually chosen; keep the default only as a fallback when no vectors exist for any set.

## Consequences
- **Positive**: Restored databases immediately use the vectors/HNSW that were saved, eliminating silent fallbacks to empty/default relations. Search and similarity behave consistently across restarts. Index creation becomes deterministic and parameterized by the intended embedding set.
- **Negative**: Adds a metadata write on save and a selection step on load when multiple sets are present. Requires embedder-runtime reconfiguration logic on load and handling for missing providers/models.
- **Risks/mitigations**: Multiple-set backups need clear UX for selection; embedder availability must be surfaced as warnings, not crashes. Default-set fallback must be explicit to avoid masking missing data.

## Implementation Notes
- Add a small meta record (or extend `crate_context`/`db_meta`) that stores the active embedding set hash/provider/model/dims at save time; persist within the backup so no sidecar file is needed.
- On load: (1) read meta/`embedding_set` rows, (2) pick/set the active set (auto if single populated; else require choice), (3) pass that set into index creation/validation, (4) reconfigure the embedder runtime if possible.
- Tests: restore a backup containing a non-default set and assert the active set switches, searches hit restored vectors, and no default HNSW is created unless chosen explicitly.
