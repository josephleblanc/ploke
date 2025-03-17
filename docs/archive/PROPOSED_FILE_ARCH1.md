# PROPOSED_FILE_ARCHITECTURE

**Proposed Workspace Structure**:
```
rust-rag/
├── Cargo.toml            # Workspace configuration
├── crates/
│   ├── core/             # Shared types and traits
│   ├── ingest/           # Code analysis pipeline
│   │   ├── parser/       # syn_parser integration
│   │   ├── embed/        # Vector embeddings 
│   │   └── graph/        # AST ➔ CozoDB transformations
│   ├── context/          # Query processing & ranking
│   ├── llm/              # Local LLM integration
│   ├── interface/        # CLI/GUI entrypoints
│   ├── ide/              # IDE-aware file watcher
│   └── error/            # Cross-crate error types
├── operators/            # (Future) Custom cozo DB ops
├── examples/             # Documentation examples
└── benches/              # Performance benchmarks
```

**Key Architectural Decisions**:

1. **Type System Design**:
   - Content Addressing: `ContentHash(blake3::Hash)` for AST nodes
   - Temporal Versioning: `TypeStamp(uuid::Uuid)` using UUIDv7
   - CozoDB Schema Integration:
     ```cozo
     :create nodes {
         content_hash: Bytes,     // Blake3 hash (32 bytes)
         type_versions: [Uuid],   // Temporal versions
         relations: [{target: Bytes, kind: String}],
         vec: <F32; 384>          // HNSW-compatible dimension
     }
     ```
   - Concurrency: `DashMap` for thread-safe content caching

2. **Hybrid Vector/Graph Architecture**:
   - CozoDB HNSW vector index integration:
     ```cozo
     ::hnsw create code_graph:embeddings {
         dim: 384,
         dtype: F32,
         fields: [embedding],
         distance: Cosine
     }
     ```
   - Graph storage schema:
     ```cozo
     ::create code_graph {
         content_hash: Bytes, 
         type_stamp: Uuid,
         embedding: <F32; 384>,
         relations: [{target: Bytes, kind: String}]
     }
     ```
   - Test vectors:
     ```rust
     #[test]
     fn test_embedding_dim() {
         let node = sample_node();
         assert_eq!(node.embedding.len(), 384);
     }
     ```

3. **Crate Structure Rationale**:
   - `core/`: Shared data structures (AST representations, embedding types)
   - `ingest/`: Parallelizable pipeline stages (parse→embed→store)
   - `context/`: Hybrid ranking using VectorGraphDB + Core embeddings
   - `llm/`: Abstracted LLM backend (Candle/Ollama/etc)
   - `ide/`: Platform-specific watcher implementations

3. **Critical Cross-Crate Considerations**:
   ```mermaid
   graph TD
    A[IDE] -->|file changes| F[Ingest]
    A -->|events| B[Context]
    B -->|query| C[VectorGraphDB]
    C -->|results| B
    B -->|enhanced query| D[LLM]
    D -->|response| E[Interface]
    E -->|output| A
    F -->|AST| G[Core]
    F -->|processed| C
    C -->|schema| G
    B -->|semantic search| G
    D -->|embeddings| C
   ```

 **Implementation Priorities for MVP**:
1. Focus on `ingest` → `VectorGraphDB` → `context` → `llm` pipeline
2. Concurrency primitives:
   - Async-ready LLM interface
   - Pooled DB connections
   - Atomic type IDs
3. Interface layers:
   - CLI for initial MVP
   - Response validation & formatting
   - Output routing to correct channel
4. Mockable components for:
   - IDE watcher (file system events only)
   - LLM backend (dummy responses)
   - CozoDB (in-memory)

