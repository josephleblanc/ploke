use std::collections::{HashSet, VecDeque};

use log::debug;

use crate::{
    error::SynParserError,
    parser::{
        graph::{GraphAccess, GraphNode},
        nodes::{AnyNodeId, AsAnyNodeId, ImportNodeId, ModuleNodeId, NodePath, PrimaryNodeId},
        relations::SyntacticRelation,
        types::VisibilityKind,
        ParsedCodeGraph,
    },
    utils::{AccLogCtx, LogDataStructure, LogStyle, LOG_TARGET_MOD_TREE_BUILD},
};

use super::*;

/// Calculates the shortest public path from the crate root to a given item.
///
/// This function performs a Breadth-First Search (BFS) starting from the item's
/// containing module and exploring upwards towards the crate root (`self.root`).
/// It considers both module containment (`Contains` relation) and public re-exports
/// (`ReExports` relation via `ImportNode`s with public visibility).
///
/// # Arguments
/// * `item_any_id`: The `AnyNodeId` of the item whose public path is required.
/// * `graph`: Access to the `ParsedCodeGraph` for node lookups and dependency info.
///
/// # Returns
/// * `Ok(ResolvedItemInfo)`: Contains the shortest public path, the public name,
///   the resolved ID (definition or re-export), and target kind information.
/// * `Err(ModuleTreeError)`: If the item is not found, not publicly accessible,
///   or if inconsistencies are detected in the graph/tree structure.
// TODO: Refactor with more restrictive type parameters.
// This function will only work for primary node types as is. That is good. We can have a
// separate function that can use this as a helper after we have found the containing primary
// node, and use that. If necessary we can compose the two into a third function that will work
// for all node types.
pub(super) fn shortest_public_path(
    tree: ModuleTree,
    item_pid: PrimaryNodeId, // Changed: Input is AnyNodeId
    graph: &ParsedCodeGraph,
) -> Result<ResolvedItemInfo, ModuleTreeError> {
    // --- 1. Initial Setup ---

    let item_any_id = item_pid.as_any();
    // Use map_err for SynParserError -> ModuleTreeError conversion
    let item_node = graph
        .find_node_unique(item_any_id)
        .map_err(ModuleTreeError::from)?;
    if !item_node.visibility().is_pub() {
        // If the item's own visibility isn't Public, it can never be reached.
        tree.log_spp_item_not_public(item_node);
        return Err(ModuleTreeError::ItemNotPubliclyAccessible(item_any_id));
        // Use AnyNodeId in error
    }
    let item_name = item_node.name().to_string();

    tree.log_spp_start(item_node);

    // Handle special case: asking for the path to the root module ittree
    if let Some(module_node) = item_node.as_module() {
        if module_node.id == tree.root {
            return Ok(ResolvedItemInfo {
                path: NodePath::new_unchecked(vec!["crate".to_string()]),
                public_name: "crate".to_string(),
                resolved_id: item_any_id,
                target_kind: ResolvedTargetKind::InternalDefinition {
                    definition_id: item_any_id,
                },
                definition_name: None,
            });
        }
    }

    // Find the direct parent module ID using the index with AnyNodeId
    let initial_parent_mod_id = tree
        .get_iter_relations_to(&item_any_id) // Use AnyNodeId for lookup
        .ok_or_else(|| ModuleTreeError::no_relations_found(item_node))?
        .find_map(|tr| match tr.rel() {
            // Find the first 'Contains' relation targeting the item_any_id
            SyntacticRelation::Contains { source, target } if *target == item_pid => {
                Some(*source) // Source is ModuleNodeId
            }
            _ => None,
        })
        // If no 'Contains' relation found, return ContainingModuleNotFound error
        .ok_or(ModuleTreeError::ContainingModuleNotFound(item_any_id))?;

    let mut queue: VecDeque<(ModuleNodeId, Vec<String>)> = VecDeque::new();
    let mut visited: HashSet<ModuleNodeId> = HashSet::new();

    // Enqueue the *parent* module. Path starts with the item's name.
    queue.push_back((initial_parent_mod_id, vec![item_name]));
    visited.insert(initial_parent_mod_id);

    // --- 2. BFS Loop ---
    while let Some((current_mod_id, path_to_item)) = queue.pop_front() {
        // --- 3. Check for Goal ---
        tree.log_spp_check_root(current_mod_id, &path_to_item);
        if current_mod_id == tree.root {
            // Reached the crate root! Construct the final path.
            tree.log_spp_found_root(current_mod_id, &path_to_item);
            let mut final_path = vec!["crate".to_string()];
            // The path_to_item is currently [item_name, mod_name, parent_mod_name, ...]
            // We need to reverse it and prepend "crate".
            final_path.extend(path_to_item.into_iter().rev());

            // --- Determine Public Name, Resolved ID, Target Kind, and Definition Name ---
            let public_name = final_path.last().cloned().unwrap_or_default(); // Get the last segment as public name

            // The BFS started with the original item's definition ID.
            // If the path involves re-exports, the final `resolved_id` should still be the definition ID
            // for internal items. For external items, it should be the ImportNode ID.
            // We need to trace back or determine this based on the path/re-export info.
            // For now, assume SPP correctly resolves through internal re-exports.

            // Let's refine the target_kind determination:
            // Use map_err for SynParserError -> ModuleTreeError conversion
            let (resolved_id, target_kind) = match graph
                .find_node_unique(item_any_id)
                .map_err(ModuleTreeError::from)?
                .as_import()
            {
                // If the original item_any_id points to an ImportNode (meaning it was a re-export)
                Some(import_node) => {
                    // Check if it's an external re-export
                    if import_node.is_extern_crate()
                        || import_node
                            .source_path()
                            .first()
                            .is_some_and(|seg| graph.iter_dependency_names().any(|dep| dep == seg))
                    {
                        // External: resolved_id is the ImportNode's ID (as AnyNodeId)
                        (
                            import_node.id.as_any(), // Convert ImportNodeId to AnyNodeId
                            ResolvedTargetKind::ExternalReExport {
                                external_path: import_node.source_path().to_vec(),
                            },
                        )
                    } else {
                        // Internal re-export: SPP should have resolved *through* this.
                        // The resolved_id should be the ultimate definition ID.
                        // We need to find the target of the ReExports relation from this import_node.
                        let reexport_target_id = tree
                            .get_iter_relations_from(&import_node.id.as_any()) // Use AnyNodeId
                            .and_then(|mut iter| {
                                iter.find_map(|tr| match tr.rel() {
                                    SyntacticRelation::ReExports { target, .. } => Some(*target), // Target is PrimaryNodeId
                                    _ => None,
                                })
                            })
                            .map(|pid| pid.as_any()) // Convert PrimaryNodeId to AnyNodeId
                            .unwrap_or(item_any_id); // Fallback to original item_any_id

                        (
                            reexport_target_id, // Already AnyNodeId
                            ResolvedTargetKind::InternalDefinition {
                                definition_id: reexport_target_id, // Use AnyNodeId
                            },
                        )
                    }
                }
                // If the original item_any_id points to a definition node
                None => (
                    item_any_id, // resolved_id is the definition ID (AnyNodeId)
                    ResolvedTargetKind::InternalDefinition {
                        definition_id: item_any_id, // Use AnyNodeId
                    },
                ),
            };

            // Determine definition_name
            // Use map_err for SynParserError -> ModuleTreeError conversion
            let definition_name =
                if let ResolvedTargetKind::InternalDefinition { definition_id } = target_kind {
                    let def_node = graph
                        .find_node_unique(definition_id)
                        .map_err(ModuleTreeError::from)?;
                    def_node
                        .name()
                        .ne(&public_name)
                        .then(|| def_node.name().to_string())
                } else {
                    None // Not an internal definition
                };

            // --- Construct Final Result ---
            let final_node_path = NodePath::new_unchecked(final_path); // Path is Vec<String>
            return Ok(ResolvedItemInfo {
                path: final_node_path, // Module path as NodePath
                public_name,           // Name at the end of the path
                resolved_id,           // ID of definition or import node
                target_kind,           // Kind of resolved target
                definition_name,       // Original name if renamed internally
            });
        }

        // --- 4. Explore Upwards (Containing Module) ---
        tree.log_spp_explore_containment(current_mod_id, &path_to_item);
        explore_up_via_containment(
            &tree,
            current_mod_id,
            &path_to_item,
            &mut queue,
            &mut visited,
            // graph, // Removed argument
        )?; // Propagate potential errors

        // --- 5. Explore Sideways/Upwards (Re-exports) ---
        tree.log_spp_explore_reexports(current_mod_id, &path_to_item);
        let _ = explore_up_via_reexports(
            &tree,
            current_mod_id,
            &path_to_item,
            &mut queue,
            &mut visited,
            graph,
        ); // Need to handle errors
           // When should this return error for invalid graph state?
    } // End while loop

    // --- 6. Not Found ---
    Err(ModuleTreeError::ItemNotPubliclyAccessible(item_any_id)) // Use AnyNodeId in error
}

