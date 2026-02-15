Okay, I understand completely. You've grappled with the conceptual representation, weighed the trade-offs between semantic purity and implementation complexity/progress, and arrived at a pragmatic path forward that solves the immediate blocking issue (`TypeId` collision for `Self`/`T`) while deferring the deeper graph representation questions.

Your plan makes perfect sense given your goals:

1. ✅ **Stick with `TypeId { Resolved, Synthetic }`:** Keep the core ID simple.
2. ✅  **Fix `GenericParamNode` Declaration `NodeId`:** Use the correct span. This is essential regardless.
3.  **Solve `TypeId` Collision for Usages via Contextual Hashing:**
    *   When `get_or_create_type` (or a renamed version) detects a usage of `Self` or a generic parameter `T` belonging to the current definition context (identified by `current_definition_id`), it will generate a `TypeId::Synthetic`.
    *   The input to the hash function for this synthetic ID will incorporate the `current_definition_id` (either its UUID bytes or a derived string) and a marker indicating which parameter it is (e.g., `b"SELF"` or `b"GENERIC:T"`).
    *   This ensures `Self` in `impl A` gets a different `TypeId` than `Self` in `impl B`, and `T` in `Foo<T>` gets a different `TypeId` than `T` in `Bar<T>`.
    *   This generated `TypeId` will be stored in the existing `type_id` / `return_type` fields of nodes like `FieldNode`, `ParamData`, `FunctionNode`.
4.  **Modify `TypeId::generate_synthetic` (or add a new method):** Adapt it to take the necessary context (e.g., `context_node_id: NodeId`, `param_marker: &[u8]`) and use that for hashing instead of just a type string when generating these contextual IDs. Using `&[u8]` as input for hashing is indeed more flexible.
5.  **Visitor Logic:** The visitor needs to pass the `current_definition_id` and identify when a `Type::Path` refers to `Self` or a contextual generic parameter to call the correct ID generation logic.
6.  **Defer Relations:** Stick to the plan of creating most relations (except `Contains`, `DeclaresGenericParam`) in Phase 3.
7.  **Phase 3 Responsibility:** Phase 3 logic will need to be aware that certain `TypeId::Synthetic` values represent these contextual parameters and interpret them accordingly when building the final relational graph (e.g., linking the `TypeId` for `Self` back to the concrete type of the `impl`).

**This approach:**

*   **Solves the Collision:** Directly addresses the immediate problem blocking testing and progress.
*   **Minimizes Structural Changes:** Avoids introducing `TypeRefId` and altering many node definitions or `GraphId`.
*   **Is Pragmatic:** Prioritizes getting a working Phase 2/3 pipeline over achieving perfect semantic representation in the intermediate `CodeGraph`.
*   **Defers Complexity:** Pushes the interpretation of these contextual `TypeId`s to Phase 3.

It's a solid, workable plan. You've clearly thought through the implications and chosen a path that balances correctness with development velocity. I'm fully on board with supporting you in implementing this strategy. Let me know when you're ready to proceed with the specific code changes for this approach.
