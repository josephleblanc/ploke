## Investigation: Partial Embedding Updates and Background Indexing (2025-11-18)

### Context and Current Behavior

- **Change detection and partial graph updates**
  - `scan_for_change` in `app_state/database.rs` is the primary entrypoint for detecting file-level changes and driving partial re-indexing:

```318:525:crates/ploke-tui/src/app_state/database.rs
pub(super) async fn scan_for_change(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_tx: oneshot::Sender<Option<Vec<std::path::PathBuf>>>,
) -> Result<(), ploke_error::Error> {
    use ploke_error::Error as PlokeError;
    let guard = state.system.read().await;
    // TODO: Make a wrapper type for this and make it a method to get just the crate
    // name.
    // 1. Get the currently focused crate name, checking for errors.
    let crate_path = guard.crate_focus.as_ref().ok_or_else(|| {
        error!("Missing crate focus, cannot scan unspecified target crate");
        let e = PlokeError::from(StateError::MissingCrateFocus {
            msg: "Missing crate focus is None, cannot scan unspecified target crate",
        });
        e.emit_warning();
        e
    })?;
    let crate_name = crate_path.file_name().and_then(|os_str| os_str.to_str()).ok_or_else(|| { 
        error!("Crate name is empty, cannot scan empty crate name");
        let e = PlokeError::from(StateError::MissingCrateFocus {msg: "Missing crate focus is empty or non-utf8 string, cannot scan unspecified target crate"});
        e.emit_warning();
        e
    })?;

    info!("scan_for_change in crate_name: {}", crate_name);
    // 2. get the files in the target project from the db, with hashes
    let file_data = state.db.get_crate_files(crate_name)?;
    trace!(target: SCAN_CHANGE, "file_data: {:#?}", file_data);

    // 2.5. Check for files that have been removed
    let (file_data, removed_file_data): ( Vec<_>, Vec<_> ) = file_data.into_iter().partition(|f| f.file_path.exists());

    // 3. scan the files, returning a Vec<Option<FileData>>, where None indicates the file has not
    //    changed.
    //  - Note that this does not do anything for those files which may have been added, which will
    //  be handled in parsing during the IndexFiles event process mentioned in step 5 below.
    let result = state.io_handle.scan_changes_batch(file_data).await.inspect_err(|e| {
            error!("Error in state.io_handle.scan_changes_batch: {e}");
    })?;
    let vec_ok = result?;

    if !vec_ok.iter().any(|f| f.is_some()) && removed_file_data.is_empty() {
        // 4. if no changes, send complete in oneshot
        match scan_tx.send(None) {
            Ok(()) => {
                info!("No file changes detected");
            }
            Err(e) => {
                error!("Error sending parse oneshot from ScanForChange");
            }
        };
    } else {
        // 5. if changes, send IndexFiles event (not yet made) or handle here.
        //  Let's see how far we get handling it here first.
        //  - Since we are parsing the whole target in any case, we might as well do it
        //  concurrently. Test sequential approach first, then move to be parallel earlier.

        // TODO: Move this into `syn_parser` probably
        // WARN: Just going to use a quick and dirty approach for now to get proof of concept, then later
        // on I'll do something more efficient.
        let ParserOutput { mut merged, tree } =
            run_parse_no_transform(Arc::clone(&state.db), Some(crate_path.clone()))?;
        // ...

        // filter nodes
        merged.retain_all(filtered_union);

        transform_parsed_graph(&state.db, merged, &tree).inspect_err(|e| {
            error!("Error transforming partial graph into database:\n{e}");
        })?;

        for file_id in module_uuids {
            for node_ty in NodeType::primary_nodes() {
                info!("Retracting type: {}", node_ty.relation_str());
                let query_res = state
                    .db
                    .retract_embedded_files(file_id, node_ty)
                    .inspect_err(|e| error!("Error in retract_embed_files: {e}"))?;
                trace!("Raw return of retract_embedded_files:\n{:?}", query_res);
                // ...
            }
        }

        trace!("Finishing scanning, sending message to reindex workspace");
        event_bus.send(AppEvent::System(SystemEvent::ReIndex {
            workspace: crate_name.to_string(),
        }));
        let _ = scan_tx.send(Some(changed_filenames));
        // TODO: Add validation step here.
    }

    Ok(())
}
```

