pub use colored::Colorize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{self, PathBuf},
}; // Keep HashSet and VecDeque

use crate::utils::LogStyle;
use crate::utils::LogStyleDebug;
use log::debug; // Import the debug macro
use ploke_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::parser::nodes::NodePath;
use crate::{error::SynParserError, parser::nodes::extract_path_attr_from_node}; // Ensure NodePath is imported

use super::{
    nodes::{GraphId, GraphNode, ImportNode, ModuleNode, ModuleNodeId}, // Add GraphId
    relations::{Relation, RelationKind},                               // Remove GraphId
    types::VisibilityKind,
    CodeGraph,
};

#[cfg(test)]
pub mod test_interface {
    use ploke_core::NodeId;

    use super::{ModuleTree, ModuleTreeError};
    use crate::CodeGraph;

    impl ModuleTree {
        pub fn test_shortest_public_path(
            &self,
            item_id: NodeId,
            graph: &CodeGraph,
        ) -> Result<Vec<String>, ModuleTreeError> {
            self.shortest_public_path(item_id, graph)
        }
    }
}

const LOG_TARGET_VIS: &str = "mod_tree_vis"; // Define log target for visibility checks
const LOG_TARGET_BUILD: &str = "mod_tree_build"; // Define log target for build checks
const LOG_TARGET_PATH_ATTR: &str = "mod_tree_path"; // Define log target for path attribute handling

#[derive(Debug, Clone)]
pub struct ModuleTree {
    // ModuleNodeId of the root file-level module, e.g. `main.rs`, `lib.rs`, used to initialize the
    // ModuleTree.
    root: ModuleNodeId,
    root_file: PathBuf,
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
    /// re-export index for faster lookup during visibility resolution.
    reexport_index: HashMap<NodePath, NodeId>,
    found_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    // Option for `take`
    pending_path_attrs: Option<Vec<ModuleNodeId>>,
}