// Helper function for exploring via parent modules
pub(super) fn explore_up_via_containment(
    tree: &ModuleTree,
    current_mod_id: ModuleNodeId,
    path_to_item: &[String],
    queue: &mut VecDeque<(ModuleNodeId, Vec<String>)>,
    visited: &mut HashSet<ModuleNodeId>,
    // graph: &ParsedCodeGraph, // Removed unused parameter
) -> Result<(), ModuleTreeError> {
    // Added Result return

    let current_mod_node = tree.get_module_checked(&current_mod_id)?; // O(1)
    tree.log_spp_containment_start(current_mod_node);
    // Determine the ID and visibility source (declaration or definition)
    let (effective_source_id, visibility_source_node) =
        if current_mod_node.is_file_based() && current_mod_id != tree.root {
            // For file-based modules, find the declaration using AnyNodeId
            let currom_mod_any_id = &current_mod_id.as_any();
            let mut decl_relations = tree
                .get_iter_relations_to(currom_mod_any_id) // Use AnyNodeId
                .ok_or_else(|| ModuleTreeError::no_relations_found(current_mod_node))?;

            tree.log_spp_containment_vis_source(current_mod_node);

            // Find the first relation that links a declaration to this definition
            let decl_id_opt = decl_relations.find_map(|tr| match tr.rel() {
                SyntacticRelation::ResolvesToDefinition { source, target }
                | SyntacticRelation::CustomPath { source, target }
                    if *target == current_mod_id =>
                {
                    Some(*source) // Source is ModuleNodeId
                }
                _ => None,
            });

            if let Some(decl_id) = decl_id_opt {
                // Visibility comes from the declaration node
                tree.log_spp_containment_vis_source_decl(decl_id);
                (decl_id, tree.get_module_checked(&decl_id)?)
            } else {
                // Unlinked file-based module, treat as private/inaccessible upwards
                tree.log_spp_containment_unlinked(current_mod_id);
                return Ok(()); // Cannot proceed upwards via containment
            }
        } else {
            tree.log_spp_containment_vis_source_inline(current_mod_node);
            // Inline module or root, use ittree
            (current_mod_id, current_mod_node)
        };

    // Find the parent of the effective source (declaration or inline module) using AnyNodeId
    let parent_mod_id_opt = tree
        .get_iter_relations_to(&effective_source_id.as_any()) // Use AnyNodeId
        .and_then(|mut iter| {
            iter.find_map(|tr| match tr.rel() {
                SyntacticRelation::Contains { source, target }
                    if target.as_any() == effective_source_id.as_any() =>
                // Compare AnyNodeId representations
                {
                    Some(*source) // Source is ModuleNodeId
                }
                _ => None,
            })
        });

    if let Some(parent_mod_id) = parent_mod_id_opt {
        // Check visibility: Is the declaration/inline module visible FROM the parent?
        // We need the parent module node to check its scope if visibility is restricted
        let parent_mod_node = tree.get_module_checked(&parent_mod_id)?;

        tree.log_spp_containment_check_parent(parent_mod_node);
        if tree.is_accessible_from(parent_mod_id, effective_source_id) {
            // Need is_accessible_from helper
            if visited.insert(parent_mod_id) {
                // Check if parent is newly visited
                let mut new_path = path_to_item.to_vec();
                // Prepend the name used to declare/define the current module
                new_path.push(visibility_source_node.name().to_string());
                tree.log_spp_containment_queue_parent(parent_mod_id, &new_path);
                queue.push_back((parent_mod_id, new_path));
            } else {
                tree.log_spp_containment_parent_visited(parent_mod_id);
            }
        } else {
            tree.log_spp_containment_parent_inaccessible(
                visibility_source_node,
                effective_source_id,
                parent_mod_id,
            );
        }
    } else if effective_source_id != tree.root {
        // Should only happen if root has no parent relation, otherwise inconsistent tree
        tree.log_spp_containment_no_parent(effective_source_id);
    }
    Ok(())
}