- **What `scan_for_change` already does for “partial updates”**
  - It narrows change scope to **files**:
    - Uses `get_crate_files(crate_name)` as authoritative file list.
    - Uses `io_handle.scan_changes_batch` to detect changed vs unchanged files, and tracks removed files.
    - Re-parses the crate via `run_parse_no_transform(db, Some(crate_path))` but then aggressively slices the parsed graph down to:
      - modules corresponding to changed files (via `module_uuids` → `ModuleNodeId`),
      - contained nodes (via `mods_in_file` / `ModuleTree` traversal),
      - and a PrimaryNode-only “union” of those node IDs.
    - Calls `transform_parsed_graph(&state.db, merged, &tree)` with a pruned graph, so only nodes in changed files are transformed back into the DB.
  - It **invalidates embeddings** only for nodes in changed files:
    - Iterates `module_uuids` (changed files) and `NodeType::primary_nodes()`.
    - For each `(file_id, node_type)`, uses `retract_embedded_files(file_id, node_ty)` to null out legacy embeddings for nodes tied to that file.
    - Triggers a `SystemEvent::ReIndex { workspace: crate_name }`, which causes the indexing path to pick up “pending” (null-embedding) nodes only.
  - Net effect: embeddings for unchanged files are preserved; only nodes in changed files are marked as needing re-embedding and will be picked up by `get_unembedded_node_data` / `update_embeddings_batch` during the next indexing run.

- **Indexing and embedding pipeline**
  - `/index start …` maps to `index_workspace` in `app_state/handlers/indexing.rs`, which:

```13:124:crates/ploke-tui/src/app_state/handlers/indexing.rs
pub async fn index_workspace(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    workspace: String,
    needs_parse: bool,
) {
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);
    let target_dir = {
        match state.system.read().await.crate_focus.clone() {
            Some(path) => path,
            None => {
                match std::env::current_dir() {
                    Ok(current_dir) => {
                        let mut pwd = current_dir;
                        pwd.push(&workspace);
                        pwd
                    }
                    Err(e) => {
                        tracing::error!("Error resolving current dir: {e}");
                        return;
                    }
                }
            }
        }
    };

    // Set crate focus to the resolved target directory and update IO roots
    {
        let mut system_guard = state.system.write().await;
        system_guard.crate_focus = Some(target_dir.clone());
    }
    state
        .io_handle
        .update_roots(
            Some(vec![target_dir.clone()]),
            Some(SymlinkPolicy::DenyCrossRoot),
        )
        .await;

    if needs_parse {
        match run_parse(Arc::clone(&state.db), Some(target_dir.clone())) {
            Ok(_) => tracing::info!(
                "Parse of target workspace {} successful",
                &target_dir.display()
            ),
            Err(e) => {
                tracing::info!("Failure parsing directory from IndexWorkspace event: {}", e);
                return;
            }
        }
    }

    // ...

    if let Some(indexer_task) = state_arc
        && let Ok((callback_manager, db_callbacks, unreg_codes_arc, shutdown)) =
            ploke_db::CallbackManager::new_bounded(Arc::clone(&indexer_task.db), 1000)
    {
        // Spawns the IndexerTask that drives embedding updates
        let res = tokio::spawn(async move {
            let indexing_result = ploke_embed::indexer::IndexerTask::index_workspace(
                indexer_task,
                workspace,
                progress_tx,
                progress_rx,
                control_rx,
                callback_handler,
                db_callbacks,
                counter,
                shutdown,
            )
            .await;
            // ...
        })
        .await;
        // ...
    }
}
```

  - `ploke-db::Database` exposes the helpers that determine which nodes need embedding vs are already embedded:

