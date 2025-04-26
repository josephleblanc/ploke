use crate::{parser::graph::GraphAccess, utils::logging::LOG_TARGET_BFS};
pub use colored::Colorize;
use log::debug; // Import the debug macro
use ploke_core::NodeId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    default,
    path::PathBuf,
};

#[allow(unused_imports)]
use std::collections::VecDeque;

use crate::{
    error::SynParserError,
    parser::{
        nodes::{
            self, extract_path_attr_from_node, GraphId, GraphNode, ImportNode, ModuleNode,
            ModuleNodeId, NodePath,
        },
        relations::{Relation, RelationKind},
        types::VisibilityKind,
        ParsedCodeGraph,
    },
    utils::{
        logging::{LogDataStructure, PathProcessingContext},
        AccLogCtx, LogStyle, LogStyleDebug, LOG_TARGET_MOD_TREE_BUILD, LOG_TARGET_PATH_ATTR,
        LOG_TARGET_VIS,
    },
};

#[cfg(test)]
pub mod test_interface {
    use std::collections::HashMap;

    use ploke_core::NodeId;

    use super::{ModuleTree, ModuleTreeError, TreeRelation};
    use crate::parser::{nodes::NodePath, ParsedCodeGraph};

    impl ModuleTree {
        pub fn test_shortest_public_path(
            &self,
            item_id: NodeId,
            graph: &ParsedCodeGraph,
        ) -> Result<Vec<String>, ModuleTreeError> {
            self.shortest_public_path(item_id, graph)
        }
    }
}

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
    /// Temporary storage for unresolved exports (e.g. `pub use` statements).
    /// Wrapped in Option to allow taking ownership without cloning in `process_export_rels`.
    pending_exports: Option<Vec<PendingExport>>,
    /// Reverse path indexing to find NodeId on a given path
    /// HashMap appropriate for many -> few possible mapping
    /// Contains all `NodeId` items except module declarations due to
    /// path collision with defining module.
    path_index: HashMap<NodePath, NodeId>,
    /// Maps declaration module IDs with `#[path]` attributes pointing outside the crate's
    /// `src` directory to the resolved absolute external path. These paths do not have
    /// corresponding `ModuleNode` definitions within the analyzed crate context.
    external_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    /// Separate HashMap for module declarations.
    /// Reverse lookup, but can't be in the same HashMap as the modules that define them, since
    /// they both have the same `path`. This should be the only case in which two items have the
    /// same path.
    decl_index: HashMap<NodePath, NodeId>,
    tree_relations: Vec<TreeRelation>,
    /// re-export index for faster lookup during visibility resolution.
    reexport_index: HashMap<NodePath, NodeId>,
    /// Stores resolved absolute paths for modules declared with `#[path]` attributes
    /// that point to files *within* the crate's `src` directory.
    /// Key: ID of the declaration module (`mod foo;`).
    /// Value: Resolved absolute `PathBuf` of the target file.
    found_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    /// Temporarily stores the IDs of module declarations that have a `#[path]` attribute.
    /// Used during the initial tree building phase before paths are fully resolved.
    /// Wrapped in `Option` to allow taking ownership via `take()` during processing.
    pending_path_attrs: Option<Vec<ModuleNodeId>>,

    /// Index mapping a source `GraphId` (Node or Type) to a list of indices
    /// into the `tree_relations` vector where that ID appears as the source.
    /// Used for efficient lookup of outgoing relations.
    relations_by_source: HashMap<GraphId, Vec<usize>>,
    /// Index mapping a target `GraphId` (Node or Type) to a list of indices
    /// into the `tree_relations` vector where that ID appears as the target.
    /// Used for efficient lookup of incoming relations.
    relations_by_target: HashMap<GraphId, Vec<usize>>,
}

/// Indicates a file-level module whose path has been resolved from a declaration that has the
/// `#[path]` attribute, e.g.
/// ```rust,ignore
/// // somewhere in project, e.g. project/src/my_module.rs
/// #[path = "path/to/file.rs"]
/// pub mod path_attr_mod;
///
/// // In project/src/path/to/file.rs
/// pub(crate) struct HiddenStruct;
/// ```
/// The module represented by the file `path/to/file.rs`, here containing `HiddenStruct`, will have
/// its `ModuleNode { path: .. }` field resolved to ``
#[allow(dead_code)]
struct ResolvedModule {
    original_path: NodePath,     // The declared path (e.g. "path::to::file")
    filesystem_path: PathBuf,    // The resolved path from #[path] attribute
    source_span: (usize, usize), // Where the module was declared
    is_path_override: bool,      // Whether this used #[path]
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingImport {
    containing_mod_id: ModuleNodeId, // Keep private
    import_node: ImportNode,         // Keep private
}

impl PendingImport {
    pub(crate) fn from_import(import: ImportNode, containing_mod_id: NodeId) -> Self {
        // Make crate-visible if needed internally
        PendingImport {
            containing_mod_id: ModuleNodeId::new(containing_mod_id),
            import_node: import,
        }
    }

    /// Returns the ID of the module containing this pending import.
    pub fn containing_mod_id(&self) -> ModuleNodeId {
        self.containing_mod_id
    }

