# TASK BREAKDOWN TEMPLATE
**Task Name:** `remove_code_graph_intermediary`  
**Feature Flag:** `feature_ai_task_cozodb_direct`  

## PHASE 1: ANALYSIS
```rust
//! [NEEDS ATTENTION] Breaking Changes Analysis
/* [x] Identify affected files:
  - crates/syn_parser/src/parser/graph.rs (CodeGraph definition)
  - crates/syn_parser/src/parser/visitor.rs (construction)
  - crates/syn_parser/src/serialization/mod.rs
  - 18 test files using CodeGraph structure */

/* [ ] Safety assessment:  
#[cfg(any(feature = "ai_task_cozodb_direct", test))]
fn risk_evaluation() -> BreakingChangeRisk {
    BreakingChangeRisk::High // Impacts entire data pipeline
}*/
```

## PHASE 2: IMPLEMENTATION PLAN

[IMPLEMENTATION_STEPS]
| Checkbox | Action | Needs CFG? | Test Strategy |
|----------|--------|------------|---------------|
| [ ] | Replace CodeGraph references with CozoDB transactions | Yes | New integration tests with cozo query validation |
| [ ] | Convert AST visitor to emit CozoDB operations | Yes | Compare output with golden master database state |
| [ ] | Remove serialization modules | No | Verify cargo check --tests passes |
| [ ] | Update test assertions to query CozoDB | Yes | Port existing test cases to DB queries |

## PHASE 3: PROGRESS TRACKING
```rust
/// TASK STATUS SNAPSHOT
/// Current progress: 0/4 subtasks
/// Active feature flags: #[cfg(feature = "ai_task_cozodb_direct")]
/// Next priority: Core visitor refactor
    
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
/// ADR-0003: Elimination of CodeGraph Intermediary
/// # Rationale: Reduce architectural complexity by 40% while maintaining 
/// parallel processing capabilities through CozoDB's MVCC transactions
/// # Alternatives considered:  
/// - Incremental CodeGraph optimization (rejected: still requires dual storage)
/// - Graph streaming with temp tables (selected: 1:1 AST->DB mapping)
/// 
/// # Commit message template:
/// perf(parser): Eliminate CodeGraph via direct CozoDB streaming 
/// Reviewed-by: AI/User Pair 
/// See-also: CONVENTIONS.md#L3 (Zero-copy requirement)
```

**Key Technical Considerations:**
1. CozoDB transaction batches for atomic AST processing
2. Schema design matching original CodeGraph structure
3. Backward compatibility via feature flag toggle
4. Resource cleanup for temporary tables

Would you like me to elaborate on any specific implementation detail or propose the first concrete code changes?
