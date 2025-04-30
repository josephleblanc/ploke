use crate::{define_node_info_struct, parser::types::GenericParamNode}; // Import macro
use ploke_core::{NodeId, TrackingHash}; // Import NodeId
use serde::{Deserialize, Serialize};

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Union Node ---

define_node_info_struct! {
    /// Temporary info struct for creating a UnionNode.
    UnionNodeInfo {
        name: String,
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

// Represents a union definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

    /// Creates a new `UnionNode` from `UnionNodeInfo`.
    pub(crate) fn new(info: UnionNodeInfo) -> Self {
        Self {
            id: UnionNodeId(info.id), // Wrap the raw ID here
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

impl GraphNode for UnionNode {
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

    fn as_union(&self) -> Option<&UnionNode> {
        Some(self)
    }
}

impl HasAttributes for UnionNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
