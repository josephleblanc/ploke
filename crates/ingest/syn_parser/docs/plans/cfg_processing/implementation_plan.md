# Implementation Plan: CFG Attribute Processing

**Version:** 1.0
**Date:** 2025-04-18
**Status:** Proposed

## 1. Goal

Integrate the processing of `#[cfg(...)]` attributes into the `syn_parser` pipeline to:
1.  Disambiguate `NodeId::Synthetic` generation for code items defined under different conditional compilation flags.
2.  Enable filtering of code nodes based on a target configuration during RAG context building, following the "Alternative A" (Rust evaluation post-Cozo query) database interaction model.

## 2. Approach Summary

This plan adopts the "Alternative A" strategy, where the final evaluation of CFG conditions happens in Rust using `cfg-expr` after retrieving candidate nodes from the Cozo database.

-   **Phase 2 (Parsing):** Calculate and store a *provisional* effective CFG condition (`Option<cfg_expr::Expression>`) on each parsed node (`StructNode`, `FunctionNode`, etc.). This provisional CFG combines the condition of the node's immediate scope (file or containing module/item) with the node's own `#[cfg]` attributes. This provisional CFG is hashed into the `NodeId::Synthetic`.
-   **Phase 3 (Resolution/DB Prep):** Calculate the *final* effective CFG for each node by recursively combining its provisional CFG with any `#[cfg]` attributes found on `mod module;` declarations in its module hierarchy. Serialize this final CFG and store it in a dedicated `CfgCondition` relation in Cozo, linked to the code node via a `HasCondition` relation.
-   **RAG Querying:** Fetch candidate nodes, retrieve their associated final CFG condition from Cozo, and evaluate it in Rust against the target context using `cfg_expr::eval()`.

## 3. Phase 2 Changes (Parsing - `syn_parser`)

**3.1. Dependencies:**
-   Ensure `cfg-expr = { version = "0.15", features = ["serde", "hash"] }` is present in `syn_parser/Cargo.toml`. (`hash` might be default, verify).

**3.2. `ploke-core` Changes:**
-   **Confirmed:** `cfg-expr` with the `serde` feature derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`. No changes needed in `ploke-core` itself for this.
-   **Decision:** `NodeId::generate_synthetic` will *not* take `Expression` directly. The `syn_parser` visitor will handle hashing the `Expression` (if present) and provide the resulting bytes to `NodeId::generate_synthetic`.

**3.3. `syn_parser::parser::nodes` Changes:**
-   Add `provisional_effective_cfg: Option<cfg_expr::Expression>` field to relevant node structs (renamed from `effective_cfg` for clarity):
    -   `FunctionNode`
    -   `StructNode`
    *   `EnumNode`
    *   `UnionNode`
    *   `TypeAliasNode`
    *   `TraitNode`
    *   `ImplNode`
    *   `ValueNode` (Const/Static)
    *   `MacroNode`
    *   `ImportNode`
    *   `FieldNode`
    *   `VariantNode`
-   Modify `ModuleNode`:
    -   Add `item_cfg: Option<cfg_expr::Expression>` (Stores the module's *own* item-level CFG, e.g., `#[cfg(...)] mod my_mod { ... }`).
-   Modify `ModuleDef`:
    -   Add `declaration_cfg: Option<cfg_expr::Expression>` field to `ModuleDef::Declaration` (Stores CFG from `#[cfg(...)] mod my_mod;`).
    -   Add `file_cfg: Option<cfg_expr::Expression>` field to `ModuleDef::FileBased` (Stores CFG from `#![cfg(...)]` at file level).

**3.4. `syn_parser::parser::visitor::state::VisitorState` Changes:**
-   Add state fields:
    -   `current_scope_cfg: Option<cfg_expr::Expression>` (Renamed from `current_effective_cfg` for clarity - represents the CFG inherited from the surrounding scope).
    -   `cfg_stack: Vec<Option<cfg_expr::Expression>>` (Stores previous `current_scope_cfg` values when entering new scopes).
-   Modify helper method: `generate_synthetic_node_id`:
    -   Change signature to `generate_synthetic_node_id(&self, name: &str, item_kind: ItemKind, cfg_bytes: Option<&[u8]>) -> NodeId`.
    -   The caller (visitor) is now responsible for hashing the `Expression` and providing the bytes.
    -   Inside the method:
        -   Construct the standard byte vector (namespace, file path, relative path, parent ID bytes, item kind bytes, item name bytes).
        -   If `cfg_bytes` is `Some(bytes)`, append `bytes` to the standard vector.
        -   Call `uuid::Uuid::new_v5(&ploke_core::PROJECT_NAMESPACE_UUID, &combined_bytes)`.
        -   Return `NodeId::Synthetic`.