// Helper function for exploring via re-exports
pub(super) fn explore_up_via_reexports(
    tree: &ModuleTree,
    // The ID of the item/module *potentially* being re-exported
    target_id: ModuleNodeId, // Changed name for clarity
    path_to_item: &[String],
    queue: &mut VecDeque<(ModuleNodeId, Vec<String>)>,
    visited: &mut HashSet<ModuleNodeId>,
    graph: &ParsedCodeGraph,
) -> Result<(), ModuleTreeError> {
    // Added Result return
    tree.log_spp_reexport_start(target_id, path_to_item);
    // Find ImportNodes that re-export the target_id using AnyNodeId
    // Need reverse ReExport lookup: target = target_id -> source = import_node_id
    let target_id_as_any = target_id.as_any();
    let reexporting_imports = tree
        .get_iter_relations_to(&target_id_as_any) // Use AnyNodeId
        .map(|iter| {
            iter.filter_map(|tr| match tr.rel() {
                SyntacticRelation::ReExports { source, target }
                    if target.as_any() == target_id.as_any() =>
                {
                    Some(*source) // Source is ImportNodeId
                }
                _ => None,
            })
        })
        .into_iter() // Convert Option<impl Iterator> to Iterator
        .flatten(); // Flatten to get ImportNodeIds

    for import_node_id in reexporting_imports {
        let import_node = match graph.get_import_checked(import_node_id) {
            Ok(node) => node,
            Err(_) => {
                tree.log_spp_reexport_missing_import_node(import_node_id);
                continue; // Skip this relation
            }
        };
        // Check for extern crate, return error that needs to be handled by caller.
        if import_node.is_extern_crate() {
            tree.log_spp_reexport_is_external(import_node);
            return Err(ModuleTreeError::ExternalItemNotResolved(
                import_node_id.as_any(), // Use AnyNodeId in error
            ));
        }
        tree.log_spp_reexport_get_import_node(import_node);

        // Check if the re-export ittree is public (`pub use`, `pub(crate) use`, etc.)
        if !import_node.is_public_use() {
            tree.log_spp_reexport_not_public(import_node);
            continue; // Skip private `use` statements
        }

        // Find the module containing this ImportNode using AnyNodeId
        let container_mod_id_opt = tree
            .get_iter_relations_to(&import_node_id.as_any()) // Use AnyNodeId
            .and_then(|mut iter| {
                iter.find_map(|tr| match tr.rel() {
                    SyntacticRelation::Contains { source, target }
                        if target.as_any() == import_node_id.as_any() =>
                    // Compare AnyNodeId representations
                    {
                        Some(*source) // Source is ModuleNodeId
                    }
                    _ => None,
                })
            });

        if let Some(reexporting_mod_id) = container_mod_id_opt {
            // IMPORTANT: Check if the *re-exporting module* ittree is accessible
            // This requires knowing *from where* we are checking. In BFS, we don't have
            // a single "current location" in the same way as the downward search.
            // We need to ensure the path *up to* reexporting_mod_id is public.
            // The BFS naturally handles this: if we reach reexporting_mod_id, it means
            // we got there via a public path from the original item's parent.
            // So, we only need to check if we've visited this module before.

            if visited.insert(reexporting_mod_id) {
                let mut new_path = path_to_item.to_vec();
                // Prepend the name the item is re-exported AS
                new_path.push(import_node.visible_name.clone());
                tree.log_spp_reexport_queue_module(import_node, reexporting_mod_id, &new_path);
                queue.push_back((reexporting_mod_id, new_path));
            } else {
                tree.log_spp_reexport_module_visited(reexporting_mod_id);
            }
        } else {
            tree.log_spp_reexport_no_container(import_node_id);
        }
    }
    Ok(())
}

