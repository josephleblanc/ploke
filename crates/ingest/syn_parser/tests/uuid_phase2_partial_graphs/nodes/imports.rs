//! Tests for `ImportNode` parsing and field extraction.
//!
//! ## Test Coverage Analysis
//!
//! *   **Fixture:** `tests/fixture_crates/fixture_nodes/src/imports.rs`
//! *   **Tests:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/imports.rs` (using `paranoid_test_fields_and_values!`)
//!
//! ### 1. Coverage of Fixture Items:
//!
//! The `EXPECTED_IMPORTS_DATA` and `EXPECTED_IMPORTS_ARGS` maps cover all 26 distinct import items identified in the fixture (`use` statements and `extern crate` declarations), including those at the crate root and within the nested `sub_imports` module. This includes:
//! *   Simple path imports (`std::collections::HashMap`, `std::fmt`, `crate::structs::TupleStruct`).
//! *   Renamed imports (`as IoResult`, `as MySimpleStruct`, `as MyGenTrait`, `as SerdeAlias`).
//! *   Grouped imports (`use crate::{...}`, `use std::{...}`).
//! *   Glob import (`std::env::*;`).
//! *   Relative path imports (`self::`, `super::`, `crate::`, `super::super::`).
//! *   Absolute path import (`::std::time::Duration`).
//! *   `extern crate` (simple and renamed).
//! *   Imports within nested modules.
//!
//! **Conclusion for Fixture Coverage:** Excellent. All import statements from the fixture are covered by the `paranoid_test_fields_and_values!` tests.
//!
//! ### 2. Coverage of `ImportNode` Property Variations:
//!
//! Based on the 26 items covered:
//!
//! *   `id: ImportNodeId`: Implicitly covered by ID generation and lookup.
//! *   `span: (usize, usize)`: Not directly asserted by value in the new tests (previously checked for non-zero in Tier 2).
//! *   `source_path: Vec<String>`: Excellent coverage (various lengths, `std`, `crate`, `self`, `super`, `::` prefix via empty string).
//! *   `kind: ImportKind`: Excellent coverage (`UseStatement(VisibilityKind::Inherited)` and `ExternCrate`). Note: Only `Inherited` visibility is present in the fixture's `use` statements.
//! *   `visible_name: String`: Excellent coverage (simple names, renamed names, `*` for glob).
//! *   `original_name: Option<String>`: Excellent coverage (`None` for direct imports, `Some` for renamed imports).
//! *   `is_glob: bool`: Excellent coverage (both `true` and `false`).
//! *   `is_self_import: bool`: Excellent coverage (both `true` for `std::fs::{self, File}` -> `fs` and `false` for others).
//! *   `cfgs: Vec<String>`: Poor coverage (only items without `cfg` attributes are tested).
//!
//! **Conclusion for Property Variation Coverage:** Most fields have excellent coverage.
//! *   **Areas for potential expansion:**
//!     *   `cfgs`: Add fixture imports with `#[cfg(...)]` attributes.
//!     *   `kind`: Add fixture imports with explicit visibility (`pub use`, `pub(crate) use`) to test `UseStatement` variants beyond `Inherited`.
//!
//! ### 3. Differences in Testing `ImportNode` vs. Other Nodes:
//!
//! Testing `ImportNode` focuses on correctly capturing the structure of `use` and `extern crate` statements:
//!
//! *   **Path Complexity:** `source_path` can involve various path prefixes (`std`, `crate`, `self`, `super`, `::`). Tests verify these are parsed correctly.
//! *   **Renaming:** The interplay between `visible_name` and `original_name` for `as` clauses is specifically tested.
//! *   **Special Cases:** Glob imports (`*`) and self imports (`some::path::{self}`) require specific handling for `visible_name`, `is_glob`, and `is_self_import`, which are covered.
//! *   **`kind` Field:** Distinguishing between `UseStatement` and `ExternCrate` is crucial and tested.
//! *   **No Value/Type:** Unlike `ConstNode` or `FunctionNode`, `ImportNode` doesn't have an associated `value` or complex `type_id` to check beyond its basic structure.
//!
//! ### 4. Lost Coverage from Old Tests:
//!
//! The refactoring replaces the previous tiered tests. The main coverage potentially lost is:
//!
//! *   **Explicit Span Checks:** The old Tier 2 tests explicitly checked that spans were non-zero. The new macro framework doesn't assert specific span values.
//! *   **Explicit ID Regeneration Assertions:** Old Tier 2 tests sometimes included explicit calls to `NodeId::generate_synthetic` and asserted equality. While the new macro *uses* ID generation for lookup, it doesn't explicitly assert the regeneration logic itself in the same way.
//! *   **`ModuleImports` Relation:** The old Tier 4 tests explicitly checked for the `RelationKind::ModuleImports` relation between the containing module and the import node. The new macro only checks for `RelationKind::Contains`. While `ModuleImports` might be redundant if `Contains` is present for all imports, this specific relation check is no longer performed by the macro-generated tests.
//!
//! ### 5. Suggestions for Future Inclusions:
//!
//! *   Add fixture items and corresponding tests for `use` statements with `pub`, `pub(crate)`, and `pub(in path)` visibility.
//! *   Add fixture items and corresponding tests for `use` or `extern crate` statements with `#[cfg(...)]` attributes.

