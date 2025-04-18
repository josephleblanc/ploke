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

**3.2. `ploke-core` Changes (Potential - Defer if possible):**
-   `cfg_expr::Expression` needs to derive `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`. Verify this in the `cfg-expr` crate or add necessary wrappers/feature flags if missing. (Seems likely `Hash` and `Eq` are derived, `serde` is feature-gated).
-   Consider if `NodeId::generate_synthetic` needs direct `Expression` input or if hashing bytes provided by `syn_parser` is sufficient. Assume the latter for now.

**3.3. `syn_parser::parser::nodes` Changes:**
-   Add `effective_cfg: Option<cfg_expr::Expression>` field to relevant node structs:
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
    *   `ModuleNode` (Store the module's *own* item-level CFG here, e.g., `item_cfg: Option<Expression>`)
-   Add `declaration_cfg: Option<cfg_expr::Expression>` field to `ModuleDef::Declaration`.

**3.4. `syn_parser::parser::visitor::state::VisitorState` Changes:**
-   Add state fields:
    -   `current_effective_cfg: Option<cfg_expr::Expression>`
    -   `cfg_stack: Vec<Option<cfg_expr::Expression>>`
-   Add helper method: `generate_synthetic_node_id_with_cfg(...)` that takes context (path, name, kind, parent ID) and `effective_cfg: Option<&Expression>`, constructs the byte vector including hashing the `effective_cfg` if `Some`, and calls the core `NodeId::generate_synthetic` function.

**3.5. `syn_parser::parser::visitor::attribute_processing` Changes:**
-   Add helper function: `parse_and_combine_cfgs_from_attrs(attrs: &[syn::Attribute]) -> Option<Expression>`:
    -   Filters for `#[cfg(...)]` attributes.
    -   Parses the inner tokens using `cfg_expr::Expression::parse`. Handle errors appropriately.
    *   If multiple `#[cfg]` attributes exist, combine them into a single `Expression::All(...)`. Ensure the inner `Vec` is sorted based on the `Expression`'s `Hash` or `Ord` implementation before creating `All` to guarantee deterministic representation.
    -   Returns `Some(Expression)` if any valid `cfg` attributes are found, `None` otherwise.
-   Modify `extract_attributes` to filter out and ignore `#[cfg(...)]` attributes.

**3.6. `syn_parser::parser::visitor::mod` Changes (`analyze_file_phase2`):**
-   Call `parse_and_combine_cfgs_from_attrs` on `file.attrs` to get `file_expr`.
-   Initialize `state.current_effective_cfg = file_expr.clone()`.
-   Store `file_expr` in the root `ModuleNode`'s `FileBased` definition (e.g., add `file_cfg: Option<Expression>` field to `ModuleDef::FileBased`).

**3.7. `syn_parser::parser::visitor::code_visitor::CodeVisitor` Changes:**
-   **Helper:** Implement `combine_cfgs(opt_a: Option<Expression>, opt_b: Option<Expression>) -> Option<Expression>` (handles `None` cases and creates `Expression::All` for `Some`/`Some`, ensuring deterministic ordering within `All`).
-   **`visit_item_mod`:**
    -   Parse `mod_expr` from `module.attrs`.
    -   Calculate `new_effective_cfg = combine_cfgs(state.current_effective_cfg.clone(), mod_expr.clone())`.
    -   Store `mod_expr` on the `ModuleNode` being created (`item_cfg` field).
    -   If it's a `Declaration`, store `mod_expr` in `ModuleDef::Declaration.declaration_cfg`.
    -   Push old `state.current_effective_cfg` to `state.cfg_stack`.
    -   Set `state.current_effective_cfg = new_effective_cfg`.
    -   Visit children.
    -   Pop `state.cfg_stack` to restore `state.current_effective_cfg`.
-   **`visit_item_*` (General Pattern):**
    1.  Get `scope_cfg = state.current_effective_cfg.clone()`.
    2.  Parse `item_expr` from the item's attributes using `parse_and_combine_cfgs_from_attrs`.
    3.  Calculate `provisional_effective_cfg = combine_cfgs(scope_cfg, item_expr)`.
    4.  Generate `NodeId::Synthetic` using `state.generate_synthetic_node_id_with_cfg(...)` passing the `provisional_effective_cfg`.
    5.  Store `provisional_effective_cfg` in the new `effective_cfg` field of the `*Node` struct being created.
    6.  **Scope Update (If item defines a CFG-affecting scope for children, e.g., struct, enum):**
        *   Push old `state.current_effective_cfg` to `state.cfg_stack`.
        *   Set `state.current_effective_cfg = provisional_effective_cfg.clone()` (The item's combined CFG becomes the scope CFG for its direct children like fields/variants).
        *   Visit children (fields, variants, generics, etc.).
        *   Pop `state.cfg_stack`.
    7.  **Scope Update (If item does *not* define CFG scope, e.g., function, type alias):**
        *   Visit children without changing `state.current_effective_cfg`.
-   **Field/Variant Visits (within Struct/Enum):** Follow the general pattern, using the (temporarily updated) `state.current_effective_cfg` as the scope CFG.

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
-   Consider modifying the generation logic to include the `cond_id` (derived from the *final* effective CFG) as part of the input hash, ensuring resolved IDs are also unique based on the final CFG.

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
-   **Hashing Stability:** Ensure the `Hash` implementation for `cfg_expr::Expression` is stable across versions if relying on it for `NodeId` generation. Hashing the serialized string might be safer long-term.

## 7. Testing Strategy

-   Unit tests for `parse_and_combine_cfgs_from_attrs`, `combine_cfgs`.
-   Unit tests for `VisitorState` CFG tracking logic.
-   Unit tests for `calculate_final_effective_cfg` logic.
-   Integration tests (`syn_parser`) using fixtures with various combinations:
    -   File-level `cfg`.
    -   Inline module `cfg`.
    *   Item-level `cfg`.
    *   `cfg` on `mod x;` declarations (testing Phase 3 combination).
    *   Nested `cfg` logic (`all`, `any`, `not`).
    *   Multiple `cfg` attributes on one item.
-   Verify `NodeId::Synthetic` uniqueness/difference for items under different provisional CFGs.
-   Verify `NodeId::Resolved` uniqueness/difference based on final CFGs (if implemented).
-   Tests for the RAG filtering logic, simulating different `TargetContext`s and verifying correct node inclusion/exclusion.

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
