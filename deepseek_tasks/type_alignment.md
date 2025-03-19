**Task Name:** `type_alignment`  
**Feature Flag:** `feature_ai_task_type_align`  

## PHASE 1: ANALYSIS
```rust
//! [NEEDS ATTENTION] Potential breaking changes analysis
/* [x] Identify affected files:
  - crates/syn_parser/src/parser/nodes.rs 
  - crates/syn_parser/src/parser/types.rs
  - crates/syn_parser/src/serialization/ron.rs */

/* [x] Safety assessment:  
#[cfg(any(feature = "type_alignment", test))]
fn risk_evaluation() -> BreakingChangeRisk {
    BreakingChangeRisk::High // Changing fundamental types impacts serialization
}*/
```

## PHASE 2: IMPLEMENTATION PLAN
```markdown
| Checkbox | Action | Needs CFG? | Test Strategy |
|----------|--------|------------|---------------|
| [ ] | Replace String with Bytes for content hashes in FunctionNode/StructNode | 
Yes | `cargo test --features type_alignment -p syn_parser` |
| [ ] | Add F32/F64 vector types to TypeKind enum | No | Unit tests for type_graph 
|
| [ ] | Update RON serializer for new numeric types | Yes | Integration 
serialization tests |
| [ ] | Add CozoDB type validation in visitor.rs | No | Doc tests with #[doc = 
include_str!] |
| [ ] | Update TypeNode relationships for new type hierarchy | Yes | Graph 
consistency checks |
```

## PHASE 3: PROGRESS TRACKING
```rust
/// TASK STATUS SNAPSHOT
/// Current progress: 0/5 subtasks
/// Active feature flags: #[cfg(feature = "type_alignment")]
/// Next priority: Content hash type migration
    
//![CHECKLIST]
mod progress {
    /* [ ] Analysis completed 
    /* [ ] Core implementation
    /* [ ] Documentation updated
    /* [ ] Cross-crate impacts addressed */
}
```

## PHASE 4: KNOWLEDGE PRESERVATION
```rust
/// ADR (Architectural Decision Record)
/// # Rationale: CozoDB requires strict type alignment for efficient storage
/// # Alternatives considered:  
/// - Custom type wrappers (rejected due to serde complexity)
/// - Dual type maintenance (rejected due to tech debt)
/// 
/// # Commit message template:
/// feat(type_system): align {component} with CozoDB schema 
/// Reviewed-by: AI/User 
/// See-also: CONVENTIONS.md#L3 (zero-copy requirement)
```

Key implementation details from the codebase:
1. In nodes.rs, content hashes are currently strings:
```rust
pub struct FunctionNode {
    pub docstring: Option<String>, // Change to Bytes
}
```
2. CozoDB requires explicit numeric types:
```rust
// Current TypeKind in types.rs needs F32/F64 variants
pub enum TypeKind {
    Named { path: Vec<String> },
    // Add:
    // VectorF32 { dim: usize },
    // VectorF64 { dim: usize }
}
``` 
3. Serialization impacts in ron.rs:
```rust
fn save_to_ron(code_graph: &CodeGraph) {
    // Will need to handle new vector types
}
```
