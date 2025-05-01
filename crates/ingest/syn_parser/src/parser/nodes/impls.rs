use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{TypeId};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Impl Node ---

// Removed the macro invocation for ImplNodeInfo

// Represents an implementation block
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct ImplNode {
    pub id: ImplNodeId, // Use typed ID
    pub self_type: TypeId,
    pub span: (usize, usize),
    pub trait_type: Option<TypeId>,
    pub methods: Vec<MethodNode>, // Changed from FunctionNode
    pub generic_params: Vec<GenericParamNode>,
    pub cfgs: Vec<String>,
    // TODO: Add fields for associated consts and types if needed
    // pub associated_consts: Vec<ConstNode>,
    // pub associated_types: Vec<TypeAliasNode>,
}

impl ImplNode {
    /// Returns the typed ID for this impl node.
    pub fn impl_id(&self) -> ImplNodeId {
        self.id
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
