# todo

## ploke-tui

  - [ ] change the `indexer_task` field on `AppState` to `Arc<Mutex<IndexerTask>>`

  - [ ] remove `embedding_shape` from `IndexerTask`
  - [ ] add `user_config_hsnw_builder: HnswEmbedInfoBuilder` field to `IndexerTask`

## ploke-db
- [ ] Survey the functions related to embeddings in ploke-db

- [ ] Remove/Restructure to remove duplicate data structures
  - [ ] Merge data structures `EmbeddingSetId` and `ExperimentalVectorRelation`
  - [ ] Verify `EmbeddingSetId` has fields/methods
    - [ ] derives name correctly, with the name `{model}_{dims}`

  - [ ] update `clear_hnsw_idx` 
    - The clear_hnsw_idx() method just deletes all HNSW indices in the database
    - new method needs to be selective based on `{model}_{dims}:hnsw_idx_{n}`

  - [ ]  Check the number of already present hnsw indices for a given
  `EmbeddingSetId`
  - [ ]  Remove all `hnsw` indices constructed for a given model
  - [ ] Check the `hnsw` settings used to create a given `hnsw` index set (which
  are stored in the `HnswEmbedInfo`)
    - [ ] Therefore we will want to ensure the `HnswEmbedInfo` stores the
    relation name for the hnsw set it was used to create for quick reference.

- [ ] Assess required database methods to manage embeddings
- [ ] (if necessary) Add new embeddings management functions
- [ ] add tests for each database method managing embeddings (happy + sad path)
- [ ] run tests and debug
- [ ] where possible, add multi_embedding versions of current ploke-db API
- [ ] run tests and debug

## ploke-transform
- [ ] add multi_embedding path that does not have `embedding` field in primary node types
- [ ] run tests and debug

## ploke-embed
- [ ] run tests in ploke-embed with/without multi_embedding cfg
- [ ] assess any failures and identify areas in need of attention re: multi_embedding
- more here
