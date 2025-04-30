use ploke_core::{NodeId, TypeId};

use crate::parser::types::GenericParamNode;

use super::*;

// Represents an implementation block
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ImplNode {
    pub id: ImplNodeId, // Use typed ID
    pub self_type: TypeId,
    pub span: (usize, usize), // Byte start/end offsets
    pub trait_type: Option<TypeId>,
    pub methods: Vec<FunctionNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl ImplNode {
    /// Returns the typed ID for this impl node.
    pub fn impl_id(&self) -> ImplNodeId {
        self.id
    }
}

impl GraphNode for ImplNode {
    fn visibility(&self) -> VisibilityKind {
        VisibilityKind::Public // Impls don't have inherent visibility in the same way items do
    }

    fn name(&self) -> &str {
        // Placeholder
        // TODO: Think through this and improve it
        "impl block"
    }

    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_impl(&self) -> Option<&ImplNode> {
        Some(self)
    }
}