pub(super) fn is_accessible(tree: &ModuleTree, source: ModuleNodeId, target: ModuleNodeId) -> bool {
    // --- Determine Effective Visibility of the Target ---
    // Use the refactored helper function.
    let effective_vis = match tree.get_effective_visibility(target) {
        Some(vis) => vis,
        None => {
            // Target module doesn't exist in the tree.
            let log_ctx = AccLogCtx::new(source, target, None, tree);
            tree.log_access(&log_ctx, "Target Module Not Found", false);
            return false;
        }
    };

    // --- Create Log Context ---
    let log_ctx = AccLogCtx::new(source, target, Some(effective_vis), tree);

    // --- Perform Accessibility Check based on Effective Visibility ---
    match effective_vis {
        VisibilityKind::Public => {
            tree.log_access(&log_ctx, "Public Visibility", true);
            true // Public is always accessible
        }
        VisibilityKind::Crate => {
            tree.log_access(&log_ctx, "Crate Visibility", true);
            true // Crate is always accessible within the same ModuleTree
        }
        VisibilityKind::Restricted(ref restricted_path_vec) => {
            // Attempt to resolve the restriction path to a ModuleNodeId
            let restriction_path = match NodePath::try_from(restricted_path_vec.clone()) {
                Ok(p) => p,
                Err(_) => {
                    tree.log_access(&log_ctx, "Restricted Visibility (Invalid Path)", false);
                    return false; // Invalid path format
                }
            };

            // Find the module ID corresponding to the restriction path.
            // Check both definition and declaration indices.
            let restriction_module_id_opt = tree
                .path_index // Check definitions first
                .get(&restriction_path)
                .and_then(|any_id| ModuleNodeId::try_from(*any_id).ok()) // Convert AnyNodeId
                .or_else(|| tree.decl_index.get(&restriction_path).copied()); // Check declarations

            let restriction_module_id = match restriction_module_id_opt {
                Some(id) => id,
                None => {
                    tree.log_access(&log_ctx, "Restricted Visibility (Path Not Found)", false);
                    return false; // Restriction path doesn't resolve to a known module
                }
            };

            // Check 1: Is the source module the restriction module ittree?
            if source == restriction_module_id {
                tree.log_access(&log_ctx, "Restricted (Source is Restriction)", true);
                return true;
            }

            // Check 2: Is the source module a descendant of the restriction module?
            // Traverse upwards from the source using the refactored get_parent_module_id.
            let mut current_ancestor_opt = tree.get_parent_module_id(source);
            while let Some(ancestor_id) = current_ancestor_opt {
                tree.log_access_restricted_check_ancestor(ancestor_id, restriction_module_id);
                if ancestor_id == restriction_module_id {
                    tree.log_access(&log_ctx, "Restricted (Ancestor Match)", true);
                    return true; // Found restriction module in ancestors
                }
                if ancestor_id == tree.root {
                    break; // Reached crate root without finding it
                }
                current_ancestor_opt = tree.get_parent_module_id(ancestor_id);
                // Continue upwards
            }

            // If loop finishes without finding the restriction module in ancestors
            tree.log_access(&log_ctx, "Restricted (No Ancestor Match)", false);
            false
        }
        VisibilityKind::Inherited => {
            // Inherited means private to the defining module.
            // Access is allowed ONLY if the source *is* the target's direct parent module.
            // Note: `source == target` check is removed; an item cannot access ittree via visibility,
            // it's just in scope. Visibility applies to accessing items *from other modules*.
            let target_parent_opt = tree.get_parent_module_id(target);
            let accessible = target_parent_opt == Some(source);
            tree.log_access(&log_ctx, "Inherited Visibility", accessible);
            accessible
        }
    }
}

