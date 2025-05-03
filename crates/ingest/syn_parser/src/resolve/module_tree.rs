use crate::parser::{
    graph::GraphAccess,
    nodes::{
        AnyNodeId, AsAnyNodeId, ImportNodeId, PrimaryNodeId, PrimaryNodeIdTrait, ReexportNodeId,
        TryFromPrimaryError,
    },
};
pub use colored::Colorize;
use log::debug; // Import the debug macro
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[allow(unused_imports)]
use std::collections::VecDeque;

use crate::{
    error::SynParserError,
    parser::{
        nodes::{
            self, extract_path_attr_from_node, GraphNode, ImportNode, ModuleNode, ModuleNodeId,
            NodePath,
        },
        relations::SyntacticRelation,
        types::VisibilityKind,
        ParsedCodeGraph,
    },
    utils::{
        logging::{LogDataStructure, PathProcessingContext},
        AccLogCtx, LogStyle, LogStyleDebug, LOG_TARGET_MOD_TREE_BUILD, LOG_TARGET_VIS,
    },
};

#[cfg(test)]
pub mod test_interface {

    use super::{ModuleTree, ModuleTreeError, ResolvedItemInfo};
    use crate::parser::{
        nodes::{AnyNodeId, GraphNode, PrimaryNodeId},
        ParsedCodeGraph,
    };

    impl ModuleTree {
        pub fn test_shortest_public_path(
            &self,
            item_id: PrimaryNodeId,
            graph: &ParsedCodeGraph,
        ) -> Result<ResolvedItemInfo, ModuleTreeError> {
            self.shortest_public_path(item_id, graph)
        }

        pub fn test_log_node_id_verbose(&self, node_id: AnyNodeId) {
            self.log_node_id_verbose(node_id);
        }

