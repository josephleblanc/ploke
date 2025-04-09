#[cfg(not(feature = "uuid_ids"))]
#[cfg(not(feature = "uuid_ids"))]
use crate::TypeId;
#[cfg(feature = "uuid_ids")]
use ploke_core::NodeId; // Use new type when feature is enabled // Use compat type when feature is disabled

#[cfg(feature = "uuid_ids")]
use ploke_core::TypeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum GraphId {
    Node(NodeId),
    Type(TypeId),
}

impl std::fmt::Display for GraphId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphId::Node(node_id) => write!(f, "GraphID: {}", node_id),
            GraphId::Type(type_id) => write!(f, "GraphID: {}", type_id),
        }
    }
}

// ANCHOR: Relation
// Represents a relation between nodes

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Relation {
    #[cfg(not(feature = "uuid_ids"))]
    pub source: NodeId,
    #[cfg(not(feature = "uuid_ids"))]
    pub target: NodeId,
    #[cfg(feature = "uuid_ids")]
    pub source: GraphId,
    #[cfg(feature = "uuid_ids")]
    pub target: GraphId,

    pub kind: RelationKind,
}

// ANCHOR: Uses
// Different kinds of relations
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    FunctionParameter,
    FunctionReturn,
    StructField,
    Method, // e.g. StructNode -> FunctionNode
    EnumVariant,
    VariantField,
    ImplementsFor,
    ImplementsTrait,
    Inherits,
    References,
    Contains,
    Uses,
    ValueType,
    MacroUse,
    ModuleImports,
    // MacroExpansion,
    // This is outside the scope of this project right now, but if it were to be implemented, it
    // would probably go here.
}
//ANCHOR_END: Uses
//ANCHOR_END: Relation
