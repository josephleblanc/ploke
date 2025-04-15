use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::nodes::*;
use syn_parser::parser::visitor::ParsedCodeGraph;

/// Finds the specific ParsedCodeGraph for the target file, then finds the StructNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_struct_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/structs.rs"
    expected_module_path: &[String],      // Module path within the target file
    struct_name: &str,
) -> &'a StructNode {
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

    // 3. Filter candidates by name and type within the target graph
    let name_candidates: Vec<&StructNode> = graph
        .defined_types
        .iter()
        .filter_map(|td| match td {
            TypeDefNode::Struct(s) if s.name() == struct_name => Some(s),
            _ => None,
        })
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No StructNode found with name '{}' in file '{}'",
        struct_name,
        file_path.display()
    );

    // 4. Filter further by module association within the target graph
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path == expected_module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for path: {:?} in file '{}'",
                expected_module_path,
                file_path.display()
            )
        });

    let module_candidates: Vec<&StructNode> = name_candidates
        .into_iter()
        .filter(|s| module_node.items().is_some_and(|m| m.contains(&s.id())))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one StructNode named '{}' associated with module path {:?} in file '{}', found {}",
        struct_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let struct_node = module_candidates[0];
    let struct_id = struct_node.id();
    // let actual_span = struct_node.span; // Span no longer used for ID generation

    // 6. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        struct_name,
        ItemKind::Struct,     // Pass the correct ItemKind
        Some(module_node.id), // Pass the containing module's ID as parent scope
    );

    assert_eq!(
        struct_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for struct '{}' in file '{}' (ItemKind: {:?}, ParentScope: {:?})",
        struct_id, regenerated_id, struct_name, file_path.display(), ItemKind::Struct, Some(module_node.id)
    );

    // 7. Return the validated node
    struct_node
}
