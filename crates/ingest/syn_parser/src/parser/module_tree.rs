use std::collections::{HashMap, HashSet, VecDeque}; // Keep HashSet and VecDeque

use colored::*; // Import colored for terminal colors
use log::debug; // Import the debug macro
use ploke_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::error::SynParserError;
use crate::parser::nodes::NodePath; // Ensure NodePath is imported

use super::{
    nodes::{GraphId, GraphNode, ImportNode, ModuleNode, ModuleNodeId}, // Add GraphId
    relations::{Relation, RelationKind},                               // Remove GraphId
    types::VisibilityKind,
    CodeGraph,
};

const LOG_TARGET_VIS: &str = "mod_tree_vis"; // Define log target for visibility checks
const LOG_TARGET_BUILD: &str = "mod_tree_build"; // Define log target for build checks

/// Helper struct to hold context for accessibility logging.
struct AccLogCtx<'a> {
    // source_id: ModuleNodeId, // Removed unused field
    // target_id: ModuleNodeId, // Removed unused field
    source_name: &'a str,
    target_name: &'a str,
    effective_vis: Option<&'a VisibilityKind>, // Store as Option<&VisibilityKind>
}

impl<'a> AccLogCtx<'a> {
    /// Creates a new context for logging accessibility checks.
    fn new(
        source_id: ModuleNodeId,                   // Keep ID args for name lookup
        target_id: ModuleNodeId,                   // Keep ID args for name lookup
        effective_vis: Option<&'a VisibilityKind>, // Accept Option<&VisibilityKind>
        tree: &'a ModuleTree,                      // Need tree to look up names
    ) -> Self {
        let source_name = tree
            .modules
            .get(&source_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        let target_name = tree
            .modules
            .get(&target_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        Self {
            // source_id, // Removed unused field
            // target_id, // Removed unused field
            source_name,
            target_name,
            effective_vis,
        }
    }
}

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

    #[error("Item with ID {0} is not publicly accessible from the crate root.")]
    ItemNotPubliclyAccessible(NodeId), // New error variant for SPP
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
        debug!(target: LOG_TARGET_BUILD, "{} {} ({}) | Visibility: {}",
            "Inserting into tree.modules:".green(),
            module.name.yellow(),
            module_id.to_string().magenta(),
            format!("{:?}", module.visibility).cyan()
        );
        let dup_node = self.modules.insert(module_id, module); // module is moved here
        if let Some(dup) = dup_node {
            // Box the duplicate node when creating the error variant
            // Log the duplicate insertion before returning error
            debug!(target: LOG_TARGET_BUILD, "{} {} ({})",
                "Duplicate module ID insertion detected:".red().bold(),
                dup.name.yellow(),
                dup.id.to_string().magenta()
            );
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
                        source: GraphId::Node(*decl_id),
                        target: GraphId::Node(module.id()),
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

    /// Calculates the shortest public path to access a given item ID from the crate root.
    ///
    /// Performs a Breadth-First Search (BFS) starting from the crate root, exploring
    /// only publicly accessible modules.
    ///
    /// # Arguments
    /// * `item_id` - The `NodeId` of the item (function, struct, etc.) to find the path for.
    /// * `graph` - A reference to the `CodeGraph` containing the item and module definitions.
    ///
    /// # Returns
    /// * `Some(Vec<String>)` containing the path segments (e.g., `["crate", "module", "submodule"]`)
    ///   if a public path is found.
    /// * `None` if the item is not publicly accessible from the crate root.
    ///
    /// # Limitations
    /// * Currently does not handle re-exports (`pub use`). It only considers items directly
    ///   defined within a module.
    pub fn shortest_public_path(
        &self,
        item_id: NodeId,
        graph: &CodeGraph, // Need graph access for item visibility
    ) -> Result<Vec<String>, ModuleTreeError> {
        // Changed return type to Result
        // BFS queue: (module_id, current_path_segments)
        let mut queue: VecDeque<(ModuleNodeId, Vec<String>)> = VecDeque::new();
        let mut visited: HashSet<ModuleNodeId> = HashSet::new();

        // Start BFS from the crate root
        let start_module_id = self.root();
        let initial_path = vec!["crate".to_string()]; // Path always starts with "crate"

        queue.push_back((start_module_id, initial_path.clone()));
        visited.insert(start_module_id);

        while let Some((current_mod_id, current_path)) = queue.pop_front() {
            // --- Target Check ---
            // Check if the current module *contains* the item_id directly.
            let contains_relation_exists = self.tree_relations.iter().any(|tr| {
                let rel = tr.relation();
                rel.source == GraphId::Node(current_mod_id.into_inner())
                    && rel.target == GraphId::Node(item_id)
                    && rel.kind == RelationKind::Contains
            });

            if contains_relation_exists {
                // Found the containing module. Now check if the *item itself* is public.
                if let Some(item_node) = graph.find_node(item_id) {
                    if item_node.visibility().is_pub() {
                        // Item is directly contained and public, return Ok with the path.
                        // TODO: The path should arguably include the item name itself.
                        // For now, return path to containing module to match test expectations.
                        return Ok(current_path); // Return Ok(path)
                    }
                }
                // If item not found in graph or not public, continue search (maybe re-exported?)
                // For now, since re-exports aren't handled, we stop here if contained but not pub.
            }

            // --- Neighbor (Public Child) Exploration ---
            // Find direct children (both inline and file-based via declarations)
            let child_relations = self.tree_relations.iter().filter(|tr| {
                let rel = tr.relation();
                rel.source == GraphId::Node(current_mod_id.into_inner())
                    && rel.kind == RelationKind::Contains
            });

            for child_rel in child_relations {
                // Access the inner relation using .relation() before accessing .target
                if let GraphId::Node(child_id) = child_rel.relation().target {
                    // Check if this child is a module
                    if let Some(child_module_node) = self.modules.get(&ModuleNodeId::new(child_id))
                    {
                        let child_mod_id_to_check = if child_module_node.is_declaration() {
                            // If it's a declaration, find its definition for visibility check
                            self.tree_relations
                                .iter()
                                .find_map(|tr_res| {
                                    let rel_res = tr_res.relation();
                                    if rel_res.source == GraphId::Node(child_id)
                                        && rel_res.kind == RelationKind::ResolvesToDefinition
                                    {
                                        match rel_res.target {
                                            GraphId::Node(defn_id) => {
                                                Some(ModuleNodeId::new(defn_id))
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(ModuleNodeId::new(child_id)) // Fallback if no def found
                        } else {
                            // It's an inline or file-based definition node already
                            ModuleNodeId::new(child_id)
                        };

                        // Check if the child module (definition) is effectively public
                        // We need its effective visibility. Reuse logic from is_accessible's start.
                        let effective_vis = self.get_effective_visibility(child_mod_id_to_check);

                        if effective_vis.is_some_and(|vis| vis.is_pub()) {
                            // If public and not visited, add to queue
                            if visited.insert(child_mod_id_to_check) {
                                // Use definition ID for visited set
                                let mut new_path = current_path.clone();
                                // Use the name from the actual module node (could be decl or defn)
                                new_path.push(child_module_node.name.clone());
                                queue.push_back((child_mod_id_to_check, new_path));
                                // Enqueue definition ID
                            }
                        }
                    }
                }
            }
        }

        // Item not found via any public path
        Err(ModuleTreeError::ItemNotPubliclyAccessible(item_id)) // Return Err
    }

    /// Helper to get parent module ID (using existing ModuleTree fields)
    fn get_parent_module_id(&self, module_id: ModuleNodeId) -> Option<ModuleNodeId> {
        self.tree_relations
            .iter()
            .find_map(|r| {
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
            .or_else(|| {
                self.tree_relations.iter().find_map(|r_decl| {
                    if r_decl.relation().target == GraphId::Node(module_id.into_inner())
                        && r_decl.relation().kind == RelationKind::ResolvesToDefinition
                    {
                        self.tree_relations.iter().find_map(|r_cont| {
                            if r_decl.relation().source == r_cont.relation().target
                                && r_cont.relation().kind == RelationKind::Contains
                            {
                                match r_cont.relation().source {
                                    GraphId::Node(id) => Some(ModuleNodeId::new(id)),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
            })
    }

    /// Determines the effective visibility of a module definition.
    /// For inline modules or the root, it's the stored visibility.
    /// For file-based modules, it's the visibility of the corresponding declaration.
    fn get_effective_visibility(&self, module_def_id: ModuleNodeId) -> Option<&VisibilityKind> {
        // Return Option<&VisibilityKind>
        let module_node = self.modules.get(&module_def_id)?;

        if module_node.is_inline() || module_def_id == self.root {
            // Return a reference directly
            Some(&module_node.visibility)
        } else {
            // File-based module (not root), find declaration visibility
            let decl_id_opt = self.tree_relations.iter().find_map(|tr| {
                let rel = tr.relation();
                if rel.target == GraphId::Node(module_def_id.into_inner())
                    && rel.kind == RelationKind::ResolvesToDefinition
                {
                    match rel.source {
                        GraphId::Node(id) => Some(id),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            decl_id_opt
                .and_then(|decl_id| self.modules.get(&ModuleNodeId::new(decl_id)))
                .map(|decl_node| &decl_node.visibility) // Return reference from decl_node
                .or({
                    // If declaration not found (e.g., unlinked), use definition's visibility
                    Some(&module_node.visibility) // Return reference from module_node
                })
        }
    }

    /// Logs the details of an accessibility check using the provided context.
    fn log_access(
        &self,               // Keep &self if needed for other lookups, otherwise remove
        context: &AccLogCtx, // Pass context by reference
        step: &str,          // Description of the check step
        result: bool,
    ) {
        // Use debug! macro with the specific target
        debug!(target: LOG_TARGET_VIS,
            "{} {} -> {} | Target Vis: {} | Step: {} | Result: {}",
            "Accessibility Check:".bold(),
            context.source_name.yellow(), // Get name from context
            context.target_name.blue(),   // Get name from context
            context.effective_vis.map(|v| format!("{:?}", v).magenta()).unwrap_or_else(|| "NotFound".red().bold()), // Get visibility from context
            step.white().italic(),
            if result { "Accessible".green().bold() } else { "Inaccessible".red().bold() }
        );
    }

    /// Visibility check using existing types
    pub fn is_accessible(&self, source: ModuleNodeId, target: ModuleNodeId) -> bool {
        // 1. Get the target definition node from the map
        // --- Early Exit if Target Not Found ---
        if !self.modules.contains_key(&target) {
            // Create a temporary context just for this log message
            let log_ctx = AccLogCtx::new(source, target, None, self);
            self.log_access(&log_ctx, "Target Module Not Found", false);
            return false;
        }
        // We know target exists now, safe to unwrap later if needed, but prefer get
        let target_defn_node = self.modules.get(&target).unwrap(); // Safe unwrap

        // --- Determine Effective Visibility ---
        let effective_vis = if target_defn_node.is_inline() || target == self.root {
            // For inline modules or the crate root, the stored visibility is the effective one
            target_defn_node.visibility()
        } else {
            // For file-based modules (that aren't the root), find the corresponding declaration
            let target_defn_id = target_defn_node.id();
            let decl_id_opt = self.tree_relations.iter().find_map(|tr| {
                let rel = tr.relation();
                // CORRECTED LOOKUP LOGIC: Checks if target is Definition and source is Declaration
                if rel.target == GraphId::Node(target_defn_id) // Expects Decl -> Defn
                    && rel.kind == RelationKind::ResolvesToDefinition
                {
                    match rel.source {
                        // Source should be Decl ID
                        GraphId::Node(id) => Some(id),
                        _ => None, // Should not happen for this relation kind
                    }
                } else {
                    None
                }
            });

            match decl_id_opt {
                Some(decl_id) => {
                    // Found the declaration, get its visibility
                    self.modules.get(&ModuleNodeId::new(decl_id))
                        .map(|decl_node| decl_node.visibility())
                        .unwrap_or_else(|| {
                            // Should not happen if tree is consistent, but default to Inherited if decl node missing
                            log::warn!(target: LOG_TARGET_VIS, "Declaration node {} not found for definition {}", decl_id, target_defn_id);
                            VisibilityKind::Inherited // Default to Inherited if decl node missing
                        })
                }
                None => {
                    // No declaration found (e.g., unlinked module file). Treat as private/inherited.
                    // This aligns with how unlinked files behave.
                    target_defn_node.visibility() // Use the definition's (likely Inherited) visibility
                }
            }
        };

        // --- Create Log Context ---
        // Pass Some(&effective_vis) which is Option<&VisibilityKind>
        let log_ctx = AccLogCtx::new(source, target, Some(&effective_vis), self);

        // --- Perform Accessibility Check ---
        let result = match effective_vis {
            VisibilityKind::Public => {
                self.log_access(&log_ctx, "Public Visibility", true);
                true
            }
            VisibilityKind::Crate => {
                let accessible = true; // Always true within the same tree/crate
                self.log_access(&log_ctx, "Crate Visibility", accessible);
                accessible
            }
            VisibilityKind::Restricted(ref restricted_path_vec) => {
                let restriction_path = match NodePath::try_from(restricted_path_vec.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        self.log_access(&log_ctx, "Restricted Visibility (Invalid Path)", false);
                        return false; // Invalid restriction path
                    }
                };
                let restriction_module_id = match self.path_index.get(&restriction_path) {
                    Some(id) => ModuleNodeId::new(*id),
                    None => {
                        self.log_access(&log_ctx, "Restricted Visibility (Path Not Found)", false);
                        return false; // Restriction path doesn't exist in the index
                    }
                };

                // Check if the source module *is* the restriction module
                if source == restriction_module_id {
                    self.log_access(
                        &log_ctx,
                        "Restricted Visibility (Source is Restriction)",
                        true,
                    );
                    return true;
                }

                // Check if the source module is a descendant of the restriction module
                let mut current_ancestor = self.get_parent_module_id(source);
                while let Some(ancestor_id) = current_ancestor {
                    // Keep this specific debug log inside the loop for ancestor tracing
                    debug!(target: LOG_TARGET_VIS, "  {} Checking ancestor: {} ({}) against restriction: {} ({})",
                        "->".dimmed(), // Indentation marker
                        self.modules.get(&ancestor_id).map(|m| m.name.as_str()).unwrap_or("?").yellow(), // Ancestor name yellow
                        ancestor_id.to_string().magenta(), // Ancestor ID magenta
                        self.modules.get(&restriction_module_id).map(|m| m.name.as_str()).unwrap_or("?").blue(), // Restriction name blue
                        restriction_module_id.to_string().magenta() // Restriction ID magenta
                    );
                    if ancestor_id == restriction_module_id {
                        self.log_access(&log_ctx, "Restricted Visibility (Ancestor Match)", true);
                        return true; // Found restriction module in ancestors
                    }
                    if ancestor_id == self.root {
                        break; // Reached crate root without finding it
                    }
                    current_ancestor = self.get_parent_module_id(ancestor_id);
                }
                let accessible = false; // Not the module itself or a descendant
                self.log_access(
                    &log_ctx,
                    "Restricted Visibility (Final - No Ancestor Match)",
                    accessible,
                );
                accessible
            }
            VisibilityKind::Inherited => {
                // Inherited means private to the defining module.
                // Access is allowed if the source *is* the target's parent,
                // or if the source *is* the target itself.
                let target_parent = self.get_parent_module_id(target);
                let accessible = source == target || Some(source) == target_parent;
                self.log_access(&log_ctx, "Inherited Visibility", accessible);
                accessible
            }
        };
        result // Return the final calculated result
    }

    // Removed unused get_module_path_vec and get_root_path methods
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
