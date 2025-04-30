use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Struct Node ---

// Removed the macro invocation for StructNodeInfo

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct StructNode {
    pub id: StructNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        fields: Vec<FieldNode>,
        generic_params: Vec<GenericParamNode>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

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
    pub cfgs: Vec<String>,
}

impl StructNode {
    /// Returns the typed ID for this struct node.
    pub fn struct_id(&self) -> StructNodeId {
        self.id
    }

    /// Creates a new `StructNode` from `StructNodeInfo`.
    pub(crate) fn new(info: StructNodeInfo) -> Self {
        Self {
            id: StructNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            fields: info.fields,
            generic_params: info.generic_params,
            attributes: info.attributes,
            docstring: info.docstring,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

// --- Field Node ---

// Removed the macro invocation for FieldNodeInfo

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct FieldNode {
    pub id: FieldNodeId, // Use typed ID
    pub name: Option<String>,
        type_id: TypeId,
        visibility: VisibilityKind,
        attributes: Vec<Attribute>,
        cfgs: Vec<String>,
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

    /// Creates a new `FieldNode` from `FieldNodeInfo`.
    pub(crate) fn new(info: FieldNodeInfo) -> Self {
        Self {
            id: FieldNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            type_id: info.type_id,
            visibility: info.visibility,
            attributes: info.attributes,
            cfgs: info.cfgs,
        }
    }
}

impl HasAttributes for FieldNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
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
