# Survey of types + Functions for multi_embedding feature

1. Metadata Database entry for overall embedding info

### Todos for this section
- Use consistent wrapper type `EmbeddingModelId` instead of `String` for `EmbeddingConfig.model_id`
- Add a `tracking_num` field to `HnswEmbedInfo` corresponding to the `n` used in the hnsw index creation of the form `emb_{model}_{dims}:hnsw_idx_{n}`
- Add a `hnsw_idx_rel` field to `HnswEmbedInfo` corresponding to the relation name of the associated hnsw index for those config settings

- `VectorDimensionSpec` ( will be renamed to `HnswEmbedInfo` soon )
```rust
#[derive(Clone, Debug)]
pub struct VectorDimensionSpec {
    dims: i64,
    provider: EmbeddingProviderSlug,
    embedding_model: EmbeddingModelId,
    offset: f32,
    hnsw_m: i64,
    hnsw_ef_construction: i64,
    hnsw_search_ef: i64,
}
```

- `EmbeddingSetId`
```rust
pub struct EmbeddingSetId {
    pub provider: EmbeddingProviderSlug,
    pub model: EmbeddingModelId,
    pub shape: EmbeddingShape,
    /// The name created by emb_{model}_{dims}, used as the relation name in the database for the
    /// vector embeddings generated from this embedding model.
    pub rel_name: EmbRelName
}
```

We have these two data structures, which seem to lean into the model info vs. encapsulate some model info as well as the hnsw settings for that model.

Rather than key everything by either one of these two data structures, since the embedding model is only ever used to create a set of vectors which will also have an hnsw index, we might as well use the `VectorDimensionSpec` as the primary way to refer to the vector embeddings.

Also, let's rename the `VectorDimensionSpec` to `HnswEmbedInfo` (done)

TODO:
- [ ] Restructure `HnswEmbedInfo` to remove `provider` and `embedding_model` and instead hold `EmbeddingSetId` in a field
- [ ] Add `hnsw_rel_name` to `HnswEmbedInfo` and builder.

- Question: Does HnswEmbedInfo need to know about `EmbeddingSource` from `ploke-embed`?

```rust
// ploke-embed::indexer
pub enum EmbeddingSource {
    Local(LocalEmbedder),
    HuggingFace(HuggingFaceBackend),
    OpenAI(OpenAIBackend),
    Cozo(CozoBackend),
    #[cfg(test)]
    Test(TestEmbeddingBackend),
}

// ploke-embed::local
pub struct LocalEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    config: EmbeddingConfig,
    max_length: usize,
    dimensions: usize,
}

#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub model_id: String,
    pub revision: Option<String>,
    pub device_preference: DevicePreference,
    pub cuda_device_index: usize,
    pub allow_fallback: bool,
    pub approximate_gelu: bool,
    pub use_pth: bool,
    pub model_batch_size: usize,   // NEW: Configurable batch size
    pub max_length: Option<usize>, // NEW: Optional max length override
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            revision: None,
            device_preference: DevicePreference::Auto,
            cuda_device_index: 0,
            allow_fallback: true,
            approximate_gelu: false,
            use_pth: false,
            model_batch_size: 8,
            max_length: None,
        }
    }
}
```

No, but we do want to make sure that the fields like `model_id`, and any other overlapping fields (I think that is the only one right now) are consistent between the two, and use the same types. For example, `model_id` could instead use the `EmbeddingModelId` type.

- We may also want to do something about the `dimensions` field in `LocalEmbedder` to add verification that the `LocalEmbedder` is using the same dimensions as the `EmbeddingSetId` or something. 

Because we have two data structures associated with a given hnsw index (the `HnswEmbedInfo` with specific settings, the `EmbeddingSetId` with general model info), we need to ensure that when we are creating or removing a hnsw index or removing a model from the database, we also clean up the corresponding `HnswEmbedInfo` or `EmbeddingSetId` corresponding to the removed item.

2. The embedding vector relation & hnsw index

The embedding vector relation itself will have a name of the form: `emb_{model}_{dims}`, and the hnsw index will have the form `emb_{model}_{dims}:hnsw_idx_{n}`, where `n` is the count of the number of hnsw indices created for that set of embedding vectors.

For example, we might have 

vector relation: `sentence-transformers/all-MiniLM-L6-v2` \
dims: `384` \
relation name: `sentence-transformers/all-MiniLM-L6-v2_384` 

Then if we made one hnsw index, with some standard settings and choosing, e.g. `hnsw_ef_construction: 16`, then that would be

