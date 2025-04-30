use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Trait Node ---

// Removed the macro invocation for TraitNodeInfo

// Represents a trait definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct TraitNode {
    pub id: TraitNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        methods: Vec<MethodNode>, // Changed from FunctionNode
        generic_params: Vec<GenericParamNode>,
        super_traits: Vec<TypeId>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
        // Note: Associated consts/types would need to be added here if handled
    }
}

// Represents a trait definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TraitNode {
    pub id: TraitNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub methods: Vec<MethodNode>, // Changed from FunctionNode
    pub generic_params: Vec<GenericParamNode>,
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

    /// Creates a new `TraitNode` from `TraitNodeInfo`.
    pub(crate) fn new(info: TraitNodeInfo) -> Self {
        Self {
            id: TraitNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            methods: info.methods,
            generic_params: info.generic_params,
            super_traits: info.super_traits,
            attributes: info.attributes,
            docstring: info.docstring,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for TraitNode {
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
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
