# Structuring files + data structure locations

- `EmbeddingDbExt`: extension trait for vector-embedding aware database
  - ensure_relation_registered
    - checks if vector embedding relation exists,
      creates the relation if it is not already present
  - assert_vector_column_layout
    - verifies vector dims are of expected length

## `emb_utils.rs`

- Trait: DbEmbUtils
  - `upsert_vector_values`

- Trait: VectorEmbeddingStatusExt
  - `embedded_ids_for_vectors`
  - count_embedded_for_vector
  - pending_ids_for_type
  - count_pending_for_type