/// Indicates a file-level module whose path has been resolved from a declaration that has the
/// `#[path]` attribute, e.g.
/// ```rust
/// // somewhere in project, e.g. project/src/my_module.rs
/// #[path = "path/to/file.rs"]
/// pub mod path_attr_mod;
///
/// // In project/src/path/to/file.rs
/// pub(crate) struct HiddenStruct;
/// ```
/// The module represented by the file `path/to/file.rs`, here containing `HiddenStruct`, will have
/// its `ModuleNode { path: .. }` field resolved to ``
struct ResolvedModule {
    original_path: NodePath,     // The declared path (e.g. "path::to::file")
    filesystem_path: PathBuf,    // The resolved path from #[path] attribute
    source_span: (usize, usize), // Where the module was declared
    is_path_override: bool,      // Whether this used #[path]
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
    #[error("Duplicate definition module_id '{module_id}' found in module tree. Existing path attribute: {existing_path}, Conflicting path attribute: {conflicting_path}")]
    DuplicatePathAttribute {
        module_id: ModuleNodeId,
        existing_path: PathBuf,
        conflicting_path: PathBuf,
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

    #[error("Graph ID conversion error: {0}")]
    GraphIdConversion(#[from] super::nodes::GraphIdConversionError), // Add #[from] for automatic conversion

    #[error("Node error: {0}")]
    NodeError(#[from] super::nodes::NodeError), // Add #[from] for NodeError

    #[error("Syn parser error: {0}")]
    SynParserError(Box<SynParserError>), // REMOVE #[from]
    //
    #[error("Could not determine parent directory for file path: {0}")]
    FilePathMissingParent(PathBuf), // Store the problematic path
    #[error("Root module {0} is not file-based, which is required for path resolution.")]
    RootModuleNotFileBased(ModuleNodeId),

    // --- NEW VARIANT ---
    #[error("Conflicting re-export path '{path}' detected. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    ConflictingReExportPath {
        path: NodePath,
        existing_id: NodeId,
        conflicting_id: NodeId,
    },

    // --- NEW VARIANT ---
    #[error("Re-export chain starting from {start_node_id} exceeded maximum depth (32). Potential cycle or excessively deep re-export.")]
    ReExportChainTooLong { start_node_id: NodeId },

    #[error("Implement me!")]
    UnresolvedPathAttr(Box<ModuleTreeError>), // Placeholder, fill in with contextual information

    #[error("ModuleId not found in ModuleTree.modules: {0}")]
    ModuleNotFound(ModuleNodeId),

    // --- NEW VARIANTS for process_path_attributes ---
    #[error("Duplicate module definitions found for path attribute target: {0}")]
    DuplicateDefinition(String), // Store detailed message
    #[error("Module definition not found for path attribute target: {0}")]
    ModuleDefinitionNotFound(String), // Store detailed message
}

// Manual implementation to satisfy the `?` operator
impl From<SynParserError> for ModuleTreeError {
    fn from(err: SynParserError) -> Self {
        ModuleTreeError::SynParserError(Box::new(err))
    }
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

    pub fn new_from_root(root: &ModuleNode) -> Result<Self, ModuleTreeError> {
        let root_id = ModuleNodeId::new(root.id());
        let root_file = root
            .file_path()
            .ok_or(ModuleTreeError::RootModuleNotFileBased(root_id))?;
        Ok(Self {
            root: root_id,
            root_file: root_file.to_path_buf(),
            modules: HashMap::new(),
            pending_imports: vec![],
            pending_exports: vec![],
            path_index: HashMap::new(),
            decl_index: HashMap::new(),
            tree_relations: vec![],
            reexport_index: HashMap::new(),
            found_path_attrs: HashMap::new(),
            pending_path_attrs: Some(Vec::new()),
        })
        // Should never happen, but might want to handle this sometime
        // // Initialize path attributes from root if present
        // if let Some(path_val) = extract_attribute_value(&root.attributes, "path") {
        //     if let Some(root_file) = root.file_path() {
        //         tree.path_attributes.insert(
        //             root_id,
        //             root_file.parent().unwrap().join(path_val).normalize(),
        //         );
        //     }
        // }
    }

    pub fn get_root_module(&self) -> Result<&ModuleNode, ModuleTreeError> {
        self.modules
            .get(&self.root)
            .ok_or_else(|| ModuleTreeError::ContainingModuleNotFound(*self.root.as_inner()))
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
        self.log_module_insert(&module, module_id);

        // Store path attribute if present

        if module.has_path_attr() {
            self.pending_path_attrs
                .as_mut()
                .expect("Invariant") // Safe unwrap, but should still add error for
                .push(module_id); // clarity. This should be invariant, however.
        }

        let dup_node = self.modules.insert(module_id, module); // module is moved here
        if let Some(dup) = dup_node {
            self.log_duplicate(&dup);
            return Err(ModuleTreeError::DuplicateModuleId(Box::new(dup)));
        }

        Ok(())
    }

    pub fn resolve_pending_path_attrs(&mut self) -> Result<(), ModuleTreeError> {
        let module_ids = match self.pending_path_attrs.take() {
            Some(pending_ids) => pending_ids,
            // Return early if there are no `#path` attributes
            // Info logging should go here
            None => return Ok(()),
        };
        for module_id in module_ids {
            let base_dir = match self.find_declaring_file_dir(module_id) {
                Ok(dir) => dir,
                Err(e) => {
                    log_path_attr_not_found(module_id);
                    return Err(ModuleTreeError::UnresolvedPathAttr(Box::new(e)));
                    // implement new error type with more context (placeholder)
                }
            };
            let module = self.get_module_checked(&module_id)?; // Should abort on ModuleNotFound
            let path_val = extract_path_attr_from_node(module).take().unwrap();

            let resolved = base_dir.join(path_val).normalize();
            let debug_mod_clone: ModuleNode = module.to_owned();
            let debug_path_val = path_val.to_owned();
            let debug_resolved = resolved.clone();
            let path_ctx = PathLogCtx::new(
                &debug_mod_clone,
                Some(&debug_path_val),
                Some(&debug_resolved),
            );
            self.log_path(&path_ctx, "Resolve pending", None);
            match self.found_path_attrs.entry(module_id) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Path already exists
                    let existing_path = entry.get().clone();
                    // TODO:
                    // In this case the base_dir + path_val points to the same target of the
                    // `#[path]` attribute. This is valid rust, and we need to handle this better.
                    // For now we will add extra logging and remove the error
                    self.log_path(&path_ctx, "PathAttr", None);
                    // Previous error handling (this is actually valid rust)
                    return Err(ModuleTreeError::DuplicatePathAttribute {
                        module_id,
                        existing_path,
                        conflicting_path: resolved,
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Id is free, insert it
                    entry.insert(resolved);
                }
            };
        }
        Ok(())
    }
    pub fn get_module_checked(
        &self,
        module_id: &ModuleNodeId,
    ) -> Result<&ModuleNode, ModuleTreeError> {
        self.modules
            .get(module_id)
            .ok_or(ModuleTreeError::ModuleNotFound(*module_id))
    }

    /// Links modules (declaration/definition) syntactically by building relations between them and
    /// adding the relations to the module tree's `tree_relation` field.
    /// Builds 'ResolvesToDefinition' relations between module declarations and their file-based definitions.
    /// Assumes the `path_index` and `decl_index` have been populated correctly by `add_module`.
    /// Returns `Ok(())` on complete success.
    /// Returns `Err(ModuleTreeError::FoundUnlinkedModules)` if only unlinked modules are found.
    /// Returns other `Err(ModuleTreeError)` variants on fatal errors (e.g., path validation).
    pub(crate) fn link_mods_syntactic(
        &mut self,
        modules: &[ModuleNode],
    ) -> Result<(), ModuleTreeError> {
        // Return Ok(()) or Err(ModuleTreeError)
        let mut new_relations: Vec<TreeRelation> = Vec::new();
        let mut collected_unlinked: Vec<UnlinkedModuleInfo> = Vec::new(); // Store only unlinked info
        let root_id = self.root();

        for module in modules
            .iter()
            .filter(|m| m.is_file_based() && m.id() != *root_id.as_inner())
        {
            // This is ["crate", "renamed_path", "actual_file"] for the file node
            let defn_path = module.defn_path();

            // Log the attempt to find a declaration matching the *file's* definition path
            self.log_path_resolution(module, defn_path, "Checking", Some("decl_index..."));

            match self.decl_index.get(defn_path.as_slice()) {
                Some(decl_id) => {
                    // Found declaration, create relation
                    self.log_path_resolution(
                        module,
                        defn_path,
                        "Linked",
                        Some(&format!("to decl {}", decl_id)),
                    );
                    let resolves_to_rel = Relation {
                        source: GraphId::Node(*decl_id),    // Declaration Node
                        target: GraphId::Node(module.id()), // Definition Node (the file-based one)
                        kind: RelationKind::ResolvesToDefinition,
                    };
                    new_relations.push(resolves_to_rel.into());
                }
                None => {
                    // No declaration found matching the file's definition path.
                    self.log_unlinked_module(module, defn_path);
                    let node_path = NodePath::try_from(defn_path.clone()) // Use the file's defn_path
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

    // TODO: Rename/refactored
    // This function has a misleading name. Currently it is getting all relations,
    // not just `RelationKind::Contains`
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
            // Handle path attribute if present
            let effective_path =
                if let Some(_custom_path) = self.resolve_custom_path(current_mod_id) {
                    // For modules with #[path], we need to check if the item exists at that location
                    if let Some(module_node) = self.modules.get(&current_mod_id) {
                        if let Some(items) = module_node.items() {
                            if items.contains(&item_id) {
                                return Ok(current_path);
                            }
                        }
                    }
                    continue; // Skip further processing for #[path] modules
                } else {
                    current_path
                };

            // Check both direct definitions and re-exports
            for rel in self
                .tree_relations
                .iter()
                .filter(|tr| tr.relation().source == GraphId::Node(current_mod_id.into_inner()))
            {
                match rel.relation().kind {
                    RelationKind::Contains if rel.relation().target == GraphId::Node(item_id) => {
                        // Direct containment case
                        let item_node = graph.find_node_unique(item_id)?;
                        if item_node.visibility().is_pub() {
                            return Ok(effective_path);
                        }
                    }
                    // And in shortest_public_path's ReExport case:
                    RelationKind::ReExport => {
                        let target_id: NodeId = rel.relation().target.try_into()?;

                        // Check for cycles by limiting chain length
                        let mut chain_visited = HashSet::new();
                        let mut current_chain_id = target_id;
                        let mut is_reexport_chain = false;

                        // Check chain with cycle detection
                        while chain_visited.insert(current_chain_id) {
                            if current_chain_id == item_id {
                                is_reexport_chain = true;
                                break;
                            }

                            if let Some(next_rel) = self.tree_relations.iter().find(|tr| {
                                tr.relation().kind == RelationKind::ReExport
                                    && tr.relation().source == GraphId::Node(current_chain_id)
                            }) {
                                current_chain_id = next_rel.relation().target.try_into()?;
                            } else {
                                break;
                            }

                            // Prevent infinite loops from extremely long chains
                            if chain_visited.len() > 32 {
                                // Return the new error variant
                                return Err(ModuleTreeError::ReExportChainTooLong {
                                    start_node_id: target_id, // The ID where the chain started
                                });
                            }
                        }

                        if is_reexport_chain {
                            if let Some(reexport_name) =
                                self.get_reexport_name(current_mod_id, target_id)
                            {
                                let mut reexport_path = effective_path.clone();
                                reexport_path.push(reexport_name);

                                // For chains, recursively build path to original
                                if target_id != item_id {
                                    if let Ok(/* mut */ original_path) =
                                        self.shortest_public_path(target_id, graph)
                                    {
                                        reexport_path.extend(original_path);
                                    }
                                }

                                return Ok(reexport_path);
                            }
                        }
                    }
                    _ => {}
                }
            }

            // --- Neighbor (Public Child) Exploration ---
            let child_relations = self.tree_relations.iter().filter(|tr| {
                let rel = tr.relation();
                rel.source == GraphId::Node(current_mod_id.into_inner())
                    && rel.kind == RelationKind::Contains
            });

            for child_rel in child_relations {
                let child_id: ModuleNodeId = child_rel.relation().target.try_into()?;
                // Get the module node (declaration or definition) linked by Contains
                if let Ok(child_module_node) = self.get_contained_mod(child_id) {
                    // Determine the ID of the actual module definition (handling declarations)
                    let definition_id = if child_module_node.is_declaration() {
                        self.find_definition_for_declaration(child_id)
                                .unwrap_or_else(|| {
                                    // Log fallback case
                                    log::warn!(target: LOG_TARGET_BUILD, "SPP: Could not find definition for declaration {}, falling back to using declaration ID itself.", child_id);
                                    child_id
                                })
                    } else {
                        child_id // It's already the definition
                    };

                    // Check visibility and enqueue if public and unvisited
                    if let Some(id_to_enqueue) = self
                        .get_effective_visibility(definition_id)
                        .filter(|vis| vis.is_pub()) // Keep only if public
                        .and_then(|_vis| {
                            // If public...
                            if visited.insert(definition_id) {
                                // ...and not visited...
                                Some(definition_id) // ...return the ID to enqueue.
                            } else {
                                None // Already visited
                            }
                        })
                    {
                        // If we should enqueue...
                        let mut new_path = effective_path.clone();
                        // Use the name from the original child node (decl or defn)
                        new_path.push(child_module_node.name.clone());
                        queue.push_back((id_to_enqueue, new_path));
                    }
                }
            }
        }

        // Item not found via any public path
        Err(ModuleTreeError::ItemNotPubliclyAccessible(item_id)) // Return Err
    }
    // Helper to check if an item is part of a re-export chain leading to our target
    // NOTE: Why is this currently unused? I'm fairly sure we were using it somewhere...
    fn is_part_of_reexport_chain(
        &self,
        start_id: NodeId,
        target_id: NodeId,
    ) -> Result<bool, ModuleTreeError> {
        let mut current_id = start_id;
        let mut visited = HashSet::new();

        while visited.insert(current_id) {
            // Check if current_id re-exports our target
            if let Some(reexport_rel) = self.tree_relations.iter().find(|tr| {
                tr.relation().kind == RelationKind::ReExport
                    && tr.relation().source == GraphId::Node(current_id)
                    && tr.relation().target == GraphId::Node(target_id)
            }) {
                return Ok(true);
            }

            // Move to next re-export in chain
            if let Some(next_rel) = self.tree_relations.iter().find(|tr| {
                tr.relation().kind == RelationKind::ReExport
                    && tr.relation().target == GraphId::Node(current_id)
            }) {
                current_id = next_rel.relation().source.try_into()?;
            } else {
                break;
            }
        }

        Ok(false)
    }

    // TODO: Make a parallellized version with rayon
    // fn process_export_rels(&self, graph: &CodeGraph) -> Result<Vec<TreeRelation>, ModuleTreeError> {
    //     todo!()
    // }
    // or
    pub fn process_export_rels(&mut self, graph: &CodeGraph) -> Result<(), ModuleTreeError> {
        for export in &self.pending_exports {
            let source_mod_id = export.module_node_id();
            let export_node = export.export_node();

            // Create relation
            let relation = Relation {
                source: GraphId::Node(*source_mod_id.as_inner()),
                target: GraphId::Node(export_node.id),
                kind: RelationKind::ReExport,
            };
            self.tree_relations.push(relation.into());

            // Add to reexport_index
            if let Some(reexport_name) = export_node.path.last() {
                let mut reexport_path = graph.get_item_module_path(*source_mod_id.as_inner());
                // Check for renamed export path, e.g. `a::b::Struct as RenamedStruct`
                if export_node.is_renamed() {
                    // if renamed, use visible_name for path extension
                    // WARNING: Be careful when generating resolved ID not to use this `path` for
                    // NodeId of defining module.
                    // TODO: Keep a list of renamed modules specifically to track possible
                    // collisions.
                    reexport_path.push(export_node.visible_name.clone());
                } else {
                    // otherwise, use standard name
                    reexport_path.push(reexport_name.clone());
                }

                let node_path = NodePath::try_from(reexport_path)
                    .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;

                // Check for duplicate re-exports at the same path
                match self.reexport_index.entry(node_path.clone()) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        let existing_id = *entry.get();
                        if existing_id != export_node.id {
                            // Found a different NodeId already registered for this exact path.
                            return Err(ModuleTreeError::ConflictingReExportPath {
                                path: node_path, // Use the cloned path
                                existing_id,
                                conflicting_id: export_node.id,
                            });
                        }
                        // If it's the same ID, do nothing (idempotent)
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        // Path is free, insert the new re-export ID.
                        entry.insert(export_node.id);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn resolve_custom_path(&self, module_id: ModuleNodeId) -> Option<&PathBuf> {
        self.found_path_attrs.get(&module_id)
    }

    fn get_reexport_name(&self, module_id: ModuleNodeId, item_id: NodeId) -> Option<String> {
        self.pending_exports
            .iter()
            .find(|exp| exp.module_node_id() == module_id && exp.export_node().id == item_id)
            .and_then(|exp| exp.export_node().path.last().cloned())
    }

    fn get_contained_mod(&self, child_id: ModuleNodeId) -> Result<&ModuleNode, ModuleTreeError> {
        let child_module_node =
            self.modules
                .get(&child_id)
                .ok_or(ModuleTreeError::ContainingModuleNotFound(
                    *child_id.as_inner(),
                ))?;
        Ok(child_module_node)
    }

    /// Finds the NodeId of the definition module corresponding to a declaration module ID.
    fn find_definition_for_declaration(&self, decl_id: ModuleNodeId) -> Option<ModuleNodeId> {
        self.tree_relations.iter().find_map(|tr| {
            let rel = tr.relation();
            if rel.source == decl_id.to_graph_id() && rel.kind == RelationKind::ResolvesToDefinition
            {
                match rel.target {
                    GraphId::Node(defn_id) => Some(ModuleNodeId::new(defn_id)),
                    _ => None, // Should not happen for this relation kind
                }
            } else {
                None
            }
        })
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

    // Proposed new function signature and implementation
    fn find_declaring_file_dir(&self, module_id: ModuleNodeId) -> Result<PathBuf, ModuleTreeError> {
        let mut current_id = module_id;

        while let Some(current_node) = self.modules.get(&current_id) {
            // Get the current node, returning error if not found in the tree

            // Check if the current node is file-based
            if let Some(file_path) = current_node.file_path() {
                // Found a file-based ancestor. Get its parent directory.
                return file_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    // Return error if the file path has no parent (e.g., it's just "/")
                    .ok_or_else(|| ModuleTreeError::FilePathMissingParent(file_path.clone()));
            }

            // Check if we have reached the root *without* finding a file-based node
            if current_id == self.root {
                // This indicates an invalid state - the root must be file-based.
                return Err(ModuleTreeError::RootModuleNotFileBased(self.root));
            }

            // If not file-based and not the root, move up to the parent.
            // Return error if the parent link is missing (inconsistent tree).
            current_id = self.get_parent_module_id(current_id).ok_or_else(|| {
                // Log this specific case for debugging?
                log::error!(target: LOG_TARGET_BUILD, "Inconsistent ModuleTree: Parent not found for module {} during file dir search.", current_id);
                ModuleTreeError::ContainingModuleNotFound(*current_id.as_inner())
                // Re-use existing error
            })?;
        }
        Err(ModuleTreeError::ContainingModuleNotFound(
            *current_id.as_inner(),
        ))
    }
    /// Resolves a path string (either relative or absolute) relative to a base directory.
    ///
    /// # Arguments
    /// * `base_dir` - The base directory to resolve relative paths from
    /// * `relative_or_absolute` - The path string to resolve (can be relative like "../foo" or absolute "/bar")
    ///
    /// # Returns
    /// The resolved absolute PathBuf
    pub fn resolve_relative_path(
        base_dir: &std::path::Path,
        relative_or_absolute: &str,
    ) -> PathBuf {
        let path = PathBuf::from(relative_or_absolute);

        if path.is_absolute() {
            // If absolute, return as-is (normalized)
            path
        } else {
            // If relative, join with base_dir and normalize
            base_dir.join(path).normalize()
        }
    }

    pub fn resolve_path_for_module(
        &self,
        module_id: ModuleNodeId,
        path: &str,
    ) -> Result<PathBuf, ModuleTreeError> {
        let base_dir = self.find_declaring_file_dir(module_id)?;
        Ok(Self::resolve_relative_path(&base_dir, path))
    }
    pub fn process_path_attributes(&mut self) -> Result<(), ModuleTreeError> {
        for (decl_module_id, resolved_path) in self.found_path_attrs.iter() {
            let ctx = PathProcessingContext {
                module_id: *decl_module_id,
                module_name: "?", // Temporary placeholder
                attr_value: None,
                resolved_path: Some(resolved_path),
            };
            let decl_module_node = self.modules.get(decl_module_id).ok_or_else(|| {
                self.log_path_processing(&ctx, "Error", Some("Module not found"));
                ModuleTreeError::ContainingModuleNotFound(*decl_module_id.as_inner())
            })?;

            // Update context with module info
            let ctx = PathProcessingContext {
                module_name: &decl_module_node.name,
                attr_value: extract_path_attr_from_node(decl_module_node),
                ..ctx
            };
            self.log_path_processing(&ctx, "Processing", None);

            let mut targets_iter = self.modules.values().filter(|m| {
                m.is_file_based() && m.file_path().is_some_and(|fp| fp == resolved_path)
            });
            let target_defn = targets_iter.next();

            match target_defn {
                Some(target_defn_node) => {
                    // 2. Found the target file definition node. Create the relation.
                    let target_defn_id = target_defn_node.id();
                    let relation = Relation {
                        source: GraphId::Node(decl_module_id.into_inner()),
                        target: GraphId::Node(target_defn_id),
                        kind: RelationKind::CustomPath,
                    };
                    self.log_relation(
                        relation,
                        Some(&format!(
                            "Linking decl {} to file defn {}",
                            decl_module_id, target_defn_id
                        )),
                    );
                    // NOTE: Edge Case
                    // It is actually valid to have a case of duplicate definitions. We'll
                    // need to consider how to handle this case, since it is possible to have an
                    // inline module with the `#[path]` attribute that contains items which shadow
                    // the items in the linked file, in which case the shadowed items are ignored.
                    // For now, just throw error.
                    if let Some(dup) = targets_iter.next() {
                        return Err(ModuleTreeError::DuplicateDefinition(format!(
                        "Duplicate module definition for path attribute target '{}'  {}:\ndeclaration: {:#?}\nfirst: {:#?},\nsecond: {:#?}",
                            decl_module_node.id,
                        resolved_path.display(),
                            &decl_module_node,
                        &decl_module_id,
                        &target_defn_node,
                    )));
                    }
                    self.tree_relations.push(relation.into());
                }
                None => {
                    // 3. Handle case where the target file node wasn't found.
                    // This indicates an inconsistency - the path resolved, but thecorresponding
                    // module node isn't in the map.
                    self.log_module_error(
                        *decl_module_id,
                        &format!(
                            "Path attribute target file not found in modules map:  {}",
                            resolved_path.display(),
                        ),
                    );
                    // Return an error because the tree is inconsistent
                    // TODO: Consider a more specific error variant if needed.
                    return Err(ModuleTreeError::ModuleDefinitionNotFound(format!(
                        "Module definition for path attribute target '{}' not found for declaration {}:\n{:#?}",
                        resolved_path.display(),
                        decl_module_node.id,
                            &decl_module_node,
                    )));
                }
            }
        }
        Ok(())
    }

    /// Logs the details of path attribute processing using the provided context.
    fn log_path(&self, context: &PathLogCtx, step: &str, result: Option<String>) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {} {} | Path: {} | Attr: {} | Resolved: {} | {} | {}",
            "PathAttr".log_header(),
            context.module_name.log_name(),
            format!("({})", context.module_id).log_id(),
            format!("{:?}", context.module_path).log_path(),
            context.attr_value.unwrap_or("-").log_path(),
            context.resolved_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string())
                .log_path(),
            step.log_name(),  // Using name color for step for visual distinction
            result.unwrap_or_else(|| "✓".to_string()).log_vis()  // Using vis color for result
        );
    }

    fn log_module_insert(&self, module: &ModuleNode, id: ModuleNodeId) {
        debug!(target: LOG_TARGET_BUILD, "{} {} {} | {}",
            "Insert".log_header(),
            module.name.log_name(),
            format!("({})", id).log_id(),
            module.visibility.log_vis_debug()
        );
    }

    fn log_duplicate(&self, module: &ModuleNode) {
        debug!(target: LOG_TARGET_BUILD, "{} {} {}",
            "Duplicate ID".log_error(),
            module.name.log_name(),
            format!("({})", module.id).log_id()
        );
    }

    fn log_path_attr(&self, module: &ModuleNode, raw_path: &str, resolved: &path::Path) {
        debug!(target: LOG_TARGET_PATH_ATTR, "{} {} | {} → {}",
            "PathAttr".log_header(),
            module.name.log_name(),
            raw_path.log_path(),
            resolved.display().to_string().log_path()
        );
    }

    fn log_path_resolution(
        &self,
        module: &ModuleNode,
        path: &[String],
        status: &str,
        details: Option<&str>,
    ) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {} {} | Path: {} | {} {}",
            "PathResolve".log_header(),
            module.name.log_name(),
            format!("({})", module.id).log_id(),
            format!("{:?}", path).log_path(),
            status.log_vis(),
            details.unwrap_or("").log_name()
        );
    }

    fn log_unlinked_module(&self, module: &ModuleNode, path: &[String]) {
        self.log_path_resolution(module, path, "Unlinked", Some("No declaration found"));
    }

    fn log_path_processing(&self, ctx: &PathProcessingContext, step: &str, result: Option<&str>) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {} {} | Attr: {} | Resolved: {} | {} | {}",
            "PathAttr".log_header(),
            ctx.module_name.log_name(),
            format!("({})", ctx.module_id).log_id(),
            ctx.attr_value.unwrap_or("-").log_path(),
            ctx.resolved_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string())
                .log_path(),
            step.log_name(),
            result.unwrap_or("✓").log_vis()
        );
    }
    fn log_relation(&self, relation: Relation, note: Option<&str>) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {}: {} → {} {}",
            "Relation".log_header(),
            format!("{:?}", relation.kind).log_name(),
            relation.source.to_string().log_id(),
            relation.target.to_string().log_id(),
            note.map(|n| format!("({})", n)).unwrap_or_default().log_vis()
        );
    }

    fn log_module_error(&self, module_id: ModuleNodeId, message: &str) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {} {} | {}",
            "Error".log_error(),
            "module".log_name(),
            format!("({})", module_id).log_id(),
            message.log_vis()
        );
    }

    // Removed unused get_module_path_vec and get_root_path methods
}

