use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Const Node ---

// Removed the macro invocation for ConstNodeInfo
use ploke_test_macros::ExpectedData;
/// Represents a `const` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Add derive
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
}

impl GraphNode for ConstNode {
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
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
}

impl GraphNode for StaticNode {
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
