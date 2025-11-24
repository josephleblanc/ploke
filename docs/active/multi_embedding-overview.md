 Pipeline Walkthrough

  - DB + schema bootstrap: Database::init_with_schema creates an in-memory Cozo DB
    and calls ploke_transform::schema::create_schema_all to register all relations
    (crates/ploke-db/src/database.rs:203-236, crates/ingest/ploke-transform/src/
    schema/mod.rs:67-110). With multi_embedding enabled this also creates the
    *_multi_embedding metadata relations.
  - Code graph → Cozo rows: transform_parsed_graph orchestrates inserts for every node
    kind (types, functions, defined types, traits, impls, modules, consts, statics,
    macros, imports, edges, crate_context) by delegating to per-node transformers that
    emit schema :put scripts (crates/ingest/ploke-transform/src/transform/mod.rs:131-
    172; crate context write is transform_crate_context, .../crate_context.rs:8-30).
    Each transformer relies on its schema helper (e.g., StructNodeSchema::script_put)
    to materialize rows.
  - Fetch items needing embeddings: IndexerTask::next_batch walks
    NodeType::primary_nodes() and uses Database::get_rel_with_cursor to pull rows
    where embedding is null and to paginate by UUID (crates/ingest/ploke-embed/src/
    indexer/mod.rs:722-828; crates/ploke-db/src/database.rs:1143-1273). Snippets
    are fetched via IoManagerHandle::get_snippets_batch before embedding (indexer/
    mod.rs:840-906).
  - Generate + persist embeddings: EmbeddingProcessor::generate_embeddings returns
    vectors; IndexerTask::process_batch wraps them into EmbeddingInsert records and
    calls Database::update_embeddings_batch_for_set, which validates dimensions and
    funnels into update_embeddings_batch (crates/ingest/ploke-embed/src/indexer/
    mod.rs:907-969; crates/ploke-db/src/database.rs:785-810, 644-739). Today this:
      - Writes legacy vectors back into each primary relation’s embedding column (only
        when the vector length matches LEGACY_EMBEDDING_DIMS, currently 384).
      - When multi_embedding is enabled at runtime, dual-writes vectors into the per-
        dimension experimental vector relations via write_multi_embedding_relations
        (crates/ploke-db/src/database.rs:1466-1516), creating emb_<model>_<dims>
        relations on demand with ExperimentalVectorRelation.
  - HNSW index creation: Legacy path uses create_index (per node type, hard-coded
    fields: [embedding], dim: 384) and create_index_primary to build all indices
    (crates/ploke-db/src/index/hnsw.rs:767-817, 819-830). There’s also an older helper
    Database::index_embeddings that emits ::hnsw create <rel>:embedding_idx {dim:
    <arg>, fields: [embedding] ...} (crates/ploke-db/src/database.rs:606-619).
  - HNSW queries: Legacy search uses hnsw_of_type (one relation) and hnsw_all_types
    (fan-out) (crates/ploke-db/src/index/hnsw.rs:32-127), returning Embedding
    { id, name, distance }. Multi-embedding search paths exist but are gated:
    multi_embedding_hnsw_of_type loops over sample dimension specs, and
    multi_embedding_hnsw_for_set targets a specific embedding set relation and index
    (crates/ploke-db/src/index/hnsw.rs:609-729), relying on vector relations + indexes
    created via ExperimentalEmbeddingDatabaseExt::create_idx (crates/ploke-db/src/
    multi_embedding/adapter.rs:200-234).

  Multi-Embedding Migration Hotspots

  - Storage model shift: Today primary relations still own an embedding column;
    *_multi_embedding relations hold only metadata (embeddings: [(model, dims)]), and
    vector payloads live in ad-hoc emb_<model>_<dims> relations. All ingest/update/
    query code (e.g., get_rel_with_cursor, update_embeddings_batch, hnsw_of_type)
    still key off the legacy column, so they will miss data once embeddings are
    removed from primaries.
  - Dimension awareness: update_embeddings_batch only writes legacy vectors when
    len==384 and uses dimension_spec_for_length (sample table) for multi-embedding
    writes; indexes are created with a hard-coded dim: 384. Moving to arbitrary
    dimensions requires passing the actual EmbeddingSetId/shape through every call
    (update, index, search) and dropping the 384 assumption.
  - Index lifecycle: Legacy HNSW indexes are named <rel>:hnsw_idx with fields:
    [embedding]. Under multi-embedding, each emb_<model>_<dims> needs its
    own :vector_idx plus per-node-type filter (or join) back to metadata; missing
    indexes currently just return empty vectors in multi_embedding_hnsw_*.
  - Intake for “unembedded” detection: get_rel_with_cursor/get_unembedded_node_data
    filter on is_null(embedding). With multi-embedding, “missing” should mean “no
    vector row for (node_id, embedding_set)” or “metadata row lacks this set,” not a
    null field on the primary table.
  - Metadata coherence: write_multi_embedding_relations upserts vector rows but never
    updates the embeddings list in the *_multi_embedding metadata tables, so schema
    truth may drift when we stop dual-writing the legacy column.

  Gaps & Proposed ploke-db API Additions

  - Set-scoped writes: Add Database::upsert_embedding_set_batch(set: EmbeddingSetId,
    ty: NodeType, vectors: &[(Uuid, Vec<f32>)]) that (1) ensures the metadata
    relation for ty exists, (2) upserts metadata embeddings entry for the set, and
    (3) upserts vectors into the correct emb_<model>_<dims> relation (no legacy column
    dependency). Backed by a strongly-typed struct carrying provider/model/dims to
    avoid stringly plumbing.
  - Unembedded discovery by set: New get_unembedded_for_set(ty, set_id, limit,
    cursor) that checks for absence of a vector row (or metadata flag) rather than
    is_null(embedding). This unblocks the indexer from the legacy column during
    migration.
  - Index management per set: Helpers like ensure_hnsw_index_for_set(ty,
    set_id, hnsw_params) and drop_hnsw_index_for_set(...) that call
    ExperimentalEmbeddingDatabaseExt::create_idx/::hnsw drop on
    emb_<model>_<dims>:vector_idx, driven by VectorDimensionSpec instead of the 384
    default. Pair with a list_hnsw_indexes() to audit state.
  - Search API parity: Expose search_similar_for_set(ty, set_id, query_vec, k, ef)
    that wraps multi_embedding_hnsw_for_set and returns typed hits; deprecate the
    legacy hnsw_of_type once embeddings leave the primary tables.
  - Metadata introspection: Methods to enumerate available embedding sets per
    node type (read *_multi_embedding.embeddings) and to assert schema alignment
    (assert_vector_column_layout, vector_spec_for_set) so callers can select valid
    sets before indexing/searching.
  - Schema refresh utilities: A migration-friendly helper to remove or ignore the
    legacy embedding column (or at least stop writing to it) once dual-write is no
    longer needed, plus a tool to backfill metadata rows from existing vectors if we
    cut over a live DB.

  If you want, I can draft a small design note that sequences these API changes
  against the migration milestones so the indexer + TUI can flip to the per-set
  relations without losing HNSW coverage.