```867:973:crates/ploke-db/src/database.rs
pub fn get_nodes_ordered(&self, nodes: Vec<Uuid>) -> Result<Vec<EmbeddingData>, PlokeError> { /* ... */ }

pub fn get_unembed_rel(
    &self,
    node_type: NodeType,
    limit: usize,
    cursor: usize,
) -> Result<TypedEmbedData, PlokeError> {
    let rel_name = node_type.relation_str();
    let base_script = Self::build_unembedded_batch_script(rel_name, "is_null(embedding)", "");
    // ...
    let mut v = QueryResult::from(query_result).to_embedding_nodes()?;
    #[cfg(feature = "multi_embedding_db")]
    if self.multi_embedding_db_enabled() {
        if let Some(spec) = experimental_spec_for_node(node_type) {
            spec.metadata_schema.ensure_registered(self)?;
            let runtime_ids = self.runtime_embedded_ids(spec)?;
            v.retain(|entry| !runtime_ids.contains(&entry.id));
        }
    }
    v.truncate(limit.min(count_less_flat));
    let ty_embed = TypedEmbedData { v, ty: node_type };
    Ok(ty_embed)
}
```

  - HNSW index creation and search for dense embeddings is handled in `index/hnsw.rs`:

```32:42:crates/ploke-db/src/index/hnsw.rs
pub fn hnsw_all_types(
    db: &Database,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> { /* ... */ }
```

```67:76:crates/ploke-db/src/index/hnsw.rs
pub fn hnsw_of_type(
    db: &Database,
    ty: NodeType,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    #[cfg(feature = "multi_embedding_db")]
    if db.multi_embedding_db_enabled() {
        return multi_embedding_hnsw_of_type(db, ty, k, ef);
    }
    // Legacy single-embedding-column HNSW search ...
}
```

```220:355:crates/ploke-db/src/index/hnsw.rs
pub struct SimilarArgs<'a> {
    pub db: &'a Database,
    pub vector_query: &'a Vec<f32>,
    pub k: usize,
    pub ef: usize,
    pub ty: NodeType,
    pub max_hits: usize,
    pub radius: f64,
}

#[instrument(skip_all, fields(query_result))]
pub fn search_similar_args(args: SimilarArgs) -> Result<EmbedDataVerbose, ploke_error::Error> {
    // Cozo script that joins HNSW search results with ancestor/module/file_mod for snippet retrieval
}
```

  - TUI-side embedding search for chat currently *assumes* dense embeddings are primary:

```57:88:crates/ploke-tui/src/app_state/handlers/embedding.rs
async fn embedding_search_similar(
    state: &Arc<AppState>,
    context_tx: &mpsc::Sender<RagEvent>,
    new_msg_id: Uuid,
    embeddings: Vec<f32>,
) -> color_eyre::Result<()> {
    let ty_embed_data =
        search_similar(&state.db, embeddings, 100, 200, NodeType::Function).emit_error()?;
    // ...
}
```

  - BM25 is implemented in `ploke-db::bm25_index` and wired to Cozo via `upsert_bm25_doc_meta_batch`:

```1342:1385:crates/ploke-db/src/database.rs
/// Upsert BM25 document metadata in a batch transaction
pub fn upsert_bm25_doc_meta_batch(
    &self,
    docs: impl IntoIterator<Item = (Uuid, DocMeta)>,
) -> Result<(), DbError> {
    // ...
    self.run_script(script, params, cozo::ScriptMutability::Mutable)
        .map_err(|e| DbError::Cozo(e.to_string()))?;
    Ok(())
}
```

  - Cozo-side HNSW index creation for multi-embedding relations is handled via `ExperimentalVectorRelation` and `create_multi_embedding_indexes_for_type`:

```16:21:crates/ploke-db/src/multi_embedding/vectors.rs
#[derive(Copy, Clone, Debug)]
pub struct ExperimentalVectorRelation<'a> {
    dims: i64,
    relation_base: &'a str,
}
```

```475:503:crates/ploke-db/src/index/hnsw.rs
#[cfg(feature = "multi_embedding_db")]
fn create_multi_embedding_indexes_for_type(db: &Database, ty: NodeType) -> Result<(), DbError> {
    if !db.multi_embedding_db_enabled() {
        return Ok(());
    }
    let Some(spec) = experimental_spec_for_node(ty) else {
        return Ok(());
    };
    spec.metadata_schema.ensure_registered(db)?;
    for dim_spec in vector_dimension_specs() {
        let relation = ExperimentalVectorRelation::new(dim_spec.dims(), spec.vector_relation_base);
        relation.ensure_registered(db)?;
        if let Err(err) = db.create_idx(
            &relation.relation_name(),
            dim_spec.dims(),
            dim_spec.hnsw_m(),
            dim_spec.hnsw_ef_construction(),
            HnswDistance::L2,
        ) {
            let msg = err.to_string();
            if msg.contains("already exists") {
                continue;
            } else {
                return Err(err);
            }
        }
    }
    Ok(())
}
```

