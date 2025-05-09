use crate::parser::types::GenericParamNode;
use derive_test_helpers::ExpectedData;
// Removed define_node_info_struct import
use ploke_core::TrackingHash;
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Union Node ---

// Removed the macro invocation for UnionNodeInfo

// Represents a union definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Add derive
pub struct UnionNode {
    pub id: UnionNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl UnionNode {
    /// Returns the typed ID for this union node.
    pub fn union_id(&self) -> UnionNodeId {
        self.id
    }
}

impl GraphNode for UnionNode {
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

    fn as_union(&self) -> Option<&UnionNode> {
        Some(self)
    }
}

impl HasAttributes for UnionNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
