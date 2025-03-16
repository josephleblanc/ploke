
-----

**NOTE: This is a foundational design document currently under review**
This file is speculative and actively being edited as the proposed
structure for the project. It will continue to be edited as we work on the
proposed project structure and does not accurately reflect the current state of
the project.

This is a planning document **only*** and will be archived once a design
decision is chosen. The only part of this project that is at MVP status so far
is the `syn_parser`, which is the parser for the project.

-----

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

1. **Domain-Driven Separation**:
   - Isolate code parsing (ingest) from reasoning (context/llm)
   - Separate hardware-sensitive components (CUDA in llm/embed)
   - Decouple IDE integration from core logic

2. **Crate Structure Rationale**:
   - `core/`: Shared data structures (AST representations, embedding types)
   - `ingest/`: Parallelizable pipeline stages (parse→embed→graph)
   - `context/`: Hybrid ranking algorithms (LLM + traditional)
   - `llm/`: Abstracted LLM backend (Candle/Ollama/etc)
   - `ide/`: Platform-specific watcher implementations

3. **Critical Cross-Crate Considerations**:
   ```mermaid
   graph TD
     A[IDE] -->|events| B[Context]
     B -->|queries| C[GraphDB]
     C -->|results| D[LLM]
     D -->|responses| A
     ingest -->|AST| Core
     Context -->|embeddings| Core
   ```

**Implementation Priorities for MVP**:
1. Focus on `ingest` → `core` → `context` → `llm` pipeline
2. Concurrency primitives:
   - Async-ready LLM interface
   - Pooled DB connections
   - Atomic type IDs
3. Initial interface: Simple CLI
4. Mockable components for:
   - IDE watcher (file system events only)
   - LLM backend (dummy responses)
   - CozoDB (in-memory)

**Concurrency Strategy**:
```markdown
### Concurrency Policy
- Thread Safety: All public types must be `Send + Sync` by default
- Async Boundaries: LLM/DB ops use async/await; CPU-bound work uses rayon
- Channel Types: 
  - Intra-crate: `std::sync::mpsc` 
  - Cross-crate: `flume` (bounded, async-sync bridging)
- Connection Pooling: CozoDB access uses `deadpool` or similar
- Atomic ID Generation: Use `AtomicUsize` for all graph node IDs

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
   - Integration: `tests/` directory with DB interactions
   - Benchmarks: CPU/GPU comparison in `benches/`
   
2. Example-driven docs:
```rust
/// Example of basic code ingestion
/// 
/// ```rust
/// let ast = parse_rust("src/main.rs")?;
/// let embeddings = generate_embeddings(&ast)?;
/// db.store(embeddings)?;
/// ```
```

**Critical Questions for Next Steps**:
1. Should `syn_parser` become a thin wrapper around `syn` or contain custom logic?
2. How granular should CUDA/GPU support be? (Per-operation flags vs global)
3. What's the first user-facing outcome? (Code completion vs documentation gen)
4. Will you need proc macros for AST transformations?

Would you like to refine any part of this structure or discuss implementation priorities?
```
