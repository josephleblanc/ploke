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

1.  **Defer ID Unification:**
    *   **Decision:** The proposal to unify `NodeId` and `TypeId` into a single `SemanticId` (ADR-007) is **deferred**. While potentially beneficial for long-term semantic alignment, the required refactoring effort is too high for immediate implementation.
    *   **Action:** Continue using the separate `NodeId` and `TypeId` enums for now. Focus on improving their generation and usage within the existing structure.

2.  **Revamp `Synthetic` ID Generation (Using Existing ID Types):**
    *   **Action:**
        *   Modify `NodeId::generate_synthetic`: Remove `span` as an input. Add `item_kind: ItemKindEnum` and `parent_scope_id: Option<NodeId>` (or similar context) to the inputs. Ensure the function uses these new inputs for the UUIDv5 hash. `ItemKindEnum` would be like `enum ItemKind { Struct, Fn, Trait, Impl, GenericParam, ... }`.
        *   Modify `TypeId::generate_synthetic`: Change the input from `type_string_repr: &str` to use context derived from `process_type`'s analysis (e.g., hash the `TypeKind` variant + the `TypeId`s of related types). *Do not* use `to_token_stream().to_string()`.
    *   **Benefit:** Creates more stable `Synthetic` IDs, less prone to span changes, incorporates kind for disambiguation (`NodeId`), and bases type IDs on structure, not string representation (`TypeId`), mitigating issues like `Self` conflation and improving generic handling foundationally.

3.  **Enhance `VisitorState` Context:**
    *   **Action:** Add `current_definition_scope: Vec<NodeId>` to `VisitorState`. Push the `Synthetic` `NodeId` of a defining item (struct, trait, impl, fn) when entering its visit method, pop when leaving. The *last* element of this stack is the immediate parent scope ID needed for `NodeId::generate_synthetic`.
    *   **Benefit:** Provides necessary context for generating IDs for nested items (fields, variants, generic parameters, methods) that are correctly scoped relative to their defining parent.

4.  **Refactor Type Processing & Cache:**
    *   **Action:**
        *   Modify `get_or_create_type` (still returning `TypeId`) to use the new structure-based `TypeId::generate_synthetic` (Step 2b).
        *   Remove the `VisitorState.type_map` cache entirely *or* change its key to be the generated `Synthetic` `TypeId` and its value to be the `TypeNode`. Evaluate if it's still needed after fixing the ID generation (likely removable).
        *   Modify `process_type` to return the structural info needed for the new `TypeId` generation.
    *   **Benefit:** Fixes the critical flaw of using type strings for `TypeId`s/caching, handles generics more robustly at the ID level.

5.  **Refactor Generic Handling (Using Existing ID Types):**
    *   **Action:**
        *   When visiting `syn::GenericParam` (in `process_generics`), generate its `Synthetic` `NodeId` using the `current_definition_scope.last()` from `VisitorState` as the parent scope ID input.
        *   When visiting a type usage like `T` (in `process_type`), the goal is to eventually link it to the correct parameter definition's `NodeId`. During the initial parse:
            *   **(Recommended for now):** Generate a `Synthetic` `TypeId` based on the name "T" and the *file context* (using the new structure-based generation from Step 2b, which might involve marking it as a named parameter usage). Add a placeholder relation or marker (e.g., in `PendingRelation`) indicating this needs resolution.
            *   **(Advanced):** Attempt to look up "T" in the current scope (using `VisitorState` context) during the parse. If found (e.g., matching a `GenericParamNode` associated with the `current_definition_scope`), store the *parameter's* `NodeId` somehow associated with the usage `TypeId` (perhaps via a `PendingRelation`). This is closer to name resolution but adds complexity.
    *   **Benefit:** Moves towards correctly identifying generic parameters and preparing for linking usage (`TypeId`) to definition (`NodeId`).

6.  **Clarify Definition vs. Usage Representation (Using Existing ID Types):**
    *   **Action:** Reinforce the existing structural distinction. `NodeId` represents definitions (StructNode, FunctionNode, GenericParamNode, etc.). `TypeId` represents type usages (stored in fields like `param.type_id`, `field.type_id`, `return_type`, trait bounds). Review `parser/nodes.rs` and `parser/types.rs` to ensure this pattern is consistent and add documentation (doc comments) to clarify the intended role of `NodeId` vs. `TypeId` fields.
    *   **Benefit:** Improves code clarity and maintainability by explicitly documenting the chosen mechanism for representing definition vs. usage.

7.  **Cleanup:**
    *   **Action:** Search for usages of `state.current_module` and remove the field and its push/pop logic if confirmed unused.
    *   **Benefit:** Simplifies state.

This revised set of changes focuses on improving the robustness and semantic accuracy of the *existing* ID system and parsing process, addressing the most critical issues (span sensitivity, type string hashing, generic handling) without undertaking the larger ID unification refactor at this time.
