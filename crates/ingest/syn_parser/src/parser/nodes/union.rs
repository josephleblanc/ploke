use ploke_core::{NodeId, TrackingHash};

use crate::parser::types::GenericParamNode;

use super::*;

// Represents a union definition
impl UnionNode {
    /// Returns the typed ID for this union node.
    pub fn union_id(&self) -> UnionNodeId {
        self.id
    }
}

impl GraphNode for UnionNode {
    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_union(&self) -> Option<&UnionNode> {
        Some(self)
    }
}

impl HasAttributes for UnionNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct UnionNode {
    pub id: UnionNodeId, // Use typed ID
    pub name: String,
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub span: (usize, usize),
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}
