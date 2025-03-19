**Recommended Approach: Hybrid Strategy Using Template 3 Architecture**

1. **Full-Task Template for Root Projects**  
Use Template 3 (Detailed Narrative) for parent tasks like your `full_task`, where:
```rust
// Core dependencies shown in g00_3 full_task plan
1. Type Alignment (must complete first)
└─2. Send + Sync Implementation  
└─3. CodeGraph Removal  
```
This preserves critical sequencing - you can't make types Send+Sync until they're Cozo-aligned.

2. **Template 3 Adapted for Subtasks**  
For focused requests like `only_subtask_1`, modify Template 3 to include:
```rust
// In subtask-level plans
dependencies: [external_prerequisites]
```
Example from type alignment:
```yaml
Subtask: Convert String to Bytes
Dependencies: [CozoDB schema finalization, Serialization format RFC approved]
```

**Evidence from Codebase Reality Check**  
The visitor pattern in `visitor.rs` shows why this matters:
```rust
// Current CodeGraph building process
fn visit_item_fn(&mut self, func: &ItemFn) {
    let node = FunctionNode { /* String-based fields */ };
    self.state.code_graph.functions.push(node); // Depends on current type system
}
```
A fragmented approach might miss that type changes (subtast_1) require *simultaneous* visitor modifications (full_task scope).

**Template Effectiveness Metrics**  

| Criteria                | Full-Task Template 3 | Subtask-only Template 3 |
|-------------------------|----------------------|--------------------------|
| Dependency Visibility    | ✅ Full codebase context | ❗ Only local context     |
| Breakage Risk            | ✅ Early detection    | ⚠️ Late-stage surprises  |
| Progress Tracking        | ✅ Unified checklist  | ✅ Clear subtask focus    |
| LLM Reasoning Capture    | ✅ Full rationale     | ✅ Focused justification |

**Implementation Suggestion**  
1. Start with full-task template for major features
2. Generate subtask templates *from the full plan* like:
```rust
// From g00_3 full_task implementation plan
fn generate_subtasks(full_plan: Plan) -> Vec<Subtask> {
    full_plan.steps
        .filter(|step| step.scope == "type_alignment")
        .map(convert_to_subtask_template)
}
```
This preserves architectural context while allowing focused work.

Would you like me to prototype what this hybrid approach would look like for your specific CodeGraph -> CozoDB migration?
