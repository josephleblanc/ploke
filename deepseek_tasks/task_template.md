
USER_REQUEST:

---

```markdown
# TASK BREAKDOWN TEMPLATE
**Task Name:** `{{TASK_NAME}}`  
**Feature Flag:** `{{feature_ai_task_{unique_id}}}`  

## PHASE 1: ANALYSIS
```rust
//! [NEEDS ATTENTION] Potential breaking changes analysis
/* [ ] Identify affected files:
  - {{file_a.rs}} 
  - {{file_b.rs}} */

/* [ ] Safety assessment:  
#[cfg(any(feature = "example", test))]
fn risk_evaluation() -> BreakingChangeRisk {
    BreakingChangeRisk::{{High/Medium/Low}}
}*/
```

## PHASE 2: IMPLEMENTATION PLAN
```markdown
[[IMPLEMENTATION_STEPS]]
| Checkbox | Action | Needs CFG? | Test Strategy |
|----------|--------|------------|---------------|
| [ ]      | {{Modify type signatures}} | Yes | `cargo test --features {{feature}}` |
| [ ]      | {{Update serialization}} | No  | Integration test |
```

## PHASE 3: PROGRESS TRACKING
```rust
/// TASK STATUS SNAPSHOT
/// Current progress: {{X}}/{{Y}} subtasks
/// Active feature flags: #[cfg(feature = "{{feature}}")]
/// Next priority: {{subtask_name}}
    
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
/// # Rationale: {{LLM's reasoning}}
/// # Alternatives considered:  
/// - {{Alternative A}} 
/// - {{Alternative B}}
/// 
/// # Commit message template:
/// feat({{scope}}): {{short description}} 
/// Reviewed-by: {{LLM/User}} 
/// See-also: {{CONVENTIONS.md#LXX}}
```