        pub fn test_validate_unique_rels(&self) {
            self.validate_unique_rels();
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
    path_index: HashMap<NodePath, AnyNodeId>,
    /// Maps declaration module IDs with `#[path]` attributes pointing outside the crate's
    /// `src` directory to the resolved absolute external path. These paths do not have
    /// corresponding `ModuleNode` definitions within the analyzed crate context.
    external_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    /// Separate HashMap for module declarations.
    /// Reverse lookup, but can't be in the same HashMap as the modules that define them, since
    /// they both have the same `path`. This should be the only case in which two items have the
    /// same path.
    decl_index: HashMap<NodePath, ModuleNodeId>,
    tree_relations: Vec<TreeRelation>,
    /// re-export index for faster lookup during visibility resolution.
    reexport_index: HashMap<NodePath, ReexportNodeId>,
    /// Stores resolved absolute paths for modules declared with `#[path]` attributes
    /// that point to files *within* the crate's `src` directory.
    /// Key: ID of the declaration module (`mod foo;`).
    /// Value: Resolved absolute `PathBuf` of the target file.
    found_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    /// Temporarily stores the IDs of module declarations that have a `#[path]` attribute.
    /// Used during the initial tree building phase before paths are fully resolved.
    /// Wrapped in `Option` to allow taking ownership via `take()` during processing.
    pending_path_attrs: Option<Vec<ModuleNodeId>>,

    /// Index mapping a source `NodeId` to a list of indices
    /// into the `tree_relations` vector where that ID appears as the source.
    /// Used for efficient lookup of outgoing relations.
    relations_by_source: HashMap<AnyNodeId, Vec<usize>>,
    /// Index mapping a target `NodeId` to a list of indices
    /// into the `tree_relations` vector where that ID appears as the target.
    /// Used for efficient lookup of incoming relations.
    relations_by_target: HashMap<AnyNodeId, Vec<usize>>,
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
    pub(crate) fn from_import(import_node: ImportNode, containing_mod_id: ModuleNodeId) -> Self {
        // Make crate-visible if needed internally
        PendingImport {
            containing_mod_id,
            import_node,
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
    pub(crate) fn from_export(export: ImportNode, containing_module_id: ModuleNodeId) -> Self {
        // Make crate-visible if needed internally
        PendingExport {
            containing_mod_id: containing_module_id,
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
pub struct TreeRelation(SyntacticRelation); // Keep inner field private

impl TreeRelation {
    pub fn new(relation: SyntacticRelation) -> Self {
        Self(relation)
    }

    /// Returns a reference to the inner `Relation`.
    pub fn rel(&self) -> &SyntacticRelation {
        &self.0
    }
}

impl From<SyntacticRelation> for TreeRelation {
    fn from(value: SyntacticRelation) -> Self {
        Self::new(value)
    }
}

impl LogDataStructure for ModuleTree {}

// Struct to hold info about unlinked modules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedModuleInfo {
    pub module_id: ModuleNodeId,
    pub definition_path: NodePath, // Store the path that couldn't be linked
}

// Define the new ModuleTreeError enum
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ModuleTreeError {
    #[error("Duplicate definition path '{path}' found in module tree. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    DuplicatePath {
        // Change to a struct variant
        path: NodePath,
        existing_id: AnyNodeId,
        conflicting_id: AnyNodeId,
    },
    #[error("Duplicate definition module_id '{module_id}' found in module tree. Existing path attribute: {existing_path}, Conflicting path attribute: {conflicting_path}")]
    DuplicatePathAttribute {
        module_id: ModuleNodeId,
        existing_path: PathBuf,
        conflicting_path: PathBuf,
    },

    #[error("Duplicate module ID found in module tree for ModuleNode: {0:?}")]
    DuplicateModuleId(Box<ModuleNode>), // Box the large ModuleNode

    #[error("Duplicate Contains relation found: {0:?}")]
    DuplicateContains(TreeRelation), // Box the large ModuleNode

    /// Wraps SynParserError for convenience when using TryFrom<Vec<String>> for NodePath
    #[error("Node path validation error: {0}")]
    NodePathValidation(Box<SynParserError>), // Box the recursive type

    #[error("Containing module not found for node ID: {0}")]
    ContainingModuleNotFound(AnyNodeId), // Added error variant

    // NEW: Variant holding a collection of UnlinkedModuleInfo
    // Corrected format string - the caller logs the count/details
    #[error("Found unlinked module file(s) (no corresponding 'mod' declaration).")]
    FoundUnlinkedModules(Box<Vec<UnlinkedModuleInfo>>), // Use Box as requested

    #[error("Item with ID {0} is not publicly accessible from the crate root.")]
    ItemNotPubliclyAccessible(AnyNodeId), // New error variant for SPP

    #[error("Node error: {0}")]
    NodeError(#[from] nodes::NodeError), // Add #[from] for NodeError

    #[error("Syn parser error: {0}")]
    SynParserError(Box<SynParserError>), // REMOVE #[from]
    //
    #[error("Could not determine parent directory for file path: {0}")]
    FilePathMissingParent(PathBuf), // Store the problematic path
    #[error("Root module {0} is not file-based, which is required for path resolution.")]
    RootModuleNotFileBased(ModuleNodeId),
    #[error("Invalid State: Root module {0} not found")]
    RootModuleNotFound(ModuleNodeId),

    // --- NEW VARIANT ---
    #[error("Conflicting re-export path '{path}' detected. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    ConflictingReExportPath {
        path: NodePath,
        existing_id: ReexportNodeId,    // Changed: Use ReexportNodeId
        conflicting_id: ReexportNodeId, // Changed: Use ReexportNodeId
    },

    // --- NEW VARIANT ---
    #[error("Re-export chain starting from {start_node_id} exceeded maximum depth (32). Potential cycle or excessively deep re-export.")]
    ReExportChainTooLong { start_node_id: AnyNodeId },

    #[error("Implement me!")]
    UnresolvedPathAttr(Box<ModuleTreeError>), // Placeholder, fill in with contextual information

    #[error("ModuleId not found in ModuleTree.modules: {0}")]
    ModuleNotFound(ModuleNodeId),

    // --- NEW VARIANTS for process_path_attributes ---
    #[error("Duplicate module definitions found for path attribute target: {0}")]
    DuplicateDefinition(String), // Store detailed message
    #[error("Module definition not found for path attribute target: {0}")]
    ModuleKindinitionNotFound(String), // Store detailed message

    // --- NEW VARIANT ---
    #[error("Shortest public path resolution failed for external item re-export: {0}")]
    ExternalItemNotResolved(AnyNodeId),

    #[error("No relations found for node {0}: {1}")]
    NoRelationsFound(AnyNodeId, String),
    #[error("No relations found for node {0}")]
    NoRelationsFoundForId(AnyNodeId), // Placeholder, trying out copy-only values
    #[error("Could not resolve target for re-export '{path}'. Import Node ID: {import_node_id:?}")]
    UnresolvedReExportTarget {
        import_node_id: Option<AnyNodeId>,
        path: NodePath, // The original path that failed to resolve
    },

    // --- NEW VARIANT ---
    #[error("Invalid internal state: pending_exports was None when adding module {module_id}")]
    InvalidStatePendingExportsMissing { module_id: ModuleNodeId },
    #[error("Internal state error: {0}")]
    InternalState(String),
    #[error("Warning: {0}")]
    Warning(String),

    // --- NEW VARIANT ---
    #[error("Recursion limit ({limit}) exceeded while finding defining file path for node {start_node_id}")]
    RecursionLimitExceeded { start_node_id: AnyNodeId, limit: u8 },
    #[error("Error Converting from {0}")]
    TypedIdConversionError(#[from] TryFromPrimaryError),
}

/// Holds the IDs and relations pruned from the ModuleTree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PruningResult {
    // Renamed struct
    /// IDs of the top-level file ModuleNodes that were pruned because they were unlinked.
    pub pruned_module_ids: HashSet<ModuleNodeId>,
    /// IDs of all items (including the modules themselves and items they contained)
    /// that were associated with the pruned modules.
    pub pruned_item_ids: HashSet<AnyNodeId>, // Changed: Use AnyNodeId
    /// The actual TreeRelation instances that were removed from the ModuleTree.
    pub pruned_relations: Vec<TreeRelation>,
}

// Add near other public structs/enums related to ModuleTree resolution
#[derive(Debug, Clone, PartialEq, Eq)] // Eq requires NodeId and PathBuf to be Eq
pub struct ResolvedItemInfo {
    /// The shortest public module path leading to the item's accessibility point.
    /// Example: `NodePath(["crate", "some_mod"])` for `crate::some_mod::MyItem`.
    pub path: NodePath, // Changed from Vec<String>

    /// The name under which the item is publicly accessible at the end of `path`.
    /// This is the name to use in code (e.g., `MyItem`, `RenamedItem`).
    /// Example: `"MyItem"` or `"RenamedItem"`
    pub public_name: String,

    /// The NodeId of the item ultimately resolved to by the public path.
    /// For internal items, this is the ID of the definition node (e.g., FunctionNode, StructNode).
    /// For external items, this is the ID of the `ImportNode` representing the `pub use`.
    pub resolved_id: AnyNodeId, // Changed: Use AnyNodeId

    /// Provides context about the nature of the `resolved_id`.
    pub target_kind: ResolvedTargetKind,

    /// The original name of the item at its definition site, if it's an internal definition
    /// and its `public_name` is different due to renaming via re-exports.
    /// `None` if the public name matches the definition name, or if the target is external.
    pub definition_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTargetKind {
    /// The `resolved_id` points to an item defined within the current crate.
    InternalDefinition {
        /// The NodeId of the actual definition node (e.g., FunctionNode, StructNode).
        /// This will always match the outer `resolved_id` in this variant.
        definition_id: AnyNodeId, // Changed: Use AnyNodeId
    },
    /// The `resolved_id` points to an `ImportNode` that re-exports an item
    /// from an external crate.
    ExternalReExport {
        /// The path of the item within the external crate (e.g., ["log", "debug"]).
        /// The public name in the external crate is the last segment.
        external_path: Vec<String>,
    },
    // Add other kinds later if needed (e.g., Ambiguous, Private)
}

impl ModuleTreeError {
    pub(crate) fn no_relations_found(g_node: &dyn GraphNode) -> Self {
        Self::NoRelationsFound(
            g_node.any_id(),
            format!(
                "{} {: <12} {: <20} | {: <12} | {: <15}",
                "NodeInfo".log_header(),
                g_node.name().log_name(),
                g_node.any_id().to_string().log_id(),
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
    /// Adds a `ModuleNode` to the `ModuleTree`, updating internal state and indices.
    ///
    /// This function performs several key actions during the initial phase of building the module tree:
    ///
    /// 1. **Stores the Module:** Inserts the provided `ModuleNode` into the `modules` HashMap,
    ///    keyed by its `ModuleNodeId`.
    /// 2. **Indexes Paths:**
    ///    * Adds the module's definition path (`NodePath`) to the appropriate index
    ///      (`path_index` for definitions, `decl_index` for declarations).
    ///    * Checks for duplicate paths and returns `ModuleTreeError::DuplicatePath` if a
    ///      conflict is found.
    ///    * Note: The path->Id indexes for definitions and declarations must be kept separate
    ///      because a module's declaration, e.g. `mod module_a;` and the file-based module this
    ///      declaration points to, e.g. `project/src/module_a.rs` or directory, e.g.
    ///      `project/src/module_a/mod.rs`, have the same canonical path `crate::module_a`.
    /// 3. **Separates Imports/Exports:**
    ///    * Filters the module's `imports` (`use` statements).
    ///    * Adds private imports (`use some::item;` or `extern crate ...;`) identified by
    ///      `is_inherited_use()` to `pending_imports`.
    ///    * Adds all re-exports (`pub use`, `pub(crate) use`, `pub(in path) use`) identified by
    ///      `is_any_reexport()` to `pending_exports`.
    /// 4. **Tracks Path Attributes:** If the module has a `#[path]` attribute, its ID is added to
    ///    `pending_path_attrs` for later resolution.
    /// 5. **Checks for Duplicate IDs:** Returns `ModuleTreeError::DuplicateModuleId` if a module
    ///    with the same ID already exists in the `modules` map.
    ///
    /// # Arguments
    /// * `module`: The `ModuleNode` to add to the tree. The function takes ownership.
    ///
    /// # Returns
    /// * `Ok(())` on successful addition and indexing.
    /// * `Err(ModuleTreeError)` if:
    ///     * The module's path conflicts with an existing entry (`DuplicatePath`).
    ///     * A module with the same ID already exists (`DuplicateModuleId`).
    ///     * The module's path is invalid (`NodePathValidation`).
    ///     * An internal state error occurs (e.g., `InvalidStatePendingExportsMissing`).
    pub fn add_module(&mut self, module: ModuleNode) -> Result<(), ModuleTreeError> {
        let imports = module.imports.clone();
        // Add all private imports
        self.pending_imports.extend(
            // NOTE: We already have `Relation::ModuleImports` created at parsing time.
            imports
                .iter()
                .filter(|imp| imp.is_inherited_use())
                .map(|imp| PendingImport::from_import(imp.clone(), module.id)),
        );

        // Add all re-exports to the Vec inside the Option
        if let Some(exports) = self.pending_exports.as_mut() {
            exports.extend(
                imports
                    .iter()
                    .filter(|imp| imp.is_any_reexport()) // Updated method name
                    .map(|imp| PendingExport::from_export(imp.clone(), module.id)),
            );
        } else {
            // This state is invalid. pending_exports should only be None after process_export_rels
            // has been called and taken ownership. If we are adding a module, it means
            // process_export_rels hasn't run yet (or ran unexpectedly early).
            return Err(ModuleTreeError::InvalidStatePendingExportsMissing {
                module_id: module.id,
            });
        }

        // Use map_err for explicit conversion from SynParserError to ModuleTreeError
        let node_path = NodePath::try_from(module.defn_path().clone())
            .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;
        let conflicting_id = module.id; // ID of the module we are trying to add

        // Separate declaration and definition path->Id indexes.
        // Indexes for declaration vs definition (inline or filebased) must be kept separate to
        // avoid collision, as module definitions and declarations have the same canonical path.
        if module.is_declaration() {
            match self.decl_index.entry(node_path.clone()) {
                // Clone node_path for the error case
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Path already exists
                    let existing_id = *entry.get();
                    return Err(ModuleTreeError::DuplicatePath {
                        path: node_path, // Use the cloned path
                        existing_id: existing_id.as_any(),
                        conflicting_id: conflicting_id.as_any(),
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
                        conflicting_id: conflicting_id.as_any(),
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Path is free, insert it
                    entry.insert(conflicting_id.as_any());
                }
            }
        }

        // Assign new Id wrapper for modules for better type-safety
        self.log_module_insert(&module);

        // Store path attribute if present
        // Index `#[path = "dir/to/file.rs"]` for later processing in `resolve_pending_path_attrs`
        // and `process_path_attributes`
        if module.has_path_attr() {
            self.log_add_pending_path(module.id, &module.name);
            self.pending_path_attrs
                .as_mut()
                .expect("Invariant: pending_path_attrs should always be Some before take()")
                .push(module.id); // clarity. This should be invariant, however.
        }

        // Finally, if no error have been encountered, we insert all modules of any kind to a
        // shared index of ModuleId->ModuleNode for lookup later.
        let dup_node = self.modules.insert(module.id, module);
        if let Some(dup) = dup_node {
            self.log_duplicate(&dup);
            return Err(ModuleTreeError::DuplicateModuleId(Box::new(dup)));
        }

        Ok(())
    }

    pub fn add_relations_batch(
        &mut self,
        relations: &[SyntacticRelation],
    ) -> Result<(), ModuleTreeError> {
        for rel in relations.iter() {
            self.add_rel((*rel).into());
        }
        Ok(())
    }

    pub fn root(&self) -> ModuleNodeId {
        self.root
    }

    pub fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode> {
        &self.modules
    }

    /// Returns a reference to the internal path index mapping canonical paths to NodeIds.
    pub fn path_index(&self) -> &HashMap<NodePath, AnyNodeId> {
        // Changed: Return map with AnyNodeId value
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
        let root_id = root.id;
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
    /// * `source_id`: The NodeId of the source node.
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
        source_id: &AnyNodeId, // Changed: Parameter is AnyNodeId
        relation_filter: F,    // Closure parameter
    ) -> Option<Vec<&TreeRelation>>
    where
        F: Fn(&TreeRelation) -> bool, // Closure takes &TreeRelation, returns bool
    {
        self.relations_by_source.get(source_id).map(|indices| {
            // Changed: Use AnyNodeId key
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
    pub fn get_iter_relations_from<'a>(
        &'a self,
        source_id: &AnyNodeId, // Changed: Parameter is AnyNodeId
    ) -> Option<impl Iterator<Item = &'a TreeRelation>> {
        self.relations_by_source.get(source_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| self.tree_relations.get(index))
        })
    }

    pub fn get_all_relations_from(&self, source_id: &AnyNodeId) -> Option<Vec<&TreeRelation>> {
        // Changed: Parameter is AnyNodeId
        self.relations_by_source.get(source_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| self.tree_relations.get(index))
                .collect()
        })
    }

    pub fn reexport_index(&self) -> &HashMap<NodePath, ReexportNodeId> {
        &self.reexport_index
    }

    /// Finds relations pointing to `target_id` that satisfy the `relation_filter` closure.
    ///
    /// (Doc comments similar to get_relations_from)
    pub fn get_relations_to<F>(
        &self,
        target_id: &AnyNodeId, // Changed: Parameter is AnyNodeId
        relation_filter: F,    // Closure parameter
    ) -> Option<Vec<&TreeRelation>>
    where
        F: Fn(&TreeRelation) -> bool, // Closure takes &TreeRelation, returns bool
    {
        self.relations_by_target.get(target_id).map(|indices| {
            // Changed: Use AnyNodeId key
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
    pub fn get_all_relations_to(&self, target_id: &AnyNodeId) -> Option<Vec<&TreeRelation>> {
        // Changed: Parameter is AnyNodeId
        self.relations_by_target.get(target_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| self.tree_relations.get(index))
                .collect()
        })
    }

    /// Returns an iterator over all `TreeRelation`s pointing to the given `target_id`.
    ///
    /// This provides efficient access to incoming relations without collecting them into a `Vec`.
    /// Filtering by relation kind should be done by the caller on the resulting iterator,
    /// or by using `get_relations_to` with a filter closure.
    ///
    /// # Arguments
    /// * `target_id`: The ID of the target node.
    ///
    /// # Returns
    /// An `Option` containing an iterator yielding `&TreeRelation` if the target ID exists
    /// in the index, otherwise `None`.
    pub fn get_iter_relations_to<'a>(
        &'a self,
        target_id: &AnyNodeId,
    ) -> Option<impl Iterator<Item = &'a TreeRelation>> {
        self.relations_by_target.get(target_id).map(|indices| {
            // Use AnyNodeId key
            // Map indices directly to relation references
            indices
                .iter()
                .filter_map(|&index| self.tree_relations.get(index))
        })
    }

    /// Adds a relation to the tree without checking if the source/target nodes exist.
    pub fn add_rel(&mut self, tr: TreeRelation) {
        let new_index = self.tree_relations.len();
        let relation = tr.rel(); // Get the inner Relation
        let source_id = relation.source(); // Get AnyNodeId
        let target_id = relation.target(); // Get AnyNodeId

        self.tree_relations.push(tr);

        // Update indices using AnyNodeId keys
        self.relations_by_source
            .entry(source_id) // Use AnyNodeId directly
            .or_default()
            .push(new_index);
        self.relations_by_target
            .entry(target_id) // Use AnyNodeId directly
            .or_default()
            .push(new_index);
    }

    /// Adds a relation *between two modules* to the tree, first checking if both the source
    /// and target module nodes exist in the `modules` map.
    ///
    /// This is intended for relations like `ResolvesToDefinition` or `CustomPath` where both
    /// ends are expected to be modules already registered in the tree.
    ///
    /// Returns `ModuleTreeError::ModuleNotFound` if either module is not found.
    /// Returns `ModuleTreeError::InternalState` if the source or target of the relation
    /// cannot be converted to a `ModuleNodeId`.
    pub fn add_new_mod_rel_checked(&mut self, tr: TreeRelation) -> Result<(), ModuleTreeError> {
        let relation = tr.rel();
        let source_any_id = relation.source(); // Returns AnyNodeId
        let target_any_id = relation.target(); // Returns AnyNodeId

        // Attempt to convert source and target to ModuleNodeId
        let source_mod_id = ModuleNodeId::try_from(source_any_id).map_err(|_| {
            ModuleTreeError::InternalState(format!(
                "Source ID {} of relation {:?} is not a ModuleNodeId, cannot use add_new_mod_rel_checked",
                source_any_id, relation
            ))
        })?;
        let target_mod_id = ModuleNodeId::try_from(target_any_id).map_err(|_| {
            ModuleTreeError::InternalState(format!(
                "Target ID {} of relation {:?} is not a ModuleNodeId, cannot use add_new_mod_rel_checked",
                target_any_id, relation
            ))
        })?;

        // Check if both modules exist in the map
        if !self.modules.contains_key(&source_mod_id) {
            return Err(ModuleTreeError::ModuleNotFound(source_mod_id));
        }
        if !self.modules.contains_key(&target_mod_id) {
            return Err(ModuleTreeError::ModuleNotFound(target_mod_id));
        }

        // Checks passed, add the relation using the logic from add_rel
        let new_index = self.tree_relations.len();
        self.tree_relations.push(tr); // Push the original TreeRelation

        // Update indices using the AnyNodeIds obtained earlier
        self.relations_by_source
            .entry(source_any_id)
            .or_default()
            .push(new_index);
        self.relations_by_target
            .entry(target_any_id)
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
    /// Use `add_rel_checked` if such checks are required for individual relations.
    ///
    /// # Arguments
    /// * `relations_iter`: An iterator that yields `Relation` items to be added.
    pub(crate) fn extend_relations<I>(&mut self, relations_iter: I)
    where
        I: IntoIterator<Item = TreeRelation>,
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
        for tr in relations_iter {
            // Convert to TreeRelation (cheap wrapper)
            let source_id = tr.rel().source(); // Get AnyNodeId
            let target_id = tr.rel().target(); // Get AnyNodeId

            // Update the source index HashMap using AnyNodeId key
            // entry().or_default() gets the Vec<usize> for the source_id,
            // creating it if it doesn't exist, then pushes the current_index.
            self.relations_by_source
                .entry(source_id) // Use AnyNodeId directly
                .or_default()
                .push(current_index);

            // Update the target index HashMap similarly using AnyNodeId key
            self.relations_by_target
                .entry(target_id) // Use AnyNodeId directly
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
        target_node_id: ReexportNodeId,
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
            .ok_or_else(|| ModuleTreeError::RootModuleNotFound(self.root))
    }

    /// Finds the absolute file path of the file-based module containing the given primary node ID.
    ///
    /// This traverses upwards from the node's immediate parent module. If the parent
    /// is inline, it continues searching upwards using the `get_parent_module_id` helper
    /// until a file-based module is found.
    // TODO: Handle Assoc/Secondary items in another function.
    // Write another function or method to handle the case of Assoc or Secondary items.
    // This approach will not directly work for Associated or Secondary items, whether or not we
    // include the PrimaryNodeIdTrait here. We may wish to implement a new method that will find
    // the direct parent for any given node (e.g. Assoc nodes like MethodNode) or Secondary Nodes
    // like struct fields, to handle these cases.
    pub fn find_defining_file_path_ref_seq<T: PrimaryNodeIdTrait>(
        &self,
        typed_pid: T,
    ) -> Result<&Path, ModuleTreeError> {
        // 1. Find the immediate parent module ID using the relation index.
        //    We still need this initial lookup as the input is AnyNodeId.
        let initial_parent_mod_id = self
            .get_iter_relations_to(&typed_pid.as_any()) // Use iterator version
            .ok_or(ModuleTreeError::NoRelationsFoundForId(typed_pid.as_any()))? // Use specific error
            .find_map(|tr| tr.rel().source_contains(typed_pid))
            .ok_or(ModuleTreeError::ContainingModuleNotFound(
                typed_pid.as_any(),
            ))?; // Error if no Contains relation found

        let mut current_mod_id = initial_parent_mod_id;
        let mut recursion_limit = 100; // Safety break

        // 2. Loop upwards using get_parent_module_id
        loop {
            recursion_limit -= 1;
            if recursion_limit <= 0 {
                return Err(ModuleTreeError::RecursionLimitExceeded {
                    start_node_id: typed_pid.as_any(), // Use the original input ID
                    limit: 100,
                });
            }

            // Get the current module node using the typed ID
            let module_node = self.get_module_checked(&current_mod_id)?; // Use checked version with ?

            // 3. Check if the current module is file-based
            if let Some(file_path) = module_node.file_path() {
                // Found the defining file
                return Ok(file_path);
            }

            // 4. Check if we've reached the root (and it wasn't file-based, handled above)
            if current_mod_id == self.root {
                // If root is reached and wasn't file-based, it's an error state.
                return Err(ModuleTreeError::RootModuleNotFileBased(self.root));
            }

            // 5. If inline or declaration, find its parent module ID using the helper
            current_mod_id = self.get_parent_module_id(current_mod_id).ok_or_else(|| {
                // If get_parent_module_id returns None (and not at root), the tree is inconsistent
                self.log_find_decl_dir_missing_parent(current_mod_id); // Log helper
                ModuleTreeError::InternalState(format!(
                    "Module tree inconsistent: Parent not found for non-root module {}",
                    current_mod_id
                ))
            })?;
        }
    }

    pub fn resolve_pending_path_attrs(&mut self) -> Result<(), ModuleTreeError> {
        self.log_resolve_entry_exit(true); // Log entry

        let module_ids: Vec<ModuleNodeId> = match self.pending_path_attrs.take() {
            // Changed: Type annotation
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
                return Ok(()); // TODO: This should return error. It means the invariant of the
                               // pending path attrs always being `Some` outside of this function is not being
                               // respected.
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
                    self.log_resolve_step(module_id, "Extract Attr Value", val, false);
                    val
                }
                None => {
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

            match self.found_path_attrs.entry(module_id) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let existing_path = entry.get().clone();
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
            .filter(|m| m.is_file_based() && m.id != root_id)
        {
            // This is ["crate", "renamed_path", "actual_file"] for the file node
            let defn_path = module.defn_path();

            // Log the attempt to find a declaration matching the *file's* definition path
            self.log_path_resolution(module, defn_path, "Checking", Some("decl_index..."));

            match self.decl_index.get(defn_path.as_slice()) {
                Some(decl_id) => {
                    // Found declaration, create relation
                    let resolves_to_rel = SyntacticRelation::ResolvesToDefinition {
                        source: *decl_id,  // Declaration Node (NodeId)
                        target: module.id, // Definition Node (NodeId)
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
                        module_id: module.id,
                        definition_path: node_path,
                    });
                }
            }
        }

        // Append relations regardless of whether unlinked modules were found.
        // We only skip appending if a fatal error occurred earlier (which would have returned Err).
        for relation in new_relations.into_iter() {
            self.add_rel(relation);
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
    pub fn shortest_public_path(
        &self,
        item_pid: PrimaryNodeId, // Changed: Input is AnyNodeId
        graph: &ParsedCodeGraph,
    ) -> Result<ResolvedItemInfo, ModuleTreeError> {
        // --- 1. Initial Setup ---

        let item_any_id = item_pid.as_any();
        let item_node = graph.find_node_unique(item_any_id)?; // Use AnyNodeId for lookup
        if !item_node.visibility().is_pub() {
            // If the item's own visibility isn't Public, it can never be reached.
            self.log_spp_item_not_public(item_node);
            return Err(ModuleTreeError::ItemNotPubliclyAccessible(item_any_id));
            // Use AnyNodeId in error
        }
        let item_name = item_node.name().to_string();

        self.log_spp_start(item_node);

        // Handle special case: asking for the path to the root module itself
        if let Some(module_node) = item_node.as_module() {
            if module_node.id == self.root {
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
        let initial_parent_mod_id = self
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
            self.log_spp_check_root(current_mod_id, &path_to_item);
            if current_mod_id == self.root {
                // Reached the crate root! Construct the final path.
                self.log_spp_found_root(current_mod_id, &path_to_item);
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
                let (resolved_id, target_kind) = match graph.find_node_unique(item_any_id)?.as_import() // Use AnyNodeId
                {
                    // If the original item_any_id points to an ImportNode (meaning it was a re-export)
                    Some(import_node) => {
                        // Check if it's an external re-export
                        if import_node.is_extern_crate()
                            || import_node.source_path().first().map_or(false, |seg| {
                                graph.iter_dependency_names().any(|dep| dep == seg)
                            })
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
                            let reexport_target_id = self
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
                let definition_name =
                    if let ResolvedTargetKind::InternalDefinition { definition_id } = target_kind {
                        graph
                            .find_node_unique(definition_id)? // Use AnyNodeId
                            .name()
                            .ne(&public_name)
                            .then(|| {
                                graph
                                    .find_node_unique(definition_id) // Use AnyNodeId
                                    .unwrap() // Safe unwrap as we just found it
                                    .name()
                                    .to_string()
                            })
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
            ); // Need to handle errors
               // When should this return error for invalid graph state?
        } // End while loop

        // --- 6. Not Found ---
        Err(ModuleTreeError::ItemNotPubliclyAccessible(item_any_id)) // Use AnyNodeId in error
    }

    // Helper function for exploring via parent modules
    fn explore_up_via_containment(
        &self,
        current_mod_id: ModuleNodeId,
        path_to_item: &[String],
        queue: &mut VecDeque<(ModuleNodeId, Vec<String>)>,
        visited: &mut HashSet<ModuleNodeId>,
        graph: &ParsedCodeGraph, // NOTE: Unused variable `graph`. Why is it here?
    ) -> Result<(), ModuleTreeError> {
        // Added Result return

        let current_mod_node = self.get_module_checked(&current_mod_id)?; // O(1)
        self.log_spp_containment_start(current_mod_node);
        // Determine the ID and visibility source (declaration or definition)
        let (effective_source_id, visibility_source_node) =
            if current_mod_node.is_file_based() && current_mod_id != self.root {
                // For file-based modules, find the declaration using AnyNodeId
                let decl_relations = self
                    .get_iter_relations_to(&current_mod_id.as_any()) // Use AnyNodeId
                    .ok_or_else(|| ModuleTreeError::no_relations_found(current_mod_node))?;

                self.log_spp_containment_vis_source(current_mod_node);

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
                    self.log_spp_containment_vis_source_decl(decl_id);
                    (decl_id, self.get_module_checked(&decl_id)?)
                } else {
                    // Unlinked file-based module, treat as private/inaccessible upwards
                    self.log_spp_containment_unlinked(current_mod_id);
                    return Ok(()); // Cannot proceed upwards via containment
                }
            } else {
                self.log_spp_containment_vis_source_inline(current_mod_node);
                // Inline module or root, use itself
                (current_mod_id, current_mod_node)
            };

        // Find the parent of the effective source (declaration or inline module) using AnyNodeId
        let parent_mod_id_opt = self
            .get_iter_relations_to(&effective_source_id.as_any()) // Use AnyNodeId
            .and_then(|iter| {
                iter.find_map(|tr| match tr.rel() {
                    SyntacticRelation::Contains { source, target }
                        if target.base_id() == effective_source_id.base_id() =>
                    // Compare base IDs in case target is AnyNodeId
                    {
                        Some(*source) // Source is ModuleNodeId
                    }
                    _ => None,
                })
            });

        if let Some(parent_mod_id) = parent_mod_id_opt {
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
        // Find ImportNodes that re-export the target_id using AnyNodeId
        // Need reverse ReExport lookup: target = target_id -> source = import_node_id
        let reexporting_imports = self
            .get_iter_relations_to(&target_id.as_any()) // Use AnyNodeId
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
                    self.log_spp_reexport_missing_import_node(import_node_id);
                    continue; // Skip this relation
                }
            };
            // Check for extern crate, return error that needs to be handled by caller.
            if import_node.is_extern_crate() {
                self.log_spp_reexport_is_external(import_node);
                return Err(ModuleTreeError::ExternalItemNotResolved(
                    import_node_id.as_any(), // Use AnyNodeId in error
                ));
            }
            self.log_spp_reexport_get_import_node(import_node);

            // Check if the re-export itself is public (`pub use`, `pub(crate) use`, etc.)
            if !import_node.is_public_use() {
                self.log_spp_reexport_not_public(import_node);
                continue; // Skip private `use` statements
            }

            // Find the module containing this ImportNode using AnyNodeId
            let container_mod_id_opt = self
                .get_iter_relations_to(&import_node_id.as_any()) // Use AnyNodeId
                .and_then(|iter| {
                    iter.find_map(|tr| match tr.rel() {
                        SyntacticRelation::Contains { source, target }
                            if target.base_id() == import_node_id.base_id() =>
                        // Compare base IDs
                        {
                            Some(*source) // Source is ModuleNodeId
                        }
                        _ => None,
                    })
                });

            if let Some(reexporting_mod_id) = container_mod_id_opt {
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

    /// Checks if an item (`target_item_id`) is reachable via a chain of `ReExports` relations
    /// starting from a specific `ImportNode` (`start_import_id`).
    /// Used to detect potential re-export cycles or verify paths.
    #[allow(
        dead_code,
        reason = "May be useful later for cycle detection or validation"
    )]
    fn is_part_of_reexport_chain(
        &self,
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
            let found_direct_reexport = self
                .get_iter_relations_from(&current_import_id.as_any()) // Relations FROM the import node
                .map_or(false, |iter| {
                    iter.any(|tr| match tr.rel() {
                        SyntacticRelation::ReExports { source, target }
                            if *source == current_import_id
                                && target.as_any() == target_item_id =>
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
            let next_import_in_chain = self
                .get_iter_relations_to(&current_import_id.as_any()) // Relations TO the import node
                .and_then(|iter| {
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
            // let relation = Relation { // Relation creation moved to resolve_single_export
            //     // WARNING:
            //     // Bug, currently forms relation with it's containing module, NOT the target that
            //     // the `ImportNode` is actually re-exporting.
            //     source: *source_mod_id.as_inner(), // Use NodeId directly
            //     target: export_node.id, // Use NodeId directly
            //     kind: RelationKind::ReExports,
            // };
            // self.log_rel(relation, None);

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
            self.add_rel(new_tr);
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
                        id => id,
                        // Target is always NodeId now
                        // _ => {
                        //     log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "ReExport relation target is not a NodeId: {:?}", relation);
                        //     // Decide how to handle this unexpected case, maybe continue or return error
                        //     continue;
                        // }
                    };

                    self.log_rel(relation, Some("ReExport Target Resolved")); // Log before potential error

                    // Update the reexport_index: public_path -> target_node_id
                    self.add_reexport_checked(public_reexport_path, target_node_id)?;

                    self.add_rel_checked(relation.into())?;
                    // If index update succeeded, add relation using the unchecked method
                    self.add_rel(relation.into());
                }
                Err(e) => {
                    // Decide error handling: Propagate first error or collect all?
                    // Propagating first error for now.
                    log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Failed to resolve pending export {:#?}: {}", export, e);
                    return Err(e);
                }
            } // End match resolve_single_export
        } // End loop through pending_exports

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
    ) -> Result<(SyntacticRelation, NodePath), ModuleTreeError> {
        // Changed return type
        let source_mod_id = export.containing_mod_id();
        let export_node = export.export_node();
        let export_node_id = export_node.id(); // Get ImportNodeId

        // Always use the original source_path to find the target item
        let target_path_segments = export_node.source_path();

        if target_path_segments.is_empty() {
            return Err(ModuleTreeError::NodePathValidation(Box::new(
                SynParserError::NodeValidation("Empty export path".into()),
            )));
        }

        let first_segment = &target_path_segments[0];

        // --- Delegate ALL path resolution to resolve_path_relative_to ---
        let (base_module_id, segments_to_resolve) = if first_segment == "crate" {
            (self.root(), &target_path_segments[1..]) // Start from root, skip "crate"
        } else {
            (source_mod_id, target_path_segments) // Start from containing mod, use full path
        };

        // Check for external crate re-exports *before* attempting local resolution
        if base_module_id == self.root()
            && !segments_to_resolve.is_empty()
            && graph
                .iter_dependency_names()
                .any(|dep_name| dep_name == segments_to_resolve[0])
        {
            self.log_resolve_single_export_external(segments_to_resolve);
            // Return specific error for external re-exports that SPP might handle later
            return Err(ModuleTreeError::ExternalItemNotResolved(
                export_node_id.as_any(),
            )); // Use AnyNodeId
        }

        // Resolve the path, expecting AnyNodeId
        let target_any_id: AnyNodeId = self // Changed: Expect AnyNodeId
            .resolve_path_relative_to(
                base_module_id,
                segments_to_resolve,
                graph, // Pass graph access
            )
            .map_err(|e| self.wrap_resolution_error(e, export_node_id, target_path_segments))?;

        // --- If target_any_id was found ---

        // Try to convert the resolved AnyNodeId to PrimaryNodeId, as required by ReExports relation
        let target_primary_id = PrimaryNodeId::try_from(target_any_id).map_err(|_| {
            // If conversion fails, it means the resolved item is not a primary node type
            // (e.g., it resolved to a Field or Variant, which cannot be directly re-exported this way).
            log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Re-export target {} resolved to a non-primary node type ({:?}), which is invalid for ReExports relation.", target_any_id, target_any_id);
            ModuleTreeError::UnresolvedReExportTarget {
                path: NodePath::try_from(target_path_segments.to_vec()).unwrap_or_default(), // Provide path context
                import_node_id: Some(export_node_id.as_any()), // Provide import node context
            }
        })?;

        // Create the SyntacticRelation::ReExports
        let relation = SyntacticRelation::ReExports {
            source: export_node_id,    // Source is ImportNodeId
            target: target_primary_id, // Target must be PrimaryNodeId
        };
        self.log_relation(relation, Some("resolve_single_export created relation")); // Use new log helper

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
    ) -> Result<AnyNodeId, ModuleTreeError> {
        // Changed: Return AnyNodeId
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
                return Ok(current_module_id.as_any()); // Changed: Return AnyNodeId
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
                    return Ok(current_module_id.as_any()); // Changed: Return AnyNodeId
                }
            }
        }

        // 3. Iterative Resolution through remaining segments
        let mut resolved_any_id: Option<AnyNodeId> = None; // Changed: Store AnyNodeId

        for (i, segment) in remaining_segments.iter().enumerate() {
            // Determine the module to search within for this segment
            let search_in_module_id = match resolved_any_id {
                Some(any_id) => ModuleNodeId::try_from(any_id).map_err(|_| {
                    // The previously resolved item was not a module, cannot continue path
                    ModuleTreeError::UnresolvedReExportTarget {
                        path: NodePath::try_from(path_segments.to_vec())?,
                        import_node_id: None, // Indicate failure due to non-module segment
                    }
                })?,
                None => current_module_id, // Start in the initial/adjusted module
            };

            // 4. Find items named `segment` directly contained within `search_in_module_id` using AnyNodeId
            let contains_relations = self
                .get_iter_relations_from(&search_in_module_id.as_any()) // Use AnyNodeId
                .map(|iter| iter.collect::<Vec<_>>()) // Collect for logging/multiple checks
                .unwrap_or_default();

            let mut candidates: Vec<AnyNodeId> = Vec::new(); // Changed: Store AnyNodeId
            self.log_resolve_segment_start(segment, search_in_module_id, contains_relations.len());

            for rel_ref in &contains_relations {
                // Iterate by reference
                let target_any_id = rel_ref.rel().target(); // Target is AnyNodeId
                self.log_resolve_segment_relation(target_any_id);
                match graph.find_node_unique(target_any_id) {
                    Ok(target_node) => {
                        let name_matches = target_node.name() == segment;
                        self.log_resolve_segment_found_node(target_node, segment, name_matches);
                        if name_matches {
                            // 5. Visibility Check (Simplified)
                            // Check if the target node is accessible from the module we are searching in.
                            // For modules, use is_accessible. For other items, assume accessible if contained (needs refinement).
                            let is_target_accessible = match ModuleNodeId::try_from(target_any_id) {
                                Ok(target_mod_id) => {
                                    self.is_accessible(search_in_module_id, target_mod_id)
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
        self.tree_relations
            .iter()
            .filter_map(|tr| tr.rel().resolves_to_defn(decl_id))
            .next()
    }

    /// Finds the parent `ModuleNodeId` of a given `ModuleNodeId`.
    ///
    /// Handles both inline modules (direct parent via `Contains`) and file-based modules
    /// (finds the declaration via `ResolvesToDefinition`/`CustomPath`, then finds the declaration's parent).
    fn get_parent_module_id(&self, module_id: ModuleNodeId) -> Option<ModuleNodeId> {
        // First, try finding a direct 'Contains' relation targeting this module_id.
        // This covers inline modules and declarations contained directly.
        let direct_parent = self
            .get_iter_relations_to(&module_id.into())
            .and_then(|iter| {
                iter.find_map(|tr| match tr.rel() {
                    SyntacticRelation::Contains { source, target }
                        if *target == module_id.into() =>
                    {
                        Some(*source) // Source is already ModuleNodeId
                    }
                    _ => None,
                })
            });

        if direct_parent.is_some() {
            return direct_parent;
        }

        // If no direct parent found via Contains, check if `module_id` is a file-based definition.
        // If so, find its declaration and then the declaration's parent.
        self.get_iter_relations_to(&module_id.into())
            .and_then(|iter| {
                iter.find_map(|tr| match tr.rel() {
                    // Find the relation linking a declaration *to* this definition
                    SyntacticRelation::ResolvesToDefinition {
                        source: decl_id,
                        target,
                    }
                    | SyntacticRelation::CustomPath {
                        source: decl_id,
                        target,
                    } if *target == module_id => {
                        // Found the declaration ID (`decl_id`). Now find *its* parent.
                        self.get_iter_relations_to(&decl_id.into())
                            .and_then(|decl_iter| {
                                decl_iter.find_map(|decl_tr| match decl_tr.rel() {
                                    SyntacticRelation::Contains {
                                        source: parent_id,
                                        target: contains_target,
                                    } if contains_target.base_id() == decl_id.base_id() =>
                                    // Compare base IDs just in case target is AnyNodeId
                                    {
                                        Some(*parent_id) // Parent is already ModuleNodeId
                                    }
                                    _ => None,
                                })
                            })
                    }
                    _ => None,
                })
            })
    }

    /// Determines the effective visibility of a module definition for access checks.
    ///
    /// For inline modules or the crate root, it's the visibility stored on the `ModuleNode` itself.
    /// For file-based modules (that are not the root), it's the visibility of the corresponding
    /// `mod name;` declaration statement found via `ResolvesToDefinition` or `CustomPath` relations.
    /// If the declaration cannot be found (e.g., unlinked module file), it defaults to the
    /// visibility stored on the definition node itself (which is typically `Inherited`).
    fn get_effective_visibility(&self, module_def_id: ModuleNodeId) -> Option<&VisibilityKind> {
        let module_node = self.modules.get(&module_def_id)?; // Get the definition node

        // Inline modules and the root module use their own declared visibility.
        if module_node.is_inline() || module_def_id == self.root {
            return Some(module_node.visibility());
        }

        // For file-based modules (not root), find the visibility of the declaration.
        // Find incoming ResolvesToDefinition or CustomPath relations.
        self.get_iter_relations_to(&module_def_id.into())
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
                        self.modules
                            .get(decl_id)
                            .map(|decl_node| decl_node.visibility())
                    }
                    _ => None, // Ignore other relation kinds
                })
            })
            .or_else(|| {
                // If no declaration relation was found (e.g., unlinked module file),
                // fall back to the visibility defined on the module file itself.
                // This usually defaults to Inherited/private.
                self.log_effective_vis_fallback(module_def_id);
                Some(module_node.visibility())
            })
    }

    fn log_effective_vis_fallback(&self, module_def_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_VIS, "  {} No declaration found for file-based module {}. Falling back to definition visibility.",
            "Fallback:".log_yellow(),
            module_def_id.to_string().log_id()
        );
    }

    /// Checks if the `target` module is accessible from the `source` module based on visibility rules.
    pub fn is_accessible(&self, source: ModuleNodeId, target: ModuleNodeId) -> bool {
        // --- Determine Effective Visibility of the Target ---
        // Use the refactored helper function.
        let effective_vis = match self.get_effective_visibility(target) {
            Some(vis) => vis,
            None => {
                // Target module doesn't exist in the tree.
                let log_ctx = AccLogCtx::new(source, target, None, self);
                self.log_access(&log_ctx, "Target Module Not Found", false);
                return false;
            }
        };

        // --- Create Log Context ---
        let log_ctx = AccLogCtx::new(source, target, Some(effective_vis), self);

        // --- Perform Accessibility Check based on Effective Visibility ---
        match effective_vis {
            VisibilityKind::Public => {
                self.log_access(&log_ctx, "Public Visibility", true);
                true // Public is always accessible
            }
            VisibilityKind::Crate => {
                self.log_access(&log_ctx, "Crate Visibility", true);
                true // Crate is always accessible within the same ModuleTree
            }
            VisibilityKind::Restricted(ref restricted_path_vec) => {
                // Attempt to resolve the restriction path to a ModuleNodeId
                let restriction_path = match NodePath::try_from(restricted_path_vec.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        self.log_access(&log_ctx, "Restricted Visibility (Invalid Path)", false);
                        return false; // Invalid path format
                    }
                };

                // Find the module ID corresponding to the restriction path.
                // Check both definition and declaration indices.
                let restriction_module_id_opt = self
                    .path_index // Check definitions first
                    .get(&restriction_path)
                    .and_then(|any_id| ModuleNodeId::try_from(*any_id).ok()) // Convert AnyNodeId
                    .or_else(|| self.decl_index.get(&restriction_path).copied()); // Check declarations

                let restriction_module_id = match restriction_module_id_opt {
                    Some(id) => id,
                    None => {
                        self.log_access(&log_ctx, "Restricted Visibility (Path Not Found)", false);
                        return false; // Restriction path doesn't resolve to a known module
                    }
                };

                // Check 1: Is the source module the restriction module itself?
                if source == restriction_module_id {
                    self.log_access(&log_ctx, "Restricted (Source is Restriction)", true);
                    return true;
                }

                // Check 2: Is the source module a descendant of the restriction module?
                // Traverse upwards from the source using the refactored get_parent_module_id.
                let mut current_ancestor_opt = self.get_parent_module_id(source);
                while let Some(ancestor_id) = current_ancestor_opt {
                    self.log_access_restricted_check_ancestor(ancestor_id, restriction_module_id);
                    if ancestor_id == restriction_module_id {
                        self.log_access(&log_ctx, "Restricted (Ancestor Match)", true);
                        return true; // Found restriction module in ancestors
                    }
                    if ancestor_id == self.root {
                        break; // Reached crate root without finding it
                    }
                    current_ancestor_opt = self.get_parent_module_id(ancestor_id);
                    // Continue upwards
                }

                // If loop finishes without finding the restriction module in ancestors
                self.log_access(&log_ctx, "Restricted (No Ancestor Match)", false);
                false
            }
            VisibilityKind::Inherited => {
                // Inherited means private to the defining module.
                // Access is allowed ONLY if the source *is* the target's direct parent module.
                // Note: `source == target` check is removed; an item cannot access itself via visibility,
                // it's just in scope. Visibility applies to accessing items *from other modules*.
                let target_parent_opt = self.get_parent_module_id(target);
                let accessible = target_parent_opt == Some(source);
                self.log_access(&log_ctx, "Inherited Visibility", accessible);
                accessible
            }
        }
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
                    let target_defn_id = target_defn_node.id; // Get ModuleNodeId
                    let relation = SyntacticRelation::CustomPath {
                        source: *decl_module_id, // Use ModuleNodeId directly
                        target: target_defn_id,  // Use ModuleNodeId directly
                    };
                    self.log_relation(relation, None); // Use new log helper
                                                       // NOTE: Edge Case
                                                       // It is actually valid to have a case of duplicate definitions. We'll
                                                       // need to consider how to handle this case, since it is possible to have an
                                                       // inline module with the `#[path]` attribute that contains items which shadow
                                                       // the items in the linked file, in which case the shadowed items are ignored.
                                                       // For now, just throw error.
                    if let Some(dup) = targets_iter.next() {
                        return Err(ModuleTreeError::DuplicateDefinition(format!(
                        "Duplicate module definition for path attribute target '{}' {}:\ndeclaration: {:#?}\nfirst: {:#?},\nsecond: {:#?}",
                            decl_module_node.id, // Use ModuleNodeId
                        resolved_path.display(),
                            &decl_module_node,
                            &target_defn_node, // Use the found node
                            &dup
                    )));
                    }
                    internal_relations.push(relation); // Push SyntacticRelation
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
                        return Err(ModuleTreeError::ModuleKindinitionNotFound(format!(
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
        // Add relations one by one using add_rel which takes TreeRelation
        for rel in internal_relations {
            self.add_rel(rel.into());
        }
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
        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
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
                    if self.external_path_attrs.contains_key(&decl_mod_id) {
                        // Log a warning using the helper method, but continue processing.
                        self.log_update_path_index_skip_external(decl_mod_id);
                        continue; // Skip to the next decl_mod_id
                    } else {
                        // Log the fatal error using the helper method and abort.
                        self.log_update_path_index_abort_inconsistent(decl_mod_id);
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
            let def_mod_any_id = def_mod_id.as_any(); // Get AnyNodeId
            if let Some(removed_id) = self.path_index.remove(&original_path) {
                if removed_id != def_mod_any_id {
                    // Compare AnyNodeId
                    self.log_update_path_index_remove_inconsistency(
                        removed_id, // This is AnyNodeId
                        &original_path,
                        def_mod_id,
                    );
                    return Err(ModuleTreeError::InternalState(format!("Path index inconsistency during removal for path {}: expected {}, found {}. This suggests the path_index was corrupted earlier.", original_path, def_mod_any_id, removed_id)));
                }
                self.log_update_path_index_remove(&original_path, def_mod_id);
            } else {
                self.log_update_path_index_remove_missing(&original_path, def_mod_id);
            }

            // 5. Insert the new path index entry using the canonical path
            // Use the canonical_path (from the declaration) as the key, mapping to the definition ID (as AnyNodeId).
            if let Some(existing_id) = self
                .path_index
                .insert(canonical_path.clone(), def_mod_any_id)
            // Insert AnyNodeId
            {
                if existing_id != def_mod_any_id {
                    // Compare AnyNodeId
                    self.log_update_path_index_insert_conflict(
                        &canonical_path,
                        def_mod_id,
                        existing_id, // This is AnyNodeId
                    );
                    return Err(ModuleTreeError::DuplicatePath {
                        path: canonical_path,
                        existing_id,                    // AnyNodeId
                        conflicting_id: def_mod_any_id, // AnyNodeId
                    });
                }
                self.log_update_path_index_reinsert(&canonical_path, def_mod_id);
            } else {
                self.log_update_path_index_insert(&canonical_path, def_mod_id);
            }
        }

        self.log_update_path_index_entry_exit(false);

        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        Ok(())
    }

    // Helper function to find the target of a CustomPath relation
    // (Ensure this exists or add it if it doesn't)
    fn find_custom_path_target(
        &self,
        decl_mod_id: ModuleNodeId,
    ) -> Result<ModuleNodeId, ModuleTreeError> {
        self.relations_by_source
         .get(&decl_mod_id.into()) // Changed: Use .into() to get AnyNodeId for lookup
         .and_then(|indices| {
             indices.iter().find_map(|&index| {
                 // Use .get() for safe access and .rel() to get inner Relation
                 let relation = self.tree_relations.get(index)?.rel();
                 // Match on the specific variant to get the correctly typed target
                 match relation {
                     SyntacticRelation::CustomPath { target, .. } => Some(*target), // Target is already ModuleNodeId
                     _ => None,
                 }
             })
         })
         .ok_or_else(|| {
             log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "CustomPath relation target not found for declaration module {}", decl_mod_id);
             // Use a more specific error if available, otherwise adapt ModuleKindinitionNotFound
             ModuleTreeError::ModuleKindinitionNotFound(format!(
                 "Definition module for declaration {} (via CustomPath relation) not found",
                 decl_mod_id
             ))
         })
    }

    /// Removes unlinked file-based modules and their contained items from the tree state.
    ///
    /// This should be called after linking (`link_mods_syntactic`) and path attribute
    /// processing (`process_path_attributes`, `update_path_index_for_custom_paths`)
    /// are complete.
    ///
    /// It identifies file-based modules (excluding the root) that are not targeted by
    /// either a `ResolvesToDefinition` or `CustomPath` relation. These modules, along
    /// with the items they contain, are removed from the `ModuleTree`'s state
    /// (`modules`, `path_index`, `tree_relations`). The relation indices are rebuilt after pruning.
    ///
    /// Note: This function currently does *not* modify the underlying `ParsedCodeGraph`.
    ///
    /// # Returns
    /// A `Result` containing `PruningResult` which lists the IDs of the pruned modules,
    /// all contained items, and the relations that were removed, or a `ModuleTreeError`
    /// if an issue occurs during processing.
    pub(crate) fn prune_unlinked_file_modules(&mut self) -> Result<PruningResult, ModuleTreeError> {
        #[cfg(feature = "validate")]
        let _rels_before = self.validate_unique_rels(); // Store result to avoid unused warning

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Starting pruning of unlinked file modules from ModuleTree...", "Begin".log_header());

        // --- Step 1: Initial Identification of Unlinked File Modules ---
        let mut initial_prunable_module_ids: HashSet<ModuleNodeId> = HashSet::new();
        let root_any_id = self.root.as_any(); // Get AnyNodeId for comparison

        // 1. Identify prunable module IDs
        for (mod_id, module_node) in self.modules.iter() {
            // Skip root and non-file-based modules
            if module_node.id.as_any() == root_any_id || !module_node.is_file_based() {
                continue;
            }

            // Check for incoming ResolvesToDefinition or CustomPath relations using get_relations_to with AnyNodeId
            let is_linked = self
                .get_iter_relations_to(&mod_id.as_any()) // Use AnyNodeId
                .map_or(false, |iter| {
                    iter.any(|tr| {
                        matches!(
                            tr.rel(), // Use rel() to get SyntacticRelation
                            SyntacticRelation::ResolvesToDefinition { .. }
                                | SyntacticRelation::CustomPath { .. }
                        )
                    })
                });

            if !is_linked {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Marking initially unlinked module for pruning: {} ({})", module_node.name.log_name(), mod_id.to_string().log_id());
                initial_prunable_module_ids.insert(*mod_id);
            }
        }

        if initial_prunable_module_ids.is_empty() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} No unlinked modules found to prune.", "Info".log_comment());
            return Ok(PruningResult::default()); // Return empty PruningResult
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Identified {} initially unlinked modules.", initial_prunable_module_ids.len().to_string().log_id());

        // --- Step 2: Recursively Collect All Items Defined Within Unlinked Files ---
        // Changed: Use AnyNodeId for the set and queue
        let mut all_prunable_item_ids: HashSet<AnyNodeId> = initial_prunable_module_ids
            .iter()
            .map(|id| id.as_any()) // Convert ModuleNodeId to AnyNodeId
            .collect();
        // Queue for BFS traversal, starting with the initial unlinked module IDs as AnyNodeId
        let mut queue: VecDeque<AnyNodeId> = all_prunable_item_ids.iter().copied().collect();

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Recursively finding items contained within unlinked modules...");
        while let Some(current_any_id) = queue.pop_front() {
            // Find items directly contained by the current node using AnyNodeId
            if let Some(contained_relations) = self.get_iter_relations_from(&current_any_id) {
                for rel in contained_relations {
                    // Get target AnyNodeId using the helper method
                    let target_any_id = rel.rel().target();
                    // If this target item is newly discovered as prunable...
                    if all_prunable_item_ids.insert(target_any_id) {
                        // ...add it to the queue to explore its contents
                        // (Important if it's an inline module defined within the unlinked file)
                        queue.push_back(target_any_id);
                    }
                }
            }
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Collected {} total item IDs (including modules) to prune via containment.", all_prunable_item_ids.len().to_string().log_id());

        // --- Step 3: Identify Relations to Prune ---
        let mut relations_to_prune_indices = HashSet::new();
        for (index, tr) in self.tree_relations.iter().enumerate() {
            // Get source and target AnyNodeIds using helper methods
            let source_any_id = tr.rel().source();
            let target_any_id = tr.rel().target();

            // Check if either source or target is in the set of prunable AnyNodeIds
            let source_is_pruned = all_prunable_item_ids.contains(&source_any_id);
            let target_is_pruned = all_prunable_item_ids.contains(&target_any_id);

            if source_is_pruned || target_is_pruned {
                relations_to_prune_indices.insert(index);
            }
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Identified {} relations to prune.", relations_to_prune_indices.len());

        // --- Step 4: Prune ModuleTree state ---
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruning ModuleTree structures...");
        // Prune modules based on the initial set of unlinked module IDs
        let modules_before = self.modules.len();
        self.modules
            .retain(|id, _| !initial_prunable_module_ids.contains(id));
        let modules_after = self.modules.len();
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruned modules map: {} -> {} (removed {})", modules_before, modules_after, modules_before - modules_after);

        // Prune path_index: Remove entries whose VALUE (AnyNodeId) is in the full prunable item set
        let path_index_before = self.path_index.len();
        self.path_index
            .retain(|_path, any_node_id| !all_prunable_item_ids.contains(any_node_id)); // Use AnyNodeId directly
        let path_index_after = self.path_index.len();
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruned path_index: {} -> {} (removed {})", path_index_before, path_index_after, path_index_before - path_index_after);

        // Prune decl_index: Remove entries whose VALUE (ModuleNodeId) corresponds to an AnyNodeId in the prunable set
        let decl_index_before = self.decl_index.len();
        self.decl_index.retain(|_path, module_node_id| {
            !all_prunable_item_ids.contains(&module_node_id.as_any())
        }); // Convert to AnyNodeId for check
        let decl_index_after = self.decl_index.len();
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruned decl_index: {} -> {} (removed {})", decl_index_before, decl_index_after, decl_index_before - decl_index_after);

        // Collect the actual relations being pruned before modifying tree_relations
        let pruned_relations: Vec<TreeRelation> = relations_to_prune_indices
            .iter()
            .filter_map(|&index| self.tree_relations.get(index).copied()) // Copy the TreeRelation
            .collect();

        // Filter tree_relations: Keep only those whose index is NOT in relations_to_prune_indices
        let original_relation_count = self.tree_relations.len();
        let mut i = 0;
        self.tree_relations.retain(|_| {
            let keep = !relations_to_prune_indices.contains(&i);
            i += 1;
            keep
        });
        let removed_relation_count = original_relation_count - self.tree_relations.len();
        // Sanity check
        assert_eq!(
            removed_relation_count,
            pruned_relations.len(),
            "Mismatch between counted pruned relations and collected pruned relations"
        );
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Removed {} relations involving pruned items.", removed_relation_count);

        // --- Step 5: Rebuild relation indices ---
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Rebuilding relation indices...");
        self.relations_by_source.clear();
        self.relations_by_target.clear();
        // Iterate over the *filtered* tree_relations
        for (index, tr) in self.tree_relations.iter().enumerate() {
            // Use AnyNodeId keys for indices by calling helper methods
            self.relations_by_source
                .entry(tr.rel().source()) // Use AnyNodeId directly
                .or_default()
                .push(index);
            self.relations_by_target
                .entry(tr.rel().target()) // Use AnyNodeId directly
                .or_default()
                .push(index);
        }

        // --- Step 6: Prepare return data ---
        let result_data = PruningResult {
            pruned_module_ids: initial_prunable_module_ids, // Return the IDs of the modules that triggered pruning
            pruned_item_ids: all_prunable_item_ids, // Return all items that were pruned as a result (as AnyNodeId)
            pruned_relations,                       // Return the actual relations removed
        };

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Finished pruning ModuleTree state. Pruned {} modules, {} total items, and {} relations.", "End".log_header(), result_data.pruned_module_ids.len(), result_data.pruned_item_ids.len(), result_data.pruned_relations.len());

        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        Ok(result_data)
    }

    pub(crate) fn validate_unique_rels(&self) -> bool {
        let rels = &self.tree_relations();
        let unique_rels = rels.iter().fold(Vec::new(), |mut acc, rel| {
            if !acc.contains(rel) {
                acc.push(*rel);
            } else {
                self.log_relation_verbose(*rel.rel());
            }
            acc
        });
        unique_rels.len() == rels.len()
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

    fn log_update_path_index_skip_external(&self, decl_mod_id: ModuleNodeId) {
        let decl_mod_name = self
            .modules
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Skipping index update for external path declaration {} ({}). Target not found as expected.",
            "Warning:".log_yellow(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_abort_inconsistent(&self, decl_mod_id: ModuleNodeId) {
        let decl_mod_name = self
            .modules
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Aborting path index update: Inconsistent state. CustomPath target not found for internal declaration {} ({}), but it wasn't marked as external.",
            "Error:".log_error(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    /// Logs detailed information about a relation for debugging purposes.
    /// This function is intended for verbose debugging and may perform lookups.
    pub fn log_relation_verbose(&self, rel: Relation) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Relation Details:", "Verbose Log:".log_header());
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Kind: {}", format!("{:?}", rel.kind).log_name());

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Source:");
        self.log_node_id_verbose(rel.source.into()); // Changed: Convert NodeId to AnyNodeId

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Target:");
        self.log_node_id_verbose(rel.target.into()); // Changed: Convert NodeId to AnyNodeId
    }

    /// Logs detailed information about a NodeId for debugging purposes.
    /// This function is intended for verbose debugging and may perform lookups within the ModuleTree.
    pub(crate) fn log_node_id_verbose(&self, node_id: AnyNodeId) {
        // Changed: Parameter is AnyNodeId
        // Try to convert AnyNodeId to ModuleNodeId for module lookup
        let mod_id_result: Result<ModuleNodeId, _> = node_id.try_into();

        if let Ok(mod_id) = mod_id_result {
            if let Some(module) = self.modules.get(&mod_id) {
                // Changed: Use typed ID key
                // Log ModuleNode details
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    ID: {} ({})", node_id.to_string().log_id(), "Module".log_spring_green());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Name: {}", module.name.log_name());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Path: {}", module.path.join("::").log_path());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Visibility: {}", format!("{:?}", module.visibility).log_vis());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Kind: {}", crate::utils::logging::get_module_def_kind_str(module).log_orange());
                if let Some(fp) = module.file_path() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      File Path: {}", fp.display().to_string().log_path());
                }
                if let Some(span) = module.inline_span() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Inline Span: {:?}", span);
                }
                if let Some(span) = module.declaration_span() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Decl Span: {:?}", span);
                }
                if !module.cfgs.is_empty() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      CFGs: {}", module.cfgs.join(", ").log_magenta());
                }
            } else {
                // Node ID was convertible to ModuleNodeId but not found in map
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    ID: {} ({})", node_id.to_string().log_id(), "Module (Not Found in Map)".log_comment());
            }
        } else {
            // Node is not a module found in self.modules
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    ID: {} ({})", node_id.to_string().log_id(), "Node (Non-Module)".log_comment());
        }

        // Check pending imports/exports using AnyNodeId
        let is_in_pending_import = self
            .pending_imports
            .iter()
            .any(|p| p.import_node().id.as_any() == node_id); // Changed: Compare AnyNodeId
        if is_in_pending_import {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_imports", "Found".log_yellow());
        }
        if let Some(exports) = self.pending_exports.as_deref() {
            let is_in_pending_export = exports
                .iter()
                .any(|p| p.export_node().id.as_any() == node_id); // Changed: Compare AnyNodeId
            if is_in_pending_export {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_exports", "Found".log_yellow());
            }
        }

        // Log relations FROM this node using AnyNodeId
        if let Some(relations_from) = self.get_all_relations_from(&node_id) {
            // Changed: Use AnyNodeId
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From ({}):", relations_from.len());
            for rel_ref in relations_from {
                // Target is always NodeId now
                let target_id: AnyNodeId = rel_ref.rel().target.into(); // Changed: Convert target NodeId to AnyNodeId
                let target_id_str = target_id.to_string().log_id();
                // Try to get target name if it's a module
                let target_name = ModuleNodeId::try_from(target_id) // Changed: TryFrom AnyNodeId
                    .ok()
                    .and_then(|mid| self.modules.get(&mid))
                    .map(|m| m.name.as_str());
                let target_display = target_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| target_id_str.to_string());

                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      -> {:<18} {}", format!("{:?}", rel_ref.rel().kind).log_name(), target_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From: {}", "None".log_error());
        }

        // Log relations TO this node using AnyNodeId
        if let Some(relations_to) = self.get_all_relations_to(&node_id) {
            // Changed: Use AnyNodeId
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To ({}):", relations_to.len());
            for rel_ref in relations_to {
                // Source is always NodeId now
                let source_id: AnyNodeId = rel_ref.rel().source.into(); // Changed: Convert source NodeId to AnyNodeId
                let source_id_str = source_id.to_string().log_id();
                // Try to get source name if it's a module
                let source_name = ModuleNodeId::try_from(source_id) // Changed: TryFrom AnyNodeId
                    .ok()
                    .and_then(|mid| self.modules.get(&mid))
                    .map(|m| m.name.as_str());
                let source_display = source_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| source_id_str.to_string());

                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      <- {:<18} {}", format!("{:?}", rel_ref.rel().kind).log_name(), source_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To: {}", "None".log_error());
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Visibility: {}", format!("{:?}", module.visibility).log_vis());
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Kind: {}", crate::utils::logging::get_module_def_kind_str(module).log_orange());
        if let Some(fp) = module.file_path() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      File Path: {}", fp.display().to_string().log_path());
        }
        if let Some(span) = module.inline_span() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Inline Span: {:?}", span);
        }
        if let Some(span) = module.declaration_span() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Decl Span: {:?}", span);
        }
        if !module.cfgs.is_empty() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      CFGs: {}", module.cfgs.join(", ").log_magenta());
        }

        // Check pending imports/exports
        let is_in_pending_import = self
            .pending_imports
            .iter()
            .any(|p| p.import_node().id == node_id);
        if is_in_pending_import {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_imports", "Found".log_yellow());
        }
        if let Some(exports) = self.pending_exports.as_deref() {
            let is_in_pending_export = exports.iter().any(|p| p.export_node().id == node_id);
            if is_in_pending_export {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_exports", "Found".log_yellow());
            }
        }

        // Log relations FROM this node using NodeId
        if let Some(relations_from) = self.get_all_relations_from(&node_id) {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From ({}):", relations_from.len());
            for rel_ref in relations_from {
                // Target is always NodeId now
                let target_id = rel_ref.rel().target;
                let target_id_str = target_id.to_string().log_id();
                // Try to get target name if it's a module
                let target_name = self
                    .modules
                    .get(&ModuleNodeId::new(target_id))
                    .map(|m| m.name.as_str());
                let target_display = target_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| target_id_str.to_string());

                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      -> {:<18} {}", format!("{:?}", rel_ref.rel().kind).log_name(), target_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From: {}", "None".log_error());
        }

        // Log relations TO this node using NodeId
        if let Some(relations_to) = self.get_all_relations_to(&node_id) {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To ({}):", relations_to.len());
            for rel_ref in relations_to {
                // Source is always NodeId now
                let source_id = rel_ref.rel().source;
                let source_id_str = source_id.to_string().log_id();
                // Try to get source name if it's a module
                let source_name = self
                    .modules
                    .get(&ModuleNodeId::new(source_id))
                    .map(|m| m.name.as_str());
                let source_display = source_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| source_id_str.to_string());

                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      <- {:<18} {}", format!("{:?}", rel_ref.rel().kind).log_name(), source_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To: {}", "None".log_error());
        }
    }
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
