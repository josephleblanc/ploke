//! Tests for `ModuleNode` parsing and field extraction.
//!
//! ## Test Coverage Analysis
//!
//! *   **Fixture:** `tests/fixture_crates/file_dir_detection/`
//! *   **Tests:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/modules.rs` (using `paranoid_test_fields_and_values!`)
//!
//! ### 1. Coverage of Fixture Items:
//!
//! The `EXPECTED_MODULES_DATA` and `EXPECTED_MODULES_ARGS` maps cover 19 distinct module declarations or definitions from the `file_dir_detection` fixture. This includes:
//! *   Crate root module (`main.rs`).
//! *   Top-level file-based modules (e.g., `top_pub_mod.rs`, `top_priv_mod.rs`).
//! *   Inline modules (e.g., `inline_pub_mod`, `inline_priv_mod` in `main.rs`).
//! *   Modules declared with `#[path]` attributes (e.g., `logical_name` pointing to `custom_path/real_file.rs`).
//! *   Nested modules, both file-based (e.g., `example_mod/mod.rs`, `example_mod/example_submod/mod.rs`) and inline.
//! *   Deeply nested modules (e.g., `deeply_nested_mod` and `deeply_nested_file`).
//! *   Modules within subdirectories.
//!
//! **Conclusion for Fixture Coverage:** Good. A diverse range of module structures (declarations, file-based definitions, inline definitions, path attributes, nesting) from the fixture are covered by the `paranoid_test_fields_and_values!` tests.
//!
//! ### 2. Coverage of `ModuleNode` Property Variations:
//!
//! Based on the 19 items covered by `paranoid_test_fields_and_values!`:
//!
//! *   `id: ModuleNodeId`: Implicitly covered by ID generation and lookup.
//! *   `name: String`: Excellent coverage (e.g., "crate", "top_pub_mod", "inline_pub_mod", "real_file" from file stem, "logical_name").
//! *   `path: Vec<String>`: Excellent coverage (crate root `["crate"]`, simple `["crate", "foo"]`, nested `["crate", "foo", "bar"]`, path-attribute influenced paths like `["crate", "custom_path", "real_file"]`).
//! *   `visibility: VisibilityKind`: Good coverage (`Public`, `Inherited`, `Crate`). `VisibilityKind::Restricted` (e.g., `pub(in path)`) is not explicitly tested.
//! *   `attributes: Vec<Attribute>`: Good coverage (no attributes, `#[path = "..."]`, `#[cfg(...)]` which is then moved to `cfgs` field).
//! *   `docstring: Option<String>`: Good coverage (`Some` for `inline_pub_mod`, `None` for most declarations and file-based module definitions). File-level docstrings (`//!`) are checked via `file_docs_is_some`.
//! *   `imports: Vec<ImportNode>`: Covered by `ExpectedModuleNode.imports_count`. Good coverage (modules with 0, 1, or 2 imports are tested).
//! *   `exports: Vec<ImportNodeId>`: Covered by `ExpectedModuleNode.exports_count`. Currently always 0 as Phase 2 does not populate `exports`. This is a known limitation.
//! *   `span: (usize, usize)`: Not directly asserted by value.
//! *   `tracking_hash: Option<TrackingHash>`: Covered by `ExpectedModuleNode.tracking_hash_check`. Excellent coverage (checked for `Some` on declarations/inline modules, and `None` for file-based root module definitions as per current implementation).
//! *   `module_def: ModuleKind`:
//!     *   `mod_disc: ModDisc`: Excellent coverage (all three variants `FileBased`, `Inline`, `Declaration` are tested).
//!     *   `expected_file_path_suffix: Option<&'static str>`: Excellent coverage (checked for `FileBased` modules, `None` for others).
//!     *   `items_count: usize`: Good coverage (various counts, including 0 for declarations, and counts for items within file-based and inline modules).
//!     *   `file_attrs_count: usize`: Good coverage (tested for `main.rs` which has `#![allow(unused)]`).
//!     *   `file_docs_is_some: bool`: Good coverage (tested for `main.rs` which has `//! ...`).
//! *   `cfgs: Vec<String>`: Fair coverage (tested `#[cfg(test)]` on `inline_pub_mod`). More complex `cfg` attributes or combinations are not explicitly tested.
//!
//! **Conclusion for Property Variation Coverage:** Most `ModuleNode` fields have good to excellent coverage.
//! *   **Areas for potential expansion:**
//!     *   `VisibilityKind::Restricted`.
//!     *   More complex `#[cfg(...)]` attributes.
//!     *   Testing `exports_count` once Phase 3 populates `ModuleNode.exports`.
//!
//! ### 3. Differences in Testing `ModuleNode` vs. Other Nodes:
//!
//! Testing `ModuleNode`s has several unique aspects:
//!
//! *   **`ModuleKind` Variants:** The core distinction is between `FileBased`, `Inline`, and `Declaration` modules. `ExpectedModuleNode` uses `mod_disc` and associated fields (`expected_file_path_suffix`, `items_count`, `file_attrs_count`, `file_docs_is_some`) to verify these.
//! *   **Declarations vs. Definitions:** The parser creates distinct `ModuleNode`s for a module declaration (`mod foo;`) and its definition (e.g., `foo.rs` or `mod foo {}`). Tests verify properties specific to each, like `tracking_hash` presence or `imports_count` (declarations should have 0).
//! *   **`#[path]` Attribute:** The `logical_name` test case specifically covers a `mod logical_name;` declaration with a `#[path = "custom_path/real_file.rs"]` attribute. The test for the `Declaration` node checks for this attribute, while the test for the `FileBased` definition node (`real_file`) verifies its file-derived path and name.
//! *   **File-Level vs. Item-Level Metadata:** For `FileBased` modules, `ModuleNode.module_def.FileBased.file_attrs` and `file_docs` capture `#![...]` and `//!` from the module's file. These are distinct from attributes/docs on the `mod item;` itself (which are on `ModuleNode.attributes` and `ModuleNode.docstring`). `ExpectedModuleNode` checks these via `file_attrs_count` and `file_docs_is_some`.
//! *   **`items` Field:** `ExpectedModuleNode.items_count` verifies the number of `PrimaryNodeId`s directly contained within a module's definition. The `paranoid_test_fields_and_values!` macro also asserts the `SyntacticRelation::Contains` between the parent module and the tested node.
//! *   **`imports` and `exports`:** `imports_count` checks the number of `ImportNode`s parsed directly within the module. `exports_count` is for re-exported `ImportNodeId`s, which are not populated in Phase 2.
//!
//! ### 4. Lost Coverage from Old Tests:
//!
//! The refactoring to `paranoid_test_fields_and_values!` replaces older, more manual tests. Potential areas of lost coverage include:
//!
//! *   **Explicit Span Checks:** Older tests might have explicitly checked `ModuleNode.span`, `ModuleKind::Declaration.declaration_span`, or `ModuleKind::Inline.span` for non-zero values or specific ranges. The new macro framework does not assert specific span values.
//! *   **Explicit ID Regeneration Assertions:** While the new macro framework uses ID generation for lookup, it doesn't explicitly assert the regeneration logic for module IDs in the same direct way some older tests might have.
//! *   **Specific Relation Checks (Phase 3 concepts):**
//!     *   `RelationKind::ModuleDeclarationResolvesToDefinition`: Older tests (potentially targeting Phase 3 logic) would have checked the link between a `ModuleNode` of kind `Declaration` and its corresponding `FileBased` or `Inline` definition node. This relation is established in Phase 3 and not covered by the current Phase 2 `paranoid_test_fields_and_values!` tests for `ModuleNode`.
//! *   **`ModuleKind::Declaration.resolved_definition` Field:** This `Option<ModuleNodeId>` field is populated during Phase 3 to link a declaration to its definition. Phase 2 tests would only see this as `None`. The `ExpectedModuleNode` does not currently have a field to check this.
//! *   **Detailed `items` List Verification:** Old tests might have asserted the exact set and order of `PrimaryNodeId`s within a `ModuleNode.items` list. The new `ExpectedModuleNode.items_count` only checks the length. However, individual tests for other node types (e.g., `FunctionNode`) do verify their `Contains` relation from their parent module, providing indirect coverage.
//!
//! ### 5. Suggestions for Future Inclusions:
//!
//! *   Add fixture modules using `pub(in path) some_module;` to test `VisibilityKind::Restricted`.
//! *   Expand `cfgs` coverage with more complex `#[cfg(...)]` attributes on modules (e.g., `#[cfg(all(unix, target_pointer_width = "64"))]`).
//! *   Once Phase 3 logic is integrated into testing:
//!     *   Add tests to verify that `ModuleNode.exports` (and `ExpectedModuleNode.exports_count`) are correctly populated for modules containing `pub use` statements.
//!     *   Add tests to verify that `ModuleKind::Declaration.resolved_definition` is correctly populated.
//!     *   Add tests to verify the presence of `RelationKind::ModuleDeclarationResolvesToDefinition`.
//! *   If precise span checking for module declarations or inline blocks becomes critical, consider adding specific assertions for `declaration_span` or `inline_span`, possibly through an extension to `ExpectedModuleNode` or separate, targeted tests.
//! *   Add tests for modules containing `extern crate` items, ensuring they are correctly included in `items_count` and that the corresponding `ImportNode` is created.
//! *   Add tests for modules that are part of a `#[cfg_attr(..., path = "...")]` scenario.
use ploke_core::ItemKind;
use syn_parser::parser::graph::GraphAccess;
// Import TypeAliasNode specifically
use syn_parser::parser::types::VisibilityKind;
// Import EnumNode specifically
use crate::common::{new_path_attribute, ParanoidArgs};
use lazy_static::lazy_static;
use std::collections::HashMap;
use syn_parser::parser::nodes::{ExpectedModuleNode, GraphNode, ImportNode, ModDisc};