    /// Returns a reference to the `ImportNode` associated with this pending import.
    pub fn import_node(&self) -> &ImportNode {
        &self.import_node
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingExport {
    containing_mod_id: ModuleNodeId, // Keep private
    export_node: ImportNode,         // Keep private
}

impl PendingExport {
    #[allow(unused_variables)]
    pub(crate) fn from_export(export: ImportNode, containing_module_id: NodeId) -> Self {
        // Make crate-visible if needed internally
        PendingExport {
            containing_mod_id: ModuleNodeId::new(containing_module_id),
            export_node: export,
        }
    }

    /// Returns the ID of the module containing this pending export.
    pub fn containing_mod_id(&self) -> ModuleNodeId {
        self.containing_mod_id
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

impl LogDataStructure for ModuleTree {}

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
    GraphIdConversion(#[from] nodes::GraphIdConversionError), // Add #[from] for automatic conversion

    #[error("Node error: {0}")]
    NodeError(#[from] nodes::NodeError), // Add #[from] for NodeError

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

    // --- NEW VARIANT ---
    #[error("Shortest public path resolution failed for external item re-export: {0}")]
    ExternalItemNotResolved(NodeId),

    #[error("No relations found for node {0}: {1}")]
    NoRelationsFound(NodeId, String),
    #[error("Could not resolve target for re-export '{path}'. Import Node ID: {import_node_id:?}")]
    UnresolvedReExportTarget {
        import_node_id: Option<NodeId>,
        path: NodePath, // The original path that failed to resolve
    },

    // --- NEW VARIANT ---
    #[error("Invalid internal state: pending_exports was None when adding module {module_id}")]
    InvalidStatePendingExportsMissing { module_id: NodeId },
    #[error("Internal state error: {0}")]
    InternalState(String),
    #[error("Warning: {0}")] // New variant for non-fatal issues
    Warning(String),
}

impl ModuleTreeError {
    pub(crate) fn no_relations_found(g_node: &dyn GraphNode) -> Self {
        Self::NoRelationsFound(
            g_node.id(),
            format!(
                "{} {: <12} {: <20} | {: <12} | {: <15}",
                "NodeInfo".log_header(),
                g_node.name().log_name(),
                g_node.id().to_string().log_id(),
                g_node.kind().log_vis_debug(),
                g_node.visibility().log_name_debug(),
            ),
        )
    }
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
    /// Returns an empty slice if the internal Option is None.
    pub fn pending_exports(&self) -> &[PendingExport] {
        self.pending_exports
            .as_deref() // Converts Option<Vec<T>> to Option<&[T]>
            .unwrap_or(&[]) // Returns &[] if None
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
            pending_exports: Some(vec![]), // Initialize with Some(empty_vec)
            path_index: HashMap::new(),
            decl_index: HashMap::new(),
            tree_relations: vec![],
            reexport_index: HashMap::new(),
            found_path_attrs: HashMap::new(),
            external_path_attrs: HashMap::new(), // Initialize the new field
            pending_path_attrs: Some(Vec::new()),
            relations_by_source: HashMap::new(),
            relations_by_target: HashMap::new(),
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

    /// Finds relations originating from `source_id` that satisfy the `relation_filter` closure.
    ///
    /// The closure receives a reference to each candidate `TreeRelation` and should return `true`
    /// if the relation should be included in the results.
    ///
    /// # Arguments
    /// * `source_id`: The GraphId of the source node.
    /// * `relation_filter`: A closure `Fn(&Relation) -> bool` used to filter relations.
    ///
    /// # Returns
    /// A `Vec` containing references to the matching `Relation`s.
    ///
    /// # Complexity
    /// O(1) average lookup for the source ID + O(k) filter application, where k is the
    /// number of relations originating from `source_id`.
    pub fn get_relations_from<F>(
        &self,
        source_id: &GraphId,
        relation_filter: F, // Closure parameter
    ) -> Option<Vec<&TreeRelation>>
    where
        F: Fn(&TreeRelation) -> bool, // Closure takes &Relation, returns bool
    {
        self.relations_by_source.get(source_id).map(|indices| {
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| {
                    self.tree_relations
                        .get(index)
                        // filter() on Option returns Some only if the closure is true.
                        .filter(|&relation| relation_filter(relation))
                })
                .collect()
        })
    }

    pub fn reexport_index(&self) -> &HashMap<NodePath, NodeId> {
        &self.reexport_index
    }

    /// Finds relations pointing to `target_id` that satisfy the `relation_filter` closure.
    ///
    /// (Doc comments similar to get_relations_from)
    pub fn get_relations_to<F>(
        &self,
        target_id: &GraphId,
        relation_filter: F, // Closure parameter
    ) -> Option<Vec<&TreeRelation>>
    where
        F: Fn(&TreeRelation) -> bool, // Closure takes &TreeRelation, returns bool
    {
        self.relations_by_target.get(target_id).map(|indices| {
            indices
                .iter()
                .filter_map(|&index| {
                    self.tree_relations
                        .get(index)
                        .filter(|&tr| relation_filter(tr))
                })
                .collect()
        })
    }

    /// Adds a relation to the tree without checking if the source/target nodes exist.
    pub fn add_relation(&mut self, tr: TreeRelation) {
        // TODO: Optionally check if source/target nodes exist in self.nodes first?
        let new_index = self.tree_relations.len();
        let source_id = tr.relation().source;
        let target_id = tr.relation().target;

        self.tree_relations.push(tr);

        // Update indices
        self.relations_by_source
            .entry(source_id)
            .or_default()
            .push(new_index);
        self.relations_by_target
            .entry(target_id)
            .or_default()
            .push(new_index);
    }

    /// Adds a relation to the tree, first checking if the source and target nodes
    /// (if they are `GraphId::Node`) exist in the `modules` map.
    /// Returns `ModuleTreeError::ModuleNotFound` if a check fails.
    pub fn add_relation_checked(&mut self, tr: TreeRelation) -> Result<(), ModuleTreeError> {
        let relation = tr.relation();
        let source_id = relation.source;
        let target_id = relation.target;

        // Check source node if it's a NodeId
        if let GraphId::Node(node_id) = source_id {
            let mod_id = ModuleNodeId::new(node_id);
            if !self.modules.contains_key(&mod_id) {
                return Err(ModuleTreeError::ModuleNotFound(mod_id));
            }
        }

        // Check target node if it's a NodeId
        if let GraphId::Node(node_id) = target_id {
            let mod_id = ModuleNodeId::new(node_id);
            if !self.modules.contains_key(&mod_id) {
                return Err(ModuleTreeError::ModuleNotFound(mod_id));
            }
        }

        // Checks passed, add the relation using the unchecked method's logic
        let new_index = self.tree_relations.len();
        self.tree_relations.push(tr);

        // Update indices
        self.relations_by_source
            .entry(source_id)
            .or_default()
            .push(new_index);
        self.relations_by_target
            .entry(target_id)
            .or_default()
            .push(new_index);

        Ok(())
    }
    /// Extends the module tree with multiple relations efficiently.
    ///
    /// This method takes an iterator yielding `Relation` items and adds them to the
    /// tree's internal storage. It updates the `tree_relations` vector and both
    /// index HashMaps (`relations_by_source` and `relations_by_target`).
    ///
    /// This is generally more efficient than calling `add_relation` repeatedly in a loop,
    /// especially for large numbers of relations, as it reserves capacity for the
    /// `tree_relations` vector upfront if the iterator provides a size hint.
    ///
    /// Note: This method performs *unchecked* insertion, meaning it does not verify
    /// if the source or target nodes of the relations exist within the `modules` map.
    /// Use `add_relation_checked` if such checks are required for individual relations.
    ///
    /// # Arguments
    /// * `relations_iter`: An iterator that yields `Relation` items to be added.
    pub(crate) fn extend_relations<I>(&mut self, relations_iter: I)
    where
        I: IntoIterator<Item = Relation>,
    {
        let relations_iter = relations_iter.into_iter(); // Ensure we have an iterator

        // Get the starting index for the new relations
        let mut current_index = self.tree_relations.len();

        // Reserve capacity in the main vector if the iterator provides a hint
        let (lower_bound, upper_bound) = relations_iter.size_hint();
        let reserve_amount = upper_bound.unwrap_or(lower_bound);
        if reserve_amount > 0 {
            self.tree_relations.reserve(reserve_amount);
            // Note: Reserving capacity for HashMaps is more complex as it depends on
            // the number of *new* keys, not just the total number of relations.
            // We'll let the HashMaps resize as needed for simplicity here.
        }

        // Iterate through the provided relations
        for relation in relations_iter {
            // Convert to TreeRelation (cheap wrapper)
            let tr = TreeRelation::new(relation);
            let source_id = tr.relation().source;
            let target_id = tr.relation().target;

            // Update the source index HashMap
            // entry().or_default() gets the Vec<usize> for the source_id,
            // creating it if it doesn't exist, then pushes the current_index.
            self.relations_by_source
                .entry(source_id)
                .or_default()
                .push(current_index);

            // Update the target index HashMap similarly
            self.relations_by_target
                .entry(target_id)
                .or_default()
                .push(current_index);

            // Add the relation to the main vector
            self.tree_relations.push(tr);

            // Increment the index for the next relation
            current_index += 1;
        }
    }

    fn add_reexport_checked(
        &mut self,
        public_reexport_path: NodePath,
        target_node_id: NodeId,
    ) -> Result<(), ModuleTreeError> {
        match self.reexport_index.entry(public_reexport_path.clone()) {
            // Clone path for error case
            std::collections::hash_map::Entry::Occupied(entry) => {
                let existing_target_id = *entry.get();
                if existing_target_id != target_node_id {
                    // Found a *different* item already re-exported at this exact public path.
                    return Err(ModuleTreeError::ConflictingReExportPath {
                        path: public_reexport_path, // Use the cloned path
                        existing_id: existing_target_id,
                        conflicting_id: target_node_id, // The item we just resolved
                    });
                }
                // If it's the same target ID, do nothing (idempotent)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Path is free, insert the mapping: public_path -> actual_target_id
                entry.insert(target_node_id);
            }
        };
        Ok(())
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
            // NOTE: We already have `Relation::ModuleImports` created at parsing time.
            imports
                .iter()
                .filter(|imp| imp.is_inherited_use())
                .map(|imp| PendingImport::from_import(imp.clone(), module.id())),
        );

        // Add all re-exports to the Vec inside the Option
        if let Some(exports) = self.pending_exports.as_mut() {
            exports.extend(
                imports
                    .iter()
                    .filter(|imp| imp.is_local_reexport())
                    .map(|imp| PendingExport::from_export(imp.clone(), module.id())),
            );
        } else {
            // This state is invalid. pending_exports should only be None after process_export_rels
            // has been called and taken ownership. If we are adding a module, it means
            // process_export_rels hasn't run yet (or ran unexpectedly early).
            return Err(ModuleTreeError::InvalidStatePendingExportsMissing {
                module_id: module.id(),
            });
        }

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
            // *** NEW LOGGING CALL ***
            self.log_add_pending_path(module_id, &module.name);
            // *** END NEW ***
            self.pending_path_attrs
                .as_mut()
                .expect("Invariant: pending_path_attrs should always be Some before take()")
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
        // *** NEW LOGGING CALL ***
        self.log_resolve_entry_exit(true); // Log entry

        let module_ids = match self.pending_path_attrs.take() {
            Some(pending_ids) => {
                if pending_ids.is_empty() {
                    self.log_resolve_pending_status(None);
                    self.log_resolve_entry_exit(false); // Log exit
                    return Ok(());
                }
                self.log_resolve_pending_status(Some(pending_ids.len()));
                pending_ids
            }
            None => {
                self.log_resolve_pending_status(None);
                self.log_resolve_entry_exit(false); // Log exit
                return Ok(()); // Should not happen if take() is only called once, but handle defensively
            }
        };
        for module_id in module_ids {
            // Log which ID we are starting to process
            self.log_resolve_step(module_id, "Processing ID", &module_id.to_string(), false);

            let base_dir = match self.find_declaring_file_dir(module_id) {
                Ok(dir) => {
                    self.log_resolve_step(
                        module_id,
                        "Find Base Dir",
                        &dir.display().to_string(),
                        false,
                    );
                    dir
                }
                Err(e) => {
                    // *** NEW LOGGING CALL ***
                    self.log_resolve_step(module_id, "Find Base Dir", &format!("{:?}", e), true);
                    Self::log_path_attr_not_found(module_id);
                    // Continue processing other IDs, maybe collect errors later?
                    // For now, let's return the first error encountered to halt processing.
                    return Err(ModuleTreeError::UnresolvedPathAttr(Box::new(e)));
                }
            };

            let module = match self.get_module_checked(&module_id) {
                Ok(m) => m,
                Err(e) => {
                    self.log_resolve_step(module_id, "Get Module Node", &format!("{:?}", e), true);
                    continue; // Skip this ID if module node not found
                }
            };

            let path_val = match extract_path_attr_from_node(module) {
                Some(val) => {
                    // *** NEW LOGGING CALL ***
                    self.log_resolve_step(module_id, "Extract Attr Value", val, false);
                    val
                }
                None => {
                    // *** NEW LOGGING CALL ***
                    self.log_resolve_step(
                        module_id,
                        "Extract Attr Value",
                        "Attribute value not found",
                        true,
                    );
                    continue; // Skip if attribute value missing
                }
            };

            // Consider adding error handling for normalization if needed
            let resolved = base_dir.join(path_val).normalize();
            // *** NEW LOGGING CALL ***
            self.log_resolve_step(
                module_id,
                "Normalize Path",
                &resolved.display().to_string(),
                false,
            );

            // Remove the old PathLogCtx logging block as it's replaced by the step-by-step logs

            match self.found_path_attrs.entry(module_id) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let existing_path = entry.get().clone();
                    // *** NEW LOGGING CALL ***
                    self.log_resolve_duplicate(module_id, &existing_path, &resolved);
                    return Err(ModuleTreeError::DuplicatePathAttribute {
                        module_id,
                        existing_path,
                        conflicting_path: resolved,
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    Self::log_resolve_insert(module_id, &resolved);
                    entry.insert(resolved);
                }
            };
        }

        // *** NEW LOGGING CALL ***
        self.log_resolve_entry_exit(false); // Log exit
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
                    let resolves_to_rel = Relation {
                        source: GraphId::Node(*decl_id),    // Declaration Node
                        target: GraphId::Node(module.id()), // Definition Node (the file-based one)
                        kind: RelationKind::ResolvesToDefinition,
                    };
                    self.log_relation(resolves_to_rel, None);
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
        for relation in new_relations.into_iter() {
            self.add_relation(relation);
        }

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
    pub fn add_relations_batch(&mut self, relations: &[Relation]) -> Result<(), ModuleTreeError> {
        for rel in relations.iter() {
            self.add_relation((*rel).into());
        }

        Ok(())
    }

    pub fn shortest_public_path(
        &self,
        item_id: NodeId,
        graph: &ParsedCodeGraph, // Still need graph for node details (visibility, name)
    ) -> Result<Vec<String>, ModuleTreeError> {
        // --- 1. Initial Setup ---

        use ploke_core::ItemKind;
        let item_node = graph.find_node_unique(item_id)?; // O(n) lookup, need to refactor
                                                          // find_node_unique
        if !item_node.visibility().is_pub() {
            // If the item's own visibility isn't Public, it can never be reached
            // via a public path from the crate root.
            self.log_spp_item_not_public(item_node);
            return Err(ModuleTreeError::ItemNotPubliclyAccessible(item_id));
        }
        if item_node.kind_matches(ItemKind::ExternCrate) {
            return Err(ModuleTreeError::ExternalItemNotResolved(item_node.id()));
        }
        let item_gid = &GraphId::Node(item_node.id());
        let item_name = item_node.name().to_string();

        self.log_spp_start(item_node);

        // Find the direct parent module ID using the index
        let initial_parent_relations = self
            .get_relations_to(item_gid, |tr| tr.relation().kind == RelationKind::Contains)
            .ok_or_else(|| ModuleTreeError::no_relations_found(item_node))?;
        let parent_mod_id = match initial_parent_relations.first() {
            Some(tr) => ModuleNodeId::new(tr.relation().source.try_into()?), // O(1)+O(k) lookup
            None => {
                // Item isn't contained in any module? Maybe it's the root module itself?
                if let Some(module_node) = item_node.as_module() {
                    if module_node.id() == *self.root.as_inner() {
                        // Special case: asking for the path to the root module itself
                        return Ok(vec!["crate".to_string()]);
                    }
                }
                // Otherwise, it's an error or uncontained item
                return Err(ModuleTreeError::ContainingModuleNotFound(item_id));
            }
        };

        let mut queue: VecDeque<(ModuleNodeId, Vec<String>)> = VecDeque::new();
        let mut visited: HashSet<ModuleNodeId> = HashSet::new();

        // Enqueue the *parent* module. Path starts with the item's name.
        queue.push_back((parent_mod_id, vec![item_name]));
        visited.insert(parent_mod_id);

        // --- 2. BFS Loop ---
        while let Some((current_mod_id, path_to_item)) = queue.pop_front() {
            // --- 3. Check for Goal ---
            self.log_spp_check_root(current_mod_id, &path_to_item);
            if current_mod_id == self.root {
                // Reached the crate root! Construct the final path.
                self.log_spp_found_root(current_mod_id, &path_to_item);
                let mut final_path = vec!["crate".to_string()];
                // The path_to_item is currently [item, mod, parent_mod, ...]
                // We need to reverse it.
                final_path.extend(path_to_item.into_iter().rev());
                // NOTE: Popping item's own identitity name for now to conform to same test
                // structure as old version. Re-evaluate this strategy later.
                final_path.pop();
                return Ok(final_path);
            }

            // --- 4. Explore Upwards (Containing Module) ---
            self.log_spp_explore_containment(current_mod_id, &path_to_item);
            self.explore_up_via_containment(
                current_mod_id,
                &path_to_item,
                &mut queue,
                &mut visited,
                graph,
            ); // Need to handle errors
               // When should this return error for invalid graph state?

            // --- 5. Explore Sideways/Upwards (Re-exports) ---
            self.log_spp_explore_reexports(current_mod_id, &path_to_item);
            self.explore_up_via_reexports(
                current_mod_id,
                &path_to_item,
                &mut queue,
                &mut visited,
                graph,
            )?; // Need to handle errors
                // When should this return error for invalid graph state?
        } // End while loop

        // --- 6. Not Found ---
        Err(ModuleTreeError::ItemNotPubliclyAccessible(item_id))
    }

    // Helper function for exploring via parent modules
    fn explore_up_via_containment(
        &self,
        current_mod_id: ModuleNodeId,
        path_to_item: &[String],
        queue: &mut VecDeque<(ModuleNodeId, Vec<String>)>,
        visited: &mut HashSet<ModuleNodeId>,
        // NOTE: Unused variable `graph`. Why is it here?
        graph: &ParsedCodeGraph, // NOTE: Unused variable `graph`. Why is it here?
    ) -> Result<(), ModuleTreeError> {
        // Added Result return

        let current_mod_node = self.get_module_checked(&current_mod_id)?; // O(1)
        self.log_spp_containment_start(current_mod_node);
        // Determine the ID and visibility source (declaration or definition)
        let (effective_source_id, visibility_source_node) =
            if current_mod_node.is_file_based() && current_mod_id != self.root {
                // For file-based modules, find the declaration
                let decl_relations = self
                    .get_relations_to(&current_mod_id.to_graph_id(), |tr| {
                        matches!(
                            tr.relation().kind,
                            RelationKind::ResolvesToDefinition | RelationKind::CustomPath
                        )
                    })
                    .ok_or_else(|| ModuleTreeError::no_relations_found(current_mod_node))?;
                self.log_spp_containment_vis_source(current_mod_node);
                if let Some(decl_rel) = decl_relations.first() {
                    let decl_id = ModuleNodeId::new(decl_rel.relation().source.try_into()?);
                    // Visibility comes from the declaration node
                    self.log_spp_containment_vis_source_decl(decl_id);
                    (decl_id, self.get_module_checked(&decl_id)?)
                } else {
                    // Unlinked file-based module, treat as private/inaccessible upwards
                    // Or log a warning and use the definition itself? Let's treat as inaccessible.
                    self.log_spp_containment_unlinked(current_mod_id);
                    return Ok(()); // Cannot proceed upwards via containment
                }
            } else {
                self.log_spp_containment_vis_source_inline(current_mod_node);
                // Inline module or root, use itself
                (current_mod_id, current_mod_node)
            };

        // Find the parent of the effective source (declaration or inline module)
        let parent_relations = self
            .get_relations_to(&effective_source_id.to_graph_id(), |tr| {
                tr.relation().kind == RelationKind::Contains
            })
            .ok_or_else(|| ModuleTreeError::no_relations_found(current_mod_node))?;
        if let Some(parent_rel) = parent_relations.first() {
            let parent_mod_id = ModuleNodeId::new(parent_rel.relation().source.try_into()?);

            // Check visibility: Is the declaration/inline module visible FROM the parent?
            // We need the parent module node to check its scope if visibility is restricted
            let parent_mod_node = self.get_module_checked(&parent_mod_id)?;

            self.log_spp_containment_check_parent(parent_mod_node);
            if self.is_accessible_from(parent_mod_id, effective_source_id) {
                // Need is_accessible_from helper
                if visited.insert(parent_mod_id) {
                    // Check if parent is newly visited
                    let mut new_path = path_to_item.to_vec();
                    // Prepend the name used to declare/define the current module
                    new_path.push(visibility_source_node.name().to_string());
                    self.log_spp_containment_queue_parent(parent_mod_id, &new_path);
                    queue.push_back((parent_mod_id, new_path));
                } else {
                    self.log_spp_containment_parent_visited(parent_mod_id);
                }
            } else {
                self.log_spp_containment_parent_inaccessible(
                    visibility_source_node,
                    effective_source_id,
                    parent_mod_id,
                );
            }
        } else if effective_source_id != self.root {
            // Should only happen if root has no parent relation, otherwise inconsistent tree
            self.log_spp_containment_no_parent(effective_source_id);
        }
        Ok(())
    }

    // Helper function for exploring via re-exports
    fn explore_up_via_reexports(
        &self,
        // The ID of the item/module *potentially* being re-exported
        target_id: ModuleNodeId, // Changed name for clarity
        path_to_item: &[String],
        queue: &mut VecDeque<(ModuleNodeId, Vec<String>)>,
        visited: &mut HashSet<ModuleNodeId>,
        graph: &ParsedCodeGraph,
    ) -> Result<(), ModuleTreeError> {
        // Added Result return
        self.log_spp_reexport_start(target_id, path_to_item);
        // Find ImportNodes that re-export the target_id
        // Need reverse ReExport lookup: target = target_id -> source = import_node_id
        let reexport_relations = self
            .get_relations_to(&target_id.to_graph_id(), |tr| {
                tr.relation().kind == RelationKind::ReExports
            })
            .ok_or_else(|| {
                // WARNING:
                // Placeholder `unwrap` here, need better error conversions for
                // SynParserError <--> ModuleTreeError
                let node = graph.find_node_unique(target_id.into_inner()).unwrap();
                ModuleTreeError::no_relations_found(node)
            })?;

        for rel in reexport_relations {
            let import_node_id = rel.relation().source.try_into()?; // ID of the ImportNode itself
            let import_node = match graph.get_import_checked(import_node_id) {
                // O(1) <--- not actually true, refactor graph method `get_import_checked`
                Ok(node) => node,
                Err(_) => {
                    self.log_spp_reexport_missing_import_node(import_node_id);
                    continue; // Skip this relation
                }
            };
            // Check for extern crate, return error that needs to be handled by caller.
            // This should only happen for items that are not defined in the target parsed crate.
            if import_node.is_extern_crate() {
                self.log_spp_reexport_is_external(import_node);
                return Err(ModuleTreeError::ExternalItemNotResolved(import_node_id));
            }
            self.log_spp_reexport_get_import_node(import_node);

            // Check if the re-export itself is public (`pub use`, `pub(crate) use`, etc.)
            if !import_node.is_public_use() {
                self.log_spp_reexport_not_public(import_node);
                continue; // Skip private `use` statements
            }

            // Find the module containing this ImportNode
            let container_relations = self
                .get_relations_to(&GraphId::Node(import_node_id), |r| {
                    r.relation().kind == RelationKind::Contains
                })
                .ok_or_else(|| ModuleTreeError::no_relations_found(import_node))?;
            if let Some(container_rel) = container_relations.first() {
                let reexporting_mod_id =
                    ModuleNodeId::new(container_rel.relation().source.try_into()?);

                // IMPORTANT: Check if the *re-exporting module* itself is accessible
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
                    self.log_spp_reexport_queue_module(import_node, reexporting_mod_id, &new_path);
                    queue.push_back((reexporting_mod_id, new_path));
                } else {
                    self.log_spp_reexport_module_visited(reexporting_mod_id);
                }
            } else {
                self.log_spp_reexport_no_container(import_node_id);
            }
        }
        Ok(())
    }

    // Helper needed for visibility check upwards (simplified version of ModuleTree::is_accessible)
    // Checks if `target_id` (decl or inline mod) is accessible *from* `potential_parent_id`
    #[allow(unused_variables)]
    fn is_accessible_from(
        &self,
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
        self.get_effective_visibility(target_id)
            .is_some_and(|vis| vis.is_pub()) // TODO: Replace with full check
    }

    // Resolves visibility for target node as if it were a dependency.
    // Only used as a helper in the shortest public path.
    #[allow(unused_variables)]
    pub fn resolve_visibility<T: GraphNode>(
        &self,
        node: &T,
        graph: &ParsedCodeGraph,
    ) -> Result<VisibilityKind, ModuleTreeError> {
        let parent_module_vis = graph
            .modules()
            .iter()
            .find(|m| m.items().is_some_and(|m| m.contains(&node.id())))
            .map(|m| m.visibility())
            // Use ok_or_else to handle Option and create the specific error
            .ok_or_else(|| ModuleTreeError::ContainingModuleNotFound(node.id()))?;
        todo!() // Rest of the visibility logic still needs implementation
    }

    // Helper to check if an item is part of a re-export chain leading to our target
    // NOTE: Why is this currently unused? I'm fairly sure we were using it somewhere...
    #[allow(dead_code, reason = "This is almost certainly useful somewhere")]
    fn is_part_of_reexport_chain(
        &self,
        start_id: NodeId,
        target_id: NodeId,
    ) -> Result<bool, ModuleTreeError> {
        let mut current_id = start_id;
        let mut visited = HashSet::new();

        while visited.insert(current_id) {
            // Check if current_id re-exports our target
            if let Some(_reexport_rel) = self.tree_relations.iter().find(|tr| {
                tr.relation().kind == RelationKind::ReExports
                    && tr.relation().source == GraphId::Node(current_id)
                    && tr.relation().target == GraphId::Node(target_id)
            }) {
                return Ok(true);
            }

            // Move to next re-export in chain
            if let Some(next_rel) = self.tree_relations.iter().find(|tr| {
                tr.relation().kind == RelationKind::ReExports
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
    // fn process_export_rels(&self, graph: &ParsedCodeGraph) -> Result<Vec<TreeRelation>, ModuleTreeError> {
    //     todo!()
    // }
    //
    // or

    #[cfg(not(feature = "reexport"))]
    pub fn process_export_rels(&mut self, graph: &ParsedCodeGraph) -> Result<(), ModuleTreeError> {
        // Take ownership of the pending exports Vec, leaving None in its place.
        let pending = match self.pending_exports.take() {
            Some(p) => p,
            None => {
                log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "process_export_rels called when pending_exports was already None.");
                return Ok(()); // Nothing to process
            }
        };

        let mut new_relations: Vec<TreeRelation> = Vec::new();
        for export in pending {
            // Iterate over the owned Vec
            // get the target node:
            let source_mod_id = export.containing_mod_id(); // Use owned export
            let export_node = export.export_node();

            // Create relation
            let relation = Relation {
                // WARNING:
                // Bug, currently forms relation with it's containing module, NOT the target that
                // the `ImportNode` is actually re-exporting.
                source: GraphId::Node(*source_mod_id.as_inner()),
                target: GraphId::Node(export_node.id),
                kind: RelationKind::ReExports,
            };
            self.log_relation(relation, None);

            new_relations.push(relation.into());
            // Add to reexport_index
            if let Some(reexport_name) = export_node.source_path.last() {
                let mut reexport_path = graph.get_item_module_path(*source_mod_id.as_inner());
                // Check for renamed export path, e.g. `a::b::Struct as RenamedStruct`
                if export_node.is_renamed() {
                    // if renamed, use visible_name for path extension
                    // TODO: Keep a list of renamed modules specifically to track possible
                    // collisions.
                    reexport_path.push(export_node.visible_name.clone());
                } else {
                    // otherwise, use standard name
                    reexport_path.push(reexport_name.clone());
                }

                let node_path = NodePath::try_from(reexport_path)
                    .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;

                let debug_node_path = node_path.clone();

                // Check for duplicate re-exports at the same path
                match self.reexport_index.entry(node_path) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        // NOTE: Could filter for cfg here, to make graph cfg aware in a
                        // relatively easy way.
                        let existing_id = *entry.get();
                        if existing_id != export_node.id {
                            // Found a different NodeId already registered for this exact path.
                            return Err(ModuleTreeError::ConflictingReExportPath {
                                path: debug_node_path, // Use the cloned path
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
        for new_tr in new_relations {
            self.add_relation(new_tr);
        }

        Ok(())
    }

    /// Iterates through pending re-exports (`pub use`), resolves their targets,
    /// updates the `reexport_index`, and adds the corresponding `ReExports`
    /// relations to the `tree_relations`.
    ///
    /// Relies on `path_index` and `decl_index` being populated correctly beforehand.
    /// Returns an error if a re-export target cannot be resolved or if conflicting
    /// re-exports are found at the same public path.
    #[cfg(feature = "reexport")]
    pub fn process_export_rels(
        &mut self,
        graph: &ParsedCodeGraph, // Keep graph for potential future use in logging or resolution
    ) -> Result<(), ModuleTreeError> {
        // Take ownership of the pending exports Vec, leaving None in its place.
        let pending = match self.pending_exports.take() {
            Some(p) => p,
            None => {
                log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "process_export_rels called when pending_exports was already None.");
                return Ok(()); // Nothing to process
            }
        };

        for export in pending {
            // Iterate over the owned Vec
            // Call the helper function to resolve this single export
            match self.resolve_single_export(&export, graph) {
                Ok((relation, public_reexport_path)) => {
                    // Log the correctly formed relation
                    // Note: The target_node_id is relation.target
                    let target_node_id = match relation.target {
                        GraphId::Node(id) => id,
                        // Handle other GraphId variants if necessary, though ReExports target should be Node
                        _ => {
                            log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "ReExport relation target is not a NodeId: {:?}", relation);
                            // Decide how to handle this unexpected case, maybe continue or return error
                            continue;
                        }
                    };

                    self.log_relation(relation, Some("ReExport Target Resolved")); // Log before potential error

                    // Update the reexport_index: public_path -> target_node_id
                    self.add_reexport_checked(public_reexport_path, target_node_id)?;

                    self.add_relation_checked(relation.into())?;
                    // If index update succeeded, add relation to the batch
                }
                Err(e) => {
                    // Decide error handling: Propagate first error or collect all?
                    // Propagating first error for now.
                    log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Failed to resolve pending export {:#?}: {}", export, e);
                    return Err(e);
                }
            } // End match resolve_single_export
        } // End loop through pending_exports

        // Add all newly created relations to the tree's main relation store in one go

        Ok(())
    }

    /// Processes pending re-exports (`pub use`) to:
    /// 1. Create `RelationKind::ReExports` between the `ImportNode` and the actual item being  re-exported.
    /// 2. Populate the `reexport_index` mapping the new public path to the ID of the re-exported item.
    ///
    /// Relies on `path_index` and `decl_index` being populated correctly beforehand.
    /// Returns an error if a re-export target cannot be resolved or if conflicting re-exports are found.
    #[cfg(feature = "reexport")]
    fn resolve_single_export(
        &self,
        export: &PendingExport,
        graph: &ParsedCodeGraph, // Needed for graph lookups during relative resolution
    ) -> Result<(Relation, NodePath), ModuleTreeError> {
        let source_mod_id = export.containing_mod_id();
        let export_node = export.export_node();

        // Always use the original source_path to find the target item
        let target_path_segments = export_node.source_path();

        if target_path_segments.is_empty() {
            return Err(ModuleTreeError::NodePathValidation(Box::new(
                SynParserError::NodeValidation("Empty export path".into()),
            )));
        }

        let first_segment = &target_path_segments[0];
        // We need to find this

        // --- Delegate ALL path resolution to resolve_path_relative_to ---
        let (base_module_id, segments_to_resolve) = if first_segment == "crate" {
            (self.root(), &target_path_segments[1..]) // Start from root, skip "crate"
        } else {
            (source_mod_id, target_path_segments) // Start from containing mod, use full path
        };

        // Check for external crate re-exports *before* attempting local resolution
        // This check might need refinement depending on how extern crates are represented.
        // Assuming iter_dependency_names gives names of direct dependencies.
        if base_module_id == self.root() // Only check for external if path starts relative to root
            && !segments_to_resolve.is_empty() // Ensure there's a segment to check
            && graph.iter_dependency_names().any(|dep_name| dep_name == segments_to_resolve[0])
        {
            self.log_resolve_single_export_external(segments_to_resolve);
            // Return specific error for external re-exports that SPP might handle later
            return Err(ModuleTreeError::ExternalItemNotResolved(export_node.id));
        }

        let target_node_id: NodeId = self
            .resolve_path_relative_to(
                base_module_id,
                segments_to_resolve,
                graph, // Pass graph access
            )
            .map_err(|e| self.wrap_resolution_error(e, export_node.id, target_path_segments))?;

        // --- If target_node_id was found ---
        let relation = Relation {
            source: GraphId::Node(export_node.id), // Source is the ImportNode itself
            target: GraphId::Node(target_node_id), // Target is the resolved item
            kind: RelationKind::ReExports,
        };
        self.log_relation(relation, Some("resolve_single_export created relation"));

        // Construct the public path using the visible_name
        let containing_module = self.get_module_checked(&source_mod_id)?;
        let mut public_path_vec = containing_module.defn_path().clone();
        public_path_vec.push(export_node.visible_name.clone()); // Use the name it's exported AS
        let public_reexport_path = NodePath::try_from(public_path_vec)?;

        Ok((relation, public_reexport_path))
    }

    /// Helper to resolve a path relative to a starting module.
    /// This is a complex function mimicking Rust's name resolution.
    fn resolve_path_relative_to(
        &self,
        base_module_id: ModuleNodeId,
        path_segments: &[String],
        graph: &ParsedCodeGraph, // Need graph access
    ) -> Result<NodeId, ModuleTreeError> {
        if path_segments.is_empty() {
            return Err(ModuleTreeError::NodePathValidation(Box::new(
                SynParserError::NodeValidation(
                    "Empty path segments for relative resolution".into(),
                ),
            )));
        }

        let mut current_module_id = base_module_id;
        let mut remaining_segments = path_segments;

        // 1. Handle `self::` prefix
        if remaining_segments[0] == "self" {
            remaining_segments = &remaining_segments[1..];
            if remaining_segments.is_empty() {
                // Path was just "self", refers to the module itself
                return Ok(*current_module_id.as_inner());
            }
        }
        // 2. Handle `super::` prefix (potentially multiple times)
        else {
            while remaining_segments[0] == "super" {
                let node_path = NodePath::try_from(path_segments.to_vec())?;
                current_module_id = self.get_parent_module_id(current_module_id).ok_or({
                    ModuleTreeError::UnresolvedReExportTarget {
                        path: node_path,      // Original path for error
                        import_node_id: None, // Indicate failure resolving 'super'
                    }
                })?;
                remaining_segments = &remaining_segments[1..];
                if remaining_segments.is_empty() {
                    // Path ended with "super", refers to the parent module
                    return Ok(*current_module_id.as_inner());
                }
            }
        }

        // 3. Iterative Resolution through remaining segments
        let mut resolved_id: Option<NodeId> = None;

        for (i, segment) in remaining_segments.iter().enumerate() {
            let search_in_module_id = resolved_id
                .map(ModuleNodeId::new) // If we resolved to a module last iteration
                .unwrap_or(current_module_id); // Otherwise, start in the initial/adjusted module

            // 4. Find items named `segment` directly contained within `search_in_module_id`
            let contains_relations = self
                .get_relations_from(&search_in_module_id.to_graph_id(), |tr| {
                    tr.relation().kind == RelationKind::Contains
                })
                .unwrap_or_default(); // Use unwrap_or_default for empty vec if no relations

            let mut candidates: Vec<NodeId> = Vec::new();
            self.log_resolve_segment_start(segment, search_in_module_id, contains_relations.len());

            for rel in &contains_relations {
                // Iterate by reference
                if let GraphId::Node(target_id) = rel.relation().target {
                    self.log_resolve_segment_relation(target_id);
                    match graph.find_node_unique(target_id) {
                        Ok(target_node) => {
                            let name_matches = target_node.name() == segment;
                            self.log_resolve_segment_found_node(target_node, segment, name_matches);
                            if name_matches {
                                // Original visibility check logic follows...
                                // 5. Visibility Check (Simplified: Check if accessible from the module we are searching *in*)
                                // TODO: Refine visibility check if needed. is_accessible might be too broad here?
                                //       Maybe need a check specific to direct children?
                                //       For now, using is_accessible.
                                if let Some(target_mod_id) =
                                    target_node.as_module().map(|m| ModuleNodeId::new(m.id()))
                                {
                                    // If the target is a module, check its accessibility
                                    if self.is_accessible(search_in_module_id, target_mod_id) {
                                        candidates.push(target_id);
                                    }
                                } else {
                                    // If the target is not a module (e.g., function, struct),
                                    // its visibility is inherent. Check if it's public or accessible
                                    // within the crate/restricted path.
                                    // For simplicity here, let's assume if it's contained, it's accessible
                                    // for the purpose of path resolution *within* the module structure.
                                    // A more robust check might involve the item's own visibility field.
                                    candidates.push(target_id); // Assume accessible for now if contained
                                }
                            }
                        }
                        Err(e) => {
                            debug!(target: LOG_TARGET_MOD_TREE_BUILD,
                                "    {} Error finding node for ID {}: {:?}",
                                "✗".log_error(),
                                target_id.to_string().log_id(),
                                e.to_string().log_error()
                            );
                        }
                    }
                    // Else: Relation target was not a GraphId::Node
                } else {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD,
                        "  {} Relation Target was not GraphId::Node: {:?}",
                        "->".log_comment(),
                        rel.relation().target.log_id_debug()
                    );
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
                    let found_id = candidates[0];
                    resolved_id = Some(found_id); // Store the resolved ID for the next iteration

                    // Check if it's the last segment
                    if i == remaining_segments.len() - 1 {
                        return Ok(found_id);
                    } else {
                        // More segments remain, ensure the found item is a module
                        if graph.find_node_unique(found_id)?.as_module().is_none() {
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
    // ... other methods ...

    pub fn resolve_custom_path(&self, module_id: ModuleNodeId) -> Option<&PathBuf> {
        self.found_path_attrs.get(&module_id)
    }

    #[allow(dead_code)]
    fn get_reexport_name(&self, module_id: ModuleNodeId, item_id: NodeId) -> Option<String> {
        self.pending_exports
            .as_deref() // Get Option<&[PendingExport]>
            .and_then(|exports| {
                // Iterate over the slice if Some
                exports
                    .iter()
                    .find(|exp| {
                        exp.containing_mod_id() == module_id && exp.export_node().id == item_id
                    })
                    .and_then(|exp| exp.export_node().source_path.last().cloned())
            })
    }

    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
                    self.modules
                        .get(&ModuleNodeId::new(decl_id))
                        .map(|decl_node| decl_node.visibility())
                        .unwrap_or_else(|| {
                            // Should not happen if tree is consistent, but default to Inherited if decl node missing
                            self.log_access_missing_decl_node(decl_id, target_defn_id);
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
                    self.log_access_restricted_check_ancestor(ancestor_id, restriction_module_id);
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
                self.log_find_decl_dir_missing_parent(current_id);
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

    pub(crate) fn process_path_attributes(&mut self) -> Result<(), ModuleTreeError> {
        let mut internal_relations = Vec::new();
        let mut external_path_files: Vec<(ModuleNodeId, PathBuf)> = Vec::new();

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
                    self.log_relation(relation, None);
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
                            &target_defn,
                            &dup

                    )));
                    }
                    internal_relations.push(relation);
                }
                None => {
                    // 3. Handle case where the target file node wasn't found.
                    // This indicates an inconsistency - the path resolved, but thecorresponding
                    // module node isn't in the map.
                    // Either the file is outside the target directory, and we return a warning,
                    // since we don't want to do a second parse, or if the path is inside the
                    // directory, we abort the process of resolving the tree because the parsed
                    // files are inconsistent.

                    // Determine the crate's src directory
                    let src_dir = match self.root_file.parent() {
                        Some(dir) => dir,
                        None => {
                            // Should be rare, but handle if root file has no parent
                            self.log_module_error(
                                *decl_module_id,
                                &format!(
                                    "Could not determine src directory from root file: {}",
                                    self.root_file.display()
                                ),
                            );
                            return Err(ModuleTreeError::FilePathMissingParent(
                                self.root_file.clone(),
                            ));
                        }
                    };

                    // Check if the resolved path is outside the src directory
                    if !resolved_path.starts_with(src_dir) {
                        // External path target not found - Log a warning.
                        // This is not an error that will result in an invalid state, only a
                        // slightly pruned one. Unavoidable without having complete access to the
                        // user's filesystem, which we don't want for security reasons.
                        self.log_path_attr_external_not_found(*decl_module_id, resolved_path);
                        external_path_files.push((*decl_module_id, resolved_path.clone()))
                    } else {
                        // Path is inside src, but module node not found - this is an error
                        self.log_module_error(
                            *decl_module_id,
                            &format!(
                                "Path attribute target file not found in modules map: {}",
                                resolved_path.display(),
                            ),
                        );
                        return Err(ModuleTreeError::ModuleDefinitionNotFound(format!(
                            "Module definition for path attribute target '{}' not found for declaration {}:\n{:#?}",
                            resolved_path.display(),
                            decl_module_node.id,
                            &decl_module_node,
                        )));
                    }
                }
            }
        }
        self.external_path_attrs.extend(external_path_files);
        self.add_relations_batch(&internal_relations)?;
        Ok(())
    }

    /// Updates the `path_index` to use canonical paths for modules affected by `#[path]` attributes.
    ///
    /// This function iterates through modules that had a `#[path]` attribute (identified via
    /// `found_path_attrs`). For each such module declaration, it finds the corresponding
    /// definition module (linked by `RelationKind::CustomPath`). It then:
    /// 1. Removes the `path_index` entry based on the definition module's original file-system path.
    /// 2. Inserts a new `path_index` entry using the declaration module's canonical path as the key
    ///    and the definition module's ID as the value.
    ///
    /// This ensures that lookups in `path_index` using canonical paths will correctly resolve
    /// to the definition module, even when `#[path]` is used.
    ///
    /// This function assumes it's called after `process_path_attributes` has run and created
    /// the necessary `CustomPath` relations. It does *not* yet handle updating paths for items
    /// *contained within* the modules affected by `#[path]`.
    pub(crate) fn update_path_index_for_custom_paths(&mut self) -> Result<(), ModuleTreeError> {
        self.log_update_path_index_entry_exit(true);
        // Collect keys to avoid borrowing issues while modifying the map inside the loop.
        // We iterate based on the declarations found to have path attributes.
        let decl_ids_with_path_attrs: Vec<ModuleNodeId> =
            self.found_path_attrs.keys().copied().collect();

        if decl_ids_with_path_attrs.is_empty() {
            self.log_update_path_index_status(None);
            self.log_update_path_index_entry_exit(false);
            return Ok(());
        }

        self.log_update_path_index_status(Some(decl_ids_with_path_attrs.len()));

        for decl_mod_id in decl_ids_with_path_attrs {
            self.log_update_path_index_processing(decl_mod_id);

            // 1. Find the definition module ID using the CustomPath relation
            let def_mod_id = match self.find_custom_path_target(decl_mod_id) {
                Ok(id) => id,
                Err(e) => {
                    self.log_update_path_index_target_error(decl_mod_id, &e);
                    if self.external_path_attrs.contains_key(decl_mod_id) {
                        // AI: log warning here. This is an a warning, but not invalid state.
                    } else {
                        // AI: However, if we have checked the known files pointing externally and
                        // still don't know the source, then it is an error and we should abort.
                        // Indicates an inconsistent graph. AI!
                        return Err(e);
                    }
                }
            };
            self.log_update_path_index_found_target(decl_mod_id, def_mod_id);

            // 2. Get paths (convert Vec<String> to NodePath)
            // We need the nodes temporarily to get their paths, but avoid cloning them.
            let canonical_path = {
                let decl_mod = self.get_module_checked(&decl_mod_id)?;
                NodePath::try_from(decl_mod.path.clone())? // Clone path Vec, not whole node
            };
            let original_path = {
                let def_mod = self.get_module_checked(&def_mod_id)?;
                NodePath::try_from(def_mod.path.clone())? // Clone path Vec, not whole node
            };

            // 3. Log using IDs
            self.log_update_path_index_paths(
                decl_mod_id,
                def_mod_id,
                &canonical_path,
                &original_path,
            );

            // 4. Remove the old path index entry for the definition module
            // Use the original_path (derived from file system) as the key to remove.
            if let Some(removed_id) = self.path_index.remove(&original_path) {
                if removed_id != *def_mod_id.as_inner() {
                    self.log_update_path_index_remove_inconsistency(
                        removed_id,
                        &original_path,
                        def_mod_id,
                    );
                    return Err(ModuleTreeError::InternalState(format!("Path index inconsistency during removal for path {}: expected {}, found {}. This suggests the path_index was corrupted earlier.", original_path, def_mod_id, removed_id)));
                }
                self.log_update_path_index_remove(&original_path, def_mod_id);
            } else {
                self.log_update_path_index_remove_missing(&original_path, def_mod_id);
            }

            // 5. Insert the new path index entry using the canonical path
            // Use the canonical_path (from the declaration) as the key, mapping to the definition ID.
            let def_mod_inner_id = *def_mod_id.as_inner(); // Get the inner NodeId
            if let Some(existing_id) = self
                .path_index
                .insert(canonical_path.clone(), def_mod_inner_id)
            {
                if existing_id != def_mod_inner_id {
                    self.log_update_path_index_insert_conflict(
                        &canonical_path,
                        def_mod_id,
                        existing_id,
                    );
                    return Err(ModuleTreeError::DuplicatePath {
                        path: canonical_path,
                        existing_id,
                        conflicting_id: def_mod_inner_id,
                    });
                }
                self.log_update_path_index_reinsert(&canonical_path, def_mod_id);
            } else {
                self.log_update_path_index_insert(&canonical_path, def_mod_id);
            }
        }

        self.log_update_path_index_entry_exit(false);
        Ok(())
    }

    // Helper function to find the target of a CustomPath relation
    // (Ensure this exists or add it if it doesn't)
    fn find_custom_path_target(
        &self,
        decl_mod_id: ModuleNodeId,
    ) -> Result<ModuleNodeId, ModuleTreeError> {
        let source_gid = decl_mod_id.to_graph_id();
        self.relations_by_source
         .get(&source_gid)
         .and_then(|indices| {
             indices.iter().find_map(|&index| {
                 // Use .get() for safe access and .relation() to get inner Relation
                 let relation = self.tree_relations.get(index)?.relation();
                 if relation.kind == RelationKind::CustomPath {
                     // Target should be the definition module's NodeId
                     match relation.target {
                         GraphId::Node(id) => Some(ModuleNodeId::new(id)),
                         _ => None, // CustomPath should target a Node
                     }
                 } else {
                     None
                 }
             })
         })
         .ok_or_else(|| {
             log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "CustomPath relation target not found for declaration module {}", decl_mod_id);
             // Use a more specific error if available, otherwise adapt ModuleDefinitionNotFound
             ModuleTreeError::ModuleDefinitionNotFound(format!(
                 "Definition module for declaration {} (via CustomPath relation) not found",
                 decl_mod_id
             ))
         })
    }

    // --- Private Logging Helpers for resolve_path_relative_to ---

    fn log_resolve_segment_start(
        &self,
        segment: &str,
        search_in_module_id: ModuleNodeId,
        relation_count: usize,
    ) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "{} {} in module {} ({} relations found)",
            "Resolving segment:".log_header(),
            segment.log_name(),
            search_in_module_id.to_string().log_id(),
            relation_count.to_string().log_id()
        );
    }

    fn log_resolve_segment_relation(&self, target_id: NodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "  {} Relation Target ID: {}",
            "->".log_comment(),
            target_id.to_string().log_id()
        );
    }

    fn log_resolve_segment_found_node(
        &self,
        target_node: &dyn GraphNode,
        segment: &str,
        name_matches: bool,
    ) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "    {} Found Node: '{}' ({}), Name matches '{}': {}",
            "✓".log_green(),
            target_node.name().log_name(),
            target_node.kind().log_vis_debug(),
            segment.log_name(),
            name_matches.to_string().log_vis()
        );
    }

    fn log_resolve_segment_node_error(&self, target_id: NodeId, error: &SynParserError) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "    {} Error finding node for ID {}: {:?}",
            "✗".log_error(),
            target_id.to_string().log_id(),
            error.to_string().log_error() // Use Display impl of the error
        );
    }

    fn log_resolve_segment_target_not_node(&self, target: GraphId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "  {} Relation Target was not GraphId::Node: {:?}",
            "->".log_comment(),
            target.log_id_debug()
        );
    }

    fn log_resolve_segment_failed(&self, segment: &str, search_in_module_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "{} No candidates found for segment '{}' in module {}. Returning error.",
            "Resolution Failed:".log_error(),
            segment.log_name(),
            search_in_module_id.to_string().log_id()
        );
    }

    // --- Logging Helpers for Shortest Public Path (SPP) ---

    fn log_spp_start(&self, item_node: &dyn GraphNode) {
        self.log_bfs_step(item_node, "Starting SPP");
    }

    fn log_spp_item_not_public(&self, item_node: &dyn GraphNode) {
        self.log_bfs_step(item_node, "Item not public, terminating early");
    }

    fn log_spp_check_root(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(current_mod_id.into_inner(), path_to_item, "Check if root");
    }

    fn log_spp_found_root(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(current_mod_id.into_inner(), path_to_item, "Found root!");
    }

    fn log_spp_explore_containment(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(
            current_mod_id.into_inner(),
            path_to_item,
            "Explore Up (Containment)",
        );
    }

    fn log_spp_explore_reexports(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(
            current_mod_id.into_inner(),
            path_to_item,
            "Explore Up (Re-exports)",
        );
    }

    // --- Logging Helpers for explore_up_via_containment ---

    fn log_spp_containment_start(&self, current_mod_node: &ModuleNode) {
        self.log_bfs_step(current_mod_node, "Start explore up");
    }

    fn log_spp_containment_vis_source(&self, current_mod_node: &ModuleNode) {
        self.log_bfs_step(current_mod_node, "Check Vis Source");
    }

    fn log_spp_containment_vis_source_decl(&self, decl_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_VIS, "  {} Visibility source is Declaration: {}", "->".log_comment(), decl_id.to_string().log_id());
    }

