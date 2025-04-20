use std::collections::{HashMap, HashSet, VecDeque};

use colored::*; // Import colored for terminal colors
use log::debug; // Import the debug macro
use ploke_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::error::SynParserError;
use crate::parser::nodes::NodePath; // Ensure NodePath is imported

use super::{
    nodes::{GraphNode, ImportNode, ModuleNode, ModuleNodeId}, // Removed NodePath from here
    relations::{GraphId, Relation, RelationKind},
    types::VisibilityKind,
    CodeGraph,
};

const LOG_TARGET_VIS: &str = "mod_tree_vis"; // Define log target for visibility checks

#[derive(Debug, Clone)]
pub struct ModuleTree {
    // ModuleNodeId of the root file-level module, e.g. `main.rs`, `lib.rs`, used to initialize the
    // ModuleTree.
    root: ModuleNodeId,
    /// Index of all modules in the merged `CodeGraph`, in a HashMap for efficient lookup
    modules: HashMap<ModuleNodeId, ModuleNode>,
    /// Temporary storage for unresolved imports (e.g. `use` statements)
    pending_imports: Vec<PendingImport>,
    /// Temporary storage for unresolved exports (e.g. `pub use` statements)
    pending_exports: Vec<PendingExport>,
    /// Reverse path indexing to find NodeId on a given path
    /// HashMap appropriate for many -> few possible mapping
    /// Contains all `NodeId` items except module declarations due to path collision with defining
    /// module.
    path_index: HashMap<NodePath, NodeId>,
    /// Separate HashMap for module declarations.
    /// Reverse lookup, but can't be in the same HashMap as the modules that define them, since
    /// they both have the same `path`. This should be the only case in which two items have the
    /// same path.
    decl_index: HashMap<NodePath, NodeId>,
    tree_relations: Vec<TreeRelation>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingImport {
    module_node_id: ModuleNodeId, // Keep private
    import_node: ImportNode,      // Keep private
}

impl PendingImport {
    pub(crate) fn from_import(import: ImportNode) -> Self {
        // Make crate-visible if needed internally
        PendingImport {
            module_node_id: ModuleNodeId::new(import.id),
            import_node: import,
        }
    }

    /// Returns the ID of the module containing this pending import.
    pub fn module_node_id(&self) -> ModuleNodeId {
        self.module_node_id
    }

    /// Returns a reference to the `ImportNode` associated with this pending import.
    pub fn import_node(&self) -> &ImportNode {
        &self.import_node
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingExport {
    module_node_id: ModuleNodeId, // Keep private
    export_node: ImportNode,      // Keep private
}

impl PendingExport {
    pub(crate) fn from_export(export: ImportNode) -> Self {
        // Make crate-visible if needed internally
        PendingExport {
            module_node_id: ModuleNodeId::new(export.id),
            export_node: export,
        }
    }

    /// Returns the ID of the module containing this pending export.
    pub fn module_node_id(&self) -> ModuleNodeId {
        self.module_node_id
    }

    /// Returns a reference to the `ImportNode` associated with this pending export.
    pub fn export_node(&self) -> &ImportNode {
        &self.export_node
    }
}

/// Relations useful in the module tree.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeRelation(Relation); // Keep inner field private

impl TreeRelation {
    pub fn new(relation: Relation) -> Self {
        Self(relation)
    }

    /// Returns a reference to the inner `Relation`.
    pub fn relation(&self) -> &Relation {
        &self.0
    }
}

impl From<Relation> for TreeRelation {
    fn from(value: Relation) -> Self {
        Self::new(value)
    }
}

// Struct to hold info about unlinked modules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedModuleInfo {
    pub module_id: NodeId,
    pub definition_path: NodePath, // Store the path that couldn't be linked
}

// Define the new ModuleTreeError enum
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ModuleTreeError {
    #[error("Duplicate definition path '{path}' found in module tree. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    DuplicatePath {
        // Change to a struct variant
        path: NodePath,
        existing_id: NodeId,
        conflicting_id: NodeId,
    },

