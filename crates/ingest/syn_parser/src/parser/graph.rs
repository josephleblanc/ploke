#[cfg(feature = "visibility_resolution")]
use super::nodes::Visible;
use super::relations::RelationKind;
#[cfg(feature = "visibility_resolution")]
use crate::parser::nodes::NodeId;
#[cfg(feature = "visibility_resolution")]
use crate::parser::nodes::OutOfScopeReason;
#[cfg(feature = "visibility_resolution")]
use crate::parser::nodes::VisibilityResult;
#[cfg(feature = "visibility_resolution")]
use crate::parser::types::VisibilityKind;
use crate::parser::{
    nodes::{FunctionNode, ImplNode, MacroNode, ModuleNode, TraitNode, TypeDefNode, ValueNode},
    relations::Relation,
    types::TypeNode,
};

use serde::{Deserialize, Serialize};

#[cfg(feature = "use_statement_tracking")]
use super::nodes::ImportNode;

#[cfg(feature = "visibility_resolution")]
impl CodeGraph {
    #[cfg(feature = "module_path_tracking")]
    pub fn find_module_by_path(&self, path: &[String]) -> Option<&ModuleNode> {
        self.modules.iter().find(|m| m.path == path)
    }
    /// Resolve whether an item is visible in the given module context
    ///
    /// Assumes the source and target context are both in the user's repository, not project
    /// dependencies.
    ///
    /// # Arguments
    /// * `item_id` - ID of the item to check
    /// * `context_module` - Current module path (e.g. ["crate", "module", "submodule"])
    ///
    /// # Returns
    /// Detailed visibility information including:
    /// - Direct visibility
    /// - Required imports
    /// - Or reason for being out of scope
    pub fn resolve_visibility(
        &self,
        item_id: NodeId,
        context_module: &[String],
    ) -> VisibilityResult {
        let item = match self.find_node(item_id) {
            Some(item) => item,
            None => {
                // WARNING: Should this actually return an error?
                return VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::Private,
                    allowed_scopes: None,
                };
            }
        };
        if let Some(context_module_id) = self.find_module_by_path(context_module).map(|m| m.id) {
            let is_imported = self.relations.iter().any(|r| {
                r.source == context_module_id
                    && r.kind == RelationKind::ModuleImports
                    && r.target == item_id
            });

            if is_imported {
                return VisibilityResult::Direct;
            }
        }

