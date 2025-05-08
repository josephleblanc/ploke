use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::TypeId;
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Impl Node ---

// Removed the macro invocation for ImplNodeInfo

// Represents an implementation block
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
pub struct ImplNode {
    pub id: ImplNodeId, // Use typed ID
    pub self_type: TypeId,
    pub span: (usize, usize),
    pub trait_type: Option<TypeId>,
    pub methods: Vec<MethodNode>, // Changed from FunctionNode
    pub generic_params: Vec<GenericParamNode>,
    pub cfgs: Vec<String>,
    // TODO: Add fields for associated consts and types once we are processing them.
    // pub associated_consts: Vec<ConstNodeId>,
    // pub associated_types: Vec<TypeAliasNodeId>,
}

impl ImplNode {
    /// Returns the typed ID for this impl node.
    pub fn impl_id(&self) -> ImplNodeId {
        self.id
    }

    pub fn id(&self) -> ImplNodeId {
        self.id
    }

    pub fn self_type(&self) -> TypeId {
        self.self_type
    }

    pub fn span(&self) -> (usize, usize) {
        self.span
    }

    pub fn trait_type(&self) -> Option<TypeId> {
        self.trait_type
    }

    pub fn methods(&self) -> &[MethodNode] {
        &self.methods
    }

    pub fn generic_params(&self) -> &[GenericParamNode] {
        &self.generic_params
    }

    pub fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    pub fn cfgs_str_iter(&self) -> impl Iterator<Item = &str> {
        self.cfgs().iter().map(|c| c.as_str())
    }
}

impl GraphNode for ImplNode {
    fn visibility(&self) -> &VisibilityKind {
        &VisibilityKind::Public // Impls don't have inherent visibility in the same way items do
    }

    fn name(&self) -> &str {
        // Placeholder
        // TODO: Think through this and improve it
        "impl block"
    }

    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base NodeId
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_impl(&self) -> Option<&ImplNode> {
        Some(self)
    }
}
