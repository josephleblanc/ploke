#[cfg(feature = "visibility_resolution")]
use super::nodes::Visible;
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
use super::nodes::UseStatement;

#[cfg(feature = "visibility_resolution")]
impl CodeGraph {
    /// Resolve whether an item is visible in the given module context
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
                return VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::Private,
                    allowed_scopes: None,
                }
            }
        };

        match item.visibility() {
            VisibilityKind::Public => VisibilityResult::Direct,
            VisibilityKind::Inherited if context_module.is_empty() => VisibilityResult::Direct,
            VisibilityKind::Crate => {
                if self.same_crate(context_module) {
                    VisibilityResult::Direct
                } else {
                    VisibilityResult::OutOfScope {
                        reason: OutOfScopeReason::CrateRestricted,
                        allowed_scopes: None,
                    }
                }
            }
            VisibilityKind::Restricted(path) => {
                if self.is_path_visible(&path, context_module) {
                    VisibilityResult::Direct
                } else {
                    VisibilityResult::OutOfScope {
                        reason: OutOfScopeReason::SuperRestricted,
                        allowed_scopes: Some(path.to_vec()),
                    }
                }
            }
            _ => self.check_use_statements(item_id, context_module),
        }
    }

    #[cfg(feature = "visibility_resolution")]
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
    }

    #[cfg(feature = "workspace_vis")]
    #[allow(unused_variables)]
    fn same_crate(&self, context: &[String]) -> bool {
        // Default true until we handle workspaces
        true
    }

    fn is_path_visible(&self, path: &[String], context: &[String]) -> bool {
        // Check if context starts with path
        context.starts_with(path)
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
    pub use_statements: Vec<UseStatement>,
}
