
# PROPOSED_FILE_ARCHITECTURE

**Proposed Workspace Structure**:
```
rust-rag/
├── Cargo.toml            # Workspace configuration
├── crates/
│   ├── core/             # Core types and traits (NodeId, TypeId, ContentHash)
│   ├── ingest/           # Code analysis pipeline
│   │   ├── parser/       # syn-based parsing
│   │   ├── ast/          # Abstract Syntax Tree definitions
│   │   ├── embed/        # Vector embeddings 
│   │   └── graph/        # AST ➔ CozoDB transformations
│   ├── context/          # Query processing & ranking
│   ├── reasoning/        # Local LLM integration
│   ├── interface/        # CLI/GUI entrypoints
│   ├── ide_integration/  # IDE-aware file watcher
│   └── error/            # Cross-crate error types
├── examples/             # Documentation examples
└── benches/              # Performance benchmarks
```

**Key Architectural Decisions**:

1. **Type System Design**:
   - Content Addressing: `ContentHash(blake3::Hash)` for AST nodes – immutable identifier.
   - Temporal Versioning: `TypeStamp(ContentHash, u64)` – hash + nanosecond timestamp for change tracking.
   - CozoDB Schema Integration: Optimized for graph traversal and vector similarity search.

     ```cozo
     ::create nodes {
         content_hash: Bytes PRIMARY KEY,
         type_stamp: TypeStamp,
         embedding: <F32; 384>,
     }

     ::create edges {
         source: Bytes => (nodes.content_hash),
         target: Bytes => (nodes.content_hash),
         kind: String,
     }
     ```

   - Concurrency: Atomic ID generation and thread-safe data structures.  `DashMap` replaced with lock-free alternatives where appropriate.

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
   - Graph storage schema (as above).

3. **Crate Structure Rationale**:
   - `core/`: Fundamental data structures and traits.
   - `ingest/`: Responsible for parsing, AST construction, embedding generation, and graph storage.  Pipeline stages designed for parallelization.
   - `context/`: Query processing, ranking, and retrieval of relevant code snippets.
   - `reasoning/`: Abstraction layer for LLM integration.
   - `interface/`: CLI and GUI interfaces.
   - `ide_integration/`: IDE-specific integrations.

4. **Critical Cross-Crate Considerations**:
   ```mermaid
   graph TD
    A[IDE] -->|file changes| F[Ingest]
    A -->|events| B[Context]
    B -->|query| C[CozoDB]
    C -->|results| B
    B -->|enhanced query| D[Reasoning]
    D -->|response| E[Interface]
    E -->|output| A
    F -->|AST| G[Core]
    F -->|processed| C
    C -->|schema| G
    B -->|semantic search| G
    D -->|embeddings| C
   ```

 **Implementation Priorities for MVP**:
1.  Focus on `ingest` → `graph` → `context` → `reasoning` pipeline.
2.  Concurrency: Utilize `rayon` for parallel processing within the `ingest` crate.  Explore lock-free data structures for critical sections.
3.  Interface: CLI for initial MVP.
4.  Mockable components:  Implement mock versions of CozoDB and the LLM backend for testing.

**Concurrency Strategy**:
```markdown
### Concurrency Policy
- Thread Safety: All public types are `Send + Sync`.
- Parallelism: `rayon` work-stealing for data-parallel tasks within `ingest`.
- Atomic Operations: `Arc<AtomicUsize>` for generating unique IDs.
- Connection Pooling: `deadpool` for managing CozoDB connections.
- Error Handling:  Centralized error handling with `rag_error` crate.
```

**Hardware Optimization Strategy**:
```rust
// Feature flags in Cargo.toml
[features]
default = []
cuda = ["reasoning/cuda", "embed/cuda"]
opencl = ["reasoning/opencl"]
```

**Error Handling Approach**:
1. `error/` crate with `thiserror`-powered enum.
2. Per-crate error types implementing `Into<RAGError>`.

**Testing Strategy**:
1. Unit tests for each crate.
2. Integration tests to verify end-to-end functionality.
3. Performance benchmarks to measure throughput and latency.

**Critical Questions for Next Steps**:
1.  Refine the CozoDB schema based on performance testing.
2.  Investigate lock-free data structures for critical sections.
3.  Define a clear API for the LLM integration.

### Miscellaneous Planned Functionality

**1. Model Provenance Verification**
- **Problem:** Local models could be tampered with.
- **Solution:** XChaCha20-Poly1305 signatures.

**2. Stream Processing Architecture**
- **Problem:** IDE events need real-time response.
- **Solution:** Async streams with backpressure.

**3. Embedded Source Validation**
- **Problem:** Code dependencies could tamper with embeddings.
- **Solution:** Hierarchical Merkle trees.
