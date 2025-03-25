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
// Feature-gated context module within ploke_graph
#[cfg(feature = "context_builder_v1")]
pub mod context {
    pub use crate::hybrid_scorer::*;
    pub use crate::token_alloc::*;
}

// Legacy fallback uses syn_parser's existing graph
#[cfg(not(feature = "context_builder_v1"))]
pub mod context {
    pub use crate::graph::legacy_context::*;
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
        // Uses existing CodeGraph from syn_parser
        type_map: Arc<DashMap<String, TypeId>>, 
    }
    ```
  
- [ ] 3.2.2. Create token allocator
  - **Paths**: crates/ingest/ploke_graph/src/context/token_alloc.rs
  - **Code**:
    ```rust
    pub struct TokenAllocator {
        budget: usize,
        // Leverages existing TypeKind from syn_parser
        reserved_types: HashSet<syn_parser::parser::nodes::TypeKind>,
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
    syn_parser-->ploke_graph
    ploke_graph-->context[Context Builder]
    context-->llm
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
1. **Co-locate with ploke_graph**
   - Why: Leverages existing CozoDB integration
   - Alternative: New crate rejected due to circular deps

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