// Helper needed for visibility check upwards (simplified version of ModuleTree::is_accessible)
// Checks if `target_id` (decl or inline mod) is accessible *from* `potential_parent_id`
#[allow(unused_variables)]
pub(super) fn is_accessible_from(
    tree: &ModuleTree,
    potential_parent_id: ModuleNodeId,
    target_id: ModuleNodeId,
) -> bool {
    // This needs logic similar to ModuleTree::is_accessible, but focused:
    // 1. Get the effective visibility of `target_id` (considering its declaration if file-based).
    // 2. Check if that visibility allows access from `potential_parent_id`.
    //    - Public: Yes
    //    - Crate: Yes (within same crate)
    //    - Restricted(path): Check if potential_parent_id is or is within the restriction path.
    //    - Inherited: Yes, only if potential_parent_id *is* the direct parent module where target_id is defined/declared.
    // Placeholder - requires careful implementation matching ModuleTree::is_accessible logic
    // For now, let's assume public for testing, replace with real check
    tree.get_effective_visibility(target_id)
        .is_some_and(|vis| vis.is_pub()) // TODO: Replace with full check
}

/// Checks if an item (`target_item_id`) is reachable via a chain of `ReExports` relations
/// starting from a specific `ImportNode` (`start_import_id`).
/// Used to detect potential re-export cycles or verify paths.
#[allow(
    dead_code,
    reason = "May be useful later for cycle detection or validation"
)]
pub(super) fn is_part_of_reexport_chain(
    tree: &ModuleTree,
    start_import_id: ImportNodeId,
    target_item_id: AnyNodeId, // Target can be any node type
) -> Result<bool, ModuleTreeError> {
    let mut current_import_id = start_import_id;
    let mut visited_imports = HashSet::new(); // Track visited ImportNodeIds to detect cycles

    // Limit iterations to prevent infinite loops in case of unexpected cycles
    for _ in 0..100 {
        // Check if the current import node has already been visited in this chain
        if !visited_imports.insert(current_import_id) {
            // Cycle detected involving ImportNodes
            return Err(ModuleTreeError::ReExportChainTooLong {
                start_node_id: start_import_id.as_any(), // Report cycle start
            });
        }

        // Check if the current ImportNode directly re-exports the target item
        let found_direct_reexport = tree
            .get_iter_relations_from(&current_import_id.as_any()) // Relations FROM the import node
            .is_some_and(|mut iter| {
                iter.any(|tr| match tr.rel() {
                    SyntacticRelation::ReExports { source, target }
                        if *source == current_import_id && target.as_any() == target_item_id =>
                    {
                        true // Found direct re-export of the target
                    }
                    _ => false,
                })
            });

        if found_direct_reexport {
            return Ok(true); // Target found in the chain
        }

        // If not found directly, find the *next* ImportNode in the chain.
        // Look for a ReExports relation where the *target* is the current ImportNode.
        let next_import_in_chain = tree
            .get_iter_relations_to(&current_import_id.as_any()) // Relations TO the import node
            .and_then(|mut iter| {
                iter.find_map(|tr| match tr.rel() {
                    // Find a relation where the current import is the TARGET
                    SyntacticRelation::ReExports { source, target }
                        if target.as_any() == current_import_id.as_any() =>
                    {
                        Some(*source) // The source of this relation is the next ImportNodeId
                    }
                    _ => None,
                })
            });

        if let Some(next_id) = next_import_in_chain {
            // Move to the next import node in the chain
            current_import_id = next_id;
        } else {
            // No further re-exports found targeting the current import node. Chain ends here.
            break;
        }
    }

    // If the loop finishes without finding the target, it's not part of this chain
    Ok(false)
}