### 1. Targeted Partial Embedding Updates

#### How close we are today

- **Already implemented: file-scoped invalidation of embeddings**
  - `scan_for_change` already implements a coarse-grained version of “partial embedding updates”:
    - Change detection is per-file; only changed and removed files are considered.
    - The parse/transform step is scoped by:
      - computing module IDs per changed file,
      - recursively walking the `ModuleTree` via `mods_in_file` to gather contained nodes,
      - filtering the merged graph to nodes in the resulting union set, and
      - re-running `transform_parsed_graph` for that subset only.
    - After the transform, `retract_embedded_files(file_id, node_ty)` clears embeddings only for nodes tied to changed files.
    - Indexing then uses `get_unembedded_node_data` (and underlying `get_unembed_rel`) to drive re-embedding for null-embedding nodes only.
  - In practice, this already yields:
    - **No re-parse of unchanged files** (parse is crate-wide, but graph filtering means we do not re-transform unchanged nodes).
    - **No re-embedding of unchanged nodes** (their embeddings remain non-null and are not re-enqueued).

- **Gaps vs a more granular “code item diff” approach**
  - The current `scan_for_change` operates at **file granularity**, not at “AST item changed vs unchanged”:
    - If *any* part of a file changed, all nodes in that file are considered part of the affected set and have embeddings retracted.
    - There is no diff between the old AST and new AST at the node level to decide “this function’s body is unchanged, leave its embedding intact”.
  - `Database::get_nodes_by_file_with_cursor` is stubbed out with `todo!()` and a WARN comment indicating this was envisioned as a more flexible partial-update helper:

```1143:1151:crates/ploke-db/src/database.rs
/// Gets the primary node typed embed data needed to update the nodes in the database
/// that are within the given file.
/// Note that this does not include the module nodes for the files themselves.
/// This is useful when doing a partial update of the database following change detection in
/// previously parsed and inserted files.
// WARN: This needs to be tested
#[allow(unreachable_code)]
pub fn get_nodes_by_file_with_cursor(
    &self,
    node_type: NodeType,
    limit: usize,
    cursor: Uuid,
) -> Result<TypedEmbedData, PlokeError> {
    todo!();
    // ...
}
```

  - There is no Cozo script yet that:
    - takes a file identifier (`file_mod`/`module` ownership) plus a cursor, and
    - returns only “changed” nodes within that file based on a hash or similar signal.
  - The change-detection logic in `io_handle.scan_changes_batch` only reports **file-level** hash changes (based on `FileData`), not a per-node semantic diff.

#### Likely impacted files/functions for a more granular approach

- **TUI / change detection and partial transform**
  - `crates/ploke-tui/src/app_state/database.rs`
    - `scan_for_change` (change detection, partial graph transform, embedding invalidation).
    - `save_db` / `load_db` (must stay consistent with embedding and HNSW index behavior when restoring backups).
  - `crates/ploke-tui/src/app_state/handlers/indexing.rs`
    - `index_workspace` (drives indexing following `SystemEvent::ReIndex`).
  - `ploke_io::IoManagerHandle` (via `scan_changes_batch`) – currently signals per-file changes; a more granular approach might need:
    - additional metadata (per-file hash plus maybe per-node hashes),
    - or at least more structured change information for downstream logic.

- **DB / embedding selection and pending queues**
  - `crates/ploke-db/src/database.rs`
    - `get_unembed_rel`, `get_unembedded_node_data`, `count_pending_embeddings` and friends (determine which nodes need re-embedding).
    - `get_nodes_ordered` (used for snippet retrieval given node IDs).
    - `get_nodes_by_file_with_cursor` (currently `todo!()`, likely the right place for a “nodes in file X that need embedding” API).
    - `upsert_bm25_doc_meta_batch` (needs to stay aligned with whatever notion of “document versions” we use if we move to a more granular or incremental embedding model).

