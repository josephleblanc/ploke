use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::{
    nodes::{ValueKind, ValueNode, Visible}, // Added ValueNode, Visible
    visitor::{calculate_cfg_hash_bytes, ParsedCodeGraph}, // Import calculate_cfg_hash_bytes
};

/// Finds the specific ParsedCodeGraph for the target file, then finds the ValueNode
/// (const or static) within that graph corresponding to the given module path and name,
/// performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness or ID checks fail.
pub fn find_value_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/const_static.rs"
    expected_module_path: &[String], // Module path within the target file (e.g., ["crate"] or ["crate", "inner_mod"])
    value_name: &str,                // Name of the const/static item
) -> &'a ValueNode {
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
    //    Use defn_path() to correctly match file-based or inline modules.
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

    // 4. Filter candidates by name within the target graph's values
    let name_candidates: Vec<&ValueNode> = graph
        .values // Assuming ValueNodes are stored in graph.values
        .iter()
        .filter(|v| v.name() == value_name)
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No ValueNode found with name '{}' in file '{}'",
        value_name,
        file_path.display()
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

    let module_candidates: Vec<&ValueNode> = name_candidates
        .into_iter()
        .filter(|v| module_items.contains(&v.id()))
        .collect();

    // 6. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one ValueNode named '{}' associated with module path {:?} in file '{}', found {}. Candidates: {:?}",
        value_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len(),
        module_candidates // Print candidates for debugging
    );

    let value_node = module_candidates[0];
    let value_id = value_node.id();
    let item_cfgs = value_node.cfgs(); // Get the value's own CFGs

    // Determine ItemKind based on ValueNodeKind
    let item_kind = match value_node.kind {
        ValueKind::Constant => ItemKind::Const,
        ValueKind::Static { .. } => ItemKind::Static,
    };

    // 7. PARANOID CHECK: Regenerate expected ID using node's context, ItemKind, and CFGs
    // Calculate expected CFG hash bytes
    let scope_cfgs = module_node.cfgs(); // Get parent module's CFGs
    let mut provisional_effective_cfgs: Vec<String> = scope_cfgs
        .iter()
        .cloned()
        .chain(item_cfgs.iter().cloned())
        .collect();
    provisional_effective_cfgs.sort_unstable();
    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,            // Use the file_path from the target_data
        expected_module_path, // Use the module's definition path for context
        value_name,
        item_kind,            // Pass the determined ItemKind
        Some(module_node.id), // Pass the containing module's ID as parent scope
        cfg_bytes.as_deref(), // Pass calculated CFG bytes
    );

    assert_eq!(
        value_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for value '{}' in module {:?} file '{}'.\nItemKind: {:?}\nParentScope: {:?}\nScope CFGs: {:?}\nItem CFGs: {:?}\nCombined CFGs: {:?}",
        value_id, regenerated_id, value_name, expected_module_path, file_path.display(), item_kind, Some(module_node.id), scope_cfgs, item_cfgs, provisional_effective_cfgs
    );

    // 8. Return the validated node
    value_node
}
