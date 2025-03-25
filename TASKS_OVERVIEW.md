# Development Plan

### 1. **Database Integration Task List**
**Goal**: Fully implement CozoDB integration per architecture vision
```markdown
- [ ] Create `crates/database` core module
- [ ] Implement hybrid query engine combining:
  - [ ] Vector similarity searches (test_vector_functionality.rs)
  - [ ] Graph traversals (test_graph_queries.rs)
- [ ] Design cross-crate query API following C-ITER/C-ITER-TY conventions
- [ ] Add transaction support for code graph updates
- [ ] Implement schema versioning for evolutionary compatibility
```

### 2. **Embeddings Pipeline Task List**
**Goal**: Separate vector functionality into ploke_embed crate
```markdown
- [ ] Create `crates/embed` with HNSW index implementation
- [ ] Migrate from ploke_graph's:
  - [ ] test_vector_functionality.rs ➔ embed tests
  - [ ] code_embeddings schema ➔ embed schema
- [ ] Implement embedding cache with RLU eviction
- [ ] Add cross-lingual embedding support (AST-based normalization)
```

### 3. **File Watcher Task List**
**Goal**: Implement reactive codebase monitoring
```markdown
- [ ] Create `crates/io/watcher` with:
  - [ ] notify-backed file system watching
  - [ ] LSP protocol integration scaffolding
- [ ] Implement hierarchical event prioritization:
  - [ ] AST changes ➔ High priority
  - [ ] Comment/docs changes ➔ Medium priority
  - [ ] Whitespace ➔ Low priority
- [ ] Add deduplication layer for rapid successive changes
```

### 4. **Context Builder Task List**
**Goal**: Create hybrid retrieval system for LLM context
```markdown
- [ ] Design context aggregation pipeline:
  - [ ] Vector similarity ➔ 40% weight
  - [ ] Graph proximity ➔ 40% weight 
  - [ ] Recent edits ➔ 20% weight
- [ ] Implement context window management:
  - [ ] Token counting with tiktoken-rs
  - [ 》Priority-based truncation
- [ ] Add cross-module dependency tracing
```

### 5. **LLM Integration Task List** 
**Goal**: Implement local LLM execution
```markdown
- [ ] Create `crates/llm` with:
  - [ ] llama.cpp bindings
  - [ ] GPU offloading support
  - [ ] Seedable RNG for deterministic generations
- [ ] Implement safe code sanitization:
  - [ ] AST validation of generated code
  - [ ] Sandboxed execution for dangerous constructs
- [ ] Add progressive output streaming
```

### 6. **UI Foundation Task List**
**Goal**: Establish basic user interfaces
```markdown
- [ ] Create `crates/ui` with:
  - [ ] CLI entrypoint using clap
  - [ ] egui prototype window
  - [ ] LSP server stub
- [ ] Implement unified message bus:
  - [?] User requests
  - [?] System notifications
  - [?] Progress updates
- [ ] Add ANSI/HTML formatted output
```

**Implementation Order Recommendation**:
1. Database + Embeddings first (foundational)
2. Watcher + Context Builder (data flow)
3. LLM + UI (user-facing features)

Each task list should follow the [COMPREHENSIVE_REFACTORING_TEMPLATE](TASK_TEMPLATE_COMPREHENSIVE.md) with:
- Feature flag plans per CONVENTIONS.md
- Cross-crate error handling
- Rayon/Tokio boundary specifications
- Documentation tests for all public APIs

Would you like me to expand any of these task lists into full implementation plans?
