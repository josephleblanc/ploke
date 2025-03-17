## PROPOSED_FILE_ARCH1

---

### **Critical Architecture Alignment Check**

**1. Type System Migration** *(In Progress)*

**Current State**
- Uses ephemeral `usize` IDs in:
  ```rust
  // types.rs
  pub type NodeId = usize;  // Original implementation
  pub type TypeId = usize;
  ```
- Prevents content addressing needed for CozoDB integration

**Planned State**  
Adapter types for CozoDB integration:
  ```rust
  // From migration strategy:
  #[derive(Serialize, Deserialize, Clone)]
  pub struct TypeStamp {
      content: ContentHash,  // Blake3 hash of AST structure
      version: Uuid,         // UUIDv7 for temporal ordering
  }

  // CozoDB schema:
  :create nodes {
    content_hash: Bytes,     // Blake3 hash (32 bytes)
    type_versions: [Uuid],   // Multiple versions in Validity period
    relations: [{            // Graph structure
      target: Bytes, 
      kind: String
    }],
    vec: <F32; 384>,         // HNSW-compatible dimension
    src: String?             // Original source code snippet
  }
  ```

**Key Decisions**
1. **UUIDv7 over Blake3 for versions**
   - Needed for CozoDB's validity tracking (time-ordered)
   - Required by cozodb_docs_types.txt for temporal queries
2. **ContentHash composites**
   - Allows duplicate code detection before vectorization
3. **Hybrid storage**
   - Maintains graph relations alongside vectors per cozo_db_hnsw.txt:
     > "The HNSW index allows vector proximity searches with graph traversal"

**Migration Tracking**
- Strategy doc: `/crates/type_migration/STRATEGY.md`
- Phase 2 completion blocker: CUDA feature flag implementation
- Estimated completion: Q3 2024

**2. Vector/Graph Hybrid Handling**
- **Oversight:** Missing clear path for joint querying
- **Critical Impact:** Entire RAG use case relies on this
- **Required Diagram Fix**:
  ```mermaid
  graph TD
    C[VectorGraphDB] -->|vector search| B[Context]
    C -->|graph traversal| B
    G[Core] -->|shared schemas| C
  ```

**3. CUDA Feature Granularity**
- **Current Proposal:** `cuda` flag enables GPU for `llm` and `embed`
- **Hidden Debt Risk:** Blocks mixed CPU/GPU pipelines
- **Better Approach**:
  ```toml
  [features]
  cuda-llm = ["llm/cuda"] 
  cuda-emb = ["embed/cuda"]
  ```

**4. CodeGraph ↔ CozoDB Mapping**
- **Missing Piece:** No schema definition for vector-colocated graph
- **MVP Risk:** Will need heavy refactoring later
- **Urgent Action Needed**:
  - Add `crates/graph_db/schema` module
  - Define how AST nodes map to graph edges + vector dimensions

---

### **Concurrency Flaws**

**Current Strategy** | **Issue** | **Fix**
---|---|---
`AtomicUsize` IDs | Wasted memory for single-threaded paths | Feature-gated atomics<br>`#[cfg(feature="concurrent")]`
Rayon for CPU work | Conflicts with Tokio runtime | Explicit runtime segregation:<br>`#[tokio::main(flavor = "multi_thread")]`
Flume channels | Unbounded queues risk OOM | Add pressure-aware backpressure:<br>`.set_buffer_size(num_cpus)`

---

### **Critical Missing Components**

**Component** | **Why Missing?** | **Consequence**
---|---|---
AST Differential Updater | File watcher → incremental updates | Full reparse on every change → slow at scale
Embedding Versioning | Model updates require re-embed | Silent degradation of RAG quality
LLM Context Window Manager | Token limits vary by model | Hardcoded limits → crashes with local models

---

### **Error Handling Deep Dive**

**Proposed Approach** | **Risk** | **Improvement**
---|---|---
Cross-crate error enum | Cozo's Rust errors are unstable | Wrap Cozo errors in `#[error(transparent)]`
`thiserror` derive | No backtrace capture | Add `error-stack` crate integration
Async boundaries | Error type non-`Send` | Enforce `#[derive(Clone)]` for thread safety

---

### **Technical Debt Hotspots**

1. **`syn` vs Custom Parser**
   - Debt: Tight `syn` coupling limits WASM targets
   - Prevention: Add abstraction layer in `ingest/parser`

2. **Monolithic CodeGraph**
   - Debt: Stores entire AST in memory → 8GB RAM limit
   - Solution: Stream AST to CozoDB during parsing

3. **Static Hardware Targets**
   - Debt: Hardcoded CUDA arch for 3060 Ti
   - Mitigation: Runtime compute capability detection

---

### **Critical Unasked Questions**

1. **"How do we version stored embeddings when code changes?"**
   - Required: Content-addressed storage via BLAKE3

2. **"What's our batch vs streaming strategy?"**
   - MVP Needs: Async stream processing from file watcher

3. **"How verify offline model provenance?"**
   - Security: Use XChaCha20-Poly1305-signed models

---

### **Final Recommendations**

1. **Architecture Changes**
   - Merge `ingest/graph` into new `crates/graph_db`
   - Add `crates/schema_registry` for shared types

2. **Priority Order**
   ```text
   1. AST → CozoDB schema mapping
   2. CUDA feature flags split
   3. Pressure-aware channels
   ```

3. **Testing Critical Path**
   ```rust
   #[tokio::test]
   async fn test_rag_e2e() {
       let (code, _) = parse_file("test.rs"); 
       let db = mock_db(); // In-memory Cozo
       db.store(code).await; // Critical path
       let ctx = query_db("test query").await;
       let llm_res = generate(ctx).await;
       assert!(!llm_res.is_empty());
   }
   ```

Would you like to refine any specific area further?
