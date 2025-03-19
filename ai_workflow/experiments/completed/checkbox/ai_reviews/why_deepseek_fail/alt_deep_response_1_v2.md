► **ANSWER**

Here's a detailed breakdown using your template for the "Type Alignment" portion of
the USER_REQUEST:

**Task Name:** `type_alignment`  
**Feature Flag:** `feature_ai_task_type_align`  

## PHASE 1: ANALYSIS
```rust
//! [NEEDS ATTENTION] Potential breaking changes analysis
/* [x] Identify affected files:
  - crates/syn_parser/src/parser/nodes.rs 
  - crates/syn_parser/src/parser/visitor.rs 
  - crates/syn_parser/src/serialization/ron.rs */

/* [x] Safety assessment:  
#[cfg(any(feature = "example", test))]
fn risk_evaluation() -> BreakingChangeRisk {
    BreakingChangeRisk::High // Changes fundamental type representations
}*/
```

## PHASE 2: IMPLEMENTATION PLAN
```markdown
| Checkbox | Action | Needs CFG? | Test Strategy |
|----------|--------|------------|---------------|
| [ ] | Audit all type definitions against CozoDB schema | No | Manual review + 
`cargo check` |
| [ ] | Replace String with Bytes for content hashes | Yes | `cargo test --features
feature_ai_task_type_align` |
| [ ] | Add #[derive(Serialize, Deserialize)] for new types | Yes | Serializable 
check via proc macro |
| [ ] | Update serialize/deserialize in Ron implementation | Yes | Comparative JSON
validation |
| [ ] | Ensure numeric types match Cozo's Int/Float constraints | Yes | Fuzz 
testing with edge cases |
```

## PHASE 3: PROGRESS TRACKING
```rust
/// TASK STATUS SNAPSHOT
/// Current progress: 1/5 subtasks
/// Active feature flags: #[cfg(feature = "feature_ai_task_type_align")]
/// Next priority: Scalability testing for large codebases
    
//![CHECKLIST]
mod progress {
    /* [x] Analysis completed 
    /* [ ] Core implementation
    /* [ ] Documentation updated
    /* [ ] Cross-crate impacts addressed */
}
```

## PHASE 4: KNOWLEDGE PRESERVATION
```rust
/// ADR 0003: Type Alignment Strategy
/// # Rationale: CozoDB requires strict type consistency for efficient storage
/// # Alternatives considered:  
/// - Wrapper types (rejected due to serialization overhead)
/// - Adapter pattern (rejected for complexity)
/// 
/// # Commit message template:
/// feat(syn_parser): Align {component} with CozoDB type system 
/// Reviewed-by: LLM/User:type-safety 
/// See-also: IDIOMATIC_RUST.md#L3
```

Key considerations from the codebase:
1. Major type definitions live in `nodes.rs` (FunctionNode, TypeNode, etc.)
2. Serialization logic in `ron.rs` must preserve CozoDB-compatible representations
3. Visitor pattern in `visitor.rs` needs type creation adjustments
4. Cross-references via TypeId need validity checks after type changes

See the [PROPOSED_DOCS.md] architecture for required type mapping:
- CozoDB's Bytes ↔ Rust's Vec<u8>/bytes::Bytes
- CozoDB Int ↔ Rust i64 (not i32)
- Nested types must use Cozo's List/Vector specifications

Steps would need to verify compliance with [IDIOMATIC_RUST.md] requirements for:
- C-SERDE (proper serialization impls)  
- C-CONV (type conversion boundaries)
- C-METADATA (type documentation)

Would you like me to expand any particular section?
