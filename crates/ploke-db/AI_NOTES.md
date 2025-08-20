# AI Notes for `ploke-db`

This document provides a dense, technical overview of the `ploke-db` crate for AI assistants.

## 1. Crate Purpose

`ploke-db` is the primary read-only interface to the `CozoDB` code graph. Its main role is to retrieve code and metadata for Ploke's RAG system. It handles dense vector search, sparse keyword search, and direct graph queries.

## 2. Key Data Structures

-   **`Database`**: The main entry point. Wraps `cozo::Db`. Get an instance via `Database::init_with_schema()` or from application state.
-   **`QueryResult`**: Wrapper for `cozo::NamedRows`. This is the raw result from Cozo. Use its methods to get structured data:
    -   `.to_embedding_nodes() -> Result<Vec<EmbeddingData>, _>`: For queries that return node metadata.
    -   `.into_snippets() -> Result<Vec<CodeSnippet>, _>`: For queries that return snippet data.
    -   `.try_into_file_data() -> Result<Vec<FileData>, _>`: For file-related queries.
-   **`EmbeddingData`**: A crucial struct containing all information needed to locate and understand a code node.
    -   `id: Uuid`: The node's unique ID.
    -   `file_path: PathBuf`: Absolute path to the source file.
    -   `start_byte: usize`, `end_byte: usize`: The byte span of the node's code in the file.
    -   `node_tracking_hash: TrackingHash`, `file_tracking_hash: TrackingHash`: Hashes for change detection.
    -   `namespace: Uuid`: The crate version's unique namespace.
-   **`CodeSnippet`**: Represents a retrieved piece of code text, its file path, and span.
-   **`NodeType`**: An enum for all node types in the graph schema (e.g., `Function`, `Struct`, `Module`). Use this to specify which relations to query. `NodeType::primary_nodes()` returns the main embeddable types.
-   **`TypedEmbedData`**: A `Vec<EmbeddingData>` tagged with its `NodeType`.
-   **`EmbedDataVerbose`**: Returned by HNSW search. Contains `typed_data: TypedEmbedData` and `dist: Vec<f64>`.

## 3. Core Workflows & APIs

### Workflow 1: Hybrid Search (Dense + Sparse)

A typical hybrid search involves two parallel steps, followed by a merge and re-ranking step (which happens outside this crate).

**A. Dense Vector Search (HNSW)**
1.  **Function**: `ploke_db::index::hnsw::search_similar_args()`
2.  **Input**: `SimilarArgs { db, vector_query, k, ef, ty, ... }`
3.  **Output**: `Result<EmbedDataVerbose, _>`. The `typed_data.v` field is a `Vec<EmbeddingData>`.

**B. Sparse Keyword Search (BM25)**
1.  The `bm25_index::bm25_service` runs as a background actor.
2.  Get the `mpsc::Sender<Bm25Cmd>` from application state.
3.  **Command**: `Bm25Cmd::Search { query, top_k, resp }` where `resp` is a `oneshot::Sender`.
4.  **Output**: The oneshot receiver will yield a `Vec<(Uuid, f32)>` (node ID and score).
5.  **Hydrate IDs**: Use `db.get_nodes_ordered(uuids)` with the received IDs to get the full `Vec<EmbeddingData>`.

### Workflow 2: Populating Embeddings

1.  **Fetch Nodes**: Call `db.get_unembedded_node_data(limit, cursor)` to get batches of nodes that need embeddings. This returns `Result<Vec<TypedEmbedData>, _>`.
2.  **Generate Embeddings**: For each `EmbeddingData` in the result, read the code from `file_path` using the `start_byte` and `end_byte` span. Pass this code to an embedding model.
3.  **Store Embeddings**: Collect the results as `(Uuid, Vec<f32>)` pairs and call `db.update_embeddings_batch(updates)`.

### Workflow 3: Direct Graph Traversal

-   For complex queries not covered by helpers, use `db.raw_query("...")`.
-   The query language is CozoScript (a Datalog dialect).
-   The database schema is defined in `ploke-transform/src/schema/`. Key relations include `function`, `struct`, `module`, and `syntax_edge`.
-   Example: Find functions called by `main`.
    ```datalog
    ?[caller_name, callee_name] :=
        *function{id: caller_id, name: "main"},
        *function{id: callee_id, name: callee_name},
        *syntax_edge{source_id: caller_id, target_id: callee_id, relation_kind: "Calls"}
    ```

## 4. Important Notes

-   **Error Handling**: Most functions return `Result<_, ploke_error::Error>`. The `DbError` enum contains specific database-related failures.
-   **State**: The `Database` struct is the main stateful object. It should be managed by the application and passed by reference where needed.
-   **Schema**: To understand the graph structure, refer to the `ploke-transform` crate, especially the `schema` module. All primary nodes have fields like `id`, `name`, `tracking_hash`, `span`, and `embedding`.