**3.5. `syn_parser::parser::visitor::attribute_processing` Changes:**
-   Add helper function: `parse_and_combine_cfgs_from_attrs(attrs: &[syn::Attribute]) -> Option<Expression>`:
    -   Filters for `#[cfg(...)]` attributes.
    -   Parses the inner tokens using `cfg_expr::Expression::parse`. Handle errors appropriately (log warning, return `None`).
    -   If multiple `#[cfg]` attributes exist, combine them into a single `Expression::All(...)`. **Crucially:** Ensure the inner `Vec<Expression>` within `All` is sorted deterministically (e.g., based on the `Debug` representation or hash of each sub-expression) before creating the `All` variant. This guarantees that `#[cfg(A)] #[cfg(B)]` produces the same combined `Expression` (and thus hash) as `#[cfg(B)] #[cfg(A)]`.
    -   Returns `Some(Expression)` if any valid `cfg` attributes are found, `None` otherwise.
-   Modify `extract_attributes` to filter out and ignore `#[cfg(...)]` attributes (they are handled by the new function).

**3.6. `syn_parser::parser::visitor::mod` Changes (`analyze_file_phase2`):**
-   Call `parse_and_combine_cfgs_from_attrs` on `file.attrs` to get `file_cfg_expr`.
-   Initialize `state.current_scope_cfg = file_cfg_expr.clone()`.
-   Store `file_cfg_expr` in the root `ModuleNode`'s `FileBased` definition using the new `file_cfg` field.
-   Update the `NodeId::generate_synthetic` call for the `root_module_id`:
    -   Hash `file_cfg_expr` (if `Some`) into `cfg_bytes`.
    -   Call `NodeId::generate_synthetic(...)` passing the optional `cfg_bytes`.

**3.7. `syn_parser::parser::visitor::code_visitor::CodeVisitor` Changes:**
-   **Helper:** Implement `combine_cfgs(scope_cfg: Option<&Expression>, item_cfg: Option<&Expression>) -> Option<Expression>` (handles `None` cases and creates `Expression::All` for `Some`/`Some`, ensuring deterministic ordering within `All`). Clones expressions internally as needed.
-   **Helper:** Implement `hash_expression(expr: Option<&Expression>) -> Option<Vec<u8>>`. Uses `ByteHasher` and the `Expression`'s `Hash` impl.
-   **ID Generation:** Modify the `add_contains_rel` helper function (and other direct calls to `generate_synthetic_node_id`):
    -   It should take the calculated `provisional_effective_cfg: Option<Expression>` for the item.
    -   Call `hash_expression` on the `provisional_effective_cfg`.
    -   Call `state.generate_synthetic_node_id(...)` passing the resulting `Option<Vec<u8>>`.
