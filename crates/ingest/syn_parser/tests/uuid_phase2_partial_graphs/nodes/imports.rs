//! Tests for `ImportNode` parsing and field extraction.
//!
//! ## Test Coverage Analysis
//!
//! *   **Fixture:** `tests/fixture_crates/fixture_nodes/src/imports.rs`
//! *   **Tests:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/imports.rs` (using `paranoid_test_fields_and_values!`)
//!
//! ### 1. Coverage of Fixture Items:
//!
//! The `EXPECTED_IMPORTS_DATA` and `EXPECTED_IMPORTS_ARGS` maps cover every import item identified in the fixture (`use` statements, `pub use`, and `extern crate` declarations), including those at the crate root and within the nested `sub_imports` module. This includes:
//! *   Simple path imports (`std::collections::HashMap`, `std::fmt`, `crate::structs::TupleStruct`).
//! *   Renamed imports (`as IoResult`, `as MySimpleStruct`, `as MyGenTrait`, `as SerdeAlias`).
//! *   Grouped imports (`use crate::{...}`, `use std::{...}`).
//! *   Glob import (`std::env::*;`).
//! *   Relative path imports (`self::`, `super::`, `crate::`, `super::super::`).
//! *   Absolute path import (`::std::time::Duration`).
//! *   `extern crate` (simple and renamed).
//! *   Enum variants (`use crate::enums::SampleEnum1::Variant1 as EnumVariant1`).
//! *   Constants/statics (`TOP_LEVEL_BOOL`, `TOP_LEVEL_COUNTER`), unions (`IntOrFloat`), and macros (`documented_macro`).
//! *   Module aliasing via `pub use crate::traits as TraitsMod;`.
//! *   Imports within nested modules.
//!
//! **Conclusion for Fixture Coverage:** Excellent. All import statements from the fixture are covered by the `paranoid_test_fields_and_values!` tests, including the new unit-struct, enum-variant, const/static, union, macro, and module-alias cases.
//!
//! ### 2. Coverage of `ImportNode` Property Variations:
//!
//! Based on the currently covered items:
//!
//! *   `id: ImportNodeId`: Implicitly covered by ID generation and lookup.
//! *   `span: (usize, usize)`: Not directly asserted by value in the new tests (previously checked for non-zero in Tier 2).
//! *   `source_path: Vec<String>`: Excellent coverage (various lengths, `std`, `crate`, `self`, `super`, `::` prefix via empty string).
//! *   `kind: ImportKind`: Excellent coverage (`UseStatement(VisibilityKind::Inherited)` and `ExternCrate`). The fixture now also includes `UseStatement(VisibilityKind::Public)` via the module alias.
//! *   `visible_name: String`: Excellent coverage (simple names, renamed names, `*` for glob).
//! *   `original_name: Option<String>`: Excellent coverage (`None` for direct imports, `Some` for renamed imports).
//! *   `is_glob: bool`: Excellent coverage (both `true` and `false`).
//! *   `is_self_import: bool`: Excellent coverage (both `true` for `std::fs::{self, File}` -> `fs` and `false` for others).
//! *   `cfgs: Vec<String>`: Poor coverage (only items without `cfg` attributes are tested).
//!
//! **Conclusion for Property Variation Coverage:** Most fields have excellent coverage.
//! *   **Areas for potential expansion:**
//!     *   `cfgs`: Add fixture imports with `#[cfg(...)]` attributes.
//!     *   `kind`: Add fixture imports with additional restricted visibility (e.g., `pub(crate) use`, `pub(in path)`) to exercise more `UseStatement` variants.
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
//!
//! To run tests with debug logging:
//!     RUST_LOG=log_test_node,log_test_import,test_id_regen=debug cargo test -p syn_parser -- --test-threads=1
//!
//! e.g. for all nodes in this module:
//!     RUST_LOG=log_test_node,log_test_import,test_id_regen=debug cargo test -p syn_parser imports -- --test-threads=1
//! e.g. for only the target "crate::imports::TupleStruct"
//!     RUST_LOG=log_test_node,log_test_import,test_id_regen=debug cargo test -p syn_parser imports::node_TupleStruct -- --test-threads=1
use std::collections::HashMap;

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
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
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
        m.insert("crate::imports::UnitStruct", ExpectedImportNode {
            source_path: &["crate", "structs", "UnitStruct"],
            visible_name: "UnitStruct",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::EnumVariant1", ExpectedImportNode {
            source_path: &["crate", "enums", "SampleEnum1", "Variant1"],
            visible_name: "EnumVariant1",
            original_name: Some("Variant1"),
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::TOP_LEVEL_BOOL", ExpectedImportNode {
            source_path: &["crate", "const_static", "TOP_LEVEL_BOOL"],
            visible_name: "TOP_LEVEL_BOOL",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::TOP_LEVEL_COUNTER", ExpectedImportNode {
            source_path: &["crate", "const_static", "TOP_LEVEL_COUNTER"],
            visible_name: "TOP_LEVEL_COUNTER",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::IntOrFloat", ExpectedImportNode {
            source_path: &["crate", "unions", "IntOrFloat"],
            visible_name: "IntOrFloat",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::documented_macro", ExpectedImportNode {
            source_path: &["crate", "macros", "documented_macro"],
            visible_name: "documented_macro",
            original_name: None,
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Inherited),
            cfgs: vec![],
        });
        m.insert("crate::imports::TraitsMod", ExpectedImportNode {
            source_path: &["crate", "traits"],
            visible_name: "TraitsMod",
            original_name: Some("traits"),
            is_glob: false,
            is_self_import: false,
            kind: ImportKind::UseStatement(VisibilityKind::Public),
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
            visible_name: "std::env::*",
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
        m.insert("crate::imports::UnitStruct", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "UnitStruct",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::EnumVariant1", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "EnumVariant1",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::TOP_LEVEL_BOOL", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "TOP_LEVEL_BOOL",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::TOP_LEVEL_COUNTER", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "TOP_LEVEL_COUNTER",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::IntOrFloat", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "IntOrFloat",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::documented_macro", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "documented_macro",
            expected_cfg: None,
            expected_path: &["crate", "imports"],
            item_kind: ItemKind::Import,
        });
        m.insert("crate::imports::TraitsMod", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "TraitsMod",
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
        // NOTE: Changed env_gob after modifying how globs are handled, since now they use their
        // full paths as their names, so not strictly the `ident` in the `syn` sense
        m.insert("crate::imports::env_glob", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/imports.rs",
            ident: "std::env::*",
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
    node_UnitStruct,
    "crate::imports::UnitStruct",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
);

paranoid_test_fields_and_values!(
    node_EnumVariant1,
    "crate::imports::EnumVariant1",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
);

paranoid_test_fields_and_values!(
    node_TOP_LEVEL_BOOL,
    "crate::imports::TOP_LEVEL_BOOL",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
);

paranoid_test_fields_and_values!(
    node_TOP_LEVEL_COUNTER,
    "crate::imports::TOP_LEVEL_COUNTER",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
);

paranoid_test_fields_and_values!(
    node_IntOrFloat,
    "crate::imports::IntOrFloat",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
);

paranoid_test_fields_and_values!(
    node_documented_macro,
    "crate::imports::documented_macro",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
);

paranoid_test_fields_and_values!(
    node_TraitsMod,
    "crate::imports::TraitsMod",
    EXPECTED_IMPORTS_ARGS,
    EXPECTED_IMPORTS_DATA,
    syn_parser::parser::nodes::ImportNode,
    syn_parser::parser::nodes::ExpectedImportNode,
    as_import,
    LOG_TEST_IMPORT
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
