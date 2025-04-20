use std::collections::HashMap;

use ploke_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::error::SynParserError;

use super::{
    nodes::{GraphNode, ImportNode, ModuleNode, ModuleNodeId, NodePath},
    relations::{GraphId, Relation, RelationKind},
    types::VisibilityKind,
    CodeGraph,
};

#[derive(Debug, Clone)]
pub struct ModuleTree {
    // ModuleNodeId of the root file-level module, e.g. `main.rs`, `lib.rs`, used to initialize the
    // ModuleTree.
    root: ModuleNodeId,
    // Index of all modules in the merged `CodeGraph`, in a HashMap for efficient lookup
    modules: HashMap<ModuleNodeId, ModuleNode>,
    // Temporary storage for unresolved imports (e.g. `use` statements)
    pending_imports: Vec<PendingImport>,
    // Temporary storage for unresolved exports (e.g. `pub use` statements)
    pending_exports: Vec<PendingExport>,
    // pending_mod_decl: Vec<PendingModDecl>,
    // Reverse indexes
    // HashMap appropriate for many -> few possible mapping
    path_index: HashMap<NodePath, NodeId>,
    tree_relations: Vec<TreeRelation>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingImport {
    module_node_id: ModuleNodeId, // Keep private
    import_node: ImportNode,    // Keep private
}

impl PendingImport {
    pub(crate) fn from_import(import: ImportNode) -> Self { // Make crate-visible if needed internally
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
    export_node: ImportNode,    // Keep private
}

impl PendingExport {
    pub(crate) fn from_export(export: ImportNode) -> Self { // Make crate-visible if needed internally
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

// Define the new ModuleTreeError enum
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ModuleTreeError {
    #[error("Duplicate definition path '{path}' found in module tree. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    DuplicatePath { // Change to a struct variant
        path: NodePath,
        existing_id: NodeId,
        conflicting_id: NodeId,
    },

    #[error("Duplicate module ID found in module tree for ModuleNode: {0:?}")]
    DuplicateModuleId(Box<ModuleNode>), // Box the large ModuleNode

    /// Wraps SynParserError for convenience when using TryFrom<Vec<String>> for NodePath
    #[error("Node path validation error: {0}")]
    NodePathValidation(#[from] SynParserError),

    #[error("Definition not found in path_index for path: {0}")]
    DefinitionNotFound(NodePath), // Store the path that wasn't found

    #[error("Containing module not found for node ID: {0}")]
    ContainingModuleNotFound(NodeId), // Added error variant
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

        let node_path = NodePath::try_from(module.defn_path())?;
        let conflicting_id = module.id(); // ID of the module we are trying to add
        // Use entry API for clarity and efficiency
        match self.path_index.entry(node_path.clone()) { // Clone node_path for the error case
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

        // insert module to tree
        let module_id = ModuleNodeId::new(conflicting_id); // Use the ID we already have
        let dup_node = self.modules.insert(module_id, module);
        if let Some(dup) = dup_node {
            // Box the duplicate node when creating the error variant
            return Err(ModuleTreeError::DuplicateModuleId(Box::new(dup)));
        }

        Ok(())
    }

    /// Builds 'Contains' relations between module declarations and their file-based definitions.
    /// This function assumes the `path_index` has been populated correctly.
    pub fn build_logical_paths(&mut self, modules: &[ModuleNode]) -> Result<(), ModuleTreeError> {
        let mut new_contains: Vec<TreeRelation> = Vec::new();
        for module in modules.iter().filter(|m| m.is_file_based()) {
            let defn_path_vec = module.defn_path();
            let defn_path_slice = defn_path_vec.as_slice();

            // Look up the path in the index.
            let decl_id = self.path_index.get(defn_path_slice).ok_or_else(|| {
                // If not found, create the NodePath for the error message.
                let node_path = NodePath::try_from(defn_path_vec.clone()).unwrap();
                ModuleTreeError::DefinitionNotFound(node_path)
            })?;

            let logical_contains = Relation {
                source: GraphId::Node(module.id()), // Source is the file-based module definition
                target: GraphId::Node(*decl_id), // Target is the module declaration ID found in the index
                kind: RelationKind::ResolvesToDefinition,
            };
            new_contains.push(logical_contains.into());
        }
        self.tree_relations.append(&mut new_contains);
        Ok(()) // Return the generated relations
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

    // Resolves visibility for target node
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

    #[allow(unused_variables)]
    pub fn shortest_public_path(&self, id: NodeId) -> Result<Vec<String>, ModuleTreeError> {
        // Returns the shortest accessible path considering visibility
        todo!()
    }
}

// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
// struct ModuleInfo {
//     path: Vec<String>,
//     contains: Vec<NodeId>, // Immediate children
//     pending_imports: Vec<ImportNode>,
//     resolved_exports: Vec<NodeId>,
// }

// impl ModuleInfo {
// pub fn new() -> Self {
//     Self {
//         path,
//         short_path,
//         visibility,
//         exports,
//         children,
//         source_file,
//     }
// }
//
//     pub fn path(&self) -> &[String] {
//         &self.path
//     }
// }

impl ModuleTree {
    pub fn resolve_path(&self, _path: &[String]) -> Result<ModuleNodeId, Box<SynParserError>> {
        // 1. Try direct canonical path match
        // 2. Check re-exports in parent modules
        // 3. Try relative paths (self/super/crate)
        todo!()
    }
}