fn log_path_attr_not_found(module_id: ModuleNodeId) {
    log::error!(target: LOG_TARGET_BUILD, "Inconsistent ModuleTree: Parent not found for module {} processed with path attribute during file dir search.", module_id);
}

// Extension trait for Path normalization
trait PathNormalize {
    fn normalize(&self) -> PathBuf;
}

impl PathNormalize for std::path::Path {
    fn normalize(&self) -> PathBuf {
        let mut components = Vec::new();

        for component in self.components() {
            match component {
                std::path::Component::ParentDir => {
                    if components
                        .last()
                        .map(|c| c != &std::path::Component::RootDir)
                        .unwrap_or(false)
                    {
                        components.pop();
                    }
                }
                std::path::Component::CurDir => continue,
                _ => components.push(component),
            }
        }

        components.iter().collect()
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

/// Helper struct to hold context for path attribute logging.
struct PathLogCtx<'a> {
    module_id: ModuleNodeId,
    module_name: &'a str,
    module_path: &'a [String], // Use slice for efficiency
    attr_value: Option<&'a str>,
    resolved_path: Option<&'a PathBuf>,
}

struct PathProcessingContext<'a> {
    module_id: ModuleNodeId,
    module_name: &'a str,
    attr_value: Option<&'a str>,
    resolved_path: Option<&'a PathBuf>,
}

impl<'a> PathLogCtx<'a> {
    /// Creates a new context for logging path attribute processing.
    fn new(
        module_node: &'a ModuleNode,
        attr_value: Option<&'a str>,
        resolved_path: Option<&'a PathBuf>,
    ) -> Self {
        Self {
            module_id: ModuleNodeId::new(module_node.id()),
            module_name: &module_node.name,
            module_path: &module_node.path,
            attr_value,
            resolved_path,
        }
    }
}
