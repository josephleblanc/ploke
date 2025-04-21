//! Helper functions specifically for testing resolution logic (Phase 3).

use ploke_core::NodeId;
use syn_parser::{
    error::SynParserError,
    parser::{
        graph::CodeGraph,
        nodes::GraphNode, // Removed TypeDefNode
    },
};

/// Finds the NodeId of an item (function, struct, enum, trait, macro, etc.)
/// by its simple name within a specific module's definition path.
///
/// This function searches across various node types within the graph and ensures
/// the found item is directly contained within the specified module definition.
/// It uses checked methods and returns detailed errors.
///
/// # Arguments
/// * `graph` - The CodeGraph to search within.
/// * `module_defn_path` - The definition path of the containing module (e.g., `["crate", "local_mod"]`).
/// * `item_name` - The simple name of the item to find (e.g., `"local_func"`).
///
/// # Returns
/// * `Ok(NodeId)` if exactly one matching item is found within the specified module.
/// * `Err(SynParserError::ModulePathNotFound)` if the module path itself is not found.
/// * `Err(SynParserError::NotFound)` if no item with that name is found in the module.
/// * `Err(SynParserError::DuplicateNode)` if multiple items with the same name exist in the module.
pub fn find_item_id_in_module_by_name(
    graph: &CodeGraph,
    module_defn_path: &[String],
    item_name: &str,
) -> Result<NodeId, SynParserError> {
    // 1. Find the containing module definition node rigorously
    let module_node = graph.find_module_by_defn_path_checked(module_defn_path)?;
    let module_id = module_node.id();

    // 2. Collect all potential candidates by name across different node types
    let mut candidates: Vec<&dyn GraphNode> = Vec::new();

    // Search functions
    candidates.extend(
        graph
            .functions
            .iter()
            .filter(|n| n.name() == item_name)
            .map(|n| n as &dyn GraphNode),
    );
    // Search defined types
    candidates.extend(graph.defined_types.iter().filter_map(|n| {
        if n.name() == item_name {
            Some(n as &dyn GraphNode)
        } else {
            None
        }
    }));
    // Search traits
    candidates.extend(
        graph
            .traits
            .iter()
            .filter(|n| n.name() == item_name)
            .map(|n| n as &dyn GraphNode),
    );
    // Search macros
    candidates.extend(
        graph
            .macros
            .iter()
            .filter(|n| n.name() == item_name)
            .map(|n| n as &dyn GraphNode),
    );
    // Search values (const/static)
    candidates.extend(
        graph
            .values
            .iter()
            .filter(|n| n.name() == item_name)
            .map(|n| n as &dyn GraphNode),
    );
    // Search modules (for re-exported modules)
    candidates.extend(
        graph
            .modules
            .iter()
            .filter(|n| n.name() == item_name && !n.is_declaration()) // Exclude declarations here
            .map(|n| n as &dyn GraphNode),
    );

    // 3. Filter candidates to only those contained within the target module
    let contained_candidates: Vec<&dyn GraphNode> = candidates
        .into_iter()
        .filter(|node| graph.module_contains_node(module_id, node.id()))
        .collect();

    // 4. Check for uniqueness
    match contained_candidates.len() {
        0 => Err(SynParserError::NotFound(NodeId::Synthetic(uuid::Uuid::nil()))), // Placeholder ID for NotFound
        1 => Ok(contained_candidates[0].id()),
        _ => {
            // If duplicates found, report the ID of the first one found
            Err(SynParserError::DuplicateNode(
                contained_candidates[0].id(),
            ))
        }
    }
}