- **Multi-embedding wiring**
  - `crates/ploke-db/src/index/hnsw.rs`
    - `hnsw_of_type`, `create_index_primary`, and multi-embedding-specific helpers (must remain consistent when embeddings for some nodes are invalidated).
  - `crates/ploke-db/src/multi_embedding/vectors.rs` and `crates/ploke-db/src/multi_embedding/seeding.rs`
    - shape of per-dimension vector tables and metadata schema; if partial embedding updates are ever extended to multi-embedding, the contracts here must stay “append-only” or be updated carefully.

#### Design options and risks for more targeted partial updates

- **Option A: Stick with file-level granularity (current behavior, refined)**
  - **Behavior**: Keep `scan_for_change` as the main orchestrator; treat “any change in file” as “rebuild embeddings for all nodes in that file”.
  - **Work required**:
    - Improve documentation and tests around the current behavior (especially in `multi_embedding_runtime_db_tests` and new integration tests that assert:
      - changed file → embeddings nullified for nodes in that file only;
      - unchanged files → embeddings preserved).
    - Implement and test `get_nodes_by_file_with_cursor` as a *file-scoped* helper for incremental embedding reads (not necessarily node-level diff).
  - **Risks / trade-offs**:
    - Over-embeds nodes in large files with small edits.
    - Simpler mental model and lower risk of DB/coherence bugs; the path is already partially validated by `test_update_embed` in `app_state/database.rs`.

- **Option B: Node-level diffing between old and new AST for changed files**
  - **Behavior**:
    - Use `run_parse_no_transform` to produce an AST for the crate as today.
    - Query the DB (via a new helper or extension of `get_nodes_by_file_with_cursor`) to get the “old” node set per file (including names, spans, tracking hashes).
    - Compute a diff per file:
      - nodes that are identical (same identity + hash/span) → keep embeddings.
      - nodes that changed or are new → retract embeddings.
      - nodes that disappeared → mark as deleted.
    - Restrict `transform_parsed_graph` and `retract_embedded_files` to nodes flagged as changed/new/deleted rather than all nodes in the file.
  - **Likely contract points**:
    - Extend `FileData` / DB schema to carry a stable “per-node tracking hash” that can be derived from source content (you already have `tracking_hash` on nodes).
    - Implement Cozo queries that, given a `file_mod` or `module` ID, return node IDs + tracking hashes in DB for direct comparison.
    - Potentially add a new DB helper:
      - e.g. `get_nodes_in_file_with_hashes(file_id) -> Vec<NodeHashInfo>`.
  - **Risks / complexity**:
    - Higher risk of subtle bugs where we think a node is unchanged but its semantics differ (e.g., changes in dependencies, type environment).
    - Need to define “hash” semantics carefully (span-based vs structure-based).
    - Must keep multi-embedding metadata (per-model dimension specs) consistent when nodes are partially re-embedded.
    - More moving parts in Cozo scripts, making live debugging harder.

- **Option C: Hybrid approach – file-level invalidation, node-level prioritization**
  - **Behavior**:
    - Keep file-level `retract_embedded_files` semantics (any change in file invalidates all embeddings in the file), but:
      - Introduce node-level “last embedded at” or “change weight” metadata.
      - Use this metadata to prioritize which nodes within changed files get re-embedded first.
    - This does not reduce embedding volume but can make background re-embedding more responsive.
  - **Risks**:
    - Additional metadata complexity without reducing total work.

**Recommendation for partial updates**:

- **Near-term**: Treat the current file-level approach as “good enough partial embedding” and:
  - Tighten tests around `scan_for_change` and `test_update_embed` to explicitly document behavior.
  - Implement `get_nodes_by_file_with_cursor` in a conservative, file-scoped way (no semantic diff; just “nodes in file X that currently need embedding”).
- **Later** (if needed): Explore node-level diffing (Option B) once:
  - you have clear requirements for how much over-embedding is acceptable, and
  - there is capacity to reason about and test Cozo scripts that track change provenance per node.

### 2. Background Embedding Processing and BM25-First Behavior

#### Current search behavior and coupling

- **Chat-side retrieval assumes dense embeddings as primary**:
  - `embedding_search_similar` calls `ploke_db::search_similar` directly and then uses `io_handle.get_snippets_batch` to fetch code:

