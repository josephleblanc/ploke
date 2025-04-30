use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Type Alias Node ---

// Removed the macro invocation for TypeAliasNodeInfo

// Represents a type alias (type NewType = OldType)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct TypeAliasNode {
    pub id: TypeAliasNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        type_id: TypeId, // The ID of the aliased type
        generic_params: Vec<GenericParamNode>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

// Represents a type alias (type NewType = OldType)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TypeAliasNode {
    pub id: TypeAliasNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub type_id: TypeId, // The ID of the aliased type
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl TypeAliasNode {
    /// Returns the typed ID for this type alias node.
    pub fn type_alias_id(&self) -> TypeAliasNodeId {
        self.id
    }

    /// Creates a new `TypeAliasNode` from `TypeAliasNodeInfo`.
    pub(crate) fn new(info: TypeAliasNodeInfo) -> Self {
        Self {
            id: TypeAliasNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            type_id: info.type_id,
            generic_params: info.generic_params,
            attributes: info.attributes,
            docstring: info.docstring,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for TypeAliasNode {
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

    fn as_type_alias(&self) -> Option<&TypeAliasNode> {
        Some(self)
    }
}

impl HasAttributes for TypeAliasNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
