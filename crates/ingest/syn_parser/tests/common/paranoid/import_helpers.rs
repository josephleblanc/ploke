#![cfg(feature = "uuid_ids")]

use ploke_common::fixtures_crates_dir;
use ploke_core::NodeId;
use syn_parser::parser::{
    nodes::{ImportNode, ModuleNode, Visible}, // Added ImportNode
    visitor::ParsedCodeGraph,
};

/// Finds the specific ParsedCodeGraph for the target file, then finds the ImportNode
/// within that graph corresponding to the given module path and import details,
/// performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness or ID checks fail.
pub fn find_import_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/imports.rs"
    expected_module_path: &[String],      // Module path where the import is defined
    // --- Parameters to uniquely identify the import ---
    visible_name: &str,                   // Name used in scope (e.g., "HashMap", "IoResult", "*")
    expected_path: &[String],             // Expected path segments stored in ImportNode.path
    expected_original_name: Option<&str>, // Expected original name if renamed
    expected_is_glob: bool,               // Expected glob status
) -> &'a ImportNode {
    // 1. Construct the absolute expected file path
    let fixture_root = fixtures_crates_dir().join(fixture_name);
    let target_file_path = fixture_root.join(relative_file_path);

    // 2. Find the specific ParsedCodeGraph for the target file
    let target_data = parsed_graphs
        .iter()
        .find(|data| data.file_path == target_file_path)
        .unwrap_or_else(|| {
            panic!(
                "ParsedCodeGraph for '{}' not found in results",
                target_file_path.display()
            )
        });

    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path; // Use the path from the found graph data

    // 3. Find the ModuleNode for the expected_module_path within the target graph
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.defn_path() == expected_module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for definition path: {:?} in file '{}'",
                expected_module_path,
                file_path.display()
            )
        });

    // 4. Filter candidates by ALL identifying properties within the target graph's use_statements
    let candidates: Vec<&ImportNode> = graph
        .use_statements // Search the global list first
        .iter()
        .filter(|i| {
            i.visible_name == visible_name
                && i.path == expected_path
                && i.original_name.as_deref() == expected_original_name
                && i.is_glob == expected_is_glob
        })
        .collect();

    assert!(
        !candidates.is_empty(),
        "No ImportNode found matching criteria: visible_name='{}', path={:?}, original_name={:?}, is_glob={} in file '{}'",
        visible_name, expected_path, expected_original_name, expected_is_glob, file_path.display()
    );

    // 5. Filter further by module association using the ModuleNode's items list
    let module_items = module_node.items().unwrap_or_else(|| {
        panic!(
            "ModuleNode {:?} ({}) in file '{}' does not have items (neither Inline nor FileBased?)",
            module_node.path,
            module_node.name,
            file_path.display()
        )
    });

    let module_candidates: Vec<&ImportNode> = candidates
        .into_iter()
        .filter(|i| module_items.contains(&i.id)) // Check ID against module items
        .collect();

    // 6. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one ImportNode matching criteria associated with module path {:?} in file '{}', found {}. Candidates: {:?}",
        expected_module_path,
        file_path.display(),
        module_candidates.len(),
        module_candidates // Print candidates for debugging
    );

    let import_node = module_candidates[0];
    let import_id = import_node.id;
    let actual_span = import_node.span; // Get span from the found node

    // 7. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    //    The ID is generated based on the visible_name (or "*" for glob) and span.
    let id_gen_name = if import_node.is_glob { "*" } else { &import_node.visible_name };
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path, // Use the module's definition path for context
        id_gen_name, // Use visible_name or "*" for ID generation
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        import_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for import '{}' (path: {:?}) in module {:?} file '{}' with span {:?}",
        import_id, regenerated_id, visible_name, expected_path, expected_module_path, file_path.display(), actual_span
    );

    // 8. Return the validated node
    import_node
}