```57:88:crates/ploke-tui/src/app_state/handlers/embedding.rs
let ty_embed_data =
    search_similar(&state.db, embeddings, 100, 200, NodeType::Function).emit_error()?;
// ...
let snippets = state
    .io_handle
    .get_snippets_batch(ty_embed_data.v)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|r| r.ok())
    .collect::<Vec<String>>();
```

  - There is no runtime fallback to BM25 when embeddings are missing or incomplete.

- **Indexer and DB assume “pending embeddings” as the driver for vector work**
  - `Database::get_unembed_rel` and `count_pending_embeddings` are the main ways to know “what still needs vector embeddings”.
  - `IndexerTask::index_workspace` (in `ploke-embed`) orchestrates embedding generation for nodes returned by these helpers.

- **BM25 is present but not the default in the TUI**
  - The underlying BM25 index exists (`bm25_index::bm25_service`, `upsert_bm25_doc_meta_batch`), but:
    - TUI retrieval (`embedding.rs`) does not call BM25 at all.
    - There is no “bm25 search first, then vector if available” path in the RAG service or embedding handler.
  - Docs explicitly call out the desire to make BM25 default:

```10:12:docs/active/TECH_DEBT.md
## Search
* [ ] set up bm25 as the default for search, then use vector embedding if possible, but keep bm25 as fallback.
```

#### Desired behavior for background embedding

- **High-level goal**:
  - Allow:
    - **BM25-only search** when vector embeddings are not yet available or incomplete.
    - **Vector search + BM25** when embeddings are present, without requiring embeddings to be fully up-to-date for every node in the crate.
  - Let embedding work run **in the background**, so that:
    - `/index start <crate>` can return control once the DB graph and (possibly) BM25 metadata are ready.
    - Ongoing embedding updates (as computed by `IndexerTask`) do not block user queries.

- **More advanced goal**:
  - While background embedding is running, allow BM25 search to:
    - surface results from unchanged code (nodes with stable embeddings),
    - optionally combine with vector search that **excludes nodes in currently-changing files**, to avoid mixing stale and fresh embeddings.

#### Likely impacted areas for background embedding and BM25-first

- **TUI / RAG layer**
  - `crates/ploke-tui/src/app_state/handlers/embedding.rs`
    - would need a new decision layer:
      - if vector embeddings are “ready” for enough nodes, run dense search (current behavior).
      - otherwise, or in parallel, run BM25 search (new behavior).
  - `crates/ploke-rag/src/core/mod.rs` and `crates/ploke-rag/src/lib.rs`
    - likely the best place to centralize a “search strategy” abstraction (BM25-only, vector-only, hybrid).
  - `crates/ploke-tui/src/test_harness.rs` and `tests` (new integration tests):
    - scenarios where:
      - BM25 alone produces results before embedding finishes.
      - vector search takes over or augments once embeddings are available.

- **Indexer task and background lifecycle**
  - `crates/ingest/ploke-embed/src/indexer/mod.rs`
    - `IndexerTask::index_workspace` already runs asynchronously, but:
      - `/index start` UX currently treats completion as “indexing done”.
      - For true background embedding, you may want:
        - a separation between “parse + DB graph + BM25 metadata ready” and “embeddings still being computed”.
        - better progress reporting through `IndexingStatus` (e.g., counts of pending embeddings).
  - `crates/ploke-tui/src/lib.rs` and `AppState` construction:
    - embedding processor, indexer task, and RAG service lifetimes need to support:
      - embedding work continuing after initial `/index start`.
      - search requests during that time.

- **DB / readiness and capability checks**
  - `crates/ploke-db/src/index/hnsw.rs`
    - `hnsw_all_types` and `hnsw_of_type` surfaced errors when HNSW indexes are missing; today, these often become warnings with `PlokeDb` errors.
    - For BM25-first, you probably want an explicit “is HNSW available for this type?” helper to gate vector search.
  - `crates/ploke-db/src/bm25_index/mod.rs` and `bm25_service.rs`
    - ensure BM25 index/metadata is populated as part of indexing, even if embeddings lag behind.
  - `crates/ploke-db/src/database.rs`
    - may need lightweight introspection APIs, e.g.:
      - count of embedded vs unembedded nodes per type,
      - ability to restrict “vector search” to nodes not currently marked for re-embedding (e.g., by file or flag).