        match item.visibility() {
            VisibilityKind::Public => VisibilityResult::Direct,
            VisibilityKind::Inherited => {
                let item_module = self.get_item_module_path(item_id);
                let context = if context_module.is_empty() {
                    vec!["crate".to_string()]
                } else {
                    context_module.to_vec()
                };

                #[cfg(feature = "verbose_debug")]
                println!(
                    "\nChecking inherited visibility - item module: {:?}, context: {:?}",
                    item_module, context
                );

                // Normalize paths and compare
                if item_module == context {
                    #[cfg(feature = "verbose_debug")]
                    println!("Item is in same module - allowing access");
                    VisibilityResult::Direct
                } else {
                    #[cfg(feature = "verbose_debug")]
                    println!("Item is in different module - blocking access");
                    VisibilityResult::OutOfScope {
                        reason: OutOfScopeReason::Private,
                        allowed_scopes: None,
                    }
                }
            }
            VisibilityKind::Crate => {
                if self.same_crate(context_module) {
                    #[cfg(feature = "verbose_debug")]
                    println!("Item is in same crate - allowing access");
                    VisibilityResult::Direct
                } else {
                    #[cfg(feature = "verbose_debug")]
                    println!("Item is in different crate - denying access");
                    VisibilityResult::OutOfScope {
                        reason: OutOfScopeReason::CrateRestricted,
                        allowed_scopes: None,
                    }
                }
            }
            VisibilityKind::Restricted(path) => {
                if self.is_path_visible(&path, context_module) {
                    #[cfg(feature = "verbose_debug")]
                    println!("Item is visible to context scope - allowing access");
                    VisibilityResult::Direct
                } else {
                    let use_vis_result = self.check_use_statements(item_id, context_module);
                    #[cfg(feature = "verbose_debug")]
                    println!(
                        "use_vis_result of id: {}, in context_module {:?}: {:#?}",
                        item_id, context_module, &use_vis_result
                    );
                    match &use_vis_result {
                        VisibilityResult::NeedsUse(import_path) => {
                            if import_path == context_module {
                                VisibilityResult::Direct
                            } else {
                                VisibilityResult::NeedsUse(import_path.clone())
                            }
                        }
                        _ => {
                            // TODO: Placeholder, should consider more `use` statement variations
                            #[cfg(feature = "verbose_debug")]
                            println!("Item is in different crate - denying access");
                            VisibilityResult::OutOfScope {
                                reason: OutOfScopeReason::SuperRestricted,
                                allowed_scopes: Some(path.to_vec()),
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn debug_print_all_visible(&self) {
        #[cfg(feature = "verbose_debug")]
        {
            let mut all_ids: Vec<(&str, usize)> = vec![];
            all_ids.extend(self.functions.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.impls.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.traits.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.private_traits.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.modules.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.values.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.macros.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.defined_types.iter().map(|def| match def {
                TypeDefNode::Struct(s) => (s.name(), s.id()),
                TypeDefNode::Enum(e) => (e.name(), e.id()),
                TypeDefNode::TypeAlias(a) => (a.name(), a.id()),
                TypeDefNode::Union(u) => (u.name(), u.id()),
            }));
            // Add other fields similarly...
            // missing type_graph (different id generation lineage)
            // missing relations might want to add this, at least for ids (though visible might be a
            //  bit of a shoe-horn)
            // missing use_statements not sure about this one.

            all_ids.sort_by_key(|&(_, id)| id);
            for (name, id) in all_ids {
                println!("id: {}, name: {}", id, name);
            }
        }
    }

    /// Gets the full module path for an item by searching through all modules
    /// Returns ["crate"] if item not found in any module (should only happ for crate root items)
    /// Gets the full module path for an item by following Contains relations
    pub fn get_item_module_path(&self, item_id: NodeId) -> Vec<String> {
        #[cfg(feature = "module_path_tracking")]
        {
            // Find the module that contains this item
            let module_id = self
                .relations
                .iter()
                .find(|r| r.target == item_id && r.kind == RelationKind::Contains)
                .map(|r| r.source);

            if let Some(mod_id) = module_id {
                // Get the module's path
                self.modules
                    .iter()
                    .find(|m| m.id == mod_id)
                    .map(|m| m.path.clone())
                    .unwrap_or_else(|| vec!["crate".to_string()])
            } else {
                // Item not in any module (crate root)
                vec!["crate".to_string()]
            }
        }
        #[cfg(not(feature = "module_path_tracking"))]
        {
            vec!["crate".to_string()]
        }
    }

    fn find_node(&self, item_id: NodeId) -> Option<&dyn Visible> {
        // Check all node collections for matching ID

        self.functions
            .iter()
            .find(|n| n.id == item_id)
            .map(|n| n as &dyn Visible)
            .or_else(|| {
                self.defined_types.iter().find_map(|n| match n {
                    TypeDefNode::Struct(s) if s.id == item_id => Some(s as &dyn Visible),
                    TypeDefNode::Enum(e) if e.id == item_id => Some(e as &dyn Visible),
                    TypeDefNode::TypeAlias(t) if t.id == item_id => Some(t as &dyn Visible),
                    TypeDefNode::Union(u) if u.id == item_id => Some(u as &dyn Visible),
                    _ => None,
                })
            })
            .or_else(|| {
                self.traits
                    .iter()
                    .chain(&self.private_traits)
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn Visible)
            })
            .or_else(|| {
                self.modules
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn Visible)
            })
            .or_else(|| {
                self.values
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn Visible)
            })
            .or_else(|| {
                self.macros
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn Visible)
            })
            // TODO: Kind of a hack, or at least not logically clean - since nodes should really be
            // top-level elements in a vec in the CodeGraph. Just change CodeGraph to have a field
            // for methods already.
            .or_else(|| {
                self.impls.iter().find_map(|i| {
                    i.methods
                        .iter()
                        .find(|n| n.id == item_id)
                        .map(|n| n as &dyn Visible)
                })
            })
    }

    #[cfg(feature = "visibility_resolution")]
    #[allow(unused_variables)]
    fn same_crate(&self, context: &[String]) -> bool {
        // Default true until we handle workspaces
        true
    }

    fn is_path_visible(&self, path: &[String], context: &[String]) -> bool {
        // Check if context starts with path
        context.starts_with(path)
    }

    #[cfg(feature = "use_statement_tracking")]
    fn module_contains(&self, module_id: NodeId, item_id: NodeId) -> bool {
        // Check if module directly contains the item
        if let Some(module) = self.modules.iter().find(|m| m.id == module_id) {
            if module.items.contains(&item_id) {
                return true;
            }
        }

        // Check if module contains the item through nested modules
        self.relations.iter().any(|r| {
            r.source == module_id && r.target == item_id && r.kind == RelationKind::Contains
        })
    }

    #[cfg_attr(
        not(feature = "use_statement_tracking"),
        allow(unused_variables, unused_mut)
    )]
    // TODO: We need to add documentation for this function. It is not currently clear exactly
    // how it will interact with the visibility_resolution function, or what it should be
    // returning when we begin tracking dependency vs user code for the scope of a given item.
    #[cfg(feature = "use_statement_tracking")]
    fn check_use_statements(&self, item_id: NodeId, context_module: &[String]) -> VisibilityResult {
        let context_module_id = match self.find_module_by_path(context_module) {
            Some(m) => m.id,
            None => {
                // Placeholder, probably should replace with error handling,
                // since I'm pretty sure (but not completely sure) that this case means we don't
                // have access to the target context.
                // TODO: Implement proper error handling here.
                return VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::WorkspaceHidden,
                    allowed_scopes: None,
                };
            }
        };