    fn log_spp_containment_unlinked(&self, current_mod_id: ModuleNodeId) {
        log::warn!(target: LOG_TARGET_VIS, "SPP: Could not find declaration for file-based module {}, treating as inaccessible upwards.", current_mod_id);
    }

    fn log_spp_containment_vis_source_inline(&self, current_mod_node: &ModuleNode) {
        self.log_bfs_step(current_mod_node, "Inline/root, use self");
    }

    fn log_spp_containment_check_parent(&self, parent_mod_node: &ModuleNode) {
        self.log_bfs_step(parent_mod_node, "Checking Parent");
    }

    fn log_spp_containment_queue_parent(&self, parent_mod_id: ModuleNodeId, new_path: &[String]) {
        self.log_bfs_path(parent_mod_id.into_inner(), new_path, "Queueing Parent");
    }

    fn log_spp_containment_parent_visited(&self, parent_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_BFS, "  {} Parent {} already visited.", "->".log_comment(), parent_mod_id.to_string().log_id());
    }

    fn log_spp_containment_parent_inaccessible(
        &self,
        visibility_source_node: &ModuleNode,
        effective_source_id: ModuleNodeId,
        parent_mod_id: ModuleNodeId,
    ) {
        log::trace!(target: LOG_TARGET_VIS, "SPP: Module {} ({}) not accessible from parent {}, pruning containment path.", visibility_source_node.name().log_name(), effective_source_id.to_string().log_id(), parent_mod_id.to_string().log_id());
    }

