//! Helper functions for module testing
//!
//! These helper functions are intentionally overkill and brittle.

use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::nodes::*;
use syn_parser::parser::visitor::ParsedCodeGraph;

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

    // 3. Filter candidates by path and ModuleDef::Declaration within the target graph
    let candidates: Vec<&ModuleNode> = graph
        .modules
        .iter()
        .filter(|m| m.path == expected_module_path && m.is_declaration())
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
    // Span is no longer used for ID generation
    // let actual_span = module_node.declaration_span().unwrap_or_else(|| {
    //     panic!(
    //         "ModuleNode {:?} ({}) is Declaration but has no declaration_span",
    //         module_node.path, module_node.name
    //     )
    // });

    // 5. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    // The parent path for a declaration is the path of the module it's declared *in*.
    let parent_path: Vec<String> = expected_module_path
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect();

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        &parent_path,
        module_name,
        ItemKind::Module, // Pass the correct ItemKind
        None,             // Pass None for parent_scope_id (temporary)
    );

    assert_eq!(
        module_id, regenerated_id,
        "Mismatch between declaration node's actual ID ({}) and regenerated ID ({}) for module path {:?} (name: '{}') in file '{}' (ItemKind: {:?}, ParentScope: None)",
        module_id, regenerated_id, expected_module_path, module_name, file_path.display(), ItemKind::Module
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

    // 3. Filter candidates by path and ModuleDef::FileBased within the target graph
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
    // Span is no longer used for ID generation
    // let actual_span = module_node.span;

    // 5. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    // The parent path for a file-based module definition is its logical parent path.
    let parent_path: Vec<String> = expected_module_path
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect();

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        &parent_path,
        module_name,
        ItemKind::Module, // Pass the correct ItemKind
        None,             // Pass None for parent_scope_id (temporary)
    );

    assert_eq!(
        module_id, regenerated_id,
        "Mismatch between file module node's actual ID ({}) and regenerated ID ({}) for module path {:?} (name: '{}') in file '{}' (ItemKind: {:?}, ParentScope: None)",
        module_id, regenerated_id, expected_module_path, module_name, file_path.display(), ItemKind::Module
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

    // 3. Filter candidates by path and ModuleDef::Inline within the target graph
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
    // Span is no longer used for ID generation
    // let actual_span = module_node.inline_span().unwrap_or_else(|| {
    //     panic!(
    //         "ModuleNode {:?} ({}) is Inline but has no inline_span",
    //         module_node.path, module_node.name
    //     )
    // });

    // 5. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    // The parent path for an inline module definition is its logical parent path.
    let parent_path: Vec<String> = expected_module_path
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect();

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        &parent_path,
        module_name,
        ItemKind::Module, // Pass the correct ItemKind
        None,             // Pass None for parent_scope_id (temporary)
    );

    assert_eq!(
        module_id, regenerated_id,
        "Mismatch between inline module node's actual ID ({}) and regenerated ID ({}) for module path {:?} (name: '{}') in file '{}' (ItemKind: {:?}, ParentScope: None)",
        module_id, regenerated_id, expected_module_path, module_name, file_path.display(), ItemKind::Module
    );

    // 6. Return the validated node
    module_node
}
