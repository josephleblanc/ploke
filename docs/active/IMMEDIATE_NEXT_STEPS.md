**Connecting Discussion, Code Analysis, and Next Steps:**

Our conversation highlighted the need for:
1.  A unified, semantically grounded ID system.
2.  Deterministic `Synthetic` IDs based on stable inputs (not just span).
3.  Clear distinction between definition and usage.
4.  Better context management during parsing (especially for generics).
5.  A phased approach where resolution builds upon the initial parse.

The code analysis reveals:
1.  A split ID system (`NodeId`/`TypeId`) exists.
2.  `Synthetic` `NodeId` generation relies heavily on `span`.
3.  `Synthetic` `TypeId` generation relies on the raw type string (problematic for cache keys and semantic uniqueness).
4.  `VisitorState` lacks explicit tracking of the current defining item's ID.
5.  Generic definition IDs are mangled; usage IDs are based on the name string; linking is absent.
6.  Definition vs. Usage is structurally separated via `NodeId`/`TypeId` fields and some relations.
7.  Impl block IDs use a reasonable syntactic disambiguation method for the parsing phase.

**Proposed Refinements / Next Steps (Concrete):**

1.  **Unify IDs:**
    *   **Action:** Replace `NodeId` and `TypeId` with a single `enum SemanticId { Resolved(Uuid), Synthetic(Uuid) }` in `ploke-core`.
    *   **Refactoring:** Update all structs in `parser/nodes.rs`, `parser/types.rs`, `parser/relations.rs`, `VisitorState`, `CodeGraph`, and relevant functions (`get_or_create_type`, `process_generics`, visitor methods, etc.) to use `SemanticId`. Rename `GraphId` variants accordingly.
    *   **Benefit:** Simplifies the ID concept, aligns better with `rustc`, forces rethinking of definition vs. usage representation.

2.  **Revamp `Synthetic` ID Generation:**
    *   **Action:** Modify `SemanticId::generate_synthetic` (the unified function).
        *   **For Definitions (formerly NodeId):** Inputs should be `crate_namespace`, `file_path`, `relative_path_guess`, `item_name`, `item_kind: ItemKindEnum`, `parent_scope_id: Option<SemanticId>`. Remove `span`. `ItemKindEnum` would be like `enum ItemKind { Struct, Fn, Trait, Impl, GenericParam, ... }`.
        *   **For Type References (formerly TypeId):** Inputs should be `crate_namespace`, `file_path`, and context derived from `process_type`'s analysis (e.g., hash the `TypeKind` variant + the `SemanticId`s of related types). *Do not* use `to_token_stream().to_string()`.
    *   **Benefit:** Creates more stable `Synthetic` IDs, less prone to span changes, incorporates kind for disambiguation. Type IDs become based on structure, not string representation.

3.  **Enhance `VisitorState` Context:**
    *   **Action:** Add `current_definition_scope: Vec<SemanticId>` to `VisitorState`. Push the `Synthetic` `SemanticId` of a defining item (struct, trait, impl, fn) when entering its visit method, pop when leaving. The *last* element of this stack is the immediate parent scope ID needed for `generate_synthetic`.
    *   **Benefit:** Provides necessary context for generating IDs for nested items (fields, variants, generic parameters, methods) that are correctly scoped.

4.  **Refactor Type Processing & Cache:**
    *   **Action:**
        *   Modify `get_or_create_type` (now returning `SemanticId`) to use the new structure-based ID generation (Step 2b).
        *   Remove the `VisitorState.type_map` cache entirely *or* change its key to be the generated `Synthetic` `SemanticId` and its value to be the `TypeNode`. Evaluate if it's still needed after fixing the ID generation.
        *   Modify `process_type` to return the structural info needed for the new ID generation.
    *   **Benefit:** Fixes the critical flaw of using type strings for IDs/caching, handles generics more robustly at the ID level.

5.  **Refactor Generic Handling:**
    *   **Action:**
        *   When visiting `syn::GenericParam` (in `process_generics`), generate its `Synthetic` `SemanticId` using the `current_definition_scope.last()` from `VisitorState` as the parent scope ID input.
        *   When visiting a type usage like `T` (in `process_type`), the goal is to eventually link it to the correct parameter definition's `SemanticId`. During the initial parse, this is hard. Options:
            *   **(Recommended for now):** Generate a `Synthetic` `SemanticId` based on the name "T" and the *file context* (as done now, but using the new structure-based generation). Add a placeholder relation or marker indicating this needs resolution.
            *   **(Advanced):** Attempt to look up "T" in the current scope (using `VisitorState` context) during the parse. If found (e.g., matching a `GenericParamNode` associated with the `current_definition_scope`), use *that parameter's* `SemanticId`. This is closer to name resolution but adds complexity to the visitor.
    *   **Benefit:** Moves towards correctly identifying generic parameters and preparing for linking usage to definition.

6.  **Represent Definition vs. Usage:**
    *   **Action:** With unified IDs, the distinction lies purely in the `CodeGraph` structure. Ensure nodes representing definitions (StructNode, FunctionNode) hold their own `SemanticId`. Ensure nodes/fields representing usage (parameters, fields, return types, trait bounds) store the `SemanticId` of the item being used/referenced. Review `parser/nodes.rs` and `parser/types.rs` to ensure this pattern is consistent after unification.
    *   **Benefit:** Clear structural distinction enforced by the graph design itself.

7.  **Cleanup:**
    *   **Action:** Search for usages of `state.current_module` and remove the field and its push/pop logic if confirmed unused.
    *   **Benefit:** Simplifies state.

This set of changes directly addresses the core issues identified in the code and aligns the implementation with the desired principles discussed earlier. It's a significant refactoring but lays a much stronger foundation.
