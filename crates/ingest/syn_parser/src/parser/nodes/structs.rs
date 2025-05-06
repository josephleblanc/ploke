use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Struct Node ---

// Removed the macro invocation for StructNodeInfo

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
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
    pub cfgs: Vec<String>,
}

impl StructNode {
    /// Returns the typed ID for this struct node.
    pub fn struct_id(&self) -> StructNodeId {
        self.id
    }
}

// --- Field Node ---

// Removed the macro invocation for FieldNodeInfo

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
pub struct FieldNode {
    pub id: FieldNodeId, // Use typed ID
    pub name: Option<String>,
    pub type_id: TypeId,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub cfgs: Vec<String>,
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

impl GraphNode for StructNode {
    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base NodeId
    }
    fn visibility(&self) -> &VisibilityKind {
        &self.visibility
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
