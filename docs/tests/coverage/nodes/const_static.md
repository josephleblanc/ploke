Okay, let's perform a full review of the old `#[cfg(not(feature = "type_bearing_ids"))]` tests in `const_static.rs` and compare their coverage to what the new macros (`paranoid_test_name_check!` and `paranoid_test_fields_and_values_const!`) provide.

**Goal:** Determine if the new macros are sufficient replacements for the old tests, allowing us to remove the old ones while preserving their intent via comments.

**Old Tests Under Review (Marked with `#[cfg(not(feature = "type_bearing_ids"))]`):**

1.  `test_value_node_field_visibility_public`
2.  `test_value_node_field_visibility_inherited`
3.  `test_value_node_field_visibility_crate`
4.  `test_value_node_field_visibility_super`
5.  `test_value_node_field_type_id_presence`
6.  `test_value_node_field_kind_const`
7.  `test_value_node_field_kind_static_imm`
8.  `test_value_node_field_kind_static_mut`
9.  `test_value_node_field_value_string`
10. `test_value_node_field_attributes_single` (Focuses on CFG filtering)
11. `test_value_node_field_attributes_multiple` (Focuses on non-CFG attributes)
12. `test_value_node_field_docstring`
13. `test_value_node_field_tracking_hash_presence`
14. `test_value_node_relation_contains_file_module`
15. `test_value_node_relation_contains_inline_module`
16. `test_value_node_paranoid_const_doc_attr` (Comprehensive check for one const)
17. `test_value_node_paranoid_static_mut_inner_mod` (Comprehensive check for one static)
18. (Ignored tests for associated items - not relevant for replacement yet)

**Coverage Provided by New Macros:**

*   **`paranoid_test_name_check!(...)`:**
    *   **Primary Goal:** Verify node name matches expected ident after finding the node via regenerated PID.
    *   **Checks:**
        *   Node Name (`node.name() == args.ident`)
    *   **Implicit Checks:**
        *   Correct PID generation (`args.generate_pid`).
        *   Node existence by PID (`graph.find_node_unique(pid)`).
        *   Node uniqueness by PID (`graph.find_node_unique(pid)`).
        *   Parent module path resolution (`find_module_by_path_checked` inside `generate_pid`).

*   **`paranoid_test_fields_and_values_const!(...)`:** (Currently specific to `ConstNode`)
    *   **Primary Goal:** Verify all fields match `ExpectedConstData` and node is findable by value.
    *   **Checks (via `is_*_match_debug` methods):**
        *   Node Name
        *   Visibility
        *   Attributes (non-CFG)
        *   Type ID Presence (`matches!(..., TypeId::Synthetic(_))`)
        *   Value String (`node.value`)
        *   Docstring Content (`node.docstring`)
        *   Tracking Hash Presence (`matches!(..., Some(TrackingHash(_)))`)
        *   CFGs (`node.cfgs()`)
    *   **Checks (Direct):**
        *   Node findable by value (`expected_data.find_node_by_values(graph).count() == 1`).
    *   **Implicit Checks:**
        *   Same as `paranoid_test_name_check!` (if ID lookup path succeeds).
        *   Node existence/uniqueness by *value*.
        *   Correct downcasting (`node.as_const()`).

**Comparison: Old Tests vs. New Macros**

Let's see which old test functionalities are covered by invoking the new macros:

