use ploke_core::{NodeId, TrackingHash};
use serde::{Deserialize, Serialize};

use crate::parser::types::GenericParamNode;

use super::*;

// Represents an enum definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EnumNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    pub variants: Vec<VariantNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl EnumNode {}

impl HasAttributes for EnumNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

// Represents a variant in an enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VariantNode {
    pub id: NodeId,
    pub name: String,
    pub fields: Vec<FieldNode>,
    pub discriminant: Option<String>,
    pub attributes: Vec<Attribute>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl HasAttributes for VariantNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl GraphNode for EnumNode {
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