    #[error("Duplicate module ID found in module tree for ModuleNode: {0:?}")]
    DuplicateModuleId(Box<ModuleNode>), // Box the large ModuleNode

    /// Wraps SynParserError for convenience when using TryFrom<Vec<String>> for NodePath
    #[error("Node path validation error: {0}")]
    NodePathValidation(Box<SynParserError>), // Box the recursive type

    #[error("Containing module not found for node ID: {0}")]
    ContainingModuleNotFound(NodeId), // Added error variant

    // NEW: Variant holding a collection of UnlinkedModuleInfo
    // Corrected format string - the caller logs the count/details
    #[error("Found unlinked module file(s) (no corresponding 'mod' declaration).")]
    FoundUnlinkedModules(Box<Vec<UnlinkedModuleInfo>>), // Use Box as requested
}

impl ModuleTree {
    pub fn root(&self) -> ModuleNodeId {
        self.root
    }

    pub fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode> {
        &self.modules
    }

    /// Returns a reference to the internal path index mapping canonical paths to NodeIds.
    pub fn path_index(&self) -> &HashMap<NodePath, NodeId> {
        &self.path_index
    }

    /// Returns a slice of the relations relevant to the module tree structure.
    pub fn tree_relations(&self) -> &[TreeRelation] {
        &self.tree_relations
    }

    /// Returns a slice of the pending private imports collected during tree construction.
    pub fn pending_imports(&self) -> &[PendingImport] {
        &self.pending_imports
    }

    /// Returns a slice of the pending public re-exports collected during tree construction.
    pub fn pending_exports(&self) -> &[PendingExport] {
        &self.pending_exports
    }

    pub fn new_from_root(root: ModuleNodeId) -> Self {
        Self {
            root,
            modules: HashMap::new(),
            pending_imports: vec![],
            pending_exports: vec![],
            path_index: HashMap::new(),
            decl_index: HashMap::new(),
            tree_relations: vec![],
        }
    }