#### Design options for BM25-first and background embedding

- **Option A: Simple fallback – BM25 when vector search fails**
  - **Behavior**:
    - Keep `embedding_search_similar` as-is, but wrap the `search_similar` call:
      - if `search_similar` returns an error indicating missing HNSW index or zero hits with a known “not ready” shape, then:
        - fall back to BM25 search.
    - BM25 results could be turned into `EmbeddingData`-like structures so downstream code stays uniform.
  - **Pros**:
    - Minimal surface-area change.
    - Easy to implement and test.
  - **Cons**:
    - Does not explicitly prioritize BM25; vector search is still the first attempt.
    - Does not support hybrid BM25 + vector results.

- **Option B: Strategy-based search (BM25-first, with optional vector augmentation)**
  - **Behavior**:
    - Introduce a `SearchStrategy` in `ploke-rag` (or TUI layer) with variants:
      - `Bm25Only`, `VectorOnly`, `Hybrid` (BM25-first or vector-first, configurable).
    - At query time:
      - Check embedding readiness metrics (e.g., `count_pending_embeddings`, HNSW index presence).
      - If embeddings are sparse or HNSW missing, run BM25; otherwise, run vector search, optionally merging with BM25.
    - For hybrid mode:
      - Run BM25 against the full corpus.
      - Run vector search against:
        - all nodes (simpler), or
        - only nodes not in “currently changing files” (requires file-scoped flags or filters).
      - Merge results with a scoring strategy (e.g., interleaving or weighted ranking).
  - **Pros**:
    - Makes BM25-first a deliberate, testable choice.
    - Can be extended to express more advanced behaviors (e.g., provider-specific embeddings).
  - **Cons**:
    - Requires more design in `ploke-rag` and TUI integration.
    - Needs solid test coverage across strategies.

- **Option C: Background embedding with explicit readiness states**
  - **Behavior**:
    - Treat embedding indexing as a long-running background job:
      - `/index start` ensures:
        - DB schema and base relations exist.
        - BM25 metadata is populated (or at least scheduled).
        - HNSW indexes are created where possible.
      - Embedding generation continues after `/index start` completes from the user’s perspective.
    - Add readiness states to `IndexingStatus` and/or a new API:
      - `GraphReady`, `Bm25Ready`, `EmbeddingsInProgress`, `EmbeddingsComplete`, etc.
    - `embedding_search_similar` (or the RAG layer) decides:
      - which strategy to use based on readiness.
  - **Pros**:
    - Aligns well with your goal of “background embedding”, using strong typing for readiness flags.
  - **Cons**:
    - Requires careful coordination between TUI, `ploke-embed`, `ploke-db`, and `ploke-rag`.
    - Needs durable telemetry/metrics to avoid guessing about readiness.

**Recommendation for background embedding and BM25-first**:

- **Phase 1: BM25-first fallback with minimal surface area**
  - Implement a BM25-based search path in `ploke-rag`/TUI with:
    - a well-typed `SearchStrategy` enum.
    - a simple fallback: on vector search failure or “not ready” indication → BM25.
  - Add integration tests that:
    - run with HNSW indexes intentionally missing to prove BM25 handles queries.
    - run with both BM25 and vectors available to verify vector path remains correct.

- **Phase 2: Background embedding readiness and hybrid strategies**
  - Extend `IndexingStatus` (or introduce a new status API) to report:
    - “BM25 ready”, “HNSW indexes created”, “pending_embeddings_count”.
  - Adjust `/index start` UX so:
    - it can complete while embeddings still progress, but
    - the UI indicates “background embedding running”.
  - Implement optional hybrid search that:
    - uses BM25 across the entire corpus, and
    - uses vector search limited to nodes not in “recently changed files” if such tracking exists.

### Notable Risks and Blindspots

- **Multi-embedding interactions**
  - Any changes to partial embedding or background strategies must respect:
    - `multi_embedding_db` feature gates.
    - dual-write behavior in `update_embeddings_batch` and `write_multi_embedding_relations`.
    - runtime-owned metadata and vector relations (must not be clobbered or left inconsistent).
  - Tests like:
    - `update_embeddings_dual_writes_metadata_and_vectors`,
    - `get_unembedded_respects_runtime_embeddings`,
    - `multi_embedding_hnsw_index_and_search`
  - need to be extended (or at least re-run) for any new partial-update semantics.