        // Get all ModuleImports relations for this context module
        let import_relations = self
            .relations
            .iter()
            .filter(|r| r.source == context_module_id && r.kind == RelationKind::ModuleImports);

        for rel in import_relations {
            // Check if this is a glob import by looking for a module that contains the target
            let is_glob = self.modules.iter().any(|m| m.id == rel.target);

            if is_glob {
                // For glob imports, check if item is in the imported module
                if self.module_contains(rel.target, item_id) {
                    return VisibilityResult::Direct;
                }
            }
            // Direct import match
            else if rel.target == item_id {
                return VisibilityResult::Direct;
            }
        }

        let item = match self.find_node(item_id) {
            Some(item) => item,
            None => {
                return VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::Private,
                    allowed_scopes: None,
                }
            }
        };

        // Get current module's use statements
        let current_module = self.modules.iter().find(
            |#[cfg_attr(not(feature = "module_path_tracking"), allow(unused_variables))] m| {
                #[cfg(feature = "module_path_tracking")]
                {
                    m.path == context_module
                }
                #[cfg(not(feature = "module_path_tracking"))]
                #[cfg_attr(
                    not(feature = "module_path_tracking"),
                    allow(unused_imports, dead_code)
                )]
                {
                    false
                } // Fallback if module path tracking is disabled
            },
        );

        if let Some(module) = current_module {
            for use_stmt in &module.imports {
                // Check if use statement brings the item into scope
                if use_stmt.path.ends_with(&[item.name().to_string()]) {
                    return VisibilityResult::NeedsUse(use_stmt.path.clone());
                }
            }
        }

        // Default to private if no matching use statement found
        VisibilityResult::OutOfScope {
            reason: OutOfScopeReason::Private,
            allowed_scopes: None,
        }
    }

    #[cfg(not(feature = "use_statement_tracking"))]
    fn check_use_statements(
        &self,
        _item_id: NodeId,
        _context_module: &[String],
    ) -> VisibilityResult {
        // Fallback when use statement tracking is disabled
        VisibilityResult::OutOfScope {
            reason: OutOfScopeReason::Private,
            allowed_scopes: None,
        }
    }
}

// Main structure representing the entire code graph
// Derive Send and Sync automatically since all component types implement them
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeGraph {
    // Functions defined in the code
    pub functions: Vec<FunctionNode>,
    // Types (structs, enums) defined in the code
    pub defined_types: Vec<TypeDefNode>,
    // All observed types, including nested and generic types
    pub type_graph: Vec<TypeNode>,
    // Implementation blocks
    pub impls: Vec<ImplNode>,
    // Public traits defined in the code
    pub traits: Vec<TraitNode>,
    // Private traits defined in the code
    pub private_traits: Vec<TraitNode>,
    // Relations between nodes
    pub relations: Vec<Relation>,
    // Modules defined in the code
    pub modules: Vec<ModuleNode>,
    // Constants and static variables
    pub values: Vec<ValueNode>,
    // Macros defined in the code
    pub macros: Vec<MacroNode>,
    #[cfg(feature = "use_statement_tracking")]
    pub use_statements: Vec<ImportNode>,
}