// macro-related imports
use crate::paranoid_test_fields_and_values;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::nodes::{Attribute, ExpectedImportNode};

pub const LOG_TEST_MODULE: &str = "log_test_module";

lazy_static! {
    static ref EXPECTED_MODULES_DATA: HashMap<&'static str, ExpectedModuleNode> = {
        let mut m = HashMap::new();

        // Test case: `pub mod top_pub_mod;` (declaration in main.rs)
        // Key format: "decl::{file_where_decl_is}::{module_name_at_decl}"
        m.insert("decl::main_rs::top_pub_mod", ExpectedModuleNode {
            name: "top_pub_mod", // Name of the module as declared
            path: &["crate", "top_pub_mod"], // Canonical path of the module
            visibility: VisibilityKind::Public, // Visibility of the `mod ...;` statement
            attributes: vec![],
            docstring: None,
            imports_count: 0, // Declarations should have 0 imports
            exports_count: 0,
            tracking_hash_check: true, // Declarations have a tracking hash
            mod_disc: ModDisc::Declaration,
            expected_file_path_suffix: None, // Not FileBased
            items_count: 0, // Declarations don't have items in ModuleNode.items directly
            file_attrs_count: 0, // Not FileBased
            file_docs_is_some: false, // Not FileBased
            cfgs: vec![],
        });

        // Test case: `top_pub_mod` (definition in top_pub_mod.rs)
        // Key format: "file::{path_to_definition_file}::{module_name_as_per_file_rules}"
        m.insert("file::top_pub_mod_rs::top_pub_mod", ExpectedModuleNode {
            name: "top_pub_mod", // Name of the module (often from file stem or parent decl)
            path: &["crate", "top_pub_mod"], // Canonical path
            visibility: VisibilityKind::Inherited, // File-level module definitions have inherited visibility
            attributes: vec![],
            docstring: None,
            imports_count: 0, // No imports directly in top_pub_mod.rs
            exports_count: 0,
            tracking_hash_check: false, // File-level root module definitions don't have a separate tracking hash in current impl
            mod_disc: ModDisc::FileBased,
            expected_file_path_suffix: Some("file_dir_detection/src/top_pub_mod.rs"), // Relative to fixture root
            items_count: 6, // top_pub_func, duplicate_name, top_pub_priv_func, mod nested_pub, mod nested_priv, mod path_visible_mod
            file_attrs_count: 0, // No file-level attributes in top_pub_mod.rs
            file_docs_is_some: false, // No file-level doc comments in top_pub_mod.rs
            cfgs: vec![],
        });

        // --- main.rs declarations ---
        m.insert("decl::main_rs::top_priv_mod", ExpectedModuleNode {
            name: "top_priv_mod",
            path: &["crate", "top_priv_mod"],
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("decl::main_rs::crate_visible_mod", ExpectedModuleNode {
            name: "crate_visible_mod",
            path: &["crate", "crate_visible_mod"], // in src/main.rs
            visibility: VisibilityKind::Crate, // pub(crate)
            attributes: vec![],
            docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("decl::main_rs::logical_name", ExpectedModuleNode {
            name: "logical_name",
            path: &["crate", "logical_name"],
            visibility: VisibilityKind::Public,
            attributes: vec![new_path_attribute("custom_path/real_file.rs")],
            docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("decl::main_rs::example_mod", ExpectedModuleNode {
            name: "example_mod",
            path: &["crate", "example_mod"],
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- main.rs inline definitions ---
        m.insert("inline::main_rs::inline_pub_mod", ExpectedModuleNode {
            name: "inline_pub_mod",
            path: &["crate", "inline_pub_mod"],
            visibility: VisibilityKind::Public,
            attributes: vec![], // cfg(test) is extracted to cfgs
            docstring: Some("An inline public module doc comment."),
            imports_count: 1, // use std::collections::HashMap;
            exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Inline, expected_file_path_suffix: None,
            items_count: 5, // HashMap import, inline_pub_func, duplicate_name, inline_nested_priv decl, super_visible_inline decl
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec!["test".to_string()],
        });
        m.insert("inline::main_rs::inline_priv_mod", ExpectedModuleNode {
            name: "inline_priv_mod",
            path: &["crate", "inline_priv_mod"],
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Inline, expected_file_path_suffix: None,
            items_count: 2, // inline_priv_func, inline_nested_pub decl
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- File-based module definitions (other than top_pub_mod.rs) ---
        m.insert("file::top_priv_mod_rs::top_priv_mod", ExpectedModuleNode {
            name: "top_priv_mod",
            path: &["crate", "top_priv_mod"],
            visibility: VisibilityKind::Inherited, attributes: vec![], docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased, expected_file_path_suffix: Some("file_dir_detection/src/top_priv_mod.rs"),
            items_count: 4, // nested_pub_in_priv decl, nested_priv_in_priv decl, top_priv_func, top_priv_priv_func
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("file::crate_visible_mod_rs::crate_visible_mod", ExpectedModuleNode {
            name: "crate_visible_mod",
            path: &["crate", "crate_visible_mod"],
            visibility: VisibilityKind::Inherited, attributes: vec![], docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased, expected_file_path_suffix: Some("file_dir_detection/src/crate_visible_mod.rs"),
            items_count: 3, // crate_vis_func, nested_priv decl, nested_crate_vis decl
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("file::custom_path_real_file_rs::real_file", ExpectedModuleNode {
            name: "real_file", // Name from file stem due to #[path]
            path: &["crate", "custom_path", "real_file"], // Path from file system
            visibility: VisibilityKind::Inherited, attributes: vec![], docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased, expected_file_path_suffix: Some("file_dir_detection/src/custom_path/real_file.rs"),
            items_count: 2, // item_in_real_file, nested_in_real_file decl
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("file::example_mod_mod_rs::example_mod", ExpectedModuleNode {
            name: "example_mod",
            path: &["crate", "example_mod"],
            visibility: VisibilityKind::Inherited, attributes: vec![], docstring: None,
            imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased, expected_file_path_suffix: Some("file_dir_detection/src/example_mod/mod.rs"),
            items_count: 6, // example_submod decl, example_private_submod decl, mod_sibling_one decl, mod_sibling_two decl, mod mod_sibling_private, item_in_example_mod
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- example_mod/mod.rs declarations ---
        m.insert("decl::example_mod_mod_rs::example_submod", ExpectedModuleNode {
            name: "example_submod", path: &["crate", "example_mod", "example_submod"], visibility: VisibilityKind::Public,
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });
        m.insert("decl::example_mod_mod_rs::example_private_submod", ExpectedModuleNode {
            name: "example_private_submod", path: &["crate", "example_mod", "example_private_submod"], visibility: VisibilityKind::Inherited, // private mod
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- example_mod/example_submod/mod.rs definition ---
        m.insert("file::example_mod_example_submod_mod_rs::example_submod", ExpectedModuleNode {
            name: "example_submod", path: &["crate", "example_mod", "example_submod"], visibility: VisibilityKind::Inherited,
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased, expected_file_path_suffix: Some("file_dir_detection/src/example_mod/example_submod/mod.rs"),
            items_count: 4, // submod_sibling_one, submod_sibling_private, submod_sibling_two, item_in_example_submod
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- Crate root module (main.rs file itself) ---
        m.insert("file::main_rs::crate", ExpectedModuleNode {
            name: "crate", // Name is just "crate" for now, might replace with actual crate name,
                           // e.g. "my_project", "serde", "anyhow", later
            path: &["crate"],
            visibility: VisibilityKind::Public, // Crate root is implicitly public
            attributes: vec![], // Outer attributes of the crate, not file-level #![...]
            docstring: None, // Crate-level docstring is on file_docs
            imports_count: 2, // use std::path::Path; pub use ... as reexported_func;
            exports_count: 0, // Phase 2, exports not populated yet
            tracking_hash_check: false, // File-level root
            mod_disc: ModDisc::FileBased,
            expected_file_path_suffix: Some("file_dir_detection/src/main.rs"),
            items_count: 13, // example_mod, top_pub_mod, top_priv_mod, crate_visible_mod, logical_name, inline_pub_mod, inline_priv_mod, main_pub_func, main_priv_func, reexported_func (ImportNode), duplicate_name, main, use std::path::Path (ImportNode)
            file_attrs_count: 1, // #![allow(unused)]
            file_docs_is_some: true, // //! This is the crate root doc comment.
            cfgs: vec![], // No cfgs on the crate module item itself
        });


        // TODO: Add more entries for all modules in the fixture. This is a representative start.
        // Key areas to cover:
        // - Deeply nested modules (declarations and definitions)
        // - Modules in subdirectories (e.g., example_mod/example_private_submod/...)
        // - Inline modules within other inline or file-based modules
        // - Modules with file-level attributes and doc comments (e.g. main.rs itself)

        // --- deeply_nested_mod (declared in subsubsubmod/mod.rs) ---
        m.insert("decl::example_mod_example_private_submod_subsubmod_subsubsubmod_mod_rs::deeply_nested_mod", ExpectedModuleNode {
            name: "deeply_nested_mod",
            path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod", "deeply_nested_mod"],
            visibility: VisibilityKind::Public,
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- deeply_nested_mod (defined in deeply_nested_mod/mod.rs) ---
        m.insert("file::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs::deeply_nested_mod", ExpectedModuleNode {
            name: "deeply_nested_mod",
            path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod", "deeply_nested_mod"],
            visibility: VisibilityKind::Inherited,
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased,
            expected_file_path_suffix: Some("file_dir_detection/src/example_mod/example_private_submod/subsubmod/subsubsubmod/deeply_nested_mod/mod.rs"),
            items_count: 2, // pub mod deeply_nested_file; fn item_in_deeply_nested_mod
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- deeply_nested_file (declared in deeply_nested_mod/mod.rs) ---
        m.insert("decl::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs::deeply_nested_file", ExpectedModuleNode {
            name: "deeply_nested_file",
            path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod", "deeply_nested_mod", "deeply_nested_file"],
            visibility: VisibilityKind::Public,
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: true,
            mod_disc: ModDisc::Declaration, expected_file_path_suffix: None, items_count: 0,
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        // --- deeply_nested_file (defined in deeply_nested_file.rs) ---
        m.insert("file::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_deeply_nested_file_rs::deeply_nested_file", ExpectedModuleNode {
            name: "deeply_nested_file",
            path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod", "deeply_nested_mod", "deeply_nested_file"],
            visibility: VisibilityKind::Inherited,
            attributes: vec![], docstring: None, imports_count: 0, exports_count: 0, tracking_hash_check: false,
            mod_disc: ModDisc::FileBased,
            expected_file_path_suffix: Some("file_dir_detection/src/example_mod/example_private_submod/subsubmod/subsubsubmod/deeply_nested_mod/deeply_nested_file.rs"),
            items_count: 1, // fn item_in_deeply_nested_file
            file_attrs_count: 0, file_docs_is_some: false, cfgs: vec![],
        });

        m
    };
}

lazy_static! {
    static ref EXPECTED_MODULES_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();

        m.insert("decl::main_rs::top_pub_mod", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/main.rs", // Declaration is in main.rs
            ident: "top_pub_mod",
            expected_path: &["crate"], // Parent module is the crate root
            item_kind: ItemKind::Module,
            expected_cfg: None,
        });

        m.insert("file::top_pub_mod_rs::top_pub_mod", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/top_pub_mod.rs", // Definition is in this file
            ident: "top_pub_mod", // The name of the module itself
            expected_path: &["crate"], // The path of the module's parent defaults to "crate" itself
            item_kind: ItemKind::Module,
            expected_cfg: None,
        });

        // --- main.rs declarations ---
        m.insert("decl::main_rs::top_priv_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/main.rs", ident: "top_priv_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("decl::main_rs::crate_visible_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/main.rs", ident: "crate_visible_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("decl::main_rs::logical_name", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/main.rs", ident: "logical_name",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("decl::main_rs::example_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/main.rs", ident: "example_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- main.rs inline definitions ---
        m.insert("inline::main_rs::inline_pub_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/main.rs", ident: "inline_pub_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: Some(&["test"]),
        });
        m.insert("inline::main_rs::inline_priv_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/main.rs", ident: "inline_priv_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- File-based module definitions ---
        m.insert("file::top_priv_mod_rs::top_priv_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/top_priv_mod.rs", ident: "top_priv_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("file::crate_visible_mod_rs::crate_visible_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/crate_visible_mod.rs", ident: "crate_visible_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("file::custom_path_real_file_rs::real_file", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/custom_path/real_file.rs", ident: "real_file",
            expected_path: &["crate", "custom_path"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("file::example_mod_mod_rs::example_mod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/example_mod/mod.rs", ident: "example_mod",
            expected_path: &["crate"], item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- example_mod/mod.rs declarations ---
        m.insert("decl::example_mod_mod_rs::example_submod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/example_mod/mod.rs", ident: "example_submod",
            expected_path: &["crate", "example_mod"], item_kind: ItemKind::Module, expected_cfg: None,
        });
        m.insert("decl::example_mod_mod_rs::example_private_submod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/example_mod/mod.rs", ident: "example_private_submod",
            expected_path: &["crate", "example_mod"], item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- example_mod/example_submod/mod.rs definition ---
        m.insert("file::example_mod_example_submod_mod_rs::example_submod", ParanoidArgs {
            fixture: "file_dir_detection", relative_file_path: "src/example_mod/example_submod/mod.rs", ident: "example_submod",
            expected_path: &["crate", "example_mod"], item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- Crate root module (main.rs file itself) ---
        m.insert("file::main_rs::crate", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/main.rs",
            ident: "crate", // root name is "crate" for now, might replace later with actual crate
                            // name.
            expected_path: &[], // Its own path
            item_kind: ItemKind::Module,
            expected_cfg: None,
        });


        // TODO: Add more entries for all modules in the fixture.

        // --- deeply_nested_mod (declared in subsubsubmod/mod.rs) ---
        m.insert("decl::example_mod_example_private_submod_subsubmod_subsubsubmod_mod_rs::deeply_nested_mod", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/example_mod/example_private_submod/subsubmod/subsubsubmod/mod.rs",
            ident: "deeply_nested_mod",
            expected_path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod"],
            item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- deeply_nested_mod (defined in deeply_nested_mod/mod.rs) ---
        m.insert("file::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs::deeply_nested_mod", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/example_mod/example_private_submod/subsubmod/subsubsubmod/deeply_nested_mod/mod.rs",
            ident: "deeply_nested_mod",
            expected_path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod"], // Parent path for ID gen
            item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- deeply_nested_file (declared in deeply_nested_mod/mod.rs) ---
        m.insert("decl::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs::deeply_nested_file", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/example_mod/example_private_submod/subsubmod/subsubsubmod/deeply_nested_mod/mod.rs",
            ident: "deeply_nested_file",
            expected_path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod", "deeply_nested_mod"],
            item_kind: ItemKind::Module, expected_cfg: None,
        });

        // --- deeply_nested_file (defined in deeply_nested_file.rs) ---
        m.insert("file::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_deeply_nested_file_rs::deeply_nested_file", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/example_mod/example_private_submod/subsubmod/subsubsubmod/deeply_nested_mod/deeply_nested_file.rs",
            ident: "deeply_nested_file",
            expected_path: &["crate", "example_mod", "example_private_submod", "subsubmod", "subsubsubmod", "deeply_nested_mod"], // Parent path for ID gen
            item_kind: ItemKind::Module, expected_cfg: None,
        });

        m
    };
}

paranoid_test_fields_and_values!(
    node_decl_main_rs_top_pub_mod,
    "decl::main_rs::top_pub_mod",
    EXPECTED_MODULES_ARGS,                         // args_map
    EXPECTED_MODULES_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ModuleNode,         // node_type
    syn_parser::parser::nodes::ExpectedModuleNode, // derived Expeced*Node
    as_module,                                     // downcast_method
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_top_pub_mod_rs_top_pub_mod,
    "file::top_pub_mod_rs::top_pub_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_main_rs_top_priv_mod,
    "decl::main_rs::top_priv_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_top_priv_mod_rs_top_priv_mod,
    "file::top_priv_mod_rs::top_priv_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_main_rs_crate_visible_mod,
    "decl::main_rs::crate_visible_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_crate_visible_mod_rs_crate_visible_mod,
    "file::crate_visible_mod_rs::crate_visible_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_main_rs_logical_name,
    "decl::main_rs::logical_name",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_custom_path_real_file_rs_real_file,
    "file::custom_path_real_file_rs::real_file",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_inline_main_rs_inline_pub_mod,
    "inline::main_rs::inline_pub_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_inline_main_rs_inline_priv_mod,
    "inline::main_rs::inline_priv_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_main_rs_example_mod,
    "decl::main_rs::example_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_example_mod_mod_rs_example_mod,
    "file::example_mod_mod_rs::example_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_example_mod_mod_rs_example_submod,
    "decl::example_mod_mod_rs::example_submod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_example_mod_example_submod_mod_rs_example_submod,
    "file::example_mod_example_submod_mod_rs::example_submod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_example_mod_mod_rs_example_private_submod,
    "decl::example_mod_mod_rs::example_private_submod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_main_rs_crate,
    "file::main_rs::crate",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_example_mod_example_private_submod_subsubmod_subsubsubmod_mod_rs_deeply_nested_mod,
    "decl::example_mod_example_private_submod_subsubmod_subsubsubmod_mod_rs::deeply_nested_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs_deeply_nested_mod,
    "file::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs::deeply_nested_mod",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_decl_example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs_deeply_nested_file,
    "decl::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_mod_rs::deeply_nested_file",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
paranoid_test_fields_and_values!(
    node_file_example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_deeply_nested_file_rs_deeply_nested_file,
    "file::example_mod_example_private_submod_subsubmod_subsubsubmod_deeply_nested_mod_deeply_nested_file_rs::deeply_nested_file",
    EXPECTED_MODULES_ARGS,
    EXPECTED_MODULES_DATA,
    syn_parser::parser::nodes::ModuleNode,
    syn_parser::parser::nodes::ExpectedModuleNode,
    as_module,
    LOG_TEST_MODULE
);
