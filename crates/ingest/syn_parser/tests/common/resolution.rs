//! Helper functions specifically for testing resolution logic (Phase 3).

use ploke_core::NodeId;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::{
    error::SynParserError,
    parser::{
        graph::CodeGraph,
        nodes::{GraphId, GraphNode},
        relations::RelationKind, // Removed TypeDefNode
    },
};

use super::uuid_ids_utils::run_phases_and_collect;

pub fn build_tree_for_tests(fixture_name: &str) -> (ParsedCodeGraph, ModuleTree) {
    let results = run_phases_and_collect(fixture_name);
    let merged_graph = ParsedCodeGraph::merge_new(results).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree() // dirty, placeholder
        .expect("Failed to build module tree for edge cases fixture");
    (merged_graph, tree)
}

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
// #[deprecated(
//     since = "0.1.0",
//     note = "Use find_item_id_by_path_name_kind_checked instead. Relies on name only and can return ambiguous results."
// )] // Annoying warnings
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
        0 => Err(SynParserError::NotFound(NodeId::Synthetic(
            uuid::Uuid::nil(),
        ))), // Placeholder ID for NotFound
        1 => Ok(contained_candidates[0].id()),
        _ => {
            // If duplicates found, report the ID of the first one found
            Err(SynParserError::DuplicateNode(contained_candidates[0].id()))
        }
    }
}

/// Finds the NodeId of an item by its path, name, and kind, ensuring uniqueness.
///
/// This is the preferred helper for locating specific items in tests, as it uses
/// multiple criteria to avoid ambiguity.
///
/// # Arguments
/// * `graph` - The CodeGraph to search within.
/// * `module_defn_path` - The definition path of the containing module.
/// * `item_name` - The simple name of the item.
/// * `item_kind` - The `ploke_core::ItemKind` of the item.
///
/// # Returns
/// * `Ok(NodeId)` if exactly one matching item is found.
/// * `Err(SynParserError::ModulePathNotFound)` if the module path is not found.
/// * `Err(SynParserError::NotFound)` if no item matching all criteria is found.
/// * `Err(SynParserError::DuplicateNode)` if multiple items matching the criteria are found.
pub fn find_item_id_by_path_name_kind_checked(
    graph: &ParsedCodeGraph,
    module_defn_path: &[&str],
    item_name: &str,
    item_kind: ploke_core::ItemKind,
) -> Result<NodeId, SynParserError> {
    // 1. Find the containing module definition node rigorously
    let m_path_string = module_defn_path
        .iter()
        .map(|seg| seg.to_string())
        .collect::<Vec<_>>();
    let module_node = graph.find_module_by_defn_path_checked(&m_path_string)?;
    let module_id = module_node.id();

    // 2. Get IDs of all nodes contained within the module
    let contained_ids: Vec<NodeId> = graph
        .relations()
        .iter()
        .filter(|rel| rel.source == GraphId::Node(module_id) && rel.kind == RelationKind::Contains)
        .filter_map(|rel| match rel.target {
            GraphId::Node(id) => Some(id),
            _ => None,
        })
        .collect();

    // 3. Iterate through contained IDs, check name and kind, collect matches
    let mut matches: Vec<NodeId> = Vec::new();
    let mut errors: Vec<SynParserError> = Vec::new(); // Collect errors encountered

    for contained_id in contained_ids {
        // Use find_node_checked to ensure the contained node itself exists uniquely
        match graph.find_node_checked(contained_id) {
            Ok(node) => {
                if node.name() == item_name && node.kind_matches(item_kind) {
                    // Name matches, now check if the kind matches the *target* kind
                    matches.push(contained_id);
                    // If name matches but kind doesn't, we just ignore it for this specific search.
                }
            }
            Err(e @ SynParserError::DuplicateNode(_)) => {
                // If find_node_checked finds a duplicate ID, this is a critical graph error.
                log::error!(
                    "Graph inconsistency: Duplicate NodeId {} found during lookup.",
                    contained_id
                );
                return Err(e); // Propagate critical error immediately
            }
            Err(e @ SynParserError::NotFound(_)) => {
                // Should not happen if ID came from relations, but log if it does.
                log::warn!(
                    "Graph inconsistency: Contained NodeId {} not found.",
                    contained_id
                );
                errors.push(e); // Collect non-critical error
            }
            Err(e) => {
                // Collect other potential errors from find_node_checked
                errors.push(e);
            }
        }
    }

    // 4. Check results after iterating through all contained items
    if !errors.is_empty() {
        // Log collected non-critical errors if any matches were also found or if no matches found
        if !matches.is_empty() || matches.is_empty() {
            log::warn!(
                "Encountered errors while searching for item '{}' ({:?}) in module {:?}: {:?}",
                item_name,
                item_kind,
                module_defn_path,
                errors
            );
        }
        // If only errors occurred and no matches, return the first error encountered
        if matches.is_empty() {
            for e in &errors {
                log::error!("{:?}", e);
            }
            return Err(errors.first().unwrap().to_owned());
        }
    }

    match matches.len() {
        0 => Err(SynParserError::NotFound(NodeId::Synthetic(
            uuid::Uuid::nil(),
        ))), // Use placeholder for specific error
        1 => Ok(matches[0]),
        _ => {
            log::error!(
                "Duplicate items found for name '{}' ({:?}) in module {:?}: {:?}",
                item_name,
                item_kind,
                module_defn_path,
                matches
            );
            Err(SynParserError::DuplicateNode(matches[0])) // Report first duplicate ID
        }
    }
}