hnsw index relation name: `sentence-transformers/all-MiniLM-L6-v2_384:hnsw_idx_0`

Then suppose we made another hnsw index with `hnsw_ef_construction: 12`, we would name the resulting hnsw index

hnsw index relation name: `sentence-transformers/all-MiniLM-L6-v2_384:hnsw_idx_1`

Due to this design, we will want to have a way to:

TODO: 
  - [ ]  Check the number of already present hnsw indices for a given
  `EmbeddingSetId`
  - [ ]  Remove all `hnsw` indices constructed for a given model
  - [ ] Check the `hnsw` settings used to create a given `hnsw` index set (which
  are stored in the `HnswEmbedInfo`)
    - [ ] Therefore we will want to ensure the `HnswEmbedInfo` stores the
    relation name for the hnsw set it was used to create for quick reference.

Our current methods around using the `ExperimentalVectorRelation`, which seems to be the data structure we made earlier to manage the vectors in the database, has more fields than strictly necessary, but it doesn't seem excessive e.g.

```rust
// in ploke/crates/ploke-db/src/multi_embedding/vectors.rs
    pub fn script_identity(&self) -> String {
        format!(
            "{} {{ node_id, embedding_model, provider, at => embedding_dims, vector }}",
            self.relation_name()
        )
    }
```

And the cozoscript to create the realtion at runtime has already been created correctly:

```rust
// in ploke/crates/ploke-db/src/multi_embedding/vectors.rs
    pub fn script_create(&self) -> String {
        format!(
            ":create {} {{ node_id: Uuid, embedding_model: String, provider: String, at: Validity => embedding_dims: Int, vector: <F32; {}> }}",
            self.relation_name(),
            self.dims
        )
    }
```

However, we don't really need both a `ExperimentalVectorRelation` and
`EmbeddingSetId`, so we are going to merge them into a single `EmbeddingSetId`
that will have all the necessary fields/methods to manage the relation for a given embedding vector.

### Needs verification: hnsw index creation with CozoScript

The new hnsw creation method, using CozoScript, currently looks like this:

```rust
// in ploke/crates/ploke-db/src/multi_embedding/adapter.rs
    fn create_idx(
        &self,
        emb_set: EmbeddingSetId,
        dims: i64,
        m: i64,
        ef_construction: i64,
        distance: HnswDistance,
    ) -> Result<(), DbError> {
        // NOTE: The [vector] part of the script below indicates that this query expects that the
        // target relation will have a field called "vector", which will contain the vector of the
        // expected dim and dtype.
        let rel_name = emb_set.rel_name;
        let script = format!(
            r#"
::hnsw create {rel_name}:vector_idx {{
    fields: [vector],
    dim: {dims},
    dtype: F32,
    m: {m},
    ef_construction: {ef_construction},
    distance: {distance}
}}
"#,
            dims = dims,
            m = m,
            ef_construction = ef_construction,
            distance = distance.as_str(),
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "create_idx",
                relation: rel_name.to_string(),
                details: err.to_string(),
            })
    }
```

This will need to be adjusted, and we will just want to verify that it can have
the expected name of the `rel_name` composed of `emb_{model}_{dims}` and then the
new name of the hnsw index, `hsnw_idx_{n}` where `n` indicates it is the n-th
hnsw index for this `rel_name` (starting with 0).

3. Creation of a set of vectors and associated hnsw

When the embedding vectors are created, we will need to know the details associated with the model. Reviewing the current pipeline beginning from when the event to index a workspace is received by the function in `dispatcher.rs` and calls `index_workspace` in `ploke-tui`:

3.1 A new `Arc<IndexerTask>` reference is created, where `IndexerTask` (defined in `ploke-embed`) is used to manage the overall events and running of the indexing process, and is defined as:

```rust
pub struct IndexerTask {
    pub db: Arc<Database>,
    pub io: IoManagerHandle,
    pub embedding_processor: Arc<EmbeddingProcessor>,
    /// Shape for all embedding vectors produced by this indexer.
    ///
    /// This is derived from the `EmbeddingProcessor` at construction time and
    /// used to enforce that every batch we write is homogeneous with respect
    /// to dimension and encoding, matching the multi-embedding DB helpers.
    pub embedding_shape: EmbeddingShape,
    /// Strongly-typed identifier for the embedding set this indexer writes to.
    ///
    /// This couples the runtime indexer configuration (provider/model) with the
    /// multi-embedding DB helpers that operate on per-set relations.
    pub embedding_set: EmbeddingSetId,
    #[cfg(feature = "multi_embedding")]
    pub vector_spec: HnswEmbedInfo,
    pub cancellation_token: CancellationToken,
    pub batch_size: usize,
    pub bm25_tx: Option<mpsc::Sender<bm25_service::Bm25Cmd>>,
    pub cursors: Mutex<HashMap<NodeType, Uuid>>,
    pub total_processed: AtomicUsize,
}
```

