Let's analyze the concurrency preparedness of PROPOSED_FILE_ARCH1.md with your syn_parser experience in mind:

**Current Architecture's Concurrency Foundation**
The proposed architecture makes *implicit* but not *explicit* preparations for concurrency. Here's the breakdown:

1. **Positive Signs**:
   - Domain-driven separation (ingest vs context vs llm) creates natural parallelism boundaries
   - Pipeline structure (parse→embed→graph) suggests stream processing opportunities
   - Feature flag strategy allows incremental concurrency implementation

2. **Critical Gaps**:
   - No mention of `Send`/`Sync` requirements for core types
   - No guidance on shared state management between crates
   - Unclear error handling across thread boundaries
   - No async/await strategy for LLM interactions

**Concrete Risks of Current Approach**
```rust
// Hypothetical current code that would block concurrency
struct CodeGraph { 
    functions: Vec<FunctionNode>, // Contains Rc<> internally?
    impls: Vec<ImplNode>, 
}

// Future concurrent usage would require:
struct ConcurrentCodeGraph {
    functions: Arc<[FunctionNode]>,
    impls: RwLock<Vec<ImplNode>>,
}
```

**Required Architectural Additions**
1. **Cross-Crate Concurrency Contract**
```markdown
## Concurrency Policy (Add to ARCH1.md)

- All cross-crate data structures must implement `Send` + `Sync` unless annotated with `#![allow(non_sendable)]`
- Pipeline stages use MPSC channels for inter-stage communication
- LLM crate must expose async interface
- GraphDB accessors use connection pooling
```

2. **Component-Specific Strategies**

| Crate       | Concurrency Pattern              | MVP Preparation              |
|-------------|-----------------------------------|-------------------------------|
| ingest/     | Parallel file processing         | Make parser & collector `Send`|
| context/    | Read-heavy RwLock patterns       | Isolate mutable state        |
| llm/        | Async/await with tokio           | Abstract behind async trait  |
| graph/      | Connection pool to CozoDB        | Design for pool from start   |

**MVP Implementation Guide**
```rust
// core/src/lib.rs
pub trait Embeddable: Send + Sync {
    fn to_embedding(&self) -> Embedding;
}

// ingest/src/lib.rs
pub struct ParallelParser {
    tx: flume::Sender<ParseResult>, // Thread-safe channel
    handles: Vec<JoinHandle<()>>, 
}

// llm/src/lib.rs
#[async_trait]
pub trait LLMBackend {
    async fn generate(&self, prompt: &str) -> Result<String>;
}
```

**Why This Prevents Future Pain**
1. Type system enforces concurrency safety early
2. Clear migration path from sync to async
3. Prevents introduction of non-Send types in critical paths
4. Enables incremental optimization:
   ```rust
   // MVP (sync)
   pub fn process_file(path: &Path) -> Result<()> { ... }
   
   // Future (async)
   pub async fn process_stream(stream: impl Stream<Item=Path>) { ... }
   ```

**Recommendation**: Add a "Concurrency Strategy" section to PROPOSED_FILE_ARCH1.md that mandates:
1. Thread-safety as a first-class design constraint
2. Async/await boundaries at I/O-heavy components
3. Prohibition of non-Send types in cross-crate interfaces
4. Benchmarks of contended resources

Would you like me to draft specific architectural amendments to address these concerns?