| Old Test Functionality                                  | Covered by `paranoid_test_name_check!`? | Covered by `paranoid_test_fields_and_values_const!` (for Const)? | Notes                                                                                                |
| :------------------------------------------------------ | :-------------------------------------: | :---------------------------------------------------------------: | :--------------------------------------------------------------------------------------------------- |
| **Field Checks:**                                       |                                         |                                                                   |                                                                                                      |
| Visibility (Public, Inherited, Crate, Super)            | No                                      | **Yes** (via `is_vis_match_debug`)                                | Requires `ExpectedConstData` to have correct visibility.                                             |
| Type ID Presence (Synthetic)                            | No                                      | **Yes** (via `is_type_id_check_match_debug`)                      |                                                                                                      |
| Kind (`ItemKind::Const`)                                | No                                      | **Implicitly** (Macro is const-specific, checks `as_const()`)     | No explicit `node.kind()` check, but assumes const.                                                  |
| Kind (`ItemKind::Static`)                               | No                                      | No (Macro is const-specific)                                      | Needs a static version of the macro.                                                                 |
| Static Mutability (`is_mutable`)                        | No                                      | No (Macro is const-specific)                                      | Needs a static version of the macro.                                                                 |
| Value String (`node.value`)                             | No                                      | **Yes** (via `is_value_match_debug`)                              |                                                                                                      |
| Attributes (Non-CFG)                                    | No                                      | **Yes** (via `is_attr_match_debug`)                               | Covers `_attributes_multiple`.                                                                       |
| CFGs (Filtering & `node.cfgs()`)                        | No                                      | **Yes** (via `is_cfgs_match_debug`)                               | Covers `_attributes_single`'s intent by checking the final `cfgs` field.                             |
| Docstring (Presence & Content)                          | No                                      | **Yes** (via `is_docstring_contains_match_debug`)                 |                                                                                                      |
| Tracking Hash Presence                                  | No                                      | **Yes** (via `is_tracking_hash_check_match_debug`)                |                                                                                                      |
| **ID & Existence Checks:**                              |                                         |                                                                   |                                                                                                      |
| PID Regeneration & Validation                           | **Yes** (Implicitly & Explicitly)       | **Yes** (Implicitly & Explicitly, optional path)                  | Core functionality of the new approach.                                                              |
| Node Existence/Uniqueness by PID                        | **Yes** (via `find_node_unique`)        | **Yes** (via `find_node_unique`, optional path)                   |                                                                                                      |
| Node Existence/Uniqueness by Value                      | No                                      | **Yes** (via `find_node_by_values().count() == 1`)                |                                                                                                      |
| **Relation Checks:**                                    |                                         |                                                                   |                                                                                                      |
| `Contains` Relation (Module -> Item)                    | No                                      | No                                                                | **GAP:** Macros don't check graph relations.                                                         |
| Item present in `ModuleNode.items()`                    | No                                      | No                                                                | **GAP:** Related to the `Contains` check.                                                            |
| **Advanced Checks (from old paranoid tests):**          |                                         |                                                                   |                                                                                                      |
| `TypeId` Regeneration & Validation                      | No                                      | No                                                                | **GAP:** Was present in old paranoid tests.                                                          |
| Name Uniqueness within Module                           | No                                      | No                                                                | **GAP:** Was implicitly checked by old paranoid helpers.                                             |
| `TypeId` Uniqueness (where applicable)                  | No                                      | No                                                                | **GAP:** Was checked in old paranoid tests.                                                          |
| **Comprehensive Field Check (like old paranoid tests)** | No                                      | **Yes** (Calls all `is_*_match_debug` helpers)                    | `paranoid_test_fields_and_values_const!` essentially replaces the field checks of old paranoid tests. |

**Conclusion & Recommendation:**

1.  **Keep the Comments:** Absolutely preserve the descriptive comments above the old tests. They document the *intent* and the specific variations being tested (e.g., "Target: TOP\_LEVEL\_BOOL (pub)"). These comments should be moved to preface the corresponding macro invocations.

2.  **`paranoid_test_fields_and_values_const!` is Sufficient for *Const* Field Tests:** For `ConstNode` items, invoking `paranoid_test_fields_and_values_const!` covers *all* the individual field checks performed by the old tests (visibility, type ID presence, value, attributes, CFGs, docstring, tracking hash). It also adds the value-based lookup check.

3.  **Need `paranoid_test_fields_and_values_static!`:** You need an equivalent macro for `StaticNode`s that uses `ExpectedStaticData` (once it has the `is_*_match_debug` and `find_node_by_values` helpers). This would cover the static-specific field tests.

4.  **Gaps Remain:** The current macros **do not** cover:
    *   **Relation Checks:** `Contains` relation from module to item.
    *   **TypeID Validation:** Regenerating the expected `TypeId` and comparing it.
    *   **Explicit Kind Check:** Asserting `node.kind()`.
    *   **Broader Uniqueness:** Name uniqueness within a module, TypeID uniqueness.

5.  **Decision:**
    *   You **can remove** the old tests numbered 1-13 (`test_value_node_field_*`) *provided* you:
        *   Create and use the equivalent `paranoid_test_fields_and_values_static!` macro for static items.
        *   Invoke the appropriate macro (`..._const` or `..._static`) for each item previously targeted by tests 1-13.
        *   Migrate the descriptive comments.
    *   You **cannot yet remove** tests 14-17 (`test_value_node_relation_*`, `test_value_node_paranoid_*`) because their full functionality (relations, TypeID validation, uniqueness) is not replicated by the current field/value-checking macros alone.
    *   The `paranoid_test_fields_and_values_*` macros *do* replace the *field checking* part of the old paranoid tests (16, 17), but not the relation or TypeID parts.

**Recommendation:**

*   **Implement `ExpectedStaticData` helpers** (`is_*_match_debug`, `find_node_by_values`).
*   **Create `paranoid_test_fields_and_values_static!`** macro, mirroring the const version.
*   **Replace old tests 1-13:** For each item targeted by these old tests, add a corresponding `paranoid_test_fields_and_values_const!(...)` or `paranoid_test_fields_and_values_static!(...)` invocation. Move the descriptive comment from the old test to the new macro invocation. Then delete the old test function.
*   **Keep old tests 14-17 for now:** Leave `test_value_node_relation_*` and `test_value_node_paranoid_*` in place, perhaps still gated by the old feature flag, until you decide how to incorporate relation checking, TypeID validation, and broader uniqueness checks into your new testing strategy (either by enhancing the macros or creating new dedicated tests/macros).
*   **Consider adding explicit `node.kind()` checks** within the new macros if that level of explicitness is desired, although the `as_const()`/`as_static()` checks provide some assurance.
