use ploke_core::NodeId;

use crate::parser::nodes::{ModuleDef, ModuleNode};
use crate::parser::types::VisibilityKind;
use crate::parser::visibility::VisibilityResult;
use std::collections::HashMap;

pub struct CanonicalPathResolver<'a> {
    modules: HashMap<NodeId, &'a ModuleNode>,
    visibility_map: HashMap<NodeId, VisibilityKind>,
    crate_root: NodeId,
}

impl<'a> CanonicalPathResolver<'a> {
    pub fn new(root_module: &'a ModuleNode) -> Self {
        let mut resolver = Self {
            modules: HashMap::new(),
            visibility_map: HashMap::new(),
            crate_root: root_module.id,
        };
        resolver.build_maps(root_module);
        resolver
    }

    // Depth-first traversal to build lookup maps
    fn build_maps(&mut self, module: &'a ModuleNode) {
        self.modules.insert(module.id, module);
        self.visibility_map.insert(module.id, module.visibility);

        match &module.module_def {
            ModuleDef::FileBased { items, .. } | ModuleDef::Inline { items, .. } => {
                for &item_id in items {
                    if let Some(child_mod) = self.get_module(item_id) {
                        self.build_maps(child_mod);
                    }
                }
            }
            ModuleDef::Declaration { .. } => {}
        }
    }

    pub fn canonical_path(&self, module: &ModuleNode) -> Vec<String> {
        let mut path = Vec::new();
        self.build_path(module.id, &mut path);
        path
    }

    fn build_path(&self, node_id: NodeId, path: &mut Vec<String>) {
        // Stop at crate root
        if node_id == self.crate_root {
            path.push("crate".to_string());
            return;
        }

        let module = self.modules[&node_id];

        // Find containing module (BFS up the tree)
        let mut current = node_id;
        while let Some(parent_id) = self.find_containing_module(current) {
            let parent = self.modules[&parent_id];

            // Handle path attributes
            if let Some(override_path) = self.get_path_override(parent) {
                path.extend(override_path.iter().cloned());
                break;
            }

            // Standard case
            path.push(parent.name.clone());
            current = parent_id;
        }

        path.reverse();
    }

    fn find_containing_module(&self, node_id: NodeId) -> Option<NodeId> {
        // In real implementation, query CodeGraph relations
        self.modules
            .values()
            .find(|m| match &m.module_def {
                ModuleDef::FileBased { items, .. } | ModuleDef::Inline { items, .. } => {
                    items.contains(&node_id)
                }
                _ => false,
            })
            .map(|m| m.id)
    }

    fn get_path_override(&self, module: &ModuleNode) -> Option<Vec<String>> {
        module
            .attributes
            .iter()
            .find(|a| a.name == "path")
            .and_then(|a| a.value.as_ref())
            .map(|p| p.split("::").map(|s| s.to_string()).collect())
    }

    fn get_module(&self, item_id: NodeId) -> Option<&ModuleNode> {}
}