If we want to keep this architecture, which maintains some references to the
model embedding data, we will want to make sure we both document that this is
the source of the model info used to generate the embedding data, and ensure
that the `EmbeddingShape`, `EmbeddingSetId`, and `HnswEmbedInfo` are all
necessary separately.

We don't actually need all of these items, and we can derive the
`EmbeddingSetId` from the `EmbeddingProcessor`, since within
`EmbeddingProcessor`'s sub-fields we have access to:
- dims
- provider
- model

In order to create the `HnswEmbedInfo` we can use default settings or include
some user-configured `HnswEmbedInfo` items (e.g. if the user wants to specify
some settings for the creation of the graph). To make this possible, we can add
a `user_config_hnsw_builder` field to IndexerTask, which will hold the
user-specified fields and be a `HnswEmbedInfoBuilder` type with some number of
fields specified. That way we can optionally use the `user_config_hnsw_builder`
to build the resulting `HnswEmbedInfo`, or fall back on sane defaults set in
the builder.

- TODO:
  - [ ] remove `embedding_shape` from `IndexerTask`
  - [ ] add `user_config_hsnw_builder: HnswEmbedInfoBuilder` field to `IndexerTask`

The current data structures, using `Arc<IndexerTask>` in the `IndexWorkspace`
command, assumes that the underlying embedding model + dims are determined at
the beginning of the program when `IndexerTask` is created and the not changed
for the duration of the program. Now that we are adding a way to change the
configuration of a given `IndexerTask`, we will want to decide how to handle
this possibility.

Some ideas/concerns:
- We could add an `Arc<Mutex<HnswEmbedInfo>>` and similar for other embed data
  - pros: gives us ability to change underlying embed settings between indexing runs.
  - cons: need to handle the possibility of these values changing while indexing. there isn't a clear way to prevent this from happening, data-structure-wise.
- We could create the `IndexerTask` when we are going to run an `index_workspace`
  - pros: clean separation of concerns, no need for `Arc<Mutex<T>>`
  - cons: need to make sure `IndexerTask` doesn't *need* to exist before that for any reason.

It looks like `IndexerTask` is created at startup, then immediately put into `AppState`, but that it isn't capturing any important data that doesn't already exist in `AppState`, mostly just the `Arc` of various handles to other processes that are already being stored in `AppState`.

The primary difficulty seems to be that we would need to have mutable access to `AppState` in order to create the `IndexerTask`, while the dispatcher function and the index_workspace function both only need immutable access to `AppState`. The question would then be where to mutably change `AppState`.

Or maybe mutable access to `AppState` is the wrong approach. Maybe what we really need is to wrap `IndexerTask` in a `Mutex`, then instantiate it to run the indexing task.

- TODO: 
  - [ ] change the `indexer_task` field on `AppState` to `Arc<Mutex<IndexerTask>>`

4. Ensure consistent callsites

I'm noting some small but important variations in some of the new code in `ploke/crates/ploke-db/src/multi_embedding/` and `/crates/ploke-db/src/index/`, which could result in some annoying bugs (e.g. related to cozo and consistent naming)

- the `create_idx` method in `ExperimentalEmbeddingDatabaseExt` uses a cozo script that has `::hnsw create {rel_name}:vector_idx`, where most of the rest of the code base uses `hnsw_idx`
  - [ ] update to use the `hnsw_idx_{n}` naming convention

## Legacy/Migration issues

TODO:
  - [ ] update `clear_hnsw_idx` 
    - The clear_hnsw_idx() method just deletes all HNSW indices in the database
    - new method needs to be selective based on `{model}_{dims}:hnsw_idx_{n}`

## Embedding Model Configuration

Q: When should this configuration be loaded?

Q: Where are the other options persisted?

Q: How are other embedding model options presented?

- Use a similar approach to the general purpose model picker?
  - depends on the structure of the endpoint API, e.g. is there a query-able endpoint that provides details on the embedding model in a structured format? Is the structured format universal or dependent on the particular embedding model service router?
- Add specific endpoint support, with a limited number of supported embedding models?
- Create a kind of form that the user can use to create the new embedding model?
  - Would also need a verification interaction.
