# Implementation Plan: CFG Attribute Processing

**Version:** 1.0
**Date:** 2025-04-18
**Status:** Proposed

## 1. Goal

Integrate the processing of `#[cfg(...)]` attributes into the `syn_parser` pipeline to:
1.  Disambiguate `NodeId::Synthetic` generation for code items defined under different conditional compilation flags.
2.  Enable filtering of code nodes based on a target configuration during RAG context building, following the "Alternative A" (Rust evaluation post-Cozo query) database interaction model.

## 2. Approach Summary

This plan adopts the "Alternative A" strategy (evaluate CFGs in Rust post-Cozo query), implemented in phases as described in ADR-001 and ADR-002.

-   **Phase 2 (Parsing - Minimal Hashing):** Extract raw `#[cfg(...)]` strings attached to each item. Store these raw strings (`Vec<String>`) on the node. For `NodeId::Synthetic` generation, combine the item's raw strings with inherited raw strings from the scope, sort alphabetically, join into a delimited string, and hash the bytes of the joined string. **No `cfg-expr` parsing occurs.**
-   **Phase 3 (Resolution/DB Prep):** Collect all relevant raw CFG strings for a node (own + hierarchy). Parse these strings using `cfg-expr`. Combine the parsed `Expression`s logically into a *final* effective `Expression`. Serialize this final `Expression` (e.g., to JSON or canonical string). Store the serialized string and a hash ID (`cond_id`) in `CfgCondition`. Link via `HasCondition`. Generate `NodeId::Resolved` including `cond_id` hash input.
-   **RAG Querying:** Fetch candidate nodes, retrieve their associated final serialized CFG string from Cozo, parse it back into an `Expression` in Rust, and evaluate it against the target context using `cfg_expr::eval()`.

## 3. Phase 2 Changes (Parsing - `syn_parser`)

**3.1. Dependencies:**
-   Remove `cfg-expr` and `target-lexicon` from `syn_parser/Cargo.toml` dependencies for Phase 2.

**3.2. `ploke-core` Changes:**
-   Modify `NodeId::generate_synthetic` to accept `cfg_bytes: Option<&[u8]>` and incorporate it into the hash.

**3.3. `syn_parser::parser::nodes` Changes:**
-   Add `cfgs: Vec<String>` field to relevant node structs (`FunctionNode`, `StructNode`, `EnumNode`, `UnionNode`, `TypeAliasNode`, `TraitNode`, `ImplNode`, `ValueNode`, `MacroNode`, `ImportNode`, `FieldNode`, `VariantNode`, `ModuleNode`). This stores the raw strings from the item's *own* `#[cfg]` attributes.

**3.4. `syn_parser::parser::visitor::state::VisitorState` Changes:**
-   Add state fields:
    -   `current_scope_cfgs: Vec<String>` (Represents the combined raw CFG strings inherited from the surrounding scope).
    -   `cfg_stack: Vec<Vec<String>>` (Stores previous `current_scope_cfgs` values).
-   Modify helper method: `generate_synthetic_node_id`:
    -   Change signature to `generate_synthetic_node_id(&self, name: &str, item_kind: ItemKind, cfg_bytes: Option<&[u8]>) -> NodeId`.
    -   Pass `cfg_bytes` argument through to `ploke_core::NodeId::generate_synthetic`.

**3.5. `syn_parser::parser::visitor::attribute_processing` Changes:**
-   Add helper function: `extract_cfg_strings(attrs: &[syn::Attribute]) -> Vec<String>`:
    -   Filters for `#[cfg(...)]` attributes.
    -   Extracts the inner token stream, converts to string, trims whitespace.
    -   Returns `Vec<String>` of non-empty CFG content strings.
-   Modify `extract_attributes` and `extract_file_level_attributes` to filter out and ignore `#[cfg(...)]` attributes.
-   Remove `parse_and_combine_cfgs_from_attrs`.

**3.6. `syn_parser::parser::visitor::mod` Changes (`analyze_file_phase2`):**
-   Call `extract_cfg_strings` on `file.attrs` to get `file_cfgs`.
-   Initialize `state.current_scope_cfgs = file_cfgs.clone()`.
-   Store `file_cfgs` in the root `ModuleNode`'s `cfgs` field.
-   Update the `NodeId::generate_synthetic` call for the `root_module_id`:
    -   Calculate `root_cfg_bytes` using `calculate_cfg_hash_bytes(&file_cfgs)`.
    -   Call `NodeId::generate_synthetic(...)` passing `root_cfg_bytes.as_deref()`.

**3.7. `syn_parser::parser::visitor::code_visitor::CodeVisitor` Changes:**
-   **Helper:** Implement `calculate_cfg_hash_bytes(cfgs: &[String]) -> Option<Vec<u8>>`: Sorts input strings, joins with delimiter, hashes bytes using `ByteHasher`.
-   **Remove Old Helpers:** Delete `combine_cfgs` and `hash_expression`.
-   **ID Generation:** Modify the `add_contains_rel` helper function (and other direct calls to `generate_synthetic_node_id`):
    -   Update signature to accept `cfg_bytes: Option<&[u8]>`.
    -   Pass `cfg_bytes` through to `state.generate_synthetic_node_id`.
