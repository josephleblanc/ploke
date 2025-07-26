// #![cfg(not(feature = "type_bearing_ids"))]
//! Helper functions specifically for testing resolution logic (Phase 3).

use itertools::Itertools;
use ploke_core::ItemKind;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{AnyNodeId, AsAnyNodeId, ImportNodeId, PrimaryNodeId};
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::{
    error::SynParserError,
    parser::{graph::CodeGraph, nodes::GraphNode},
};

use super::{run_phases_and_collect, try_run_phases_and_collect};

pub fn try_build_tree_for_tests(
    fixture_name: &str,
) -> Result<(ParsedCodeGraph, ModuleTree), ploke_error::Error> {
    let results = try_run_phases_and_collect(fixture_name)?;
    let merged_graph = ParsedCodeGraph::merge_new(results)?;
    let tree = merged_graph.build_module_tree()?; // dirty, placeholder
    Ok((merged_graph, tree))
}

pub fn build_tree_for_tests(fixture_name: &str) -> (ParsedCodeGraph, ModuleTree) {
    let results = run_phases_and_collect(fixture_name);
    let merged_graph = ParsedCodeGraph::merge_new(results).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree() // dirty, placeholder
        .expect("Failed to build module tree for fixture");
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
/// * `module_path` - The definition path of the containing module (e.g., `["crate", "local_mod"]`).
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
    module_path: &[String],
    item_name: &str,
) -> Result<AnyNodeId, SynParserError> {
    // 1. Find the containing module definition node rigorously
    let module_node = graph.find_module_by_path_checked(module_path)?;
    let module_id = module_node.id;

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
            .statics
            .iter()
            .filter(|n| n.name() == item_name)
            .map(|n| n as &dyn GraphNode),
    );
    candidates.extend(
        graph
            .statics
            .iter()
            .filter(|n| n.name() == item_name)
            .map(|n| n as &dyn GraphNode),
    );
    // Search modules (for re-exported modules)
    candidates.extend(
        graph
            .modules
            .iter()
            .filter(|n| n.name() == item_name && !n.is_decl()) // Exclude declarations here
            .map(|n| n as &dyn GraphNode),
    );

    // 3. Filter candidates to only those contained within the target module
    let contained_candidates: Vec<&dyn GraphNode> = candidates
        .into_iter()
        .filter_map(|n| {
            PrimaryNodeId::try_from(n.any_id())
                .map(|n_id| (n, n_id))
                .ok()
        })
        .filter(|(_n, node_id)| graph.module_contains_node(module_id, *node_id))
        .map(|(n, _node_id)| n)
        .collect();

    // 4. Check for uniqueness
    match contained_candidates.len() {
        0 => Err(SynParserError::NotFoundInModuleByName(
            item_name.to_string(),
            module_path.to_vec(),
        )),
        1 => Ok(contained_candidates[0].any_id()),
        _ => {
            // If duplicates found, report the ID of the first one found
            Err(SynParserError::DuplicateNode(
                contained_candidates[0].any_id(),
            ))
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
/// * `module_path` - The definition path of the containing module.
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
    module_path: &[&str],
    item_name: &str,
    item_kind: ploke_core::ItemKind,
) -> Result<AnyNodeId, SynParserError> {
    // 1. Find the containing module definition node rigorously
    let m_path_string = module_path
        .iter()
        .map(|seg| seg.to_string())
        .collect::<Vec<_>>();
    let module_node = graph.find_module_by_path_checked(&m_path_string)?;
    let module_id = module_node.id;

    // 2. Get IDs of all nodes contained within the module
    let contained_ids: Vec<PrimaryNodeId> = graph
        .relations()
        .iter()
        // .filter(|rel| rel.is_contains() && rel.source() == module_id.as_any())
        .filter_map(|rel| rel.contains_target(module_id))
        .collect();

    // 3. Iterate through contained IDs, check name and kind, collect matches
    let mut matches: Vec<AnyNodeId> = Vec::new();
    let mut errors: Vec<SynParserError> = Vec::new(); // Collect errors encountered

    for contained_id in contained_ids {
        // Use find_node_checked to ensure the contained node itself exists uniquely
        match graph.find_any_node_checked(contained_id.as_any()) {
            Ok(node) => {
                if node.name() == item_name && node.kind_matches(item_kind) {
                    // Name matches, now check if the kind matches the *target* kind
                    matches.push(contained_id.as_any());
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
                module_path,
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
        0 => Err(SynParserError::NotFoundInModuleByNameKind(
            item_name.to_string(),
            module_path.iter().join("::"),
            item_kind,
        )), // Use placeholder for specific error
        1 => Ok(matches[0]),
        _ => {
            log::error!(
                "Duplicate items found for name '{}' ({:?}) in module {:?}: {:?}",
                item_name,
                item_kind,
                module_path,
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
) -> Result<ImportNodeId, SynParserError> {
    let mut import_ids = graph
        .iter_pid_pathkind(module_path, ItemKind::Import)
        .map(ImportNodeId::try_from);
    // .filter_ok();
    let import_id = import_ids.next().expect("ImportNode not found")?;

    let mut matches = Vec::new();
    let mut errors = Vec::new();
    match graph.get_import_checked(import_id) {
        Ok(import_node) => {
            // Check if it's a re-export and the visible name matches
            if import_node.is_any_reexport() && import_node.visible_name == visible_name {
                matches.push(import_id);
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
                import_id
            );
            return Err(e); // Propagate critical error
        }
        Err(e) => {
            // Collect other potential errors from get_import_checked
            errors.push(e);
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
        0 => Err(SynParserError::ReexportNotFound(
            visible_name.to_string(),
            module_path.to_vec(),
            import_id,
        )),
        1 => Ok(matches[0]),
        _ => {
            log::error!(
                "Duplicate re-exports found for visible name '{}' in module {:?}: {:?}",
                visible_name,
                module_path,
                matches
            );
            Err(SynParserError::DuplicateNode(matches[0].as_any())) // Report first duplicate ID
        }
    }
}
