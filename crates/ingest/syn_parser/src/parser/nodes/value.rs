use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Const Node ---

// Removed the macro invocation for ConstNodeInfo

/// Represents a `const` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct ConstNode {
    pub id: ConstNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        type_id: TypeId,
        value: Option<String>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

/// Represents a `const` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConstNode {
    pub id: ConstNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub value: Option<String>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl ConstNode {
    /// Returns the typed ID for this const node.
    pub fn const_id(&self) -> ConstNodeId {
        self.id
    }

    /// Creates a new `ConstNode` from `ConstNodeInfo`.
    pub(crate) fn new(info: ConstNodeInfo) -> Self {
        Self {
            id: ConstNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            type_id: info.type_id,
            value: info.value,
            attributes: info.attributes,
            docstring: info.docstring,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for ConstNode {
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
    fn as_const(&self) -> Option<&ConstNode> {
        // Changed from as_value_const
        Some(self)
    }
}

impl HasAttributes for ConstNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

// --- Static Node ---

// Removed the macro invocation for StaticNodeInfo

/// Represents a `static` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct StaticNode {
    pub id: StaticNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        type_id: TypeId,
        is_mutable: bool,
        value: Option<String>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

/// Represents a `static` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StaticNode {
    pub id: StaticNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub is_mutable: bool,
    pub value: Option<String>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl StaticNode {
    /// Returns the typed ID for this static node.
    pub fn static_id(&self) -> StaticNodeId {
        self.id
    }

    /// Creates a new `StaticNode` from `StaticNodeInfo`.
    pub(crate) fn new(info: StaticNodeInfo) -> Self {
        Self {
            id: StaticNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            type_id: info.type_id,
            is_mutable: info.is_mutable,
            value: info.value,
            attributes: info.attributes,
            docstring: info.docstring,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for StaticNode {
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
    fn as_static(&self) -> Option<&StaticNode> {
        // Changed from as_value_static
        Some(self)
    }
}

impl HasAttributes for StaticNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