-   **`visit_item_*` (General Pattern):**
    1.  Get `scope_cfgs = state.current_scope_cfgs.clone()`.
    2.  Extract `item_cfgs = super::attribute_processing::extract_cfg_strings(item.attrs)`.
    3.  Store `item_cfgs.clone()` in the node's `cfgs` field.
    4.  Combine: `provisional_effective_cfgs: Vec<String> = scope_cfgs.iter().cloned().chain(item_cfgs.iter().cloned()).collect()`.
    5.  Calculate `cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs)`.
    6.  Generate `node_id` using `add_contains_rel` or `state.generate_synthetic_node_id` (passing `cfg_bytes.as_deref()`).
    7.  **Scope Update (If item defines a CFG-affecting scope):**
        *   Calculate `next_scope_cfgs = provisional_effective_cfgs.clone()`.
        *   Push old `state.current_scope_cfgs` to `state.cfg_stack`.
        *   Set `state.current_scope_cfgs = next_scope_cfgs`.
        *   Visit children.
        *   Pop `state.cfg_stack`.
    8.  **Scope Update (If item does *not* define CFG scope):**
        *   Visit children without changing `state.current_scope_cfgs`.
-   **`process_use_tree`:** Update signature to accept `cfg_bytes: Option<&[u8]>` and pass it down to `add_contains_rel`.

## 4. Phase 3 Changes (Resolution / DB Prep)

**4.1. Module Resolution:**
-   Ensure the process that resolves `mod module;` declarations correctly links the `Declaration` node to the corresponding `FileBased` or `Inline` `ModuleNode`. This linkage is needed for CFG calculation.

**4.2. Final CFG Calculation Logic:**
-   Implement a function `calculate_final_effective_cfg(node_id, graph) -> Option<Expression>`:
    -   Retrieve the node's stored raw `cfgs: Vec<String>`.
    -   Find the node's containing `ModuleNode`.
    -   Iteratively/recursively walk *up* the module hierarchy towards the crate root, using the resolved parent/declaration links.
    -   Collect all raw `cfgs` strings from the node and its module hierarchy (including `ModuleNode.cfgs`).
    -   Parse all collected raw strings into `cfg_expr::Expression`s. Handle parsing errors.
    -   Combine all parsed `Expression`s logically (e.g., using `all(...)` structure, ensuring deterministic order).
    -   Return the final combined `Option<Expression>`.

**4.3. Database Preparation (Alternative A):**
-   Define Cozo Schema:
    ```cozo
    ::create CfgCondition {
        cond_id: Uuid, // Hash of final_cfg_str
        final_cfg_str: String // Serialized final Expression (e.g., JSON or canonical string)
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
        -   Calculate its `final_effective_cfg: Option<Expression>` using the logic from 4.2.
        -   If `Some(expr)`:
            -   Serialize `expr` to `final_cfg_str` (e.g., using `serde_json` or potentially `expr.original()` if deemed stable after combination).
            -   Calculate `cond_id` (e.g., UUIDv5 hash of `final_cfg_str`).
            -   Insert/update `CfgCondition { cond_id, final_cfg_str }`.
            -   Insert `HasCondition { node_id: node.id.uuid(), cond_id }`.

**4.4. `NodeId::Resolved` Generation:**
-   Modify the generation logic to include the `cond_id` (derived from the *final* effective CFG string) as part of the input hash. This ensures resolved IDs are unique based on the final CFG.

## 5. RAG Querying Changes

-   Implement `TargetContext` determination logic (parsing `Cargo.toml`, host info, potential overrides).
-   Implement the Cozo query to fetch candidate `node_id`s (via HNSW or other means).
-   Implement the Cozo query to fetch `cond_id`s for candidate nodes via the `HasCondition` relation.
-   Implement the Cozo query to fetch `final_cfg_str` from `CfgCondition` using the retrieved `cond_id`s.
-   Implement the Rust evaluation logic:
    -   Parse `final_cfg_str` to `cfg_expr::Expression`. Handle parsing errors.
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
-   **Hashing Stability:** Hashing the sorted, joined raw strings in Phase 2 provides basic differentiation but is not semantically canonical. `NodeId::Synthetic` might differ for semantically identical CFGs (e.g., `all(A, B)` vs `all(B, A)`). Hashing the final serialized string for `NodeId::Resolved` in Phase 3 provides better stability.

## 7. Testing Strategy

-   Unit tests for `extract_cfg_strings`, `calculate_cfg_hash_bytes`.
-   Unit tests for `VisitorState` CFG tracking logic (using raw strings).
-   Unit tests for Phase 3 `calculate_final_effective_cfg` logic (parsing raw strings, combining Expressions).
-   Integration tests (`syn_parser`) using fixtures with various combinations:
    -   File-level `cfg` (`#![cfg(...)]`).
    -   Item-level `cfg` on various items (structs, fns, enums, etc.).
    -   Multiple `cfg` attributes on one item (testing deterministic hashing via sorted strings).
    -   Items nested within CFG'd scopes.
-   Verify `NodeId::Synthetic` uniqueness/difference for items under different raw CFGs using paranoid helpers (updated to handle raw strings). Acknowledge limitations for syntactically different but semantically identical single attributes.
-   Verify `NodeId::Resolved` uniqueness/difference based on final CFGs (Phase 3).
-   Tests for the RAG filtering logic (Phase 3/RAG), simulating different `TargetContext`s and verifying correct node inclusion/exclusion based on parsed final CFGs.

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
