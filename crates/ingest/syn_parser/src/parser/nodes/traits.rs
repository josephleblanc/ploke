#![allow(unused_must_use)]
// Needed to get rid of proc-macro induced warning for `ExpectedData`

use crate::parser::types::GenericParamNode;
use derive_test_helpers::ExpectedData;
// Removed define_node_info_struct import
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Trait Node ---

// Removed the macro invocation for TraitNodeInfo

// Represents a trait definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Add derive ExpectedData
pub struct TraitNode {
    pub id: TraitNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub methods: Vec<MethodNode>, // Changed from FunctionNode
    pub generic_params: Vec<GenericParamNode>,
    // TODO: Update super_traits to use a Vec of TraitNodeId
    pub super_traits: Vec<TypeId>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
    // TODO: Add fields for associated consts and types if needed
    // pub associated_consts: Vec<ConstNode>,
    // pub associated_types: Vec<TypeAliasNode>,
}

impl TraitNode {
    /// Returns the typed ID for this trait node.
    pub fn trait_id(&self) -> TraitNodeId {
        self.id
    }
}

impl GraphNode for TraitNode {
    fn visibility(&self) -> &VisibilityKind {
        &self.visibility
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base NodeId
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_trait(&self) -> Option<&TraitNode> {
        Some(self)
    }
}

impl HasAttributes for TraitNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