-   **`visit_item_mod`:**
    1.  Get `scope_cfg = state.current_scope_cfg.clone()`.
    2.  Parse `item_cfg_expr` from `module.attrs` using `parse_and_combine_cfgs_from_attrs`.
    3.  Calculate `provisional_effective_cfg = combine_cfgs(scope_cfg.as_ref(), item_cfg_expr.as_ref())`.
    4.  Generate `module_id` using `add_contains_rel` (which now handles hashing `provisional_effective_cfg`).
    5.  Store `item_cfg_expr` in `ModuleNode.item_cfg`.
    6.  If it's a `Declaration`, store `item_cfg_expr` in `ModuleDef::Declaration.declaration_cfg`.
    7.  Calculate `next_scope_cfg = provisional_effective_cfg.clone()` (this module's combined CFG becomes the scope for its children).
    8.  Push old `state.current_scope_cfg` to `state.cfg_stack`.
    9.  Set `state.current_scope_cfg = next_scope_cfg`.
    10. Visit children.
    11. Pop `state.cfg_stack` to restore `state.current_scope_cfg`.
-   **`visit_item_*` (General Pattern):**
    1.  Get `scope_cfg = state.current_scope_cfg.clone()`.
    2.  Parse `item_cfg_expr` from the item's attributes using `parse_and_combine_cfgs_from_attrs`.
    3.  Calculate `provisional_effective_cfg = combine_cfgs(scope_cfg.as_ref(), item_cfg_expr.as_ref())`.
    4.  Generate `node_id` using `add_contains_rel` or `state.generate_synthetic_node_id` (passing the hashed `provisional_effective_cfg`).
    5.  Store `provisional_effective_cfg` in the `provisional_effective_cfg` field of the `*Node` struct being created.
    6.  **Scope Update (If item defines a CFG-affecting scope for children, e.g., struct, enum, impl):**
        *   Calculate `next_scope_cfg = provisional_effective_cfg.clone()`.
        *   Push old `state.current_scope_cfg` to `state.cfg_stack`.
        *   Set `state.current_scope_cfg = next_scope_cfg`.
        *   Visit children (fields, variants, methods, etc.).
        *   Pop `state.cfg_stack`.
    7.  **Scope Update (If item does *not* define CFG scope, e.g., function, type alias, const, static):**
        *   Visit children without changing `state.current_scope_cfg`.
-   **Field/Variant/Method Visits:** Follow the general pattern, using the (potentially updated) `state.current_scope_cfg` as the scope CFG when calculating their *own* `provisional_effective_cfg`.

## 4. Phase 3 Changes (Resolution / DB Prep)

**4.1. Module Resolution:**
-   Ensure the process that resolves `mod module;` declarations correctly links the `Declaration` node to the corresponding `FileBased` or `Inline` `ModuleNode`. This linkage is needed for CFG calculation.

**4.2. Final CFG Calculation Logic:**
-   Implement a function `calculate_final_effective_cfg(node_id, graph) -> Option<Expression>`:
    -   Retrieve the node's stored `provisional_effective_cfg`.
    -   Find the node's containing `ModuleNode`.
    -   Iteratively/recursively walk *up* the module hierarchy towards the crate root, using the resolved parent/declaration links.
    -   For each step involving a `ModuleDef::Declaration`, retrieve its stored `declaration_cfg`.
    -   Use `combine_cfgs` to accumulate these declaration CFGs with the initial `provisional_effective_cfg`.
    -   Return the final combined `Option<Expression>`.

**4.3. Database Preparation (Alternative A):**
-   Define Cozo Schema:
    ```cozo
    ::create CfgCondition {
        cond_id: Uuid, // Hash of serialized_expr or canonical string
        serialized_expr: String // JSON or other serialization of Expression
        => // No value columns
    }
    ::create HasCondition {
        node_id: Uuid, // NodeId::uuid()
        cond_id: Uuid
        => // No value columns
    }
    ```
-   Modify DB insertion logic:
    -   For each `CodeNode` (Function, Struct, etc.):
        -   Calculate its `final_effective_cfg` using the logic from 4.2.
        -   If `Some(expr)`:
            -   Serialize `expr` to `expr_str` (e.g., using `serde_json`).
            -   Calculate `cond_id` (e.g., UUIDv5 hash of `expr_str`).
            -   Insert/update `CfgCondition { cond_id, serialized_expr: expr_str }`. Use appropriate Cozo syntax for upsert/conditional insert.
            -   Insert `HasCondition { node_id: node.id.uuid(), cond_id }`.

**4.4. `NodeId::Resolved` Generation:**
-   Modify the generation logic to include the `cond_id` (derived from the *final* effective CFG) as part of the input hash. This ensures resolved IDs are unique based on the final CFG, distinguishing, e.g., `#[cfg(unix)] struct Foo` from `#[cfg(windows)] struct Foo`.

## 5. RAG Querying Changes

-   Implement `TargetContext` determination logic (parsing `Cargo.toml`, host info, potential overrides).
-   Implement the Cozo query to fetch candidate `node_id`s (via HNSW or other means).
-   Implement the Cozo query to fetch `cond_id`s for candidate nodes via the `HasCondition` relation.
-   Implement the Cozo query to fetch `serialized_expr` from `CfgCondition` using the retrieved `cond_id`s.
-   Implement the Rust evaluation logic:
    -   Deserialize `serialized_expr` to `cfg_expr::Expression`.
    -   Create the `evaluator` closure based on the `TargetContext`.
    -   Filter nodes based on `expression.eval(&evaluator)`.
    -   (Deferred) Handle `cfg_attr` evaluation separately if/when implemented.

## 6. Edge Cases & Considerations

-   **`#[cfg]` on `mod my_mod;`:** Handled by calculating the final effective CFG in Phase 3 using the stored `declaration_cfg`.
-   **Interaction with Visibility:** Filtering logic must consider both visibility rules and CFG activation. An item needs to be both visible and CFG-active.
-   **`cfg_attr`:** Explicitly deferred in this initial plan. Would require storing the conditional attributes separately and evaluating them alongside the main CFG during RAG filtering.
-   **Macros Generating CFG'd Items:** Remains a limitation of static analysis without macro expansion.
-   **Empty CFGs `#[cfg()]`:** `cfg-expr` should handle this (likely evaluates to true). Need verification.
-   **Complex/Nested Logic:** Handled correctly by `cfg-expr` parsing and evaluation.
-   **Deterministic Combination:** Ensure `combine_cfgs` produces a deterministic result (e.g., by sorting predicates within `Expression::All`).
-   **Hashing Stability:** Relying on `cfg_expr::Expression`'s `Hash` implementation is convenient but carries a risk if the implementation changes between versions. Hashing a canonical serialized string representation (e.g., sorted JSON or RON) of the `Expression` offers better long-term stability, although it adds serialization overhead in Phase 2. **Decision:** Start with direct `Hash` for simplicity, but monitor `cfg-expr` updates and be prepared to switch to serialized string hashing if stability issues arise or become a concern. The deterministic sorting within `combine_cfgs` is crucial for either approach.

## 7. Testing Strategy

-   Unit tests for `parse_and_combine_cfgs_from_attrs`, `combine_cfgs`.
-   Unit tests for `VisitorState` CFG tracking logic.
-   Unit tests for `calculate_final_effective_cfg` logic.
-   Integration tests (`syn_parser`) using fixtures with various combinations:
    -   File-level `cfg` (`#![cfg(...)]`).
    -   Item-level `cfg` on inline modules (`#[cfg(...)] mod foo { ... }`).
    -   Item-level `cfg` on various items (structs, fns, enums, etc.).
    -   `cfg` on `mod x;` declarations (testing Phase 3 combination).
    -   Nested `cfg` logic (`all`, `any`, `not`).
    -   Multiple `cfg` attributes on one item (testing deterministic combination).
    -   Items nested within CFG'd scopes (e.g., fields in a CFG'd struct).
