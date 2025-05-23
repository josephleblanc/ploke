#![allow(unused_must_use)]
// Needed to get rid of proc-macro induced warning for `ExpectedData`

use crate::parser::types::GenericParamNode;
use derive_test_helpers::ExpectedData;
use ploke_core::TrackingHash;
use serde::{Deserialize, Serialize};

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Union Node ---

// Removed the macro invocation for UnionNodeInfo

// Represents a union definition
// TODO: Add an `unsafe` field to `UnionNode` that is always `true`, since unions are ineherently
// unsafe.
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