**Concurrency Strategy**:
```markdown
### Concurrency Policy
- Thread Safety: All public types must be `Send + Sync` by default (C-SEND-SYNC)
- Runtime Aqueduct: Explicit runtime segregation for Tokio/Rayon
  ```rust
  #[tokio::main(flavor = "multi_thread")]
  async fn parse_and_ingest() {
      rayon::scope(|s| {
          s.spawn(|| process_file(recorder.clone()));
      });
  }
  ```
- Batch Writers: Thread-safe HNSW batch inserter
  ```rust
  pub struct GraphRecorder {
      hnsw_writer: Arc<Mutex<HnswWriter>>,
      graph_writer: cozo::DbWriter,
  }
  ```
- Channel Types: 
  - Intra-crate: `std::sync::mpsc` 
  - Cross-crate: `flume` (bounded, async-sync bridging)
- Connection Pooling: VectorGraphDB (Cozo) access uses `deadpool` with LRU cache
- Atomic ID Generation: Hybrid content hashing (Blake3) + UUIDv7 timestamps
- Thread-safe Caching: `DashMap<ContentHash, TypeStamp>` for parallel parsing

### Testable Verification
```rust
#[test]
fn test_type_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CodeGraph>();
}

#[tokio::test]
async fn test_async_pipeline() {
    let (tx, rx) = flume::bounded(10);
    tokio::spawn(process_stream(rx));
    tx.send("test.rs").unwrap();
}
```

**Hardware Optimization Strategy**:
```rust
// Feature flags in Cargo.toml
[features]
default = []
cuda = ["llm/cuda", "embed/cuda"]
opencl = ["llm/opencl"]
```

**Error Handling Approach**:
1. `error/` crate with `thiserror`-powered enum:
```rust
pub enum RAGError {
    #[error("Ingest error: {0}")]
    Ingest(#[from] ingest::Error),
    
    #[error("LLM processing failed")]
    LLM(#[source] llm::Error),
    
    #[error("Database operation failed")]
    Db(#[source] cozo::Error),
}
```

2. Per-crate error types implementing `Into<RAGError>`

**Testing Strategy**:
1. Three test suites:
   - Unit: `cargo test` in each crate
   - Integration: Verify ingest→VectorGraphDB roundtripping and DB interactions
   - Benchmarks: CPU/GPU comparison in `benches/`
   
2. Example-driven docs:

/// Example of basic code ingestion
/// 
/// ```rust
/// let ast = parse_rust("src/main.rs")?;
/// let embeddings = generate_embeddings(&ast)?;
/// db.store(embeddings)?;
/// ```

**Critical Questions for Next Steps**:
1. Should `syn_parser` become a thin wrapper around `syn` or contain custom logic?
2. How granular should CUDA/GPU support be? (Per-operation flags vs global)
3. What's the first user-facing outcome? (Code completion vs documentation gen)
4. Will you need proc macros for AST transformations?

### Miscellaneous Planned Functionality

**1. Model Provenance Verification**
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

**3. Embedded Source Validation**
- **Problem:** Code dependencies could tamper with embeddings
- **Solution:** Hierarchical Merkle trees
  ```rust
  // ingest/security.rs
  pub fn validate_source(path: &Path) -> Result<()> {
      let mut hasher = blake3::Hasher::new();
      hash_directory(path, &mut hasher)?;
      // Compare with pinned root hash
  }
  ```
- **Security Impact:** Ensures code provenance integrity

**Final Recommendations**

1. **Architecture Changes**
   - Merge `ingest/graph` into new `crates/graph_db`
   - Add `crates/schema_registry` for shared types
   - Create `crates/security` for cryptographic verification
   - Add `crates/provenance` for source validation

2. **Priority Order**
   ```text
   1. AST → CozoDB schema mapping
   2. Model/Code provenance validation
   3. Stream processing backpressure
   4. CUDA feature flags split
   ```

3. **Testing Critical Path**
   ```rust
   #[tokio::test]
   async fn test_rag_e2e() {
       let (code, _) = parse_file("test.rs");
       let verified = verify_model("model.bin", SECRET_KEY).unwrap();
       validate_source("src/").unwrap();
       let db = mock_db();
       db.store(code).await;
       let ctx = query_db("test query").await;
       let llm_res = generate(ctx, verified).await;
       assert!(!llm_res.is_empty());
   }
   ```

4. **Deployment Checklist**
   - [ ] Hardware-accelerated crypto for model verification
   - [ ] Stream buffer size calibration tool
   - [ ] CozoDB schema migration scripts
   - [ ] Merkle tree root hash pinning
