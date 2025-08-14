# Request: IndexerTask dense-search API & confirmation for hybrid fusion assumptions

I will implement hybrid_search in `crates/ploke-rag/src/lib.rs` to fuse BM25 (sparse) and dense HNSW results. Before I change code, I need a short confirmation or the actual code for the IndexerTask dense-search API so the implementation can call it correctly.

Please provide one of the following:

1) Add `crates/ploke-embed/src/indexer.rs` (or the file that defines `IndexerTask`) to the chat, or paste the exact method signature used to perform a dense (HNSW) search. For example, confirm whether a method like the following exists (and exact signature / error type):

```rust
impl IndexerTask {
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<(uuid::Uuid, f32)>, SomeError>;
}
```

If the method has a different name (e.g., `dense_search`, `query_embeddings`, `search_embeddings`) or different return type, paste its exact signature.

2) If you prefer I proceed with reasonable assumptions, reply with the text:
   Proceed with assumptions

Assumptions I will use if you confirm:
- IndexerTask provides an async search method callable as:
  `async fn search(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>, E>`
  where `E` can be converted into or mapped to `RagError::Channel` or a new `RagError::Embed(String)` variant.
- I will run BM25 and dense searches concurrently (tokio::join!), then fuse results using Reciprocal Rank Fusion (RRF) with default k=60 and equal weighting for BM25 and dense scores (configurable later).
- The final returned Vec will be a ranked Vec<(Uuid, f32)> where the f32 is the fused score (higher = better).
- I will extend `RagError` with a small variant for embedding/dense errors (e.g., `Embed(String)`).

If you confirm "Proceed with assumptions", I will:
- Patch `crates/ploke-rag/src/lib.rs` to:
  - Add any necessary imports (HashMap).
  - Add a new `RagError::Embed(String)` variant.
  - Implement `hybrid_search` that concurrently calls BM25 and the IndexerTask dense search, fuses results via RRF, and returns ranked results or an error.
  - Add a small internal helper `fn rrf_fuse(lists: Vec<Vec<Uuid>>) -> HashMap<Uuid, f32>` or similar to compute fused scores.

If you prefer to provide the real `IndexerTask` API instead of the assumptions, please paste it or add the relevant file(s) and I'll implement the integration using the exact signatures.

Notes:
- I will keep the change minimal and fail-safe: if the dense search method returns an error, we will still return the BM25-only results (or return an Err if you prefer strict failure). Tell me your preference: `fallback_to_bm25` or `strict`.
- By default I will `fallback_to_bm25` (prefer availability over strict failure) unless you ask otherwise.

Please respond with either:
- The IndexerTask file/method signature (or add the file), or
- "Proceed with assumptions" (and optionally `"strict"` or `"fallback_to_bm25"`).
