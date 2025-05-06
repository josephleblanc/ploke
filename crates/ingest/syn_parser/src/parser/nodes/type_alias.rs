use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Type Alias Node ---

// Removed the macro invocation for TypeAliasNodeInfo

// Represents a type alias (type NewType = OldType)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
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
}

impl GraphNode for TypeAliasNode {
    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base NodeId
    }
    fn visibility(&self) ->&VisibilityKind {
        &self.visibility
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