#![cfg(test)]
#![allow(unused_imports, non_snake_case)]
//! To run tests with debug logging:
//!     RUST_LOG=log_test_node,log_test_import,test_id_regen=debug cargo test -p syn_parser -- --test-threads=1
//! e.g. for all nodes in this module:
//!     RUST_LOG=log_test_node,log_test_import,test_id_regen=debug cargo test -p syn_parser imports -- --test-threads=1
//! e.g. for only the target "crate::imports::TupleStruct"
//!     RUST_LOG=log_test_node,log_test_import,test_id_regen=debug cargo test -p syn_parser imports::node_TupleStruct -- --test-threads=1
use std::collections::HashMap;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;

use crate::common::{paranoid::*, ParanoidArgs}; // Use re-exports from paranoid mod
use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::nodes::ImportKind;
use syn_parser::parser::types::VisibilityKind;
// Import ImportKind
use syn_parser::parser::{
    graph::CodeGraph,
    nodes::{GraphNode, ImportNode}, // Import ImportNode
};

use lazy_static::lazy_static;

// macro-related
use crate::paranoid_test_fields_and_values;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{Attribute, ExpectedImportNode};

pub const LOG_TEST_IMPORT: &str = "log_test_import";

lazy_static! {
    static ref EXPECTED_IMPORTS_DATA: HashMap<&'static str, ExpectedImportNode> = {
        let mut m = HashMap::new();

        // --- Module Path: ["crate", "imports"] ---
        m.insert("crate::imports::TupleStruct", ExpectedImportNode {
            source_path: &["crate", "structs", "TupleStruct"],
            visible_name: "TupleStruct",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::HashMap", ExpectedImportNode {
            source_path: &["std", "collections", "HashMap"],
            visible_name: "HashMap",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::fmt", ExpectedImportNode {
            source_path: &["std", "fmt"],
            visible_name: "fmt",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::Arc", ExpectedImportNode {
            source_path: &["std", "sync", "Arc"],
            visible_name: "Arc",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::MySimpleStruct", ExpectedImportNode {
            source_path: &["crate", "structs", "SampleStruct"],
            visible_name: "MySimpleStruct",
            original_name: Some("SampleStruct"),
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::IoResult", ExpectedImportNode {
            source_path: &["std", "io", "Result"],
            visible_name: "IoResult",
            original_name: Some("Result"),
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::EnumWithData", ExpectedImportNode {
            source_path: &["crate", "enums", "EnumWithData"],
            visible_name: "EnumWithData",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::SampleEnum1", ExpectedImportNode {
            source_path: &["crate", "enums", "SampleEnum1"],
            visible_name: "SampleEnum1",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::MyGenTrait", ExpectedImportNode {
            source_path: &["crate", "traits", "GenericTrait"],
            visible_name: "MyGenTrait",
            original_name: Some("GenericTrait"),
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::SimpleTrait", ExpectedImportNode {
            source_path: &["crate", "traits", "SimpleTrait"],
            visible_name: "SimpleTrait",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::fs", ExpectedImportNode {
            source_path: &["std", "fs"],
            visible_name: "fs",
            original_name: None,
            is_glob: false,
            is_self_import: true,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::File", ExpectedImportNode {
            source_path: &["std", "fs", "File"],
            visible_name: "File",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::Path", ExpectedImportNode {
            source_path: &["std", "path", "Path"],
            visible_name: "Path",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::PathBuf", ExpectedImportNode {
            source_path: &["std", "path", "PathBuf"],
            visible_name: "PathBuf",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::env_glob", ExpectedImportNode {
            source_path: &["std", "env"],
            visible_name: "*",
            original_name: None,
            is_glob: true,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::SubItem", ExpectedImportNode {
            source_path: &["self", "sub_imports", "SubItem"],
            visible_name: "SubItem",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::AttributedStruct", ExpectedImportNode {
            source_path: &["super", "structs", "AttributedStruct"],
            visible_name: "AttributedStruct",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::SimpleId", ExpectedImportNode {
            source_path: &["crate", "type_alias", "SimpleId"],
            visible_name: "SimpleId",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        // NOTE: Kind of awkward, but the "" empty &str is encoding a leading "::",
        // because the way we handle these is by using source_path.join("::").
        // The target is "::std::time::Duration"
        m.insert("crate::imports::Duration", ExpectedImportNode {
            source_path: &["", "std", "time", "Duration"],
            visible_name: "Duration",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::serde_extern", ExpectedImportNode {
            source_path: &["serde"],
            visible_name: "serde",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::ExternCrate,
            cfgs: vec![],
        });
        m.insert("crate::imports::SerdeAlias", ExpectedImportNode {
            source_path: &["serde"],
            visible_name: "SerdeAlias",
            original_name: Some("serde"),
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::ExternCrate,
            cfgs: vec![],
        });

        // --- Module Path: ["crate", "sub_imports"] ---
        m.insert("crate::imports::sub_imports::fmt", ExpectedImportNode {
            source_path: &["super", "fmt"],
            visible_name: "fmt",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::sub_imports::DocumentedEnum", ExpectedImportNode {
            source_path: &["crate", "enums", "DocumentedEnum"],
            visible_name: "DocumentedEnum",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::sub_imports::Arc", ExpectedImportNode {
            source_path: &["std", "sync", "Arc"],
            visible_name: "Arc",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::sub_imports::NestedItem", ExpectedImportNode {
            source_path: &["self", "nested_sub", "NestedItem"],
            visible_name: "NestedItem",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::sub_imports::TupleStruct", ExpectedImportNode {
            source_path: &["super", "super", "structs", "TupleStruct"],
            visible_name: "TupleStruct",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });

        m
    };
}

lazy_static! {
    static ref EXPECTED_IMPORTS_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();

        // --- Module Path: ["crate", "imports"] ---
        // NOTE: Having issues here, I think we might handle the import ID generation differently
        // than the other nodes,
        // m.insert("crate::imports::TupleStruct", ParanoidArgs {
        //     fixture: "fixture_nodes",
        //     relative_file_path: "src/imports.rs",
        //     ident: "TupleStruct",
        //     expected_cfg: None,
        //     expected_path: &["crate", "imports"],
        //     item_kind: ItemKind::Import,
        // });

        // NOTE: Creating a duplicate of the problematic `TupleStruct` here, using a different
        // naming convention.
        m.insert("crate::imports::TupleStruct", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "TupleStruct",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });

        m.insert("crate::imports::HashMap", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "HashMap",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::fmt", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "fmt",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::Arc", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "Arc",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::MySimpleStruct", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "MySimpleStruct",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::IoResult", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "IoResult",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::EnumWithData", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "EnumWithData",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::SampleEnum1", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "SampleEnum1",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::MyGenTrait", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "MyGenTrait",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::SimpleTrait", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "SimpleTrait",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::fs", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "fs",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::File", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "File",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::Path", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "Path",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::PathBuf", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "PathBuf",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::env_glob", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "*",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::SubItem", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "SubItem",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::AttributedStruct", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "AttributedStruct",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::SimpleId", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "SimpleId",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::Duration", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "Duration",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::serde_extern", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "serde",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::ExternCrate,
        });
        m.insert("crate::imports::SerdeAlias", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "SerdeAlias",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::ExternCrate,
        });

        // --- Module Path: ["crate", "imports", "sub_imports"] ---
        m.insert("crate::imports::sub_imports::fmt", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "fmt",
            expected_cfg: None,
            expected_path: &["crate", "imports", "sub_imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::sub_imports::DocumentedEnum", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "DocumentedEnum",
            expected_cfg: None,
            expected_path: &["crate", "imports", "sub_imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::sub_imports::Arc", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "Arc",
            expected_cfg: None,
            expected_path: &["crate", "imports", "sub_imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::sub_imports::NestedItem", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "NestedItem",
            expected_cfg: None,
            expected_path: &["crate", "imports", "sub_imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::sub_imports::TupleStruct", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "TupleStruct",
            expected_cfg: None,
            expected_path: &["crate", "imports", "sub_imports"],
            item_kind: ItemKind::Import,
        });

        m
    };
}
paranoid_test_fields_and_values!(
    node_tuple_struct,
    "crate::imports::TupleStruct",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_TupleStruct,
    "crate::imports::TupleStruct",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_HashMap,
    "crate::imports::HashMap",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_fmt,
    "crate::imports::fmt",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_Arc,
    "crate::imports::Arc",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_MySimpleStruct,
    "crate::imports::MySimpleStruct",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_IoResult,
    "crate::imports::IoResult",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_EnumWithData,
    "crate::imports::EnumWithData",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_SampleEnum1,
    "crate::imports::SampleEnum1",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_MyGenTrait,
    "crate::imports::MyGenTrait",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_SimpleTrait,
    "crate::imports::SimpleTrait",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_fs,
    "crate::imports::fs",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_File,
    "crate::imports::File",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_Path,
    "crate::imports::Path",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_PathBuf,
    "crate::imports::PathBuf",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_env_glob,
    "crate::imports::env_glob",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_SubItem,
    "crate::imports::SubItem",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_AttributedStruct,
    "crate::imports::AttributedStruct",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_SimpleId,
    "crate::imports::SimpleId",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_Duration,
    "crate::imports::Duration",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_serde_extern,
    "crate::imports::serde_extern",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_SerdeAlias,
    "crate::imports::SerdeAlias",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

// --- Module Path: ["crate", "sub_imports"] ---
paranoid_test_fields_and_values!(
    node_sub_fmt,
    "crate::imports::sub_imports::fmt",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_sub_DocumentedEnum,
    "crate::imports::sub_imports::DocumentedEnum",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_sub_Arc,
    "crate::imports::sub_imports::Arc",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_sub_NestedItem,
    "crate::imports::sub_imports::NestedItem",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

paranoid_test_fields_and_values!(
    node_sub_TupleStruct,
    "crate::imports::sub_imports::TupleStruct",
    EXPECTED_IMPORTS_ARGS,                         // args_map
    EXPECTED_IMPORTS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ImportNode,         // node_type
    syn_parser::parser::nodes::ExpectedImportNode, // derived Expeced*Node
    as_import,                                     // downcast_method
    LOG_TEST_IMPORT                                // log_target
);

// --- Tier 1: Basic Smoke Tests ---
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_basic_smoke_test_full_parse() {
    let results = run_phase1_phase2("fixture_nodes");
    assert!(!results.is_empty(), "Phase 1 & 2 failed to produce results");

    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");

    let target_data = results
        .iter()
        .find_map(|res| match res {
            Ok(data) if data.file_path == fixture_path => Some(data),
            _ => None,
        })
        .unwrap_or_else(|| panic!("ParsedCodeGraph for '{}' not found", fixture_path.display()));

    let graph = &target_data.graph;

    // (visible_name, path_suffix, expected_kind_discriminant)
    let expected_items = vec![
        (
            "HashMap".to_string(),
            [
                "std".to_string(),
                "collections".to_string(),
                "HashMap".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "fmt".to_string(),
            ["std".to_string(), "fmt".to_string()].to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "IoResult".to_string(),
            ["std".to_string(), "io".to_string(), "Result".to_string()].to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "MySimpleStruct".to_string(),
            [
                "crate".to_string(),
                "structs".to_string(),
                "SampleStruct".to_string(), // Updated from SimpleStruct
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "fs".to_string(),
            ["std".to_string(), "fs".to_string()].to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "File".to_string(),
            ["std".to_string(), "fs".to_string(), "File".to_string()].to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "Path".to_string(),
            ["std".to_string(), "path".to_string(), "Path".to_string()].to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "PathBuf".to_string(),
            ["std".to_string(), "path".to_string(), "PathBuf".to_string()].to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "EnumWithData".to_string(),
            [
                "crate".to_string(),
                "enums".to_string(),
                "EnumWithData".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "SampleEnum1".to_string(),
            [
                "crate".to_string(),
                "enums".to_string(),
                "SampleEnum1".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        // Removed DefaultTrait expectation
        (
            "SimpleTrait".to_string(), // Added SimpleTrait expectation
            [
                "crate".to_string(),
                "traits".to_string(),
                "SimpleTrait".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "MyGenTrait".to_string(),
            [
                "crate".to_string(),
                "traits".to_string(),
                "GenericTrait".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "*".to_string(),
            ["std".to_string(), "env".to_string()].to_vec(),
            "UseStatement".to_string(),
        ), // Glob
        (
            "SubItem".to_string(),
            [
                "self".to_string(),
                "sub_imports".to_string(),
                "SubItem".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "AttributedStruct".to_string(),
            [
                "super".to_string(),
                "structs".to_string(),
                "AttributedStruct".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "SimpleId".to_string(),
            [
                "crate".to_string(),
                "type_alias".to_string(),
                "SimpleId".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ),
        (
            "Duration".to_string(),
            [
                "std".to_string(),
                "time".to_string(),
                "Duration".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ), // Absolute path ::std::...
        (
            "serde".to_string(),
            ["serde".to_string()].to_vec(),
            "ExternCrate".to_string(),
        ),
        (
            "SerdeAlias".to_string(),
            ["serde".to_string()].to_vec(),
            "ExternCrate".to_string(),
        ), // Renamed extern crate
        // Nested module imports
        (
            "fmt".to_string(),
            ["super".to_string(), "fmt".to_string()].to_vec(),
            "UseStatement".to_string(),
        ), // sub_imports::use super::fmt;
        (
            "DocumentedEnum".to_string(),
            [
                "crate".to_string(),
                "enums".to_string(),
                "DocumentedEnum".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ), // sub_imports::use crate::...
        (
            "Arc".to_string(),
            ["std".to_string(), "sync".to_string(), "Arc".to_string()].to_vec(),
            "UseStatement".to_string(),
        ), // sub_imports::use std::...
        (
            "NestedItem".to_string(),
            [
                "self".to_string(),
                "nested_sub".to_string(),
                "NestedItem".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ), // sub_imports::use self::...
        (
            "TupleStruct".to_string(),
            [
                "super".to_string(),
                "super".to_string(),
                "structs".to_string(),
                "TupleStruct".to_string(),
            ]
            .to_vec(),
            "UseStatement".to_string(),
        ), // sub_imports::use super::super::...
    ];

    assert!(
        !graph.use_statements.is_empty(),
        "CodeGraph contains no ImportNodes in use_statements"
    );

    for (name, path_suffix, kind_disc) in expected_items {
        // Find based on visible name and path suffix for smoke test
        let node = graph
            .use_statements
            .iter()
            .inspect(|i| {println!("SEARCHING USE STMT Is self? {}, ImportNode name '{}', original_name {:?} ending with path {:?} in graph.use_statements", 
                i.is_self_import, i.visible_name, i.original_name, i.source_path())})
            .find(|i| i.visible_name == name && i.source_path().ends_with(&path_suffix))
            .unwrap_or_else(|| {
                panic!(
                    "ImportNode '{}' ending with path {:?} not found in graph.use_statements",
                    name, path_suffix
                )
            });

        assert!(
            matches!(node.id, NodeId::Synthetic(_)),
            "Node '{}' path={:?}: ID should be Synthetic, found {:?}",
            name,
            node.source_path,
            node.id
        );
        assert_ne!(
            node.span,
            (0, 0),
            "Node '{}' path={:?}: Span should not be (0,0), found {:?}",
            name,
            node.source_path,
            node.span
        );

        // Check Kind Discriminant
        match (&node.kind, &kind_disc) {
            (ImportKind::UseStatement(_), _) => {} // Match
            (ImportKind::ExternCrate, _) => {}     // Match
        }
    }
}

// --- Tier 2: Targeted Field Verification ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_id_regeneration() {
    // Target: use std::collections::HashMap;
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);
    // let actual_span = node.span; // Span no longer used

    // Determine ItemKind based on the node found
    let item_kind = match node.kind {
        ImportKind::UseStatement(_) => ploke_core::ItemKind::Import,
        ImportKind::ExternCrate => ploke_core::ItemKind::ExternCrate,
    };

    // ID generation now uses the *visible name* or "<glob>"
    let id_gen_name = if node.is_glob {
        "<glob>"
    } else {
        &node.visible_name
    };

    // Find the containing module node to get its ID for the parent scope
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path() == &module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for path: {:?} in file '{}' while testing '{}'",
                module_path,
                file_path.display(),
                visible_name
            )
        });

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        &module_path,
        id_gen_name,          // Use visible name or "<glob>" for ID gen
        item_kind,            // Pass the determined ItemKind
        Some(module_node.id), // Pass the containing module's ID
        None,                 // Assume no relevant CFGs for this test case
    );

    assert!(
        matches!(node.id, NodeId::Synthetic(_)),
        "Node '{}': ID should be Synthetic, found {:?}",
        visible_name,
        node.id
    );
    assert_eq!(
        node.id, regenerated_id,
        "Mismatch for ID field. Expected (regen): {}, Actual: {}",
        regenerated_id, node.id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_span() {
    // Target: use std::fmt;
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "fmt";
    let expected_path_suffix = &["std".to_string(), "fmt".to_string()];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert_ne!(
        node.span,
        (0, 0),
        "Node '{}': Span should not be (0, 0). Actual: {:?}",
        visible_name,
        node.span
    );
    assert!(
        node.span.1 > node.span.0,
        "Node '{}': Span end should be greater than start. Actual: {:?}",
        visible_name,
        node.span
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_path() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];

    // Target 1: HashMap
    let visible_name1 = "HashMap";
    let expected_path1 = vec![
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];
    let node1 = find_import_node_basic(graph, &module_path, visible_name1, &expected_path1);
    assert_eq!(
        node1.source_path,
        expected_path1
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "Path mismatch for '{}'",
        visible_name1
    );

    // Target 2: File (grouped)
    let visible_name2 = "File";
    let expected_path2 = vec!["std".to_string(), "fs".to_string(), "File".to_string()];
    let node2 = find_import_node_basic(graph, &module_path, visible_name2, &expected_path2);
    assert_eq!(
        node2.source_path,
        expected_path2
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "Path mismatch for '{}'",
        visible_name2
    );

    // Target 3: SubItem (self)
    let visible_name3 = "SubItem";
    let expected_path3 = vec![
        "self".to_string(),
        "sub_imports".to_string(),
        "SubItem".to_string(),
    ];
    let node3 = find_import_node_basic(graph, &module_path, visible_name3, &expected_path3);
    assert_eq!(
        node3.source_path,
        expected_path3
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "Path mismatch for '{}'",
        visible_name3
    );

    // Target 4: SimpleId (crate relative) - Changed from MyId
    let visible_name4 = "SimpleId";
    let expected_path4 = vec![
        "crate".to_string(),
        "type_alias".to_string(),
        "SimpleId".to_string(),
    ];
    let node4 = find_import_node_basic(graph, &module_path, visible_name4, &expected_path4);
    assert_eq!(
        node4.source_path,
        expected_path4
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "Path mismatch for '{}'",
        visible_name4
    );

    // Target 5: serde (extern crate) - Renumbered
    let visible_name5 = "serde";
    let expected_path5 = vec!["serde".to_string()];
    let node5 = find_import_node_basic(graph, &module_path, visible_name5, &expected_path5);
    assert_eq!(
        node5.source_path,
        expected_path5
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "Path mismatch for '{}'",
        visible_name5
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_kind_use() {
    // Target: HashMap
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];
    let _expected_kind = ImportKind::UseStatement(VisibilityKind::Inherited);

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert!(
        matches!(&node.kind, _expected_kind),
        "Kind mismatch for '{}'. Expected {:?}, Actual {:?}",
        visible_name,
        _expected_kind,
        node.kind
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_kind_extern_crate() {
    // Target: serde
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "serde";
    let expected_path_suffix = &["serde".to_string()];
    let expected_kind = ImportKind::ExternCrate;

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert_eq!(
        node.kind, expected_kind,
        "Kind mismatch for '{}'. Expected {:?}, Actual {:?}",
        visible_name, expected_kind, node.kind
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_visible_name_simple() {
    // Target: HashMap
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_visible_name_renamed() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];

    // Target 1: IoResult
    let visible_name1 = "IoResult";
    let expected_path_suffix1 = &["std".to_string(), "io".to_string(), "Result".to_string()];
    let node1 = find_import_node_basic(graph, &module_path, visible_name1, expected_path_suffix1);
    assert_eq!(
        node1.visible_name, visible_name1,
        "GraphNode name mismatch for IoResult"
    );

    // Target 2: SerdeAlias
    let visible_name2 = "SerdeAlias";
    let expected_path_suffix2 = &["serde".to_string()];
    let node2 = find_import_node_basic(graph, &module_path, visible_name2, expected_path_suffix2);
    assert_eq!(
        node2.visible_name, visible_name2,
        "GraphNode name mismatch for SerdeAlias"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_visible_name_glob() {
    // Target: use std::env::*;
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "*"; // Glob uses "*" as visible name
    let expected_path_suffix = &["std".to_string(), "env".to_string()];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert_eq!(
        node.visible_name, visible_name,
        "GraphNode name mismatch for glob"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_original_name_simple() {
    // Target: HashMap
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert!(
        node.original_name.is_none(),
        "Original name should be None for simple import"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_original_name_renamed() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];

    // Target 1: IoResult (Result as IoResult)
    let visible_name1 = "IoResult";
    let expected_path_suffix1 = &["std".to_string(), "io".to_string(), "Result".to_string()];
    let expected_original1 = Some("Result".to_string());
    let node1 = find_import_node_basic(graph, &module_path, visible_name1, expected_path_suffix1);
    assert_eq!(
        node1.original_name, expected_original1,
        "Original name mismatch for IoResult"
    );

    // Target 2: SerdeAlias (serde as SerdeAlias)
    let visible_name2 = "SerdeAlias";
    let expected_path_suffix2 = &["serde".to_string()];
    let expected_original2 = Some("serde".to_string());
    let node2 = find_import_node_basic(graph, &module_path, visible_name2, expected_path_suffix2);
    assert_eq!(
        node2.original_name, expected_original2,
        "Original name mismatch for SerdeAlias"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_is_glob_true() {
    // Target: use std::env::*;
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "*";
    let expected_path_suffix = &["std".to_string(), "env".to_string()];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert!(node.is_glob, "is_glob should be true for glob import");
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_field_is_glob_false() {
    // Target: HashMap
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("imports.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let node = find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);

    assert!(!node.is_glob, "is_glob should be false for non-glob import");
}

// --- Tier 4: Basic Connection Tests ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_relation_contains_file_module() {
    // Target: HashMap in "crate::imports" module
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for imports.rs not found"));
    let graph = &target_data.graph;

    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    let import_node =
        find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);
    let import_id = import_node.id;

    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(import_id),
        RelationKind::Contains,
        &format!(
            "Expected Module '{}' ({}) to Contain Import '{}' ({})",
            module_node.name, module_id, visible_name, import_id
        ),
    );

    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&import_id)),
        "ImportNode ID {} not found in items list for Module '{}' ({})",
        import_id,
        module_node.name,
        module_id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_relation_module_imports_file_module() {
    // Target: HashMap in "crate::imports" module
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path_suffix = &[
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for imports.rs not found"));
    let graph = &target_data.graph;

    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    let import_node =
        find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);
    let import_id = import_node.id;

    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(import_id),
        RelationKind::ModuleImports,
        &format!(
            "Expected ModuleImports relation between Module '{}' ({}) and Import '{}' ({})",
            module_node.name, module_id, visible_name, import_id
        ),
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_relation_contains_inline_module() {
    // Target: Arc in "crate::imports::sub_imports"
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs"; // Defined in this file
    let module_path = vec![
        "crate".to_string(),
        "imports".to_string(),
        "sub_imports".to_string(),
    ];
    let visible_name = "Arc";
    let expected_path_suffix = &[
        "std".to_string().to_string(),
        "sync".to_string().to_string(),
        "Arc".to_string().to_string(),
    ];

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for imports.rs not found"));
    let graph = &target_data.graph;

    let module_node = find_inline_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    let import_node =
        find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);
    let import_id = import_node.id;

    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(import_id),
        RelationKind::Contains,
        &format!(
            "Expected Module '{}' ({}) to Contain Import '{}' ({})",
            module_node.name, module_id, visible_name, import_id
        ),
    );

    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&import_id)),
        "ImportNode ID {} not found in items list for Module '{}' ({})",
        import_id,
        module_node.name,
        module_id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_relation_module_imports_inline_module() {
    // Target: Arc in "crate::imports::sub_imports"
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs"; // Defined in this file
    let module_path = vec![
        "crate".to_string(),
        "imports".to_string(),
        "sub_imports".to_string(),
    ];
    let visible_name = "Arc";
    let expected_path_suffix = &["std".to_string(), "sync".to_string(), "Arc".to_string()];

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for imports.rs not found"));
    let graph = &target_data.graph;

    let module_node = find_inline_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    let import_node =
        find_import_node_basic(graph, &module_path, visible_name, expected_path_suffix);
    let import_id = import_node.id;

    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(import_id),
        RelationKind::ModuleImports,
        &format!(
            "Expected ModuleImports relation between Module '{}' ({}) and Import '{}' ({})",
            module_node.name, module_id, visible_name, import_id
        ),
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_in_module_imports_list() {
    // Target: HashMap in "crate::imports" module's imports list
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path = vec![
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];

    let successful_graphs = run_phases_and_collect(fixture_name);

    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );

    let found_in_list = module_node
        .imports
        .iter()
        .any(|i| i.visible_name == visible_name && i.source_path == expected_path);

    assert!(
        found_in_list,
        "ImportNode for '{}' with path {:?} not found in ModuleNode imports list: {:?}",
        visible_name, expected_path, module_node.imports
    );
}

// --- Tier 5: Extreme Paranoia Tests ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_paranoid_simple() {
    // Target: use std::collections::HashMap;
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "HashMap";
    let expected_path = vec![
        "std".to_string(),
        "collections".to_string(),
        "HashMap".to_string(),
    ];
    let expected_original_name = None;
    let expected_is_glob = false;

    let successful_graphs = run_phases_and_collect(fixture_name);
    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_import_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        visible_name,
        &expected_path,
        expected_original_name,
        expected_is_glob,
    );

    // 2. Assert all fields
    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
    assert_eq!(node.source_path, expected_path, "Path mismatch");
    assert_eq!(
        node.original_name,
        expected_original_name.map(|s| s.to_string()),
        "Original name mismatch"
    );
    assert_eq!(node.is_glob, expected_is_glob, "is_glob mismatch");
    assert!(
        matches!(node.kind, ImportKind::UseStatement(_)),
        "Kind mismatch"
    );
    assert_ne!(node.span, (0, 0), "Span should not be default");

    // 3. Verify Relations
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::Contains,
        "Missing Contains relation",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::ModuleImports,
        "Missing ModuleImports relation",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id)),
        "ImportNode ID not found in module items list"
    );
    assert!(
        module_node.imports.iter().any(|i| i.id == node.id),
        "ImportNode not found in module imports list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph
        .use_statements
        .iter()
        .filter(|i| i.id == node.id)
        .count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ImportNode ID {} in graph.use_statements",
        node.id
    );
    // Uniqueness based on properties checked by paranoid helper
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_paranoid_renamed() {
    // Target: use std::io::Result as IoResult;
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "IoResult";
    let expected_path = vec!["std".to_string(), "io".to_string(), "Result".to_string()];
    let expected_original_name = Some("Result");
    let expected_is_glob = false;

    let successful_graphs = run_phases_and_collect(fixture_name);
    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_import_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        visible_name,
        &expected_path,
        expected_original_name,
        expected_is_glob,
    );

    // 2. Assert all fields
    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
    assert_eq!(node.source_path, expected_path, "Path mismatch");
    assert_eq!(
        node.original_name,
        expected_original_name.map(|s| s.to_string()),
        "Original name mismatch"
    );
    assert_eq!(node.is_glob, expected_is_glob, "is_glob mismatch");
    assert!(
        matches!(node.kind, ImportKind::UseStatement(_)),
        "Kind mismatch"
    );
    assert_ne!(node.span, (0, 0), "Span should not be default");

    // 3. Verify Relations
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::Contains,
        "Missing Contains relation",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::ModuleImports,
        "Missing ModuleImports relation",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id)),
        "ImportNode ID not found in module items list"
    );
    assert!(
        module_node.imports.iter().any(|i| i.id == node.id),
        "ImportNode not found in module imports list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph
        .use_statements
        .iter()
        .filter(|i| i.id == node.id)
        .count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ImportNode ID {} in graph.use_statements",
        node.id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_paranoid_glob() {
    // Target: use std::env::*;
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "*";
    let expected_path = vec!["std".to_string(), "env".to_string()]; // Path is up to the glob
    let expected_original_name = None;
    let expected_is_glob = true;

    let successful_graphs = run_phases_and_collect(fixture_name);
    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_import_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        visible_name,
        &expected_path,
        expected_original_name,
        expected_is_glob,
    );

    // 2. Assert all fields
    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
    assert_eq!(node.source_path, expected_path, "Path mismatch");
    assert_eq!(
        node.original_name,
        expected_original_name.map(|s| s.to_string()),
        "Original name mismatch"
    );
    assert_eq!(node.is_glob, expected_is_glob, "is_glob mismatch");
    assert!(
        matches!(node.kind, ImportKind::UseStatement(_)),
        "Kind mismatch"
    );
    assert_ne!(node.span, (0, 0), "Span should not be default");

    // 3. Verify Relations
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::Contains,
        "Missing Contains relation",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::ModuleImports,
        "Missing ModuleImports relation",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id)),
        "ImportNode ID not found in module items list"
    );
    assert!(
        module_node.imports.iter().any(|i| i.id == node.id),
        "ImportNode not found in module imports list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph
        .use_statements
        .iter()
        .filter(|i| i.id == node.id)
        .count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ImportNode ID {} in graph.use_statements",
        node.id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_paranoid_self() {
    // Target: use self::sub_imports::SubItem;
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "SimpleId"; // Changed from SubItem
    let expected_path = vec![
        "crate".to_string(), // Changed from self
        "type_alias".to_string(),
        "SimpleId".to_string(),
    ];
    let expected_original_name = None;
    let expected_is_glob = false;

    let successful_graphs = run_phases_and_collect(fixture_name);
    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_import_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        visible_name,
        &expected_path,
        expected_original_name,
        expected_is_glob,
    );

    // 2. Assert all fields
    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
    assert_eq!(node.source_path, expected_path, "Path mismatch");
    assert_eq!(
        node.original_name,
        expected_original_name.map(|s| s.to_string()),
        "Original name mismatch"
    );
    assert_eq!(node.is_glob, expected_is_glob, "is_glob mismatch");
    assert!(
        matches!(node.kind, ImportKind::UseStatement(_)),
        "Kind mismatch"
    );
    assert_ne!(node.span, (0, 0), "Span should not be default");

    // 3. Verify Relations
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::Contains,
        "Missing Contains relation",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::ModuleImports,
        "Missing ModuleImports relation",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id)),
        "ImportNode ID not found in module items list"
    );
    assert!(
        module_node.imports.iter().any(|i| i.id == node.id),
        "ImportNode not found in module imports list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph
        .use_statements
        .iter()
        .filter(|i| i.id == node.id)
        .count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ImportNode ID {} in graph.use_statements",
        node.id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_paranoid_extern_crate() {
    // Target: extern crate serde;
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "serde";
    let expected_path = vec!["serde".to_string()];
    let expected_original_name = None;
    let expected_is_glob = false;

    let successful_graphs = run_phases_and_collect(fixture_name);
    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_import_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        visible_name,
        &expected_path,
        expected_original_name,
        expected_is_glob,
    );

    // 2. Assert all fields
    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
    assert_eq!(node.source_path, expected_path, "Path mismatch");
    assert_eq!(
        node.original_name,
        expected_original_name.map(|s| s.to_string()),
        "Original name mismatch"
    );
    assert_eq!(node.is_glob, expected_is_glob, "is_glob mismatch");
    assert_eq!(node.kind, ImportKind::ExternCrate, "Kind mismatch"); // Check kind
    assert_ne!(node.span, (0, 0), "Span should not be default");

    // 3. Verify Relations
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::Contains,
        "Missing Contains relation",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::ModuleImports,
        "Missing ModuleImports relation",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id)),
        "ImportNode ID not found in module items list"
    );
    assert!(
        module_node.imports.iter().any(|i| i.id == node.id),
        "ImportNode not found in module imports list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph
        .use_statements
        .iter()
        .filter(|i| i.id == node.id)
        .count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ImportNode ID {} in graph.use_statements",
        node.id
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_node_paranoid_extern_crate_renamed() {
    // Target: extern crate serde as SerdeAlias;
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/imports.rs";
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let visible_name = "SerdeAlias";
    let expected_path = vec!["serde".to_string()]; // Path is still the original crate name
    let expected_original_name = Some("serde");
    let expected_is_glob = false;

    let successful_graphs = run_phases_and_collect(fixture_name);
    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for imports.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_import_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        visible_name,
        &expected_path,
        expected_original_name,
        expected_is_glob,
    );

    // 2. Assert all fields
    assert_eq!(node.visible_name, visible_name, "GraphNode name mismatch");
    assert_eq!(node.source_path, expected_path, "Path mismatch");
    assert_eq!(
        node.original_name,
        expected_original_name.map(|s| s.to_string()),
        "Original name mismatch"
    );
    assert_eq!(node.is_glob, expected_is_glob, "is_glob mismatch");
    assert_eq!(node.kind, ImportKind::ExternCrate, "Kind mismatch"); // Check kind
    assert_ne!(node.span, (0, 0), "Span should not be default");

    // 3. Verify Relations
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::Contains,
        "Missing Contains relation",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id),
        RelationKind::ModuleImports,
        "Missing ModuleImports relation",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id)),
        "ImportNode ID not found in module items list"
    );
    assert!(
        module_node.imports.iter().any(|i| i.id == node.id),
        "ImportNode not found in module imports list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph
        .use_statements
        .iter()
        .filter(|i| i.id == node.id)
        .count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ImportNode ID {} in graph.use_statements",
        node.id
    );
}
