#![cfg(not(feature = "type_bearing_ids"))]
//! Helper functions for module testing
//!
//! These helper functions are intentionally overkill and brittle.

use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::visitor::calculate_cfg_hash_bytes;
use syn_parser::parser::{nodes::*, ParsedCodeGraph};

/// Finds the specific ParsedCodeGraph for the target file, then finds the ModuleNode
/// representing a *declaration* (`mod name;`) within that graph, performs paranoid checks,
/// and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_declaration_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/main.rs"
    expected_module_path: &[String],      // The full path of the module being declared
) -> &'a ModuleNode {
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

    // 3. Filter candidates by path and ModuleKind::Declaration within the target graph
    let candidates: Vec<&ModuleNode> = graph
        .modules
        .iter()
        .filter(|m| m.path == expected_module_path && m.is_decl())
        .collect();

    // 4. PARANOID CHECK: Assert exactly ONE candidate remains
    assert_eq!(
        candidates.len(),
        1,
        "Expected exactly one ModuleNode declaration with path {:?} in file '{}', found {}",
        expected_module_path,
        file_path.display(),
        candidates.len()
    );

    let module_node = candidates[0];
    let module_id = module_node.id();
    let module_name = module_node.name();
    let item_cfgs = module_node.cfgs(); // Get the declaration's own CFGs
                                        //     panic!(
                                        //         "ModuleNode {:?} ({}) is Declaration but has no declaration_span",
                                        //         module_node.path, module_node.name
                                        //     )
                                        // });

    // 5. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    // The parent path for a declaration is the path of the module it's declared *in*.
    let parent_path_vec: Vec<String> = expected_module_path
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect();
    // Find the parent module node to get its ID
    let parent_mod = graph
        .modules
        .iter()
        .find(|m| m.path == parent_path_vec)
        .unwrap_or_else(|| {
            panic!(
                "Parent ModuleNode not found for path: {:?} in file '{}'",
                parent_path_vec, // Use parent_path_vec here
                file_path.display()
            )
        });
    let parent_mod_id = parent_mod.id();
    let scope_cfgs = parent_mod.cfgs(); // Get parent's CFGs

    // Combine, sort, and hash CFGs
    let mut provisional_effective_cfgs: Vec<String> = scope_cfgs
        .iter()
        .cloned()
        .chain(item_cfgs.iter().cloned())
        .collect();
    provisional_effective_cfgs.sort_unstable();
    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,        // Use the file_path from the target_data
        &parent_path_vec, // Use parent_path_vec here
        module_name,
        ItemKind::Module,     // Pass the correct ItemKind
        Some(parent_mod_id),  // Pass the PARENT module's ID as parent scope
        cfg_bytes.as_deref(), // Pass calculated CFG bytes
    );

    assert_eq!(
        module_id, regenerated_id,
        "Mismatch between declaration node's actual ID ({}) and regenerated ID ({}) for module path {:?} (name: '{}') in file '{}'.\nParentScope: {:?}\nScope CFGs: {:?}\nItem CFGs: {:?}\nCombined CFGs: {:?}",
        module_id, regenerated_id, expected_module_path, module_name, file_path.display(), Some(parent_mod_id), scope_cfgs, item_cfgs, provisional_effective_cfgs
    );

    // 6. Return the validated node
    module_node
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the ModuleNode
/// representing a *file-based definition* (`src/name.rs` or `src/name/mod.rs`) within that graph,
/// performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_file_module_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/my_mod.rs" or "src/my_mod/mod.rs"
    expected_module_path: &[String],      // The full path of the module being defined
) -> &'a ModuleNode {
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

    // 3. Filter candidates by path and ModuleKind::FileBased within the target graph
    //    In Phase 2, the file-level module node *is* the root node of that file's graph.
    let candidates: Vec<&ModuleNode> = graph
        .modules
        .iter()
        .filter(|m| m.path == expected_module_path && m.is_file_based())
        .collect();

    // 4. PARANOID CHECK: Assert exactly ONE candidate remains
    assert_eq!(
        candidates.len(),
        1,
        "Expected exactly one file-based ModuleNode definition with path {:?} in file '{}', found {}",
        expected_module_path,
        file_path.display(),
        candidates.len()
    );

    let module_node = candidates[0];
    let module_id = module_node.id();
    let module_name = module_node.name();
    let item_cfgs = module_node.cfgs(); // File-level module's CFGs are its own

    // 5. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    // The parent path for a file-based module definition is its logical parent path.
    let parent_path_vec: Vec<String> = expected_module_path // Use parent_path_vec consistently
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect();

    // For a file-level module node, the ID is generated by the visitor using `parent_scope_id = None`.
    // The CFG context comes solely from the file-level attributes (#![cfg(...)]) stored in item_cfgs.
    let parent_mod_id = None;
    let scope_cfgs: &[String] = &[]; // No parent scope CFGs to inherit

    // Calculate hash bytes using only the item's (file-level) CFGs
    let mut provisional_effective_cfgs = item_cfgs.to_vec(); // Clone item_cfgs
    provisional_effective_cfgs.sort_unstable(); // Sort for hashing
    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,        // Use the file_path from the target_data
        &parent_path_vec, // Still use the parent path for context hashing
        module_name,
        ItemKind::Module,     // Pass the correct ItemKind
        parent_mod_id,        // Explicitly pass None, mirroring visitor logic
        cfg_bytes.as_deref(), // Pass calculated CFG bytes
    );

    assert_eq!(
        module_id, regenerated_id,
        "Mismatch between file module node's actual ID ({}) and regenerated ID ({}) for module path {:?} (name: '{}') in file '{}'.\nParentScope: {:?}\nScope CFGs: {:?}\nItem CFGs: {:?}\nCombined CFGs: {:?}",
        module_id, regenerated_id, expected_module_path, module_name, file_path.display(), parent_mod_id, scope_cfgs, item_cfgs, provisional_effective_cfgs
    );

    // 6. Return the validated node
    module_node
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the ModuleNode
/// representing an *inline definition* (`mod name { ... }`) within that graph,
/// performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_inline_module_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/parent.rs"
    expected_module_path: &[String],      // The full path of the inline module being defined
) -> &'a ModuleNode {
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

    // 3. Filter candidates by path and ModuleKind::Inline within the target graph
    let candidates: Vec<&ModuleNode> = graph
        .modules
        .iter()
        .filter(|m| m.path == expected_module_path && m.is_inline())
        .collect();

    // 4. PARANOID CHECK: Assert exactly ONE candidate remains
    assert_eq!(
        candidates.len(),
        1,
        "Expected exactly one inline ModuleNode definition with path {:?} in file '{}', found {}",
        expected_module_path,
        file_path.display(),
        candidates.len()
    );

    let module_node = candidates[0];
    let module_id = module_node.id();
    let module_name = module_node.name();
    let item_cfgs = module_node.cfgs(); // Get the inline module's own CFGs
                                        //     panic!(
                                        //         "ModuleNode {:?} ({}) is Inline but has no inline_span",
                                        //         module_node.path, module_node.name
                                        //     )
                                        // });

    // 5. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    // The parent path for an inline module definition is its logical parent path.
    let parent_path_vec: Vec<String> = expected_module_path
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect();
    // Find the parent module node to get its ID
    let parent_mod = graph // Define parent_mod here
        .modules
        .iter()
        .find(|m| m.path == parent_path_vec) // Use parent_path_vec here
        .unwrap_or_else(|| {
            panic!(
                "Parent ModuleNode not found for path: {:?} in file '{}'",
                parent_path_vec, // Use parent_path_vec here
                file_path.display()
            )
        });
    let parent_mod_id = parent_mod.id();
    let scope_cfgs = parent_mod.cfgs(); // Get parent's CFGs

    // Combine, sort, and hash CFGs
    let mut provisional_effective_cfgs: Vec<String> = scope_cfgs
        .iter()
        .cloned()
        .chain(item_cfgs.iter().cloned())
        .collect();
    provisional_effective_cfgs.sort_unstable();
    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,        // Use the file_path from the target_data
        &parent_path_vec, // Use parent_path_vec here
        module_name,
        ItemKind::Module,     // Pass the correct ItemKind
        Some(parent_mod_id),  // Pass the PARENT module's ID as parent scope
        cfg_bytes.as_deref(), // Pass calculated CFG bytes
    );

    assert_eq!(
        module_id, regenerated_id,
        "Mismatch between inline module node's actual ID ({}) and regenerated ID ({}) for module path {:?} (name: '{}') in file '{}'.\nParentScope: {:?}\nScope CFGs: {:?}\nItem CFGs: {:?}\nCombined CFGs: {:?}",
        module_id, regenerated_id, expected_module_path, module_name, file_path.display(), Some(parent_mod_id), scope_cfgs, item_cfgs, provisional_effective_cfgs
    );

    // 6. Return the validated node
    module_node
}
