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
// Feature-gated context module
#[cfg(feature = "context_builder_v1")]
pub mod context {
    pub use crate::hybrid_scorer::*;
    pub use crate::token_alloc::*;
}

// Legacy simple context fallback
#[cfg(not(feature = "context_builder_v1"))]
pub mod context {
    pub use legacy_context::*;
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Analyze signal weighting requirements
  - **Purpose**: Determine optimal vector/graph/recency mix
  - **Files**: PROPOSED_ARCH_V3.md, docs/plans/TASKS_OVERVIEW.md
  - **Output**: Weighting matrix for different query types
  
- [ ] 3.1.2. Design cross-module resolution
  - **Purpose**: Handle type references across crate boundaries
  - **Output**: Dependency resolution protocol

### 3.2 Core Implementation
- [ ] 3.2.1. Implement hybrid scorer
  - **Files**: crates/context/src/hybrid_scorer.rs
  - **Code**:
    ```rust
    pub struct HybridScorer {
        vector_weight: f32,
        graph_weight: f32,
        recency_weight: f32,
        db: Arc<dyn KnowledgeGraph>,
    }
    
    impl HybridScorer {
        pub fn score(
            &self,
            query: &QueryContext
        ) -> Result<Vec<ScoredItem>, Box<dyn Error>> {
            // Combine vector similarity, graph proximity, and edit recency
        }
    }
    ```
  
- [ ] 3.2.2. Create token allocator
  - **Files**: crates/context/src/token_alloc.rs
  - **Safety**: Validate against tiktoken-rs counts
  - **Code**:
    ```rust
    pub struct TokenAllocator {
        budget: usize,
        reserved_types: HashSet<String>,
    }
    
    impl TokenAllocator {
        pub fn allocate(
            &self,
            items: Vec<ScoredItem>
        ) -> Result<Vec<ContextSnippet>, AllocationError> {
            // Priority-based truncation
        }
    }
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1. Add hybrid scoring tests
  - **Cases**: Varying weight combinations, edge case prioritization
- [ ] 3.3.2. Validate token budgeting
  - **Files**: crates/context/tests/token_alloc_test.rs
- [ ] 3.3.3. Integration with database/LLM
  - **Verify**: End-to-end context pipeline

### 3.4 Documentation & Knowledge
- [ ] 3.4.1. Document weighting strategy
- [ ] 3.4.2. Create context debugging guide
- [ ] 3.4.3. Update architecture diagrams

## 4. Rollback Strategy
1. Disable `context_builder_v1` feature
2. Fall back to legacy context system
3. Run validation: `cargo test --no-default-features`

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2
- [ ] Implementation Phase: 0/2
- [ ] Testing Phase: 0/3
- [ ] Documentation Phase: 0/3

**Implementation Notes**:
- Follows CONVENTIONS.md §3 (Concurrency Model) using flume channels
- Adheres to IDIOMATIC_RUST §8.1 (Error Handling) with custom errors
- Matches PROPOSED_ARCH_V3 data flow:
  ```mermaid
  flowchart LR
    db[(Database)] --> scorer[HybridScorer]
    scorer --> alloc[TokenAllocator]
    alloc --> llm[LLM Context]
  ```

```bash
cargo add tiktoken-rs --features python
```
