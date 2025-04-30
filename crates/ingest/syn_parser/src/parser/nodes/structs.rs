use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use crate::parser::types::GenericParamNode;

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StructNode {
    pub id: StructNodeId, // Use typed ID
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
    pub id: FieldNodeId, // Use typed ID
    pub name: Option<String>,
    pub type_id: TypeId,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl FieldNode {
    /// Returns the typed ID for this field node.
    pub fn field_id(&self) -> FieldNodeId {
        self.id
    }
}

impl HasAttributes for FieldNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl StructNode {
    /// Returns the typed ID for this struct node.
    pub fn struct_id(&self) -> StructNodeId {
        self.id
    }
}

impl GraphNode for StructNode {
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

    fn as_struct(&self) -> Option<&StructNode> {
        Some(self)
    }
}

impl HasAttributes for StructNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
