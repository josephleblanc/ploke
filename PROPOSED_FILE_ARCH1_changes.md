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

**1. AST Differential Updater**
- **Problem:** Full re-parsing wastes resources on small changes
- **Solution:** 
  ```rust
  // ingest/parser/diff.rs
  pub struct AstDiffHandler {
      previous_hashes: DashMap<PathBuf, blake3::Hash>,
      cozo: cozo::DbInstance,
  }
  
  impl AstDiffHandler {
      pub fn calculate_delta(&self, path: &Path, new_ast: &Ast) -> Vec<ContentHash> {
          let new_hash = blake3::hash(new_ast);
          let old_hash = self.previous_hashes.get(path).copied();
          
          // Cozo query to find changed relationships
          cozo::run_script!("
              ?[changed] := 
                *nodes{content_hash: old, ...},
                new_hash = $new_hash,
                changed <- if old != new_hash then [old] else []
            ", old_hash)
      }
  }
  ```
- **Architecture Impact:** Add to `ingest` pipeline diagram
- **Validation Test:**
  ```rust
  #[test]
  fn test_diff_calculation() {
      let ast_v1 = parse("fn old() {}");
      let ast_v2 = parse("fn new() {}");
      let diffs = diff_handler.calculate_delta("test.rs", &ast_v2);
      assert!(!diffs.is_empty());
  }
  ```

**2. Embedding Versioning**
- **Silent Failure Risk:** Model updates invalidate old vectors
- **Solution:** Content-addressed version tags
  ```cozo
  ::create embeddings {
      content_hash: Bytes,
      model_version: String,  // "all-mpnet-base-v2-2024Q2"
      vector: <F32; 384>,
      =>
  }
  ```
- **Validation Test** (Critical for Safety):
  ```rust
  #[test]
  fn test_embedding_version_divergence() {
      let v1 = embed("fn foo() {}", "model-v1");
      let v2 = embed("fn foo() {}", "model-v2");
      assert!(cosine_similarity(v1, v2) < 0.95);
  }
  ```

**3. LLM Context Window Manager**
- **Problem:** Token limits vary by local models
- **Solution:** CozoDB-backed token accounting
  ```cozo
  ?[total_tokens] := 
    *llm_context{chunk: c, tokens: t},
    total_tokens = sum(t),
    total_tokens < $max_tokens  // From model config
  ```
- **Implementation:**
  ```rust
  impl ContextManager {
      pub fn add_context(&self, chunk: Json) -> Result<()> {
          let tokens = token_count(chunk);
          self.cozo.run("?[total] := ..."); // Token sum check
          Ok(())
      }
  }
  ```

---

### **Error Handling Deep Dive**

**Proposed Approach** | **Risk** | **Improvement**
---|---|---
Cross-crate error enum | Cozo's Rust errors are unstable | Wrap Cozo errors in `#[error(transparent)]`
`thiserror` derive | No backtrace capture | Add `error-stack` crate integration
Async boundaries | Error type non-`Send` | Enforce `#[derive(Clone)]` for thread safety


---

### **Critical Unasked Questions**

**1. Embedding Versioning Strategy**
- **Problem:** Code changes make existing embeddings stale
- **Solution:** BLAKE3 content hashing + Cozo validity tracking
- **Implementation:**
  ```rust
  // Generate versioned embedding ID
  let embed_id = format!("{}-{}", 
      blake3::hash(code_snippet), 
      model_version
  );
  ```
- **Why Best:** Combines content addressing with model versioning
- **Risk Mitigation:** Automatic garbage collection of stale vectors

**2. Stream Processing Architecture**
- **Problem:** IDE events need real-time response
- **Solution:** Async streams with backpressure
  ```rust
  // ide/watcher.rs
  pub async fn watch_events() -> impl Stream<Item = FileEvent> {
      let (tx, rx) = flume::bounded(100);
      tokio::spawn(async move {
          while let Some(event) = ide_stream.next().await {
              tx.send_async(event).await?;
          }
      });
      rx.into_stream()
  }
  ```
- **Why Cozo:** Handles hybrid batch/stream via `::subscribe`

**3. Model Provenance Verification**
- **Problem:** Local models could be tampered with
- **Solution:** XChaCha20-Poly1305 signatures
  ```rust
  // llm/security.rs
  pub fn verify_model(path: &Path, key: &[u8]) -> Result<()> {
      let sig = read_signature(path);
      let data = read_model_bytes(path);
      chacha20poly1305::verify(key, data, sig)
  }
  ```
- **Risk Elimination:** Prevents prompt injection via corrupted models
- **Validation Test:**
  ```rust
  #[test]
  fn test_model_tampering() {
      let temp_model = TempFile::with_bad_data();
      assert!(verify_model(temp_model.path(), KEY).is_err());
  }
  ```

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
