//! Tests focusing on the parsing and handling of the `#[path]` attribute on modules.

use crate::common::uuid_ids_utils::run_phases_and_collect;
use colored::*; // Import colored for terminal colors
use log::debug;
use syn_parser::parser::nodes::ModuleDef;
use syn_parser::parser::ParsedCodeGraph;

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

    let merged_graph = ParsedCodeGraph::merge_new(results).expect("Failed to merge graphs");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Merged graph contains {} modules.", merged_graph.graph.modules.len());
    for module in &merged_graph.graph.modules {
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

    let module_tree_result = merged_graph.build_module_tree();
    debug!(target: LOG_TARGET_GRAPH_FIND, "Module tree built successfully: {:?}", module_tree_result.is_ok());
    let module_tree = module_tree_result.expect("Module tree build failed unexpectedly");

    // --- Test `logical_path_mod` (#[path = "renamed_path/actual_file.rs"]) ---

    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for declaration of 'logical_path_mod'...");
    // 1a. Find the module node declared as `logical_path_mod` in lib.rs
    let logical_mod_decl = merged_graph
        .graph
        .modules
        .iter()
        .find(|m| {
            m.name == "logical_path_mod"
                && m.path == ["crate", "logical_path_mod"] // Path of the declaration
                && m.is_declaration()
        })
        .expect("Could not find declaration for logical_path_mod");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found declaration: {} ({})", logical_mod_decl.name.yellow(), logical_mod_decl.id.to_string().magenta());

    // 1b. Assert the declaration node *does* store the #[path] attribute
    let has_path_attr = logical_mod_decl.attributes.iter().any(|a| a.name == "path");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Declaration has #[path] attribute: {}", has_path_attr);
    assert!(
        has_path_attr,
        "Declaration node for logical_path_mod should store the #[path] attribute."
    );

    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for definition node 'actual_file'...");
    // 2a. Find the module node *defined* by the file `renamed_path/actual_file.rs`
    let actual_file_defn = merged_graph
        .graph
        .modules
        .iter()
        .find(|m| {
            // Find based on its own name and path derived from file location
            m.name == "actual_file"
                && m.path == ["crate", "renamed_path", "actual_file"]
                && m.is_file_based()
        })
        .expect("Could not find definition node for actual_file.rs");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found definition: {} ({})", actual_file_defn.name.yellow(), actual_file_defn.id.to_string().magenta());

    // 2b. Assert the definition node points to the correct file
    match &actual_file_defn.module_def {
        ModuleDef::FileBased { file_path, .. } => {
            let expected_suffix = "fixture_path_resolution/src/renamed_path/actual_file.rs";
            assert!(
                file_path.ends_with(expected_suffix),
                "File path for actual_file definition should end with '{}', but was '{}'",
                expected_suffix,
                file_path.display()
            );
        }
        _ => panic!(
            "Expected actual_file definition to be FileBased, but was {:?}",
            actual_file_defn.module_def
        ),
    }

    // 3. Assert the CustomPath relation exists in the module tree
    debug!(target: LOG_TARGET_GRAPH_FIND, "Checking for CustomPath relation...");
    let custom_path_relation_exists = module_tree.tree_relations().iter().any(|tr| {
        let rel = tr.relation();
        rel.kind == syn_parser::parser::relations::RelationKind::CustomPath
            && rel.source == syn_parser::parser::nodes::GraphId::Node(logical_mod_decl.id)
            && rel.target == syn_parser::parser::nodes::GraphId::Node(actual_file_defn.id)
    });
    assert!(
        custom_path_relation_exists,
        "ModuleTree should contain a CustomPath relation from logical_path_mod ({}) to actual_file ({})",
        logical_mod_decl.id,
        actual_file_defn.id
    );
    debug!(target: LOG_TARGET_GRAPH_FIND, "CustomPath relation found: {}", custom_path_relation_exists);

    // --- Test `common_import_mod` (#[path = "../../common_file.rs"]) ---

    debug!(target: LOG_TARGET_GRAPH_FIND, "Searching for declaration of 'common_import_mod'...");
    // 1a. Find the declaration node
    let common_mod_decl = merged_graph
        .graph
        .modules
        .iter()
        .find(|m| {
            m.name == "common_import_mod"
                && m.path == ["crate", "common_import_mod"] // Path of the declaration
                && m.is_declaration()
        })
        .expect("Could not find declaration for common_import_mod");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Found declaration: {} ({})", common_mod_decl.name.yellow(), common_mod_decl.id.to_string().magenta());

    // 1b. Assert the declaration node *does* store the #[path] attribute
    let has_path_attr_common = common_mod_decl.attributes.iter().any(|a| a.name == "path");
    debug!(target: LOG_TARGET_GRAPH_FIND, "Common declaration has #[path] attribute: {}", has_path_attr_common);
    assert!(
        has_path_attr_common,
        "Declaration node for common_import_mod should store the #[path] attribute."
    );

    // 2. Assert that *NO* CustomPath relation exists for common_import_mod
    //    (because the target file is external and wasn't found/linked)
    debug!(target: LOG_TARGET_GRAPH_FIND, "Checking for absence of CustomPath relation for common_import_mod...");
    let common_custom_path_relation_exists = module_tree.tree_relations().iter().any(|tr| {
        let rel = tr.relation();
        rel.kind == syn_parser::parser::relations::RelationKind::CustomPath
            && rel.source == syn_parser::parser::nodes::GraphId::Node(common_mod_decl.id)
    });
    assert!(
        !common_custom_path_relation_exists,
        "ModuleTree should NOT contain a CustomPath relation for common_import_mod ({}) because the target is external",
        common_mod_decl.id
    );
    debug!(target: LOG_TARGET_GRAPH_FIND, "Absence of CustomPath relation confirmed: {}", !common_custom_path_relation_exists);

    // Removed the previous assertions that expected a merged definition node for common_import_mod
}