    pub fn add_module(&mut self, module: ModuleNode) -> Result<(), ModuleTreeError> {
        let imports = module.imports.clone();
        // Add all private imports
        self.pending_imports.extend(
            imports
                .iter()
                .filter(|imp| imp.is_inherited_use())
                .map(|imp| PendingImport::from_import(imp.clone())),
        );
        // Add all re-exports
        self.pending_exports.extend(
            imports
                .iter()
                .filter(|imp| imp.is_reexport())
                .map(|imp| PendingExport::from_export(imp.clone())),
        );

        // Use map_err for explicit conversion from SynParserError to ModuleTreeError
        let node_path = NodePath::try_from(module.defn_path().clone())
            .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;
        let conflicting_id = module.id(); // ID of the module we are trying to add
                                          // Use entry API for clarity and efficiency
        if module.is_declaration() {
            match self.decl_index.entry(node_path.clone()) {
                // Clone node_path for the error case
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Path already exists
                    let existing_id = *entry.get();
                    return Err(ModuleTreeError::DuplicatePath {
                        path: node_path, // Use the cloned path
                        existing_id,
                        conflicting_id,
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Path is free, insert it
                    entry.insert(conflicting_id);
                }
            }
        } else {
            match self.path_index.entry(node_path.clone()) {
                // Clone node_path for the error case
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Path already exists
                    let existing_id = *entry.get();
                    return Err(ModuleTreeError::DuplicatePath {
                        path: node_path, // Use the cloned path
                        existing_id,
                        conflicting_id,
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Path is free, insert it
                    entry.insert(conflicting_id);
                }
            }
        }

        // insert module to tree
        let module_id = ModuleNodeId::new(conflicting_id); // Use the ID we already have
        let dup_node = self.modules.insert(module_id, module);
        if let Some(dup) = dup_node {
            // Box the duplicate node when creating the error variant
            return Err(ModuleTreeError::DuplicateModuleId(Box::new(dup)));
        }

        Ok(())
    }

    /// Builds 'ResolvesToDefinition' relations between module declarations and their file-based definitions.
    /// Assumes the `path_index` and `decl_index` have been populated correctly by `add_module`.
    /// Returns `Ok(())` on complete success.
    /// Returns `Err(ModuleTreeError::FoundUnlinkedModules)` if only unlinked modules are found.
    /// Returns other `Err(ModuleTreeError)` variants on fatal errors (e.g., path validation).
    pub fn build_logical_paths(&mut self, modules: &[ModuleNode]) -> Result<(), ModuleTreeError> {
        // Return Ok(()) or Err(ModuleTreeError)
        let mut new_relations: Vec<TreeRelation> = Vec::new();
        let mut collected_unlinked: Vec<UnlinkedModuleInfo> = Vec::new(); // Store only unlinked info
        let root_id = self.root();

        for module in modules
            .iter()
            .filter(|m| m.is_file_based() && m.id() != *root_id.as_inner())
        {
            let defn_path_vec = module.defn_path();
            let defn_path_slice = defn_path_vec.as_slice();

            match self.decl_index.get(defn_path_slice) {
                Some(decl_id) => {
                    // Found declaration, create relation
                    let logical_relation = Relation {
                        source: GraphId::Node(module.id()),
                        target: GraphId::Node(*decl_id),
                        kind: RelationKind::ResolvesToDefinition,
                    };
                    new_relations.push(logical_relation.into());
                }
                None => {
                    // No declaration found. Try to create UnlinkedModuleInfo.
                    // Use map_err for explicit conversion from SynParserError to ModuleTreeError
                    let node_path = NodePath::try_from(defn_path_vec.clone())
                        .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;

                    // If path conversion succeeded, collect the unlinked info.
                    collected_unlinked.push(UnlinkedModuleInfo {
                        module_id: module.id(),
                        definition_path: node_path,
                    });
                }
            }
        }

        // Append relations regardless of whether unlinked modules were found.
        // We only skip appending if a fatal error occurred earlier (which would have returned Err).
        self.tree_relations.append(&mut new_relations);

        // Check if any unlinked modules were collected
        if collected_unlinked.is_empty() {
            Ok(()) // Complete success
        } else {
            // Only non-fatal "unlinked" issues occurred. Return the specific error variant.
            Err(ModuleTreeError::FoundUnlinkedModules(Box::new(
                collected_unlinked,
            )))
        }
    }

    pub fn register_containment_batch(
        &mut self,
        relations: &[Relation],
    ) -> Result<(), ModuleTreeError> {
        for rel in relations.iter() {
            self.tree_relations.push((*rel).into());
        }
        Ok(())
    }

    // Resolves visibility for target node as if it were a dependency.
    // Only used as a helper in the shortest public path.
    #[allow(unused_variables)]
    pub fn resolve_visibility<T: GraphNode>(
        &self,
        node: &T,
        graph: &CodeGraph,
    ) -> Result<VisibilityKind, ModuleTreeError> {
        let parent_module_vis = graph
            .modules
            .iter()
            .find(|m| m.items().is_some_and(|m| m.contains(&node.id())))
            .map(|m| m.visibility())
            // Use ok_or_else to handle Option and create the specific error
            .ok_or_else(|| ModuleTreeError::ContainingModuleNotFound(node.id()))?;
        todo!() // Rest of the visibility logic still needs implementation
    }

    pub fn shortest_public_path(
        &self,
        item_id: NodeId,
        start_module: ModuleNodeId,
    ) -> Option<Vec<String>> {
        // BFS queue: (module_id, current_path)
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back((start_module, vec![]));

        while let Some((mod_id, path)) = queue.pop_front() {
            // Check if current module contains the item publicly
            if let Some(export_path) = self.get_public_export_path(&mod_id, item_id) {
                return Some([path, export_path.to_vec()].concat());
            }

            // Queue public parent and sibling modules
            for (_rel, neighbor_mod_id) in self.get_public_neighbors(mod_id) {
                if !visited.contains(&neighbor_mod_id) {
                    let mut new_path = path.clone();
                    // Get the name from the neighbor module node
                    if let Some(neighbor_node) = self.modules.get(&neighbor_mod_id) {
                        new_path.push(neighbor_node.name.clone());
                    } else {
                        // Should not happen if graph is consistent, but handle defensively
                        eprintln!("Warning: Neighbor module {:?} not found in map during shortest_public_path.", neighbor_mod_id);
                        continue; // Skip this neighbor if node not found
                    }
                    queue.push_back((neighbor_mod_id, new_path));
                    visited.insert(neighbor_mod_id);
                }
            }
        }

        None
    }
    fn get_public_export_path(&self, mod_id: &ModuleNodeId, item_id: NodeId) -> Option<&[String]> {
        let module = self.modules().get(mod_id)?;
        let items = module.items()?;
        if items.contains(&item_id) {
            if module.visibility().is_pub() {
                Some(module.defn_path())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Gets public neighbor module IDs and the relation kind connecting them.
    /// Returns Vec<(RelationKind, ModuleNodeId)>
    pub fn get_public_neighbors(
        &self,
        module_id: ModuleNodeId,
    ) -> Vec<(RelationKind, ModuleNodeId)> {
        let mut neighbors = Vec::new();

        // 1. Get parent module if accessible
        if let Some(parent_id) = self.get_parent_module_id(module_id) {
            // Check if the *parent* is accessible *from the context of the current module*
            // (This check might need refinement depending on exact visibility rules,
            // but for now, assume if we can get the parent, we can consider it a neighbor)
            // Let's simplify: if a parent exists, consider it a potential neighbor path.
            // The accessibility check should happen when resolving the *target item*, not the path segments.
            neighbors.push((RelationKind::Contains, parent_id)); // Parent contains current
        }

        // 2. Get public siblings (children of the same parent)
        if let Some(parent_id) = self.get_parent_module_id(module_id) {
            if let Some(parent_node) = self.modules.get(&parent_id) {
                if let Some(parent_items) = parent_node.items() {
                    for &item_id in parent_items {
                        // Check if the item is a module and is public
                        if let Some(sibling_node) = self.modules.get(&ModuleNodeId::new(item_id)) {
                            // Ensure it's not the module itself and it's public
                            if sibling_node.id != module_id.into_inner()
                                && sibling_node.visibility().is_pub()
                            {
                                neighbors.push((
                                    RelationKind::Sibling,
                                    ModuleNodeId::new(sibling_node.id),
                                ));
                            }
                        }
                    }
                }
            }
        }

        neighbors
    }

    /// Helper to get parent module ID (using existing ModuleTree fields)
    fn get_parent_module_id(&self, module_id: ModuleNodeId) -> Option<ModuleNodeId> {
        self.tree_relations.iter().find_map(|r| {
            if r.relation().target == GraphId::Node(module_id.into_inner())
                && r.relation().kind == RelationKind::Contains
            {
                match r.relation().source {
                    GraphId::Node(id) => Some(ModuleNodeId::new(id)),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    /// Logs the details of an accessibility check.
    fn log_accessibility_check(
        &self,
        source: ModuleNodeId,
        target: ModuleNodeId,
        target_visibility: Option<&VisibilityKind>,
        step: &str, // Description of the check step (e.g., "Initial Check", "Restricted Path Check")
        result: bool,
    ) {
        // Use debug! macro with the specific target
        debug!(target: LOG_TARGET_VIS,
            "{} {} -> {} | Target Vis: {} | Step: {} | Result: {}",
            "Accessibility Check:".cyan().bold(),
            self.modules.get(&source).map(|m| m.name.as_str()).unwrap_or("?").yellow(),
            self.modules.get(&target).map(|m| m.name.as_str()).unwrap_or("?").blue(),
            target_visibility.map(|v| format!("{:?}", v).magenta()).unwrap_or_else(|| "NotFound".red()), // Removed .to_string()
            step.white(),
            if result { "Accessible".green() } else { "Inaccessible".red() }
        );
    }


    /// Visibility check using existing types
    pub fn is_accessible(&self, source: ModuleNodeId, target: ModuleNodeId) -> bool {
        let target_visibility = self.modules.get(&target).map(|m| m.visibility());

        let result = match target_visibility {
            Some(VisibilityKind::Public) => {
                self.log_accessibility_check(source, target, target_visibility.as_ref(), "Public Visibility", true);
                true
            }
            Some(VisibilityKind::Crate) => {
                // Check if same crate by comparing root paths
                let source_root = self.get_root_path(source);
                let source_root_path = self.get_module_path_vec(source);
                let target_root_path = self.get_module_path_vec(target);
                // Check if they share the same crate root (first segment)
                let accessible = source_root_path.first() == target_root_path.first();
                self.log_accessibility_check(source, target, target_visibility.as_ref(), "Crate Visibility", accessible);
                accessible
            }
            // Borrow restricted_path_vec instead of moving it
            Some(VisibilityKind::Restricted(ref restricted_path_vec)) => {
                 // Remove unused variable warning by prefixing with underscore
                let _source_root = self.get_root_path(source);
                // Convert the restriction path Vec<String> to NodePath for lookup
                let restriction_path = match NodePath::try_from(restricted_path_vec.clone()) {
                    Ok(p) => p,
                    Err(_) => return false, // Invalid restriction path
                };

                // Find the NodeId of the module defining the restriction scope
                let restriction_module_id = match self.path_index.get(&restriction_path) {
                    Some(id) => ModuleNodeId::new(*id),
                    None => return false, // Restriction path doesn't exist in the index
                };

                // Check if the source module *is* the restriction module
                if source == restriction_module_id {
                    return true;
                }

                // Check if the source module is a descendant of the restriction module
                let mut current_ancestor = self.get_parent_module_id(source);
                while let Some(ancestor_id) = current_ancestor {
                    if ancestor_id == restriction_module_id {
                        return true; // Found restriction module in ancestors
                    }
                    if ancestor_id == self.root {
                        break; // Reached crate root without finding it
                    }
                    current_ancestor = self.get_parent_module_id(ancestor_id);
                }
                let accessible = false; // Not the module itself or a descendant
                self.log_accessibility_check(source, target, target_visibility.as_ref(), "Restricted Visibility (Final)", accessible);
                accessible
            }
            Some(VisibilityKind::Inherited) => {
                // Inherited visibility means private to the defining module.
                // Access is only allowed if source and target are in the *same* defining module.
                // This requires finding the defining module for both source and target.
                let source_parent = self.get_parent_module_id(source);
                let target_parent = self.get_parent_module_id(target);
                let accessible = source_parent.is_some() && source_parent == target_parent;
                self.log_accessibility_check(source, target, target_visibility.as_ref(), "Inherited Visibility", accessible);
                accessible
            }
            None => {
                self.log_accessibility_check(source, target, None, "Target Module Not Found", false);
                false // Target module not found
            }
        };
        result // Return the final calculated result
    }

    /// Gets the full module path Vec<String> for a given module ID.
    fn get_module_path_vec(&self, module_id: ModuleNodeId) -> Vec<String> {
        self.modules
            .get(&module_id)
            .map(|m| m.path.clone())
            .unwrap_or_default() // Return empty path if module not found
    }

    /// Gets root path using existing ModuleTree data
    fn get_root_path(&self, module_id: ModuleNodeId) -> Vec<String> {
        let mut path = Vec::new();
        let mut current = module_id;

        while let Some(parent_id) = self.get_parent_module_id(current) {
            if let Some(module) = self.modules.get(&current) {
                path.push(module.name.clone());
            }
            current = parent_id;
        }

        path.push("crate".to_string());
        path.reverse();
        path
    }
}

// #[allow(unused_variables)]
// pub fn shortest_public_path(&self, id: NodeId) -> Result<Vec<String>, ModuleTreeError> {
//     // Returns the shortest accessible path considering visibility
//     todo!()
// }

impl ModuleTree {
    pub fn resolve_path(&self, _path: &[String]) -> Result<ModuleNodeId, Box<SynParserError>> {
        // 1. Try direct canonical path match
        // 2. Check re-exports in parent modules
        // 3. Try relative paths (self/super/crate)
        todo!()
    }
}
