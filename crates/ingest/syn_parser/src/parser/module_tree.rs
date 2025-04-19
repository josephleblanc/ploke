use std::collections::HashMap;

use ploke_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::error::{ResolutionError, SynParserError};

use super::nodes::{GraphNode, ImportNode, ModuleNode, ModuleNodeId, NodePath};

#[derive(Debug, Clone)]
pub struct ModuleTree {
    root: ModuleNodeId,
    modules: HashMap<ModuleNodeId, ModuleNode>,
    // Temporary storage for unresolved imports (e.g. `use` statements)
    pending_imports: Vec<PendingImport>,
    // pending_mod_decl: Vec<PendingModDecl>,
    // Reverse indexes (built during resolution)
    path_index: HashMap<NodePath, NodeId>,
    export_index: HashMap<NodeId, Vec<ImportNode>>,
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
            // pending_mod_decl: vec![],
            path_index: HashMap::new(),
            export_index: HashMap::new(),
        }
    }

    /// Initial processing of module into the module tree
    pub fn add_module(&mut self, module: ModuleNode) -> Result<(), Box<ModuleTreeError>> {
        let imports = module.imports.clone();
        self.pending_imports.extend(
            imports
                .iter()
                .map(|imp| PendingImport::from_import(imp.clone())),
        );

        let node_path = NodePath::try_from(module.defn_path())?;
        let dup_path = self.path_index.insert(node_path, NodeId::from(module.id));
        if let Some(dup) = dup_path {
            // AI: I'd like you to implement the new error type ModuleTreeError and implement
            // Into<SynParserError> as well as From<ModuleTreeError> for SynParserError with new
            // error types in SynParserError as needed. Put the new ModuleTreeError at the end of
            // this file. AI!
            return Err(Box::new(ModuleTreeError::DuplicatePath(dup)));
        }
        // insert module to tree
        let dup_node = self.modules.insert(ModuleNodeId::new(module.id()), module);
        if let Some(dup) = dup_node {
            return Err(Box::new(ModuleTreeError::DuplicateModuleId(dup)));
        }
        Ok(())
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
    pub fn resolve_path(&self, path: &[String]) -> Result<ModuleNodeId, Box<SynParserError>> {
        // 1. Try direct canonical path match
        // 2. Check re-exports in parent modules
        // 3. Try relative paths (self/super/crate)
        todo!()
    }

    pub fn shortest_public_path(&self, module_id: ModuleNodeId) -> Vec<String> {
        // Returns the shortest accessible path considering visibility
        todo!()
    }
}
