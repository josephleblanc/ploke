use std::collections::HashMap;

use ploke_core::{IdTrait, NodeId};
use serde::{Deserialize, Serialize};

use crate::error::SynParserError;

use super::{
    nodes::{GraphNode, ImportNode, ModuleNode, ModuleNodeId, NodePath},
    relations::{GraphId, Relation, RelationKind},
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
    path_index: HashMap<NodePath, NodeId>,
}

// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
// pub struct PendingModDecl {
//     parent_mod_id: ModuleNodeId,
//     parent_path: NodePath,
//     child_decl_id: ModuleNodeId,
//     child_defn_id: Option<ModuleNodeId>,
//     child_defn_path: NodePath,
// }
// impl PendingModDecl {
//     pub fn from_module(module: &ModuleNode) -> Option<Self> {
//         todo!()
//     }
// }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingImport {
    module_node_id: ModuleNodeId,
    import_node: ImportNode,
}

impl PendingImport {
    fn from_import(import: ImportNode) -> Self {
        PendingImport {
            module_node_id: ModuleNodeId::new(import.id),
            import_node: import,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingExport {
    module_node_id: ModuleNodeId,
    export_node: ImportNode,
}

impl PendingExport {
    fn from_export(export: ImportNode) -> Self {
        PendingExport {
            module_node_id: ModuleNodeId::new(export.id),
            export_node: export,
        }
    }
}

// Define the new ModuleTreeError enum
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ModuleTreeError {
    #[error("Duplicate path found in module tree for NodeId: {0}")]
    DuplicatePath(NodeId),

    #[error("Duplicate module ID found in module tree for ModuleNode: {0:?}")]
    DuplicateModuleId(Box<ModuleNode>), // Box the large ModuleNode

    /// Wraps SynParserError for convenience when using TryFrom<Vec<String>> for NodePath
    #[error("Node path validation error: {0}")]
    NodePathValidation(#[from] SynParserError),

    #[error("Definition not found in path_index for path: {0}")]
    DefinitionNotFound(NodePath), // Store the path that wasn't found
}

impl ModuleTree {
    pub fn root(&self) -> ModuleNodeId {
        self.root
    }

    pub fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode> {
        &self.modules
    }

    pub fn new_from_root(root: ModuleNodeId) -> Self {
        Self {
            root,
            modules: HashMap::new(),
            pending_imports: vec![],
            pending_exports: vec![],
            // pending_mod_decl: vec![],
            path_index: HashMap::new(),
        }
    }

    pub fn add_module(&mut self, module: ModuleNode) -> Result<(), ModuleTreeError> {
        let imports = module.imports.clone();
        self.pending_imports.extend(
            imports
                .iter()
                .filter(|imp| imp.is_inherited_use())
                .map(|imp| PendingImport::from_import(imp.clone())),
        );
        self.pending_exports.extend(
            imports
                .iter()
                .filter(|imp| imp.is_reexport())
                .map(|imp| PendingExport::from_export(imp.clone())),
        );

        let node_path = NodePath::try_from(module.defn_path())?;
        // Use module.id() directly, removing the useless NodeId::from() conversion.
        let dup_path = self.path_index.insert(node_path, module.id());
        if let Some(dup) = dup_path {
            return Err(ModuleTreeError::DuplicatePath(dup));
        }
        // insert module to tree
        let module_id = ModuleNodeId::new(module.id()); // Store ID before potential move
        let dup_node = self.modules.insert(module_id, module);
        if let Some(dup) = dup_node {
            // Box the duplicate node when creating the error variant
            return Err(ModuleTreeError::DuplicateModuleId(Box::new(dup)));
        }

        Ok(())
    }

    /// Builds 'Contains' relations between module declarations and their file-based definitions.
    /// This function assumes the `path_index` has been populated correctly.
    pub fn build_file_rels(&self, graph: &CodeGraph) -> Result<Vec<Relation>, ModuleTreeError> {
        let mut new_contains: Vec<Relation> = Vec::new();
        for module in graph.modules.iter().filter(|m| m.is_file_based()) {
            let defn_path_vec = module.defn_path();
            let defn_path_slice = defn_path_vec.as_slice();

            // Look up the path in the index.
            let decl_id_ref = self.path_index.get(defn_path_slice).ok_or_else(|| {
                // If not found, create the NodePath for the error message.
                // This assumes NodePath::try_from won't fail here if it succeeded during insertion.
                // Consider adding a direct conversion or handling potential error.
                let node_path = NodePath::try_from(defn_path_vec.clone())
                    .expect("Failed to create NodePath from defn_path during error generation"); // Or handle error properly
                ModuleTreeError::DefinitionNotFound(node_path)
            })?; // Use `?` to propagate the error

            // `decl_id_ref` is `&NodeId`. We need the actual `NodeId` value.
            let decl_id = *decl_id_ref;

            new_contains.push(Relation {
                source: GraphId::Node(module.id()), // Source is the file-based module definition
                target: GraphId::Node(decl_id),     // Target is the module declaration ID found in the index
                kind: RelationKind::Contains,       // Represents that the declaration "contains" the definition conceptually
            });
        }
        Ok(new_contains) // Return the generated relations
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

    pub fn shortest_public_path(&self, _module_id: ModuleNodeId) -> Vec<String> {
        // Returns the shortest accessible path considering visibility
        todo!()
    }
}
