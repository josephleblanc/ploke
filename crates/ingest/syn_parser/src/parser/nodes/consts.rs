use derive_test_helpers::ExpectedData;
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Const Node ---

// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
/// Represents a `const` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Common derives applied unconditionally
                                                                         // #[cfg_attr(test, derive(ExpectedData))]
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

    pub fn span(&self) -> (usize, usize) {
        self.span
    }

    pub fn visibility(&self) -> &VisibilityKind {
        &self.visibility
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn value(&self) -> Option<&String> {
        self.value.as_ref()
    }

    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    pub fn docstring(&self) -> Option<&String> {
        self.docstring.as_ref()
    }

    pub fn tracking_hash(&self) -> Option<TrackingHash> {
        self.tracking_hash
    }

    pub fn cfgs(&self) -> &[String] {
        &self.cfgs
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
