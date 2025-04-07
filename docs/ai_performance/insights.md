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

## Potential Insights from E-DESIGN-DRIFT (Violating Core Design)

1.  **Compiler Errors vs. Design Intent:**
    *   There's a significant risk of AI models optimizing for *local correctness* (satisfying the compiler for a specific line/function) at the expense of *global design integrity*. The model successfully identified how to make the code compile locally by decoupling ID generation, but failed to recognize *why* the original code was coupled (to prevent orphaned nodes and ensure immediate linkage).
    *   This suggests a need for better mechanisms, perhaps in prompting or fine-tuning, to weigh existing design patterns, comments explaining intent, and structural coupling more heavily against simply resolving compiler errors.

2.  **Context Window and Constraint Prioritization:**
    *   While the model demonstrated good recall of many aspects of the plan, maintaining the priority of *implicit* or *previously stated* design constraints (like the ID/relation coupling) within a large, evolving context proved difficult.
    *   It's possible that as new information (compiler errors, code snippets) is added, older constraints might lose "weight" or fall out of the effective context window, especially if they aren't explicitly re-stated alongside the immediate problem.

3.  **User Interaction Patterns:**
    *   **Guidance Specificity:** While the user correctly identified the file and the goal (fix errors), the request might have benefited from explicitly re-stating the critical constraint: "Fix the E0599 error in `add_contains_rel`, *ensuring that NodeId generation remains coupled with the creation of the Contains relation*." This primes the AI to prioritize that constraint.
    *   **Scope Management:** The user's decision to tackle the large `code_visitor.rs` file highlights the trade-off between providing complete context and risking overload. While providing the whole file seems necessary for the AI to understand the visitor's structure, it increases the chance of the AI losing track of specific details or constraints. Breaking down the task further (e.g., "Refactor only the `visit_item_fn` method according to our plan") might be more robust, albeit slower.
    *   **Feedback Loop:** The user correctly identified the flawed suggestion and provided detailed reasons. This iterative feedback is crucial, but also time-consuming. Improving the AI's initial adherence to design constraints would make the process more efficient.

4.  **System/Tooling Factors (`aider` Speculation):**
    *   **System Prompt Influence:** Could the underlying system prompt for coding assistants implicitly favor generating compilable code over strictly adhering to existing patterns if there's a conflict? Does it instruct the model on how to handle comments indicating design intent?
    *   **Context Management (`aider`):** How `aider` constructs and potentially summarizes the context provided to the LLM could be critical. If summarization occurs, is nuance about design coupling preserved? Does the repo-map feature provide sufficient surrounding context, or could it sometimes isolate functions too much, obscuring their relationship to broader patterns (like the `add_contains_rel` helper's role)?
    *   **Action Limitations:** The inability of the AI to apply its own suggestions reliably (the `SEARCH/REPLACE` block failures) adds friction and prevents rapid iteration. The lack of direct compiler feedback forces reliance on the user, slowing down the detection of flawed suggestions.

5.  **Future Directions:**
    *   Can LLMs be trained or prompted to explicitly identify and respect code comments indicating design rationale?
    *   Could tooling provide mechanisms for users to "pin" critical design constraints within the context window?
    *   How can the feedback loop be tightened, perhaps with integrated static analysis or partial compilation checks accessible to the AI?

[Related Error](#error-e-design-drift-suggesting-local-fixes-that-violate-core-design)

## Cross-Document Links
- [Error Documentation](#common-error-patterns-in-ai-assisted-development)
- [Prevention Strategies](#potential-insights-from-e0774)



