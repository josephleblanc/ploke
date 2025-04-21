//! Tests focusing on the parsing and handling of the `#[path]` attribute on modules.

use crate::common::uuid_ids_utils::run_phases_and_collect;
use ploke_common::fixtures_crates_dir;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::nodes::ModuleDef;

#[test]
fn test_path_attribute_handling() {
    let fixture_name = "fixture_path_resolution";
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();
    for parsed_graph in results {
        graphs.push(parsed_graph.graph);
    }
    let merged_graph = CodeGraph::merge_new(graphs).expect("Failed to merge graphs");

    // --- Test `logical_path_mod` (#[path = "renamed_path/actual_file.rs"]) ---

    // Find the module node declared as `logical_path_mod` in lib.rs
    let logical_mod_decl = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "logical_path_mod"
                && m.path == ["crate", "logical_path_mod"] // Path of the declaration
                && m.is_declaration()
        })
        .expect("Could not find declaration for logical_path_mod");

    // Assert the declaration node *does* store the #[path] attribute
    assert!(
        logical_mod_decl
            .attributes
            .iter()
            .any(|a| a.name == "path"), // Expect 'path' attribute to be present
        "Declaration node for logical_path_mod should store the #[path] attribute."
    );

    // Find the module node *defined* by the file specified in #[path]
    let logical_mod_defn = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "logical_path_mod" // Name comes from the declaration
                && m.path == ["crate", "logical_path_mod"] // Path also comes from the declaration
                && m.is_file_based() // Should be file-based because of #[path]
        })
        .expect("Could not find file-based definition for logical_path_mod");

    // Assert the definition node points to the correct file
    match &logical_mod_defn.module_def {
        ModuleDef::FileBased { file_path, .. } => {
            let expected_suffix = "fixture_path_resolution/src/renamed_path/actual_file.rs";
            assert!(
                file_path.ends_with(expected_suffix),
                "File path for logical_path_mod definition should end with '{}', but was '{}'",
                expected_suffix,
                file_path.display()
            );
        }
        _ => panic!(
            "Expected logical_path_mod definition to be FileBased, but was {:?}",
            logical_mod_defn.module_def
        ),
    }

    // --- Test `common_import_mod` (#[path = "../common_file.rs"]) ---

    let common_mod_decl = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "common_import_mod"
                && m.path == ["crate", "common_import_mod"] // Path of the declaration
                && m.is_declaration()
        })
        .expect("Could not find declaration for common_import_mod");

    assert!(
        !common_mod_decl
            .attributes
            .iter()
            .any(|a| a.name == "path"),
        "Declaration node for common_import_mod should not store the #[path] attribute itself."
    );

    let common_mod_defn = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "common_import_mod" // Name from declaration
                && m.path == ["crate", "common_import_mod"] // Path from declaration
                && m.is_file_based() // Should be file-based
        })
        .expect("Could not find file-based definition for common_import_mod");

    match &common_mod_defn.module_def {
        ModuleDef::FileBased { file_path, .. } => {
            // Construct the expected absolute path relative to the workspace
            let expected_path = fixtures_crates_dir().join("common_file.rs");
            assert_eq!(
                file_path, &expected_path,
                "File path for common_import_mod definition should be '{}', but was '{}'",
                expected_path.display(),
                file_path.display()
            );
        }
        _ => panic!(
            "Expected common_import_mod definition to be FileBased, but was {:?}",
            common_mod_defn.module_def
        ),
    }
}
