use crate::parser::relations::GraphId;
use ploke_core::NodeId;

use super::nodes::GraphNode;
use super::relations::RelationKind;
use crate::parser::nodes::VisibilityResult;
use crate::parser::{
    nodes::{FunctionNode, ImplNode, MacroNode, ModuleNode, TraitNode, TypeDefNode, ValueNode},
    relations::Relation,
    types::TypeNode,
};

use serde::{Deserialize, Serialize};

use super::nodes::ImportNode;

impl CodeGraph {
    pub fn find_module_by_path(&self, path: &[String]) -> Option<&ModuleNode> {
        self.modules.iter().find(|m| m.path == path)
    }

    /// Gets the full module path for an item by searching through all modules
    /// Returns ["crate"] if item not found in any module (should only happ for crate root items)
    pub fn debug_print_all_visible(&self) {
        #[cfg(feature = "verbose_debug")]
        {
            // New implementation using NodeId enum
            let mut all_ids: Vec<(&str, NodeId)> = vec![]; // Collect NodeId enum
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

            // NodeId enum derives Ord, so sorting should work
            all_ids.sort_by_key(|&(_, id)| id);
            for (name, id) in all_ids {
                println!("id: {:?}, name: {}", id, name); // Use Debug print for NodeId enum
            }
        }
    }

    pub fn get_item_module_path(&self, item_id: NodeId) -> Vec<String> {
        // Find the module that contains this item
        let module_id = self
            .relations
            .iter()
            .find(|r| r.target == GraphId::Node(item_id) && r.kind == RelationKind::Contains) // Compare target with GraphId::Node
            .map(|r| r.source); // Source should be GraphId::Node(module_id)

        if let Some(GraphId::Node(mod_id)) = module_id {
            // Unwrap GraphId::Node
            // Get the module's path
            self.modules
                .iter()
                .find(|m| m.id == mod_id) // Compare NodeId == NodeId
                .map(|m| m.path.clone())
                .unwrap_or_else(|| vec!["crate".to_string()]) // Should not happen if relation exists
        } else {
            // Item not in any module (crate root) or source wasn't a Node
            vec!["crate".to_string()]
        }
    }

    pub fn get_item_module(&self, item_id: NodeId) -> &ModuleNode {
        // Find the module that contains this item
        let module_id = self
            .relations
            .iter()
            .find(|r| r.target == GraphId::Node(item_id) && r.kind == RelationKind::Contains)
            .map(|r| r.source);

        if let Some(mod_id) = module_id {
            // Get the module's path
            self.modules
                .iter()
                .find(|m| GraphId::Node(m.id) == mod_id)
                .unwrap_or_else(|| panic!("No containing module found"))
        } else {
            panic!("No containing module found");
        }
    }

    pub fn find_containing_mod_id(&self, node_id: NodeId) -> Option<NodeId> {
        self.relations
            .iter()
            .find(|m| m.target == GraphId::Node(node_id))
            .map(|r| match r.source {
                GraphId::Node(node_id) => node_id,
                GraphId::Type(_type_id) => {
                    panic!("ModuleNode should never have TypeId for containing node")
                }
            })
    }

    pub fn find_node(&self, item_id: NodeId) -> Option<&dyn GraphNode> {
        // Check all node collections for matching ID

        self.functions
            .iter()
            .find(|n| n.id == item_id)
            .map(|n| n as &dyn GraphNode)
            .or_else(|| {
                self.defined_types.iter().find_map(|n| match n {
                    TypeDefNode::Struct(s) if s.id == item_id => Some(s as &dyn GraphNode),
                    TypeDefNode::Enum(e) if e.id == item_id => Some(e as &dyn GraphNode),
                    TypeDefNode::TypeAlias(t) if t.id == item_id => Some(t as &dyn GraphNode),
                    TypeDefNode::Union(u) if u.id == item_id => Some(u as &dyn GraphNode),
                    _ => None,
                })
            })
            .or_else(|| {
                self.traits
                    .iter()
                    .chain(&self.private_traits)
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.modules
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.values
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.macros
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            // TODO: Kind of a hack, or at least not logically clean - since nodes should really be
            // top-level elements in a vec in the CodeGraph. Just change CodeGraph to have a field
            // for methods already.
            .or_else(|| {
                self.impls.iter().find_map(|i| {
                    i.methods
                        .iter()
                        .find(|n| n.id == item_id)
                        .map(|n| n as &dyn GraphNode)
                })
            })
    }

    pub fn module_contains_node(&self, module_id: NodeId, item_id: NodeId) -> bool {
        // Check if module directly contains the item
        self.modules
            .iter()
            .find(|m| m.id == module_id)
            .map(|module| module.items().is_some_and(|m| m.contains(&item_id)));

        // Check if module contains the item through nested modules
        self.relations.iter().any(|r| {
            r.source == GraphId::Node(module_id)
                && r.target == GraphId::Node(item_id)
                && r.kind == RelationKind::Contains
        })
    }

    // TODO: Improve this. It is old code and needs to be refactored to be more idiomatic and
    // checked for correctness.
    #[allow(dead_code, reason = "Useful in upcoming uuid changes for Phase 3")]
    fn check_use_statements(&self, item_id: NodeId, context_module: &[String]) -> VisibilityResult {
        let context_module_id = match self.find_module_by_path(context_module) {
            Some(m) => m.id,
            None => {
                panic!("Trying to access another workspace.")
            }
        };

        // Get all ModuleImports relations for this context module
        let import_relations = self.relations.iter().filter(|r| {
            r.source == GraphId::Node(context_module_id) && r.kind == RelationKind::ModuleImports
        });

        for rel in import_relations {
            // Check if this is a glob import by looking for a module that contains the target
            let is_glob = self
                .modules
                .iter()
                .any(|m| GraphId::Node(m.id) == rel.target);

            if is_glob {
                // For glob imports, check if item is in the imported module
                match rel.target {
                    GraphId::Node(_node_id) => {
                        return VisibilityResult::Direct;
                    }
                    GraphId::Type(_type_id) => {
                        panic!("implement me!")
                    }
                }
            }
            // Direct import match
            else if rel.target == GraphId::Node(item_id) {
                return VisibilityResult::Direct;
            }
        }

        let item = match self.find_node(item_id) {
            Some(item) => item,
            None => {
                panic!("Node not in graph");
            }
        };

        // Get current module's use statements
        let current_module = self.modules.iter().find(|m| m.path == context_module);

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
    pub use_statements: Vec<ImportNode>,
}
