# Test Plan for MacroNode Parsing (Phase 2 - `uuid_ids`)

**Goal:** Verify that `macro_rules!` and procedural macro definitions are correctly parsed into `MacroNode` instances within the `CodeGraph` during Phase 2, focusing on identification, metadata, and basic relationships. Macro expansion and rule parsing are out of scope.

**Fixture:** `tests/fixture_crates/fixture_nodes/src/macros.rs`
**Test File:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/macros.rs`
**Helper Location:** `crates/ingest/syn_parser/tests/common/paranoid/macros_helpers.rs`

---

## Test Tiers

### Tier 1: Basic Smoke Tests

*   **Goal:** Quickly verify that `MacroNode`s are created for various macro types (`macro_rules!`, `proc_macro`, `proc_macro_derive`, `proc_macro_attribute`) and basic properties are present. Uses full crate parse.
*   **Test:** `test_macro_node_basic_smoke_test_full_parse`
    *   Use `run_phase1_phase2("fixture_nodes")`.
    *   Find the `ParsedCodeGraph` for `macros.rs`.
    *   Iterate through expected macro names (e.g., `exported_macro`, `local_macro`, `function_like_proc_macro`, `derive_proc_macro`, `attribute_proc_macro`, `inner_exported_macro`).
    *   For each:
        *   Find the `MacroNode` (e.g., using `graph.macros.iter().find(|m| m.name == ...)`).
        *   Assert node exists.
        *   Assert `node.id` is `NodeId::Synthetic(_)`.
        *   Assert `node.tracking_hash` is `Some(TrackingHash(_))`.
        *   Assert `node.kind` matches the expected variant (`DeclarativeMacro` or `ProcedureMacro { ... }`).
        *   Assert `node.visibility` is appropriate (`Public` for exported/proc macros, `Inherited` for local `macro_rules!`).

### Tier 2: Targeted Field Verification

*   **Goal:** Verify each field of the `MacroNode` struct individually for specific examples.
*   **Tests:**
    *   `test_macro_node_field_id_regeneration`: Target `exported_macro`. Regenerate ID using context + span and assert match.
    *   `test_macro_node_field_name`: Target `local_macro`. Assert `node.name == "local_macro"`.
    *   `test_macro_node_field_visibility_public`: Target `exported_macro`. Assert `node.visibility == VisibilityKind::Public`.
    *   `test_macro_node_field_visibility_inherited`: Target `local_macro`. Assert `node.visibility == VisibilityKind::Inherited`.
    *   `test_macro_node_field_kind_declarative`: Target `exported_macro`. Assert `node.kind == MacroKind::DeclarativeMacro`.
    *   `test_macro_node_field_kind_proc_func`: Target `function_like_proc_macro`. Assert `node.kind == MacroKind::ProcedureMacro { kind: ProcMacroKind::Function }`.
    *   `test_macro_node_field_kind_proc_derive`: Target `derive_proc_macro`. Assert `node.kind == MacroKind::ProcedureMacro { kind: ProcMacroKind::Derive }`.
    *   `test_macro_node_field_kind_proc_attr`: Target `attribute_proc_macro`. Assert `node.kind == MacroKind::ProcedureMacro { kind: ProcMacroKind::Attribute }`.
    *   `test_macro_node_field_attributes`: Target `attributed_macro` (declarative) and `documented_proc_macro` (procedural). Check `node.attributes` length and content (e.g., presence of `allow`, `deprecated`).
    *   `test_macro_node_field_docstring`: Target `documented_macro` (declarative) and `documented_proc_macro` (procedural). Check `node.docstring` presence and basic content.
    *   `test_macro_node_field_body`: Target `exported_macro` and `function_like_proc_macro`. Check `node.body` presence and that it contains a reasonable string representation of the macro's definition/tokens.
    *   `test_macro_node_field_tracking_hash_presence`: Target `local_macro`. Assert `node.tracking_hash.is_some()`.
    *   `test_macro_node_field_span`: Target `exported_macro`. Check `node.span` is non-zero (basic check).

### Tier 3: Subfield Variations

*   **Goal:** Explicitly verify variants within complex fields like `kind`.
*   **Coverage:** Covered by Tier 2 tests for `MacroKind` and `ProcMacroKind` variants.

### Tier 4: Basic Connection Tests

*   **Goal:** Verify the `Contains` relationship between modules and `MacroNode`s.
*   **Tests:**
    *   `test_macro_node_relation_contains_file_module`: Target `exported_macro` in `crate::macros` module. Find module and macro nodes, assert `Contains` relation exists, assert macro ID is in `module.items()`.
    *   `test_macro_node_relation_contains_inline_module`: Target `inner_local_macro` in `crate::macros::inner_macros`. Find inline module and macro nodes, assert `Contains` relation exists, assert macro ID is in `module.items()`.

### Tier 5: Extreme Paranoia Tests

*   **Goal:** Perform exhaustive checks on one declarative and one procedural macro using paranoid helpers.
*   **Helper:** `find_macro_node_paranoid` (to be created in `macros_helpers.rs`).
    *   Takes `parsed_graphs`, `fixture_name`, `relative_file_path`, `expected_module_path`, `macro_name`.
    *   Finds the `ParsedCodeGraph`.
    *   Finds the `ModuleNode`.
    *   Filters `graph.macros` by name and module association.
    *   Asserts uniqueness.
    *   Extracts span from the found `MacroNode`.
    *   Regenerates `NodeId::Synthetic` using context + extracted span.
    *   Asserts found ID == regenerated ID.
    *   Returns the validated `&MacroNode`.
*   **Tests:**
    *   `test_macro_node_paranoid_declarative`: Target `documented_macro`.
        *   Use `find_macro_node_paranoid`.
        *   Assert all fields (`id`, `name`, `span`, `visibility`, `kind`, `attributes`, `docstring`, `body`, `tracking_hash`).
        *   Verify `Contains` relation from parent module.
        *   Verify uniqueness (ID, name+path) within the graph.
    *   `test_macro_node_paranoid_procedural`: Target `documented_proc_macro`.
        *   Use `find_macro_node_paranoid`.
        *   Assert all fields (`id`, `name`, `span`, `visibility`, `kind` (including `ProcMacroKind`), `attributes`, `docstring`, `body`, `tracking_hash`).
        *   Verify `Contains` relation from parent module.
        *   Verify uniqueness (ID, name+path) within the graph.

---
**Notes:**
*   The `body` field for `MacroNode` contains the stringified token stream of the macro definition (for `macro_rules!`) or the function body (for procedural macros). Tests should verify its presence and basic non-emptiness, not attempt to parse it further.
*   Procedural macros are identified by the presence of `#[proc_macro]`, `#[proc_macro_derive]`, or `#[proc_macro_attribute]` on the *function* definition (`ItemFn`). The visitor handles creating a `MacroNode` instead of a `FunctionNode` in these cases.