-   Verify `NodeId::Synthetic` uniqueness/difference for items under different provisional CFGs using paranoid helpers.
-   Verify `NodeId::Resolved` uniqueness/difference based on final CFGs (if implemented).
-   Tests for the RAG filtering logic, simulating different `TargetContext`s and verifying correct node inclusion/exclusion based on final CFGs.

This plan provides a detailed roadmap for implementing the CFG processing functionality.


## Files required

### Direct changes to logic

src/parser/visitor/type_processing.rs
src/parser/visitor/mod.rs
src/parser/visitor/state.rs
src/parser/visitor/code_visitor.rs
src/parser/nodes.rs

### Ripple effects and updates

tests/processing/cfg_attributes.rs
tests/uuid_phase1_discovery/discovery_tests.rs
tests/uuid_phase2_partial_graphs/nodes/functions.rs
tests/uuid_phase2_partial_graphs/nodes/traits.rs
tests/uuid_phase2_partial_graphs/nodes/macros.rs
tests/uuid_phase2_partial_graphs/nodes/imports.rs
tests/uuid_phase2_partial_graphs/nodes/impls.rs
tests/uuid_phase2_partial_graphs/nodes/modules.rs
tests/uuid_phase2_partial_graphs/nodes/type_alias.rs
tests/uuid_phase2_partial_graphs/nodes/const_static.rs
tests/uuid_phase2_partial_graphs/nodes/unions.rs
tests/uuid_phase2_partial_graphs/nodes/enums.rs
tests/uuid_phase2_partial_graphs/nodes/structs.rs
tests/uuid_phase2_partial_graphs/basic.rs
tests/uuid_phase2_partial_graphs/ids.rs
tests/uuid_phase2_partial_graphs/type_conflation.rs
tests/uuid_phase2_partial_graphs/relations.rs
tests/uuid_phase2_partial_graphs/determinism.rs
tests/common/paranoid/union_helpers.rs
tests/common/paranoid/struct_helpers.rs
tests/common/paranoid/type_alias_helpers.rs
tests/common/paranoid/enum_helpers.rs
tests/common/paranoid/macros_helpers.rs
tests/common/paranoid/const_static_helpers.rs
tests/common/paranoid/impl_helpers.rs
tests/common/paranoid/import_helpers.rs
tests/common/paranoid/trait_helpers.rs
tests/common/paranoid/module_helpers.rs
tests/common/mod.rs
tests/common/uuid_ids_utils.rs