    fn log_spp_containment_no_parent(&self, effective_source_id: ModuleNodeId) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "SPP: No parent found for non-root module {} via containment.", effective_source_id.to_string().log_id());
    }

    // --- Logging Helpers for explore_up_via_reexports ---

    fn log_spp_reexport_start(&self, target_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(
            target_id.into_inner(),
            path_to_item,
            "Start Re-export Explore",
        );
    }

    fn log_spp_reexport_missing_import_node(&self, import_node_id: NodeId) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "SPP: ReExport relation points to non-existent ImportNode {}", import_node_id.to_string().log_id());
    }

    fn log_spp_reexport_is_external(&self, import_node: &ImportNode) {
        self.log_bfs_step(import_node, "Is External Crate");
    }

    fn log_spp_reexport_get_import_node(&self, import_node: &ImportNode) {
        self.log_bfs_step(import_node, "Get import node");
    }

    fn log_spp_reexport_not_public(&self, import_node: &ImportNode) {
        self.log_bfs_step(import_node, "!is_public_use");
    }

    fn log_spp_reexport_queue_module(
        &self,
        import_node: &ImportNode,
        reexporting_mod_id: ModuleNodeId,
        new_path: &[String],
    ) {
        self.log_bfs_step(import_node, "Queueing Re-exporting Module");
        self.log_bfs_path(reexporting_mod_id.into_inner(), new_path, "  New Path");
    }

    fn log_spp_reexport_module_visited(&self, reexporting_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_BFS, "  {} Re-exporting module {} already visited.", "->".log_comment(), reexporting_mod_id.to_string().log_id());
    }

    fn log_spp_reexport_no_container(&self, import_node_id: NodeId) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "SPP: No containing module found for ImportNode {}", import_node_id.to_string().log_id());
    }

    // --- Logging Helpers for is_accessible ---

    fn log_access_missing_decl_node(&self, decl_id: NodeId, target_defn_id: NodeId) {
        log::warn!(target: LOG_TARGET_VIS, "Declaration node {} not found for definition {}", decl_id.to_string().log_id(), target_defn_id.to_string().log_id());
    }

    fn log_access_restricted_check_ancestor(
        &self,
        ancestor_id: ModuleNodeId,
        restriction_module_id: ModuleNodeId,
    ) {
        debug!(target: LOG_TARGET_VIS, "  {} Checking ancestor: {} ({}) against restriction: {} ({})",
            "->".dimmed(), // Indentation marker
            self.modules.get(&ancestor_id).map(|m| m.name.as_str()).unwrap_or("?").yellow(), // Ancestor name yellow
            ancestor_id.to_string().magenta(), // Ancestor ID magenta
            self.modules.get(&restriction_module_id).map(|m| m.name.as_str()).unwrap_or("?").blue(), // Restriction name blue
            restriction_module_id.to_string().magenta() // Restriction ID magenta
        );
    }

    // --- Logging Helpers for find_declaring_file_dir ---

    fn log_find_decl_dir_missing_parent(&self, current_id: ModuleNodeId) {
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Inconsistent ModuleTree: Parent not found for module {} during file dir search.", current_id.to_string().log_id());
    }

    // --- Logging Helpers for process_path_attributes ---

    fn log_path_attr_external_not_found(
        &self,
        decl_module_id: ModuleNodeId,
        resolved_path: &PathBuf,
    ) {
        log::warn!(
            target: LOG_TARGET_PATH_ATTR,
            "{} {} | {}",
            "External Path".yellow().bold(), // Use yellow for warning
            format!("({})", decl_module_id).log_id(),
            format!(
                "Target file outside src dir not found: {}",
                resolved_path.display()
            )
            .log_vis()
        );
    }

    // --- Logging Helpers for resolve_single_export ---

    fn log_resolve_single_export_external(&self, segments_to_resolve: &[String]) {
        log::debug!(target: LOG_TARGET_MOD_TREE_BUILD, "Detected external re-export based on first segment: {:?}", segments_to_resolve.log_path_debug());
    }

    /// Wraps a resolution error from `resolve_path_relative_to` into `UnresolvedReExportTarget`.
    fn wrap_resolution_error(
        &self,
        error: ModuleTreeError,
        export_node_id: NodeId,
        original_path_segments: &[String],
    ) -> ModuleTreeError {
        match error {
            // Preserve existing UnresolvedReExportTarget if it came from the helper
            ModuleTreeError::UnresolvedReExportTarget { .. } => error,
            // Otherwise, create a new UnresolvedReExportTarget with the correct path
            _ => ModuleTreeError::UnresolvedReExportTarget {
                import_node_id: Some(export_node_id),
                // Use the original full path for the error message
                path: NodePath::try_from(original_path_segments.to_vec()).unwrap_or_else(|_| {
                    NodePath::new_unchecked(vec!["<invalid path conversion>".to_string()])
                }), // Handle potential error in path conversion for error reporting
            },
        }
    }

    // --- Logging Helpers for update_path_index_for_custom_paths ---

    fn log_update_path_index_entry_exit(&self, is_entry: bool) {
        let action = if is_entry { "Entering" } else { "Finished" };
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} {}",
            action.log_header(),
            "update_path_index_for_custom_paths.".log_name()
        );
    }

    fn log_update_path_index_status(&self, count: Option<usize>) {
        match count {
            Some(n) => {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Found {} modules with path attributes to process for index update.",
                    "Update Path Index:".log_header(),
                    n.to_string().log_id()
                );
            }
            None => {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} No path attributes found, skipping index update.",
                    "Update Path Index:".log_header()
                );
            }
        }
    }

    fn log_update_path_index_processing(&self, decl_mod_id: ModuleNodeId) {
        let decl_mod_name = self
            .modules
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} {} ({})",
            "Processing:".log_step(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_target_error(
        &self,
        decl_mod_id: ModuleNodeId,
        error: &ModuleTreeError,
    ) {
        let decl_mod_name = self
            .modules
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Failed to find CustomPath target for {} ({}): {:?}. Skipping index update.",
            "Error:".log_error(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id(),
            error
        );
    }

    fn log_update_path_index_found_target(
        &self,
        decl_mod_id: ModuleNodeId,
        def_mod_id: ModuleNodeId,
    ) {
        let decl_mod_name = self
            .modules
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        let def_mod_name = self
            .modules
            .get(&def_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Found target: {} ({}) for decl {} ({})",
            "->".log_comment(),
            def_mod_name.log_name(),
            def_mod_id.to_string().log_id(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_paths(
        &self,
        decl_mod_id: ModuleNodeId,
        def_mod_id: ModuleNodeId,
        canonical_path: &NodePath,
        original_path: &NodePath,
    ) {
        // Lookup names using IDs
        let decl_mod_name = self
            .modules
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        let def_mod_name = self
            .modules
            .get(&def_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    {} Decl Mod: {} ({}) -> Canonical Path: {}",
            "->".log_comment(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id(), // Use ID directly
            canonical_path.to_string().log_path()
        );
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    {} Def Mod:  {} ({}) -> Original Path:  {}",
             "->".log_comment(),
            def_mod_name.log_name(),
            def_mod_id.to_string().log_id(), // Use ID directly
            original_path.to_string().log_path()
        );
    }

    fn log_update_path_index_remove(&self, original_path: &NodePath, def_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Removed old path index entry: {} -> {}",
            "✓".log_green(),
            original_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_remove_inconsistency(
        &self,
        removed_id: NodeId,
        original_path: &NodePath,
        expected_def_mod_id: ModuleNodeId,
    ) {
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Path index inconsistency: Removed ID {} for original path {} but expected definition ID {}. This indicates a major inconsistency if the removed ID doesn't match",
            "Error:".log_error(),
            removed_id.to_string().log_id(),
            original_path.to_string().log_path(),
            expected_def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_remove_missing(
        &self,
        original_path: &NodePath,
        def_mod_id: ModuleNodeId,
    ) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Original path {} not found in path_index for removal (Def Mod ID: {}). This might indicate an earlier indexing issue.",
            "Warning:".log_yellow(),
            original_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_insert(&self, canonical_path: &NodePath, def_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Inserted new path index entry: {} -> {}",
            "✓".log_green(),
            canonical_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_reinsert(&self, canonical_path: &NodePath, def_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Re-inserted path index entry (idempotent): {} -> {}",
            "Info:".log_comment(),
            canonical_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_insert_conflict(
        &self,
        canonical_path: &NodePath,
        def_mod_id: ModuleNodeId,
        existing_id: NodeId,
    ) {
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Path index conflict: Tried to insert canonical path {} -> {} but path already mapped to {}. {}",
            "Error:".log_error(),
            canonical_path.to_string().log_path(),
            def_mod_id.to_string().log_id(),
            existing_id.to_string().log_id(),
            "This implies a non-unique canonical path was generated or indexed incorrectly.".log_comment()
        );
    }

    // Removed unused get_module_path_vec and get_root_path methods
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