/// Finds the NodeId of an ImportNode representing a re-export based on its visible name
/// within a specific module, ensuring uniqueness.
///
/// # Arguments
/// * `graph` - The CodeGraph to search within.
/// * `module_path` - The *logical* path of the module containing the re-export declaration.
/// * `visible_name` - The name under which the re-exported item is visible.
///
/// # Returns
/// * `Ok(NodeId)` if exactly one matching re-export `ImportNode` is found.
/// * `Err(SynParserError::ModulePathNotFound)` if the module path is not found.
/// * `Err(SynParserError::NotFound)` if no re-export with that visible name is found in the module.
/// * `Err(SynParserError::DuplicateNode)` if multiple re-exports with the same visible name exist in the module.
pub fn find_reexport_import_node_by_name_checked(
    graph: &CodeGraph,
    module_path: &[String], // Logical path of the module containing the re-export
    visible_name: &str,
) -> Result<NodeId, SynParserError> {
    // 1. Find the containing module node rigorously using the logical path
    //    We use find_module_by_path_checked because re-exports are associated with the logical module structure.
    let module_node = graph.find_module_by_path_checked(module_path)?;
    let module_id = module_node.id();

    // 2. Get IDs of all nodes contained within the module
    let contained_ids: Vec<NodeId> = graph
        .relations
        .iter()
        .filter(|rel| rel.source == GraphId::Node(module_id) && rel.kind == RelationKind::Contains)
        .filter_map(|rel| match rel.target {
            GraphId::Node(id) => Some(id),
            _ => None,
        })
        .collect();

    // 3. Iterate through contained IDs, check if it's a matching re-export ImportNode
    let mut matches: Vec<NodeId> = Vec::new();
    let mut errors: Vec<SynParserError> = Vec::new(); // Collect errors

    for contained_id in contained_ids {
        // Use get_import_checked to ensure it's a unique ImportNode
        match graph.get_import_checked(contained_id) {
            Ok(import_node) => {
                // Check if it's a re-export and the visible name matches
                if import_node.is_local_reexport() && import_node.visible_name == visible_name {
                    matches.push(contained_id);
                }
                // Ignore if it's not a re-export or name doesn't match
            }
            Err(_e @ SynParserError::NotFound(_)) => {
                // This contained ID is not an ImportNode, ignore it for this search.
            }
            Err(e @ SynParserError::DuplicateNode(_)) => {
                // Critical graph error: Duplicate ID found for an ImportNode
                log::error!(
                    "Graph inconsistency: Duplicate NodeId {} found for ImportNode.",
                    contained_id
                );
                return Err(e); // Propagate critical error
            }
            Err(e) => {
                // Collect other potential errors from get_import_checked
                errors.push(e);
            }
        }
    }

    // 4. Check results after iterating
    if !errors.is_empty() {
        if !matches.is_empty() || matches.is_empty() {
            log::warn!(
                "Encountered errors while searching for re-export '{}' in module {:?}: {:?}",
                visible_name,
                module_path,
                errors
            );
        }
        if matches.is_empty() {
            return Err(errors.remove(0));
        }
    }

    match matches.len() {
        0 => Err(SynParserError::NotFound(NodeId::Synthetic(
            uuid::Uuid::nil(),
        ))), // Placeholder ID
        1 => Ok(matches[0]),
        _ => {
            log::error!(
                "Duplicate re-exports found for visible name '{}' in module {:?}: {:?}",
                visible_name,
                module_path,
                matches
            );
            Err(SynParserError::DuplicateNode(matches[0])) // Report first duplicate ID
        }
    }
}
