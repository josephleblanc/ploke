use crate::parser::nodes::NodeId;
use serde::{Deserialize, Serialize};

// ANCHOR: Relation
// Represents a relation between nodes
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relation {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: RelationKind,
}

// ANCHOR: Uses
// Different kinds of relations
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationKind {
    FunctionParameter,
    FunctionReturn,
    StructField,
    Method, // e.g. StructNode -> FunctionNode
    EnumVariant,
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
