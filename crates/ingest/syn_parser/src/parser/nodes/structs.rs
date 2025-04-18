use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use crate::parser::types::GenericParamNode;

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StructNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>, // Replace Vec<String>
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FieldNode {
    pub id: NodeId,
    pub name: Option<String>,
    pub type_id: TypeId,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl GraphNode for StructNode {
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
}
