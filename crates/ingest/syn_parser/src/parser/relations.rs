use ploke_core::NodeId;
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
    pub source: GraphId,
    pub target: GraphId,

    pub kind: RelationKind,
}

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
    /// Links a module declaration (`mod foo;`) to its definition (the `ModuleNode` for `foo.rs` or
    /// `mod foo { ... }`).
    /// Direction: `Declaration ModuleNode` -> `Definition ModuleNode`.
    ResolvesToDefinition,
    // MacroExpansion,
    // This is outside the scope of this project right now, but if it were to be implemented, it
    // would probably go here.
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeKind {
    RequiresParent,
    CanUse,
}

impl TryInto<ScopeKind> for RelationKind {
    // AI: Correct the `None` values to an appropriate Error
    type Error;

    fn try_into(self) -> Result<ScopeKind, Self::Error> {
        match Self {
            Self::FunctionParameter => Ok(ScopeKind::RequiresParent),
            Self::FunctionReturn => Ok(ScopeKind::RequiresParent),
            Self::StructField => Ok(ScopeKind::RequiresParent),
            Self::Method => Ok(ScopeKind::RequiresParent),
            Self::EnumVariant => Ok(ScopeKind::RequiresParent),
            Self::VariantField => Ok(ScopeKind::RequiresParent),
            Self::ImplementsFor => None,
            Self::ImplementsTrait => None,
            Self::Inherits => None,
            Self::References => None,
            Self::Contains => Ok(ScopeKind::CanUse),
            Self::Uses => Ok(ScopeKind::CanUse),
            Self::ValueType => Ok(ScopeKind::CanUse),
            Self::MacroUse => None, // We don't handle macros too much.
            Self::ModuleImports => Ok(ScopeKind::CanUse),
            Self::ResolvesToDefinition => None,
        }
    }
}

// AI: Implement a new custom error type.
// It should use thiserror, and be compatable `SynParserError` AI!
