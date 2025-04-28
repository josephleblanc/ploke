use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use crate::parser::types::GenericParamNode;

use super::*;

// Represents a type alias (type NewType = OldType)
impl GraphNode for TypeAliasNode {
    fn id(&self) -> NodeId {
        self.id
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

    fn as_type_alias(&self) -> Option<&TypeAliasNode> {
        Some(self)
    }
}

impl HasAttributes for TypeAliasNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TypeAliasNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}
