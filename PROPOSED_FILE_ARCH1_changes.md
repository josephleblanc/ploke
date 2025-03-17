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

**2. Hybrid Vector/Graph Architecture**

**CozoDB Integration Strategy**  
Based on cozo_db_hnsw.txt recommendations:
```cozo
::hnsw create code_graph:embeddings {
    dim: 384,
    dtype: F32,
    fields: [embedding],
    distance: Cosine
}

::create code_graph {
    content_hash: Bytes, 
    type_stamp: Uuid,
    embedding: <F32; 384>,
    relations: [{target: Bytes, kind: String}]
}
```

**Concurrency Design** (Per IDIOMATIC_RUST C-SEND-SYNC)
```rust
// sync_parser/src/parallel.rs
pub struct GraphRecorder {
    // Thread-safe writers for concurrent ingestion
    hnsw_writer: Arc<Mutex<HnswWriter>>, // Cozo HNSW batch inserter
    graph_writer: cozo::DbWriter,        // Cozo's atomic transaction API
}

// Runtime isolation to prevent Tokio/Rayon conflicts
#[tokio::main(flavor = "multi_thread")]
async fn parse_and_ingest() {
    rayon::scope(|s| {
        s.spawn(|| process_file(recorder.clone()));
    });
}
```

**Type Converters** (Near-Term)
```rust
impl From<ast::Function> for CozoNode {
    fn from(f: ast::Function) -> Self {
        CozoNode {
            content_hash: blake3_hash(&f),
            embedding: model.embed(&f.signature),
            relations: f.calls.into_iter().map(|c| Relation {
                target: blake3_hash(c),
                kind: "CALLS".into()
            }).collect()
        }
    }
}
```

**Long-Term Optimizations**
- **Native Cozo Types in syn_parser** with `cfg(cozo)`
  ```rust
  #[cfg(feature = "cozo")]
  pub type NodeHash = cozo::Bytes;  // Blake3 directly as Cozo type
  ```
  
- **Hybrid Query Example** (From cozo_db_release_07.txt)
  ```cozo
  ?[dist, target, code] := 
    ~code_graph:embeddings{ content_hash: t, embedding: e, | query: $q, k: 5 },
    *code_graph{ content_hash: t => code, relations: r },
    relation_in(r, "IMPLEMENTS"),
    cozo::similarity(e, $query_vec) as dist
  ```

**Validation Plan**
1. Test vector dimension matches (384d F32) via:
   ```rust
   #[test]
   fn test_embedding_dim() {
       let node = sample_node();
       assert_eq!(node.embedding.len(), 384);
   }
   ```
2. Benchmark hybrid queries with `cozo-rs` mock
3. Stress test concurrent writes (10k req/s")

**3. GPU Acceleration Strategy** *(Deferred)*

**Decision:** Postpone CUDA optimizations until post-MVP 

**Rationale:**
- Initial user hardware targets (RTX 3060 Ti) can leverage CUDA automatically via Metal on macOS
- Complex shader compilation would delay MVP (concludes cozo_db_release_07.txt recommendations)

**Mitigation Plan:**
1. Retain feature flags for discovery:
   ```toml
   [features]
   cuda-llm = ["llm/cuda"]  # Unused until 2025Q1
   ```
2. Track debt via GitHub Project Board
3. Validate via periodic Metal/Vulkan benchmarks

**4. Graph Schema Definition** *(Open Questions)*

**Key Mappings Requiring Resolution:**

| AST Element         | CozoDB Entity | Example from cozo_db_hnsw.txt | Decision Needed |
|---------------------|---------------|-------------------------------|-----------------|
| Function signature  | Vector node   | `vec: <F32; 384>`             | Dimension count |
| Trait implementation| Edge          | `relations: "IMPLEMENTS"`     | Edge properties |
| Module hierarchy    | Edge + Node   | `*code_graph{...}`            | Containment vs namespacing |
| Generic parameters   | Edge labels   | N/A                           | Parametric vs concrete types |
| Macro expansions     | Subgraph      | N/A                           | Expansion depth tracking |

**Resolution Process:**
1. Cross-reference with cozodb_docs_types.txt type system
2. Prototype edge types using sample crate analysis
3. Validate via incremental schema migrations:
   ```cozo
   ::alter code_graph add_edge_type {
       edge_type: "MACRO_EXPANDS",
       properties: {depth: Int}
   }
   ```

**Temporary Schema** (For MVP):
```cozo
::create temp_graph {
    id: Bytes =>        // Blake3 of AST content
    ast_type: String,   // "Function", "Struct" etc
    vec: <F32; 384>,
    relations: [{       // Flat relationship model
        target: Bytes,
        kind: String,   // CALLS, CONTAINS, IMPLEMENTS
        locus: Int?     // Source code span
    }]
}
```

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
