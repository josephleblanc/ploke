use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use crate::parser::types::GenericParamNode;

use super::*;

// Represents a trait definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TraitNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    pub methods: Vec<MethodNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub super_traits: Vec<TypeId>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl GraphNode for TraitNode {
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> NodeId {
        self.id
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_trait(&self) -> Option<&TraitNode> {
        Some(self)
    }
}

impl HasAttributes for TraitNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
