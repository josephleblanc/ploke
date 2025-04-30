use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{NodeId, TrackingHash};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Enum Node ---

// Removed the macro invocation for EnumNodeInfo

// Represents an enum definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct EnumNode {
    pub id: EnumNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        variants: Vec<VariantNode>,
        generic_params: Vec<GenericParamNode>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

// Represents an enum definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EnumNode {
    pub id: EnumNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub variants: Vec<VariantNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl EnumNode {
    /// Returns the typed ID for this enum node.
    pub fn enum_id(&self) -> EnumNodeId {
        self.id
    }

    /// Creates a new `EnumNode` from `EnumNodeInfo`.
    pub(crate) fn new(info: EnumNodeInfo) -> Self {
        Self {
            id: EnumNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            variants: info.variants,
            generic_params: info.generic_params,
            attributes: info.attributes,
            docstring: info.docstring,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl HasAttributes for EnumNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

// --- Variant Node ---

// Removed the macro invocation for VariantNodeInfo

// Represents a variant in an enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct VariantNode {
    pub id: VariantNodeId, // Use typed ID
    pub name: String,
        fields: Vec<FieldNode>,
        discriminant: Option<String>,
        attributes: Vec<Attribute>,
        cfgs: Vec<String>,
    }
}

// Represents a variant in an enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VariantNode {
    pub id: VariantNodeId, // Use typed ID
    pub name: String,
    pub fields: Vec<FieldNode>,
    pub discriminant: Option<String>,
    pub attributes: Vec<Attribute>,
    pub cfgs: Vec<String>,
}

impl VariantNode {
    /// Returns the typed ID for this variant node.
    pub fn variant_id(&self) -> VariantNodeId {
        self.id
    }

    /// Creates a new `VariantNode` from `VariantNodeInfo`.
    pub(crate) fn new(info: VariantNodeInfo) -> Self {
        Self {
            id: VariantNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            fields: info.fields,
            discriminant: info.discriminant,
            attributes: info.attributes,
            cfgs: info.cfgs,
        }
    }
}

impl HasAttributes for VariantNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl GraphNode for EnumNode {
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

    fn as_enum(&self) -> Option<&EnumNode> {
        Some(self)
    }
}
