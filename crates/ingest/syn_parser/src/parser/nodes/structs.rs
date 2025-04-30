use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use crate::parser::types::GenericParamNode;

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StructNode {
    pub id: StructNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>, // Replace Vec<String>
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl StructNode {
    /// Returns the typed ID for this struct node.
    pub fn struct_id(&self) -> StructNodeId {
        self.id
    }

    pub fn new(info: StructNodeInfo) -> Self {
        // AI: New constructor allows us to keep `StructNodeId`'s constructor method private and
        // avoid creating a `StructNodeId::new` method, making it impossible to create a
        // `StructNodeId` without all the provided context. This will go a long way to disallow the
        // possibility of changing a typed id in the code. While it is not a perfect guarantee of
        // valid state, it is getting closer, and that is what we are striving for: compile-time
        // guarantees of only expressing valid state at the level of types, striving for the
        // Curry-Howard correspondance style.
        Self {
            id: StructNodeId(info.id),
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct StructNodeInfo {
    pub id: NodeId, // Use raw NodeId
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>, // Replace Vec<String>
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FieldNode {
    pub id: FieldNodeId, // Use typed ID
    pub name: Option<String>,
    pub type_id: TypeId,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl FieldNode {
    /// Returns the typed ID for this field node.
    pub fn field_id(&self) -> FieldNodeId {
        self.id
    }
}

impl HasAttributes for FieldNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl GraphNode for StructNode {
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

    fn as_struct(&self) -> Option<&StructNode> {
        Some(self)
    }
}

impl HasAttributes for StructNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
