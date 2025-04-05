# Insights from Common Errors

## Potential Insights from E0774
1. **Syntax Boundary Awareness**:
   - AI needs better understanding of Rust's attribute placement rules
   - Could be improved with more precise syntax tree analysis

2. **Pattern Recognition Limits**:
   - Over-reliance on surface-level patterns without deeper syntax understanding
   - Suggests need for more contextual validation

[Related Errors](#common-error-patterns-in-ai-assisted-development)

## Potential Insights from E0499
1. **Visitor Pattern Complexity**:
   - Recursive AST traversal creates hidden borrow chains
   - Need better modeling of ownership flows in documentation
   - Current implementation doesn't clearly separate lookup/mutation phases

2. **LLM Limitations**:
   - Tendency to focus on local context over full ownership graph
   - Difficulty anticipating recursive borrow scenarios
   - Over-reliance on compiler rather than preemptive design

3. **Improvement Opportunities**:
   - Could benefit from ownership-aware code generation
   - Need better visualization of borrow chains
   - Should document expected mutation patterns

[Related Error](#error-e0499-multiple-mutable-borrows-in-visitor-pattern)

## Potential Insights from E-UUID-Refactor Cascade
1.  **LLM Context Management Limits**:
    *   While capable of handling broad context, simultaneously tracking the precise implications of conditional compilation (`#[cfg]`) across multiple files (core types, node definitions, visitor logic, initializers, method calls) proved challenging.
    *   The model successfully applied *structural* changes (adding fields, changing types) but failed to consistently update the *usage* patterns in the calling code within the same pass.
2.  **Change Granularity**:
    *   My attempt to implement multiple related steps from the plan (updating node structures *and* visitor state definition) in one go was overly ambitious for this type of refactoring.
    *   The user's initial request was broad ("Go ahead with your changes... accomplish in one go"). While I listed the files, the *scope* of changes within those files was too large.
    *   A more effective interaction pattern would involve breaking down plan steps (like 3.2.2 and 3.2.3) into even smaller sub-tasks and requesting changes for only one sub-task at a time, even if it means more back-and-forth.
3.  **Verification Latency**:
    *   The LLM cannot run `cargo check` itself. It relies on the user to provide feedback after applying changes. When a large change set is applied, the resulting error list can be overwhelming and difficult to attribute to specific incorrect edits.
    *   Smaller change sets allow for quicker, more targeted feedback loops.
4.  **Conditional Compilation Complexity**:
    *   Refactoring under feature flags adds significant cognitive load. It requires reasoning about two parallel code paths simultaneously. This is inherently difficult and error-prone, both for humans and LLMs. The LLM might sometimes "forget" which branch (`#[cfg(feature =...)]` or `#[cfg(not(...))]`) it's supposed to be modifying in a specific part of the code, leading to inconsistencies (like calling `next_node_id` when it shouldn't exist).
5.  **User Guidance**:
    *   While I requested the necessary files, explicitly stating the *specific, limited goal* for the next interaction (e.g., "Now, just add the `Ord` derive to `NodeId` in `ploke-core`" or "Update only the struct initializers in `code_visitor.rs` to add `tracking_hash: None`") would likely yield better results than a more open-ended "apply the next steps".

[Related Error](#error-cascade-e-uuid-refactor-multi-file-refactor-failure)

## Cross-Document Links
- [Error Documentation](#common-error-patterns-in-ai-assisted-development)
- [Prevention Strategies](#potential-insights-from-e0774)
