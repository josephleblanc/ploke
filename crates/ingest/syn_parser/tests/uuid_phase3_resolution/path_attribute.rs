//! Tests focusing on the parsing and handling of the `#[path]` attribute on modules.

use crate::common::uuid_ids_utils::run_phases_and_collect;
use colored::*; // Import colored for terminal colors
use log::debug; // Import the debug macro
use ploke_common::fixtures_crates_dir;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::nodes::ModuleDef;

const LOG_TARGET_GRAPH_FIND: &str = "graph_find"; // Define log target for this file

#[test]
fn test_path_attribute_handling() {
    // Initialize logger without timestamps (ignore errors if already initialized)
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    debug!(target: LOG_TARGET_GRAPH_FIND, "{}", "Starting test_path_attribute_handling".green());
    let fixture_name = "fixture_path_resolution";
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();
    for parsed_graph in results {
        graphs.push(parsed_graph.graph);
    }
    let merged_graph = CodeGraph::merge_new(graphs).expect("Failed to merge graphs");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Merged graph contains {} modules.", merged_graph.modules.len());
    for module in &merged_graph.modules {
        debug!(target: LOG_TARGET_GRAPH_FIND,
            "  - Module: {} ({}), Path: {:?}, Defn Path: {:?}, IsDecl: {}, IsFile: {}, FilePath: {:?}",
            module.name.yellow(),
            module.id.to_string().magenta(),
            module.path,
            module.defn_path(),
            module.is_declaration(),
            module.is_file_based(),
            module.file_path().map(|p| p.display())
        );
    }

    let module_tree = merged_graph.build_module_tree();
    debug!(target: LOG_TARGET_GRAPH_FIND, "Module tree built successfully: {:?}", module_tree.is_ok());


    // --- Test `logical_path_mod` (#[path = "renamed_path/actual_file.rs"]) ---

    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for declaration of 'logical_path_mod'...");
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
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found declaration: {} ({})", logical_mod_decl.name.yellow(), logical_mod_decl.id.to_string().magenta());

    // Assert the declaration node *does* store the #[path] attribute
    let has_path_attr = logical_mod_decl.attributes.iter().any(|a| a.name == "path");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Declaration has #[path] attribute: {}", has_path_attr);
    assert!(
        has_path_attr,
        "Declaration node for logical_path_mod should store the #[path] attribute."
    );


    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for file-based definition of 'logical_path_mod'...");
    // Find the module node *defined* by the file specified in #[path]
    let logical_mod_defn = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "logical_path_mod" // Name comes from the declaration
                && m.path == ["crate", "logical_path_mod"] // Path also comes from the declaration
                && m.is_file_based() // Should be file-based because of #[path]
        })
        .expect("Could not find file-based definition for logical_path_mod"); // <--- THIS IS LIKELY WHERE IT PANICS
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found definition: {} ({})", logical_mod_defn.name.yellow(), logical_mod_defn.id.to_string().magenta());


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

    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for declaration of 'common_import_mod'...");
    let common_mod_decl = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "common_import_mod"
                && m.path == ["crate", "common_import_mod"] // Path of the declaration
                && m.is_declaration()
        })
        .expect("Could not find declaration for common_import_mod");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found declaration: {} ({})", common_mod_decl.name.yellow(), common_mod_decl.id.to_string().magenta());

    let has_path_attr_common = common_mod_decl.attributes.iter().any(|a| a.name == "path");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Common declaration has #[path] attribute: {}", has_path_attr_common);
    assert!(
        !has_path_attr_common,
        "Declaration node for common_import_mod should not store the #[path] attribute itself."
    );


    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for file-based definition of 'common_import_mod'...");
    let common_mod_defn = merged_graph
        .modules
        .iter()
        .find(|m| {
            m.name == "common_import_mod" // Name from declaration
                && m.path == ["crate", "common_import_mod"] // Path from declaration
                && m.is_file_based() // Should be file-based
        })
        .expect("Could not find file-based definition for common_import_mod");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found definition: {} ({})", common_mod_defn.name.yellow(), common_mod_defn.id.to_string().magenta());


    match &common_mod_defn.module_def {
        ModuleDef::FileBased { file_path, .. } => {
            // Construct the expected absolute path relative to the workspace
            let expected_path = fixtures_crates_dir().join("common_file.rs");
            assert_eq!(
                file_path,
                &expected_path,
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