- **Consistency between BM25 and vector views**
  - BM25 metadata (`bm25_doc_meta`) and embeddings may become out-of-sync during partial updates:
    - e.g., content changed and embeddings updated but BM25 metadata not yet refreshed, or vice versa.
  - You may need:
    - an explicit “document version” or `tracking_hash` alignment between BM25 and embedding relations.
    - tests that assert BM25 and embeddings agree on which snippets belong to which file/version.

- **Concurrency and background work**
  - Background embedding and concurrent BM25/vector searches introduce:
    - potential race conditions in Cozo writes vs reads.
    - need for `IoManager` and DB access patterns to avoid blocking the TUI.
  - The current callback-based setup in `IndexerTask::index_workspace` (via `CallbackManager::new_bounded`) is a good foundation but will need:
    - clear cancellation semantics,
    - strong typing around “job status” and “partial completion”.

### Tests and Evidence to Add

- **Partial embedding behavior**
  - Extend `test_update_embed` in `app_state/database.rs` to:
    - assert more directly that:
      - only nodes in changed files have embeddings nullified,
      - unchanged files’ embeddings are untouched,
      - re-indexing restores embeddings only for nodes in changed files.
  - Add new tests exercising `scan_for_change` + `ReIndex` with:
    - multiple changed files,
    - removed files,
    - no changes (ensuring no re-embedding happens).
  - Implement and test `get_nodes_by_file_with_cursor` if you decide to use it:
    - include Cozo-level tests in `ploke-db` for correctness and pagination.

- **BM25-first and background embedding**
  - Add TUI integration tests (likely under a new feature or harness) that:
    - simulate `/index start` followed by immediate queries:
      - with HNSW indexes missing → expect BM25 path only.
      - with HNSW indexes present but embeddings incomplete → expect BM25 fallback.
    - verify that once embeddings are complete, vector search path is used (or combined via strategy).
  - Add `ploke-db` tests that:
    - confirm BM25 metadata is present and queryable after indexing.
    - ensure HNSW search returns clear, typed errors when indexes are missing, so the TUI can interpret them as “not ready” instead of hard failures.

### Must-Answer Questions Before Implementation

1. **Granularity requirement**:
   - Is file-level “partial embedding” sufficient for the next slice, or do you explicitly want node-level diffing?
2. **Source of truth for “changed nodes”**:
   - Should change detection rely:
     - solely on `tracking_hash` in DB vs new AST, or
     - on a separate diffing mechanism in `syn_parser`?
3. **Search semantics during background embedding**:
   - When embeddings are partially available:
     - should vector search run at all (with “best effort” semantics), or
     - should it be disabled until a threshold of coverage is reached?
4. **BM25 vs vector ranking**:
   - In a hybrid mode, how should BM25 and vector scores be combined?
   - Is “BM25-first then vector” acceptable, or do you want a merged ranking?
5. **Multi-embedding priority**:
   - For crates with multiple embedding models/dimensions in the schema:
     - which dimension/spec is considered “primary” for search?
     - do we need per-model readiness and search strategies?

### High-Level Implementation Impact Summary

- **Feature 1: Targeted partial embedding updates**
  - **Primary impact**:
    - `scan_for_change` (change detection, graph filtering, embedding invalidation).
    - `Database` helpers in `ploke-db` (especially `get_nodes_by_file_with_cursor`).
    - Cozo scripts dealing with node ownership and `tracking_hash` / file mapping.
  - **Risk level**: **medium** if staying at file granularity; **high** if moving to node-level diffing.

- **Feature 2: Background embedding + BM25-first**
  - **Primary impact**:
    - TUI RAG and embedding handlers (`embedding.rs`, `ploke-rag` core).
    - Indexer lifecycle (`IndexerTask`, `IndexingStatus`, `AppState`).
    - `ploke-db` HNSW/BM25 helpers for readiness and capabilities.
  - **Risk level**: **medium–high**, primarily due to concurrency and consistency concerns between BM25 and vector views, and multi-embedding interactions.


