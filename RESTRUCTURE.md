# Comprehensive Refactoring Plan: Align ploke_graph with Proposed Architecture

## 1. Task Definition
**Task**: Restructure ploke_graph crate to match PROPOSED_ARCH_V3.md specifications  
**Purpose**: Achieve architectural consistency and enable future scalability  
**Success Criteria**:
1. Vector functionality moved to ploke_embed crate
2. Async/sync boundaries via flume channels implemented  
3. Custom error handling matching CONVENTIONS.md
4. All existing tests pass post-refactor
5. Architecture documentation updated

## 2. Feature Flag Configuration
**Feature Name**: `transitional_graph_layout`

**Implementation Guide**:
```rust
// Temporary flag during migration
#[cfg(feature = "transitional_graph_layout")]
mod legacy_graph {
    pub use ploke_graph::transform::*;
}

#[cfg(not(feature = "transitional_graph_layout"))]
mod new_graph {
    pub use ploke_embed::vector_transform::*;
    pub use ploke_graph_core::transformation::*;
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Audit current graph implementation
  - **Purpose**: Identify boundary violations
  - **Files**: transform.rs, schema.rs, test_*.rs
  - **Output**: Dependency graph of functionality
  
- [ ] 3.1.2. Design crate boundaries
  - **Purpose**: Define module responsibilities
  - **Output**: New crate structure diagram

### 3.2 Core Implementation
- [ ] 3.2.1. Extract vector functionality
  - **Files**:
    - Move `code_embeddings` schema ➔ ploke_embed/src/schema.rs
    - Migrate HNSW index code ➔ ploke_embed/src/vector.rs
  - **Code Changes**:
    ```rust
    // ploke_embed/src/lib.rs
    pub mod vector {
        pub fn create_hnsw_index(db: &CozoDb) -> Result<(), EmbedError> {
            // Migrated from ploke_graph schema.rs
        }
    }
    ```

- [ ] 3.2.2. Implement channel boundaries
  - **Files**:
    - Create io/channel/src/graph_messages.rs
    - Modify transform.rs to use flume
  - **Code Changes**:
    ```rust
    // io/channel/src/graph_messages.rs
    pub enum GraphMessage {
        TransformRequest {
            code_graph: Arc<CodeGraph>,
            reply: flume::Sender<TransformResult>
        },
        EmbeddingRequest {
            node_id: NodeId,
            text: String
        }
    }
    ```

- [ ] 3.2.3. Error handling overhaul
  - **Files**:
    - Create error/src/graph_error.rs
    - Update error conversions
  - **Code Changes**:
    ```rust
    // error/src/graph_error.rs
    #[derive(Debug, thiserror::Error)]
    pub enum GraphError {
        #[error("Transformation failed: {0}")]
        Transformation(#[from] cozo::Error),
        
        #[error("Validation error: {0}")]
        Validation(String)
    }
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1. Migration tests
  - **Files**: tests/migration_test.rs
  - **Cases**:
    - Vector functionality with/without feature flag
    - Channel message passing validation

- [ ] 3.3.2. Performance benchmarks
  - **Files**: benches/graph_transformation.rs
  - **Metrics**:
    - Memory usage during large graph processing
    - Channel throughput

- [ ] 3.3.3. Cross-crate integration
  - **Files**: tests/integration/graph_embed.rs
  - **Verify**:
    - ploke_graph ↔ ploke_embed data flow
    - Error propagation across crates

### 3.4 Documentation & Knowledge Preservation
- [ ] 3.4.1. Update architecture docs
  - **Files**: PROPOSED_ARCH_V3.md
  - **Changes**: Mark ploke_graph as implemented

- [ ] 3.4.2. Write transformation guide
  - **Files**: docs/transformation_patterns.md
  - **Content**: Data flow between AST → Graph → Embeddings

- [ ] 3.4.3. Annotate breaking changes
  - **Files**: MIGRATION.md
  - **Details**: Channel API changes, error handling updates

## 4. Rollback Strategy
1. Revert to `transitional_graph_layout` feature
2. Run validation tests:
   ```bash
   cargo test --features transitional_graph_layout
   ```
3. If failures persist:
   - Restore from git branch `stable-pre-refactor`
   - Re-publish previous crate versions

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2 complete
- [ ] Implementation Phase: 0/3 complete
- [ ] Testing Phase: 0/3 complete
- [ ] Documentation Phase: 0/3 complete

**Rationale**: This plan addresses architectural drift while maintaining system stability through:
1. Gradual migration via feature flags
2. Comprehensive cross-crate testing
3. Clear documentation of interface changes
4. Preservation of existing functionality during transition
