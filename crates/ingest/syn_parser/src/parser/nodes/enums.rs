#![allow(unused_must_use)]
// Needed to get rid of proc-macro induced warning for `ExpectedData`

use crate::parser::types::GenericParamNode;
use derive_test_helpers::ExpectedData; // Import ExpectedData
use ploke_core::TrackingHash;
use serde::{Deserialize, Serialize};

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Enum Node ---

// Removed the macro invocation for EnumNodeInfo

// Represents an enum definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Add derive ExpectedData
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
}

impl HasAttributes for EnumNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

// --- Variant Node ---

// Removed the macro invocation for VariantNodeInfo

// Represents a variant in an enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
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
}

impl HasAttributes for VariantNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl GraphNode for EnumNode {
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

    fn as_enum(&self) -> Option<&EnumNode> {
        Some(self)
    }
}