/// Determines the effective visibility of a module definition for access checks.
///
/// For inline modules or the crate root, it's the visibility stored on the `ModuleNode` itself.
/// For file-based modules (that are not the root), it's the visibility of the corresponding
/// `mod name;` declaration statement found via `ResolvesToDefinition` or `CustomPath` relations.
/// If the declaration cannot be found (e.g., unlinked module file), it defaults to the
/// visibility stored on the definition node itself (which is typically `Inherited`).
pub(super) fn get_effective_visibility(
    tree: &ModuleTree,
    module_def_id: ModuleNodeId,
) -> Option<&VisibilityKind> {
    let module_node = tree.modules().get(&module_def_id)?; // Get the definition node

    // Inline modules and the root module use their own declared visibility.
    if module_node.is_inline() || module_def_id == tree.root {
        return Some(module_node.visibility());
    }

    // For file-based modules (not root), find the visibility of the declaration.
    // Find incoming ResolvesToDefinition or CustomPath relations.
    tree.get_iter_relations_to(&module_def_id.into())
        .and_then(|mut iter| {
            iter.find_map(|tr| match tr.rel() {
                // Match relations pointing *to* this definition module
                SyntacticRelation::ResolvesToDefinition {
                    source: decl_id,
                    target,
                }
                | SyntacticRelation::CustomPath {
                    source: decl_id,
                    target,
                } if *target == module_def_id => {
                    // Found the declaration ID (`decl_id`). Get the declaration node.
                    tree.modules()
                        .get(decl_id)
                        .map(|decl_node| decl_node.visibility())
                }
                _ => None, // Ignore other relation kinds
            })
        })
        .or_else(|| {
            // If no declaration relation was found (e.g., unlinked module file),
            // fall back to the visibility defined on the module file ittree.
            // This usually defaults to Inherited/private.
            log_effective_vis_fallback(module_def_id);
            Some(module_node.visibility())
        })
}
pub(super) fn log_effective_vis_fallback(module_def_id: ModuleNodeId) {
    debug!(target: LOG_TARGET_VIS, "  {} No declaration found for file-based module {}. Falling back to definition visibility.",
        "Fallback:".log_yellow(),
        module_def_id.to_string().log_id()
    );
}

