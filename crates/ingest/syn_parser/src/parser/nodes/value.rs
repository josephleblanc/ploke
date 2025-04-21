use ploke_core::{NodeId, TrackingHash, TypeId};

use super::*;

// Represents a constant or static variable
impl GraphNode for ValueNode {
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
}

impl HasAttributes for ValueNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ValueNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub kind: ValueKind,
    pub value: Option<String>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub span: (usize, usize),
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ValueKind {
    Constant,
    Static { is_mutable: bool },
}

impl ValueKind {
    pub fn is_constant(&self) -> bool {
        matches!(self, Self::Constant)
    }
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static { .. })
    }
    pub fn is_static_mut(&self) -> bool {
        match self {
            Self::Static { is_mutable } => *is_mutable,
            Self::Constant => false,
        }
    }
}
