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
#[deprecated(
    since = "0.1.0",
    note = "Use find_item_id_by_path_name_kind_checked instead. Relies on name only and can return ambiguous results."
)]
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
/// * `Err(SynParserError::DuplicateNode)` if multiple items matching all criteria are found (should be rare).
pub fn find_item_id_by_path_name_kind_checked(
    graph: &CodeGraph,
    module_defn_path: &[String],
    item_name: &str,
    item_kind: ploke_core::ItemKind,
) -> Result<NodeId, SynParserError> {
    // 1. Find the containing module definition node rigorously
    let module_node = graph.find_module_by_defn_path_checked(module_defn_path)?;
    let module_id = module_node.id();

    // 2. Collect all potential candidates contained within the module
    let contained_nodes: Vec<&dyn GraphNode> = graph
        .relations
        .iter()
        .filter(|rel| {
            rel.source == GraphId::Node(module_id) && rel.kind == RelationKind::Contains
        })
        .filter_map(|rel| match rel.target {
            GraphId::Node(id) => graph.find_node(id), // Use find_node which is cheaper than checked here
            _ => None,
        })
        .collect();

    // 3. Filter candidates by name and kind
    let mut matches: Vec<NodeId> = Vec::new();
    for node in contained_nodes {
        if node.name() == item_name {
            // Check kind (requires mapping GraphNode back to specific type or having kind() on trait)
            // For now, let's assume we can get the kind. We might need to enhance GraphNode trait later.
            // Placeholder: Assume a way to get ItemKind from &dyn GraphNode
            let current_kind = match node.id() { // Infer kind based on which collection it's in (less robust)
                 _ if graph.get_function(node.id()).is_some() => ItemKind::Function,
                 _ if graph.get_struct(node.id()).is_some() => ItemKind::Struct,
                 _ if graph.get_enum(node.id()).is_some() => ItemKind::Enum,
                 _ if graph.get_union(node.id()).is_some() => ItemKind::Union,
                 _ if graph.get_type_alias(node.id()).is_some() => ItemKind::TypeAlias,
                 _ if graph.get_trait(node.id()).is_some() => ItemKind::Trait,
                 _ if graph.get_impl(node.id()).is_some() => ItemKind::Impl,
                 _ if graph.get_module(node.id()).is_some() => ItemKind::Module,
                 _ if graph.get_value(node.id()).is_some() => ItemKind::Const, // Or Static
                 _ if graph.get_macro(node.id()).is_some() => ItemKind::Macro,
                 _ if graph.get_import(node.id()).is_some() => ItemKind::Import,
                 _ => continue, // Skip if kind cannot be determined this way
            };

            if current_kind == item_kind {
                matches.push(node.id());
            }
        }
    }

    // 4. Check for uniqueness
    match matches.len() {
        0 => Err(SynParserError::NotFound(NodeId::Synthetic(uuid::Uuid::nil()))), // Placeholder ID
        1 => Ok(matches[0]),
        _ => Err(SynParserError::DuplicateNode(matches[0])), // Report first duplicate ID
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
    module_path: &[String],
    visible_name: &str,
) -> Result<NodeId, SynParserError> {
    // Find the module where the re-export is declared (use checked version)
    let module_node = graph.find_module_by_path_checked(module_path)?;
    let module_id = module_node.id();

    // Find all ImportNodes contained within this module
    let contained_imports: Vec<&ImportNode> = graph
        .relations
        .iter()
        .filter(|rel| {
            rel.source == GraphId::Node(module_id) && rel.kind == RelationKind::Contains
        })
        .filter_map(|rel| match rel.target {
            GraphId::Node(id) => graph.get_import(id), // Find ImportNode specifically
            _ => None,
        })
        .collect();

    // Filter these imports for re-exports matching the visible name
    let mut matches: Vec<&ImportNode> = contained_imports
        .into_iter()
        .filter(|imp| imp.visible_name == visible_name && imp.is_reexport())
        .collect();

    // Check for uniqueness
    match matches.len() {
        0 => Err(SynParserError::NotFound(NodeId::Synthetic(uuid::Uuid::nil()))), // Placeholder ID
        1 => Ok(matches[0].id),
        _ => Err(SynParserError::DuplicateNode(matches[0].id)), // Report first duplicate ID
    }
}
