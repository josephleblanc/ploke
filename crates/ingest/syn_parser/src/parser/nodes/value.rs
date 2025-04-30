use ploke_core::{NodeId, TrackingHash, TypeId};

use super::*;

/// Represents a `const` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConstNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub value: Option<String>, // Expression defining the const
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub span: (usize, usize),
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl GraphNode for StaticNode {
    fn id(&self) -> NodeId {
        self.id
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

impl GraphNode for ConstNode {
    fn id(&self) -> NodeId {
        self.id
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

/// Represents a `static` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StaticNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub is_mutable: bool,
    pub value: Option<String>, // Expression defining the static
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub span: (usize, usize),
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}