/// Helper to resolve a path relative to a starting module.
/// This is a complex function mimicking Rust's name resolution.
pub(super) fn resolve_path_relative_to(
    tree: &ModuleTree,
    base_module_id: ModuleNodeId,
    path_segments: &[String],
    graph: &ParsedCodeGraph, // Need graph access
) -> Result<AnyNodeId, ModuleTreeError> {
    // Changed: Return AnyNodeId
    if path_segments.is_empty() {
        return Err(ModuleTreeError::NodePathValidation(Box::new(
            SynParserError::NodeValidation("Empty path segments for relative resolution".into()),
        )));
    }

    let mut current_module_id = base_module_id;
    let mut remaining_segments = path_segments;

    // 1. Handle `tree::` prefix
    if remaining_segments[0] == "tree" {
        remaining_segments = &remaining_segments[1..];
        if remaining_segments.is_empty() {
            // Path was just "tree", refers to the module ittree
            return Ok(current_module_id.as_any()); // Changed: Return AnyNodeId
        }
    }
    // 2. Handle `super::` prefix (potentially multiple times)
    else {
        while remaining_segments[0] == "super" {
            let node_path = NodePath::try_from(path_segments.to_vec())?;
            current_module_id = tree.get_parent_module_id(current_module_id).ok_or({
                ModuleTreeError::UnresolvedReExportTarget {
                    path: node_path,      // Original path for error
                    import_node_id: None, // Indicate failure resolving 'super'
                }
            })?;
            remaining_segments = &remaining_segments[1..];
            if remaining_segments.is_empty() {
                // Path ended with "super", refers to the parent module
                return Ok(current_module_id.as_any()); // Changed: Return AnyNodeId
            }
        }
    }

    // 3. Iterative Resolution through remaining segments
    let mut resolved_any_id: Option<AnyNodeId> = None; // Changed: Store AnyNodeId

    for (i, segment) in remaining_segments.iter().enumerate() {
        // Determine the module to search within for this segment
        let node_path = NodePath::try_from(path_segments.to_vec())?;
        let search_in_module_id = match resolved_any_id {
            Some(any_id) => ModuleNodeId::try_from(any_id).map_err(|_| {
                // The previously resolved item was not a module, cannot continue path
                ModuleTreeError::UnresolvedReExportTarget {
                    path: node_path,
                    import_node_id: None, // Indicate failure due to non-module segment
                }
            })?,
            None => current_module_id, // Start in the initial/adjusted module
        };

        // 4. Find items named `segment` directly contained within `search_in_module_id` using AnyNodeId
        let contains_relations = tree
            .get_iter_relations_from(&search_in_module_id.as_any()) // Use AnyNodeId
            .map(|iter| iter.collect::<Vec<_>>()) // Collect for logging/multiple checks
            .unwrap_or_default();

        let mut candidates: Vec<AnyNodeId> = Vec::new(); // Changed: Store AnyNodeId
        tree.log_resolve_segment_start(segment, search_in_module_id, contains_relations.len());

        for rel_ref in &contains_relations {
            // Iterate by reference
            let target_any_id = rel_ref.rel().target(); // Target is AnyNodeId
            tree.log_resolve_segment_relation(target_any_id);
            match graph.find_node_unique(target_any_id) {
                Ok(target_node) => {
                    let name_matches = target_node.name() == segment;
                    tree.log_resolve_segment_found_node(target_node, segment, name_matches);
                    if name_matches {
                        // 5. Visibility Check (Simplified)
                        // Check if the target node is accessible from the module we are searching in.
                        // For modules, use is_accessible. For other items, assume accessible if contained (needs refinement).
                        let is_target_accessible = match ModuleNodeId::try_from(target_any_id) {
                            Ok(target_mod_id) => {
                                tree.is_accessible(search_in_module_id, target_mod_id)
                            }
                            Err(_) => {
                                // Assume non-module items are accessible if contained for now.
                                // A better check would involve the item's own visibility.
                                true
                            }
                        };

                        if is_target_accessible {
                            candidates.push(target_any_id); // Changed: Push AnyNodeId
                        }
                    }
                }
                Err(e) => {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD,
                        "    {} Error finding node for ID {}: {:?}",
                        "✗".log_error(),
                        target_any_id.to_string().log_id(), // Use AnyNodeId
                        e.to_string().log_error()
                    );
                }
            }
        }
        // --- DIAGNOSTIC LOGGING END ---

        // --- Filter and Select ---
        match candidates.len() {
            0 => {
                // Not found in direct definitions
                debug!(target: LOG_TARGET_MOD_TREE_BUILD,
                    "{} No candidates found for segment '{}' in module {}. Returning error.",
                    "Resolution Failed:".log_error(),
                    segment.log_name(),
                    search_in_module_id.to_string().log_id()
                );
                return Err(ModuleTreeError::UnresolvedReExportTarget {
                    path: NodePath::try_from(path_segments.to_vec())?, // Original path
                    import_node_id: None, // Indicate failure at this segment
                });
            }
            1 => {
                let found_any_id = candidates[0]; // Changed: ID is AnyNodeId
                resolved_any_id = Some(found_any_id); // Store the resolved AnyNodeId

                // Check if it's the last segment
                if i == remaining_segments.len() - 1 {
                    return Ok(found_any_id); // Changed: Return AnyNodeId
                } else {
                    // More segments remain, ensure the found item is a module
                    if graph.find_node_unique(found_any_id)?.as_module().is_none() {
                        return Err(ModuleTreeError::UnresolvedReExportTarget {
                            // Or a more specific error like "PathNotAModule"
                            path: NodePath::try_from(path_segments.to_vec())?,
                            import_node_id: None,
                        });
                    }
                    // Continue to the next segment, search will start within this module
                }
            }
            _ => {
                // Ambiguous: Multiple items with the same name found
                // TODO: Add a specific ModuleTreeError variant for ambiguity?
                return Err(ModuleTreeError::UnresolvedReExportTarget {
                    path: NodePath::try_from(path_segments.to_vec())?,
                    import_node_id: None, // Indicate ambiguity
                });
            }
        }
    }
    // Should be unreachable if path_segments is not empty, but handle defensively
    Err(ModuleTreeError::UnresolvedReExportTarget {
        path: NodePath::try_from(path_segments.to_vec())?,
        import_node_id: None,
    })
}
