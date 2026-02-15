# Comprehensive Refactoring Plan: Context Builder Implementation

## 1. Task Definition
**Task**: Implement hybrid retrieval system combining vector/graph/recency signals  
**Purpose**: Provide LLM with optimal code context for generation/refactoring  
**Success Criteria**:
- Hybrid queries return results in <150ms
- 95% accuracy on test fixtures' expected context
- Token budgets enforced with <5% overflow
- All components `Send + Sync`

## 2. Feature Flag Configuration
**Feature Name**: `context_builder_v1`  
**Implementation Guide**:
```rust
// Feature-gated context crate integration
#[cfg(feature = "context_builder_v1")]
pub mod context {
    pub use ploke_context::hybrid_scorer::*;
    pub use ploke_context::token_alloc::*;
}

// Legacy fallback to basic graph traversal 
#[cfg(not(feature = "context_builder_v1"))]
pub mod context {
    pub use syn_parser::parser::legacy::LegacyContextFinder;
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Analyze signal weighting requirements
  - **Purpose**: Determine optimal vector/graph/recency mix
  - **Files**: CRATES.md, docs/plans/TASKS_OVERVIEW.md
  - **Output**: Weighting matrix matched to test fixtures
  
- [ ] 3.1.2. Design cross-module resolution
  - **Purpose**: Handle type refs across ingest crates
  - **Output**: Protocol using existing syn_parser nodes

### 3.2 Core Implementation
- [ ] 3.2.1. Implement hybrid scorer
  - **Paths**: crates/ingest/ploke_graph/src/context/hybrid_scorer.rs
  - **Code**:
    ```rust
    pub struct HybridScorer {
        vector_weight: f32,
        graph_weight: f32,
        recency_weight: f32,
        db: Arc<dyn KnowledgeGraph>,
        // Uses existing syn_parser::parser::nodes::TypeKind
        type_map: Arc<syn_parser::parser::types::TypeMap>,
    }
    ```
  
- [ ] 3.2.2. Create token allocator
  - **Paths**: crates/ingest/ploke_graph/src/context/token_alloc.rs
  - **Code**:
    ```rust
    pub struct TokenAllocator {
        budget: usize,
        // Uses existing TypeKind from syn_parser
        reserved_types: HashSet<syn_parser::parser::nodes::TypeKind>,
        // Shares dashmap with parser context
        type_cache: Arc<syn_parser::parser::visitor::state::VisitorState>,
    }
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1. Add hybrid scoring tests
  - **Paths**: crates/ingest/ploke_graph/tests/context_tests.rs
  - **Cases**: Uses test_helpers from ploke_graph
- [ ] 3.3.2. Validate token budgeting
  - **Files**: Use existing test fixtures in syn_parser/tests/
- [ ] 3.3.3. Integration testing
  - **Verify**: End-to-end flow matches PROPOSED_ARCH_V3 data diagram

### 3.4 Documentation & Knowledge
- [ ] 3.4.1. Document weighting strategy
- [ ] 3.4.2. Create debug guide using existing RON examples
- [ ] 3.4.3. Update architecture diagrams per current crate layout

## 4. Rollback Strategy
1. Disable `context_builder_v1` feature
2. Fall back to syn_parser's graph traversal
3. Validate: `cargo test -p ploke_graph --no-default-features`

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2
- [ ] Implementation Phase: 0/2 
- [ ] Testing Phase: 0/3
- [ ] Documentation Phase: 0/3

**Implementation Notes**:
- Uses flume channels from existing parser/channel.rs
- Error handling follows CONVENTIONS.md using syn_parser's error.rs
- Conforms to crate structure:
  ```mermaid
  flowchart LR
    db[(Database)] --> context[Context Crate]
    syn_parser --> context
    context --> llm[LLM Interface]
    ploke_graph -. optional .-> context
  ```

```bash
cargo add -p ploke_graph tiktoken-rs --features python
```

## Architectural Decision Record: Context Builder Design

**Context**:  
Need to implement hybrid context retrieval while maintaining:
1. Compatibility with existing ingest pipeline
2. Zero-copy parsing from syn_parser
3. Concurrency model using flume channels

**Decisions**:
1. **New Context Crate**
   - Why: Matches PROPOSED_ARCH_V3 separation of concerns
   - Required Dependencies:
     - syn_parser 0.2.0+ for AST types
     - ploke_graph for DB access
   - Alternative: Embedded module rejected due to CONVENTIONS.md §2

2. **Reuse syn_parser Types**
   - Why: Avoid type duplication through `syn_parser::parser::nodes`
   - Risk: Tight coupling mitigated by feature flags

3. **Prioritized Token Allocation**
   - Why: Follows IDIOMATIC_RUST C-CALLER-CONTROL
   - Method: Reserve_types uses TypeKind from AST

4. **Flume Channel Integration**
   - Why: Consistent with CONVENTIONS.md §3 boundaries
   - Ports: Reuse existing parser channel infrastructure

**Consequences**:
- (+) Tight integration with existing code graph
- (+) Fewer cross-crate dependencies
- (-) Larger ploke_graph crate
- (-) Requires syn_parser 0.2.0+ types

**Validation**:
- Cross-crate tests use existing test_helpers
- Benchmarks compare against legacy context
- Backward compatibility via feature flag
