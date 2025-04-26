use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::{
    nodes::*,
    visitor::calculate_cfg_hash_bytes,
    ParsedCodeGraph, // Import calculate_cfg_hash_bytes
};

/// Finds the specific ParsedCodeGraph for the target file, then finds the TraitNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_trait_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/traits.rs"
    expected_module_path: &[String],      // Module path within the target file
    trait_name: &str,
) -> &'a TraitNode {
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

    // 3. Filter candidates by name within the target graph
    // Traits are stored directly in graph.traits (assuming public/crate) or graph.private_traits
    let name_candidates: Vec<&TraitNode> = graph
        .traits // Check public/crate traits first
        .iter()
        .filter(|t| t.name() == trait_name)
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No TraitNode found with name '{}' in file '{}'",
        trait_name,
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

    let module_candidates: Vec<&TraitNode> = name_candidates
        .into_iter()
        .filter(|t| module_node.items().is_some_and(|m| m.contains(&t.id())))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one TraitNode named '{}' associated with module path {:?} in file '{}', found {}",
        trait_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let trait_node = module_candidates[0];
    let trait_id = trait_node.id();
    let item_cfgs = trait_node.cfgs(); // Get the trait's own CFGs

    // 6. PARANOID CHECK: Regenerate expected ID using node's context, ItemKind, and CFGs
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
        file_path, // Use the file_path from the target_data
        expected_module_path,
        trait_name,
        ItemKind::Trait,      // Pass the correct ItemKind
        Some(module_node.id), // Pass the containing module's ID as parent scope
        cfg_bytes.as_deref(), // Pass calculated CFG bytes
    );

    assert_eq!(
        trait_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for trait '{}' in module {:?} file '{}'.\nItemKind: {:?}\nParentScope: {:?}\nScope CFGs: {:?}\nItem CFGs: {:?}\nCombined CFGs: {:?}",
        trait_id, regenerated_id, trait_name, expected_module_path, file_path.display(), ItemKind::Trait, Some(module_node.id), scope_cfgs, item_cfgs, provisional_effective_cfgs
    );

    // 7. Return the validated node
    trait_node
}
