# Test Plan for ImportNode Parsing (Phase 2 - `uuid_ids`)

**Goal:** Verify that `use` and `extern crate` statements are correctly parsed into `ImportNode` instances within the `CodeGraph` during Phase 2. This includes accurate capture of paths, names (including renames), kinds, glob status, spans, and relationships to the containing module.

**Fixture:** `tests/fixture_crates/fixture_nodes/src/imports.rs`
**Test File:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/imports.rs`
**Helper Location:** `crates/ingest/syn_parser/tests/common/paranoid/import_helpers.rs`

---

## Test Tiers

### Tier 1: Basic Smoke Tests

*   **Goal:** Quickly verify that `ImportNode`s are created for various import types (`use` simple, rename, group, glob, relative paths, `extern crate`) and basic properties are present. Uses full crate parse.
*   **Test:** `test_import_node_basic_smoke_test_full_parse`
    *   Use `run_phase1_phase2("fixture_nodes")`.
    *   Find the `ParsedCodeGraph` for `imports.rs`.
    *   Iterate through expected import targets (e.g., `HashMap`, `IoResult`, `File`, `*` from `std::env`, `SubItem`, `AttributedStruct`, `MyId`, `Duration`, `serde`, `SerdeAlias`).
    *   For each:
        *   Find the `ImportNode` (e.g., using `graph.use_statements.iter().find(|i| i.visible_name == ... && i.path == ...)`).
        *   Assert node exists.
        *   Assert `node.id` is `NodeId::Synthetic(_)`.
        *   Assert `node.span` is non-zero.
        *   Assert `node.kind` matches the expected variant (`UseStatement` or `ExternCrate`).

### Tier 2: Targeted Field Verification

*   **Goal:** Verify each field of the `ImportNode` struct individually for specific examples.
*   **Tests:**
    *   `test_import_node_field_id_regeneration`: Target `HashMap`. Regenerate ID using context (`visible_name`, span) and assert match.
    *   `test_import_node_field_span`: Target `fmt`. Check `node.span` is non-zero.
    *   `test_import_node_field_path`: Target `HashMap` (expected `["std", "collections", "HashMap"]`), `File` (expected `["std", "fs", "File"]`), `SubItem` (expected `["self", "sub_imports", "SubItem"]`), `serde` (expected `["serde"]`). Assert `node.path` matches expected `Vec<String>`.
    *   `test_import_node_field_kind_use`: Target `HashMap`. Assert `node.kind == ImportKind::UseStatement`.
    *   `test_import_node_field_kind_extern_crate`: Target `serde`. Assert `node.kind == ImportKind::ExternCrate`.
    *   `test_import_node_field_visible_name_simple`: Target `HashMap`. Assert `node.visible_name == "HashMap"`.
    *   `test_import_node_field_visible_name_renamed`: Target `IoResult`. Assert `node.visible_name == "IoResult"`. Target `SerdeAlias`. Assert `node.visible_name == "SerdeAlias"`.
    *   `test_import_node_field_visible_name_glob`: Target `std::env::*`. Assert `node.visible_name == "*"`.
    *   `test_import_node_field_original_name_simple`: Target `HashMap`. Assert `node.original_name.is_none()`.
    *   `test_import_node_field_original_name_renamed`: Target `IoResult`. Assert `node.original_name == Some("Result")`. Target `SerdeAlias`. Assert `node.original_name == Some("serde")`.
    *   `test_import_node_field_is_glob_true`: Target `std::env::*`. Assert `node.is_glob == true`.
    *   `test_import_node_field_is_glob_false`: Target `HashMap`. Assert `node.is_glob == false`.

### Tier 3: Subfield Variations

*   **Goal:** Explicitly verify variants within complex fields like `kind`.
*   **Coverage:** Covered by Tier 2 tests for `ImportKind` variants (`UseStatement`, `ExternCrate`).

### Tier 4: Basic Connection Tests

*   **Goal:** Verify the `Contains` and `ModuleImports` relationships between modules and `ImportNode`s.
*   **Tests:**
    *   `test_import_node_relation_contains_file_module`: Target `HashMap` in `crate::imports` module. Find module and import nodes, assert `Contains` relation exists, assert import ID is in `module.items()`.
    *   `test_import_node_relation_module_imports_file_module`: Target `HashMap` in `crate::imports` module. Find module and import nodes, assert `ModuleImports` relation exists.
    *   `test_import_node_relation_contains_inline_module`: Target `Arc` in `crate::imports::sub_imports`. Find inline module and import nodes, assert `Contains` relation exists, assert import ID is in `module.items()`.
    *   `test_import_node_relation_module_imports_inline_module`: Target `Arc` in `crate::imports::sub_imports`. Find inline module and import nodes, assert `ModuleImports` relation exists.
    *   `test_import_node_in_module_imports_list`: Target `HashMap` in `crate::imports`. Find module node, assert its `imports` list contains an `ImportNode` matching the expected properties for `HashMap`.

### Tier 5: Extreme Paranoia Tests

*   **Goal:** Perform exhaustive checks on representative import types using paranoid helpers.
*   **Helper:** `find_import_node_paranoid` (to be created in `import_helpers.rs`).
    *   Takes `parsed_graphs`, `fixture_name`, `relative_file_path`, `expected_module_path`.
    *   Takes identifying info: `visible_name`, `expected_path`, `expected_original_name`, `expected_is_glob`.
    *   Finds the `ParsedCodeGraph`.
    *   Finds the `ModuleNode`.
    *   Filters `graph.use_statements` by ALL identifying info.
    *   Ensures the found node's ID is in `module.items()`.
    *   Asserts uniqueness.
    *   Extracts span from the found `ImportNode`.
    *   Regenerates `NodeId::Synthetic` using context (`visible_name` or `*`) + extracted span.
    *   Asserts found ID == regenerated ID.
    *   Returns the validated `&ImportNode`.
*   **Tests:**
    *   `test_import_node_paranoid_simple`: Target `use std::collections::HashMap;`.
        *   Use `find_import_node_paranoid`.
        *   Assert all fields (`id`, `span`, `path`, `kind`, `visible_name`, `original_name`, `is_glob`).
        *   Verify `Contains` and `ModuleImports` relations from parent module.
        *   Verify uniqueness (ID, path+name+module) within the graph.
    *   `test_import_node_paranoid_renamed`: Target `use std::io::Result as IoResult;`.
        *   Use `find_import_node_paranoid`.
        *   Assert all fields, paying attention to `visible_name` vs `original_name`.
        *   Verify relations and uniqueness.
    *   `test_import_node_paranoid_glob`: Target `use std::env::*;`.
        *   Use `find_import_node_paranoid`.
        *   Assert all fields, paying attention to `visible_name == "*"` and `is_glob == true`.
        *   Verify relations and uniqueness.
    *   `test_import_node_paranoid_self`: Target `use self::sub_imports::SubItem;`.
        *   Use `find_import_node_paranoid`.
        *   Assert all fields, paying attention to `path` starting with `self`.
        *   Verify relations and uniqueness.
    *   `test_import_node_paranoid_extern_crate`: Target `extern crate serde;`.
        *   Use `find_import_node_paranoid`.
        *   Assert all fields, paying attention to `kind == ExternCrate`, `path == ["serde"]`, `visible_name == "serde"`.
        *   Verify relations and uniqueness.
    *   `test_import_node_paranoid_extern_crate_renamed`: Target `extern crate serde as SerdeAlias;`.
        *   Use `find_import_node_paranoid`.
        *   Assert all fields, paying attention to `kind == ExternCrate`, `path == ["serde"]`, `visible_name == "SerdeAlias"`, `original_name == Some("serde")`.
        *   Verify relations and uniqueness.

---
**Notes:**
*   The parser correctly handles `::` absolute paths via `syn`'s `ItemUse.leading_colon`, which influences the base path passed to `process_use_tree`. Tests should verify the resulting `path` field.
*   The `RelationKind::Uses` is *not* expected to be generated by the visitor for `use` or `extern crate` statements in Phase 2. Resolution of what an import *points to* happens in Phase 3.
