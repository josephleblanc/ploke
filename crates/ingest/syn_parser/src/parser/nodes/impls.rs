use ploke_core::{NodeId, TypeId};

use crate::parser::types::GenericParamNode;

use super::*;

// Represents an implementation block
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ImplNode {
    pub id: NodeId,
    pub self_type: TypeId,
    pub span: (usize, usize), // Byte start/end offsets
    pub trait_type: Option<TypeId>,
    pub methods: Vec<FunctionNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl GraphNode for ImplNode {
    fn visibility(&self) -> VisibilityKind {
        VisibilityKind::Public
    }

    fn name(&self) -> &str {
        // Placeholder
        // TODO: Think through this and improve it
        "impl block"
    }

    fn id(&self) -> NodeId {
        self.id
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }
}
