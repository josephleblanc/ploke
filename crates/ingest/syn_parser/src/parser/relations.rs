use ploke_core::NodeId;
use ploke_core::TypeId;
use serde::{Deserialize, Serialize};
use thiserror::Error; // Add thiserror import

// Define the new error type
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationConversionError {
    #[error("Relation kind {0:?} is not applicable for ScopeKind conversion")]
    NotApplicable(RelationKind),
}

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

impl RelationKind {
    /// Checks if this relation kind implies a scoping relationship (either requiring a parent or allowing use).
    /// Returns `true` if `TryInto::<ScopeKind>::try_into(self)` succeeds, `false` otherwise.
    pub fn is_scoping(self) -> bool {
        TryInto::<ScopeKind>::try_into(self).is_ok()
    }

    /// Checks if this relation kind specifically requires a parent scope.
    /// Returns `true` if `try_into::<ScopeKind>()` results in `Ok(ScopeKind::RequiresParent)`.
    pub fn is_parent_required(self) -> bool {
        matches!(self.try_into(), Ok(ScopeKind::RequiresParent))
    }

    /// Checks if this relation kind represents a usage relationship (can use).
    /// Returns `true` if `try_into::<ScopeKind>()` results in `Ok(ScopeKind::CanUse)`.
    pub fn is_use(self) -> bool {
        matches!(self.try_into(), Ok(ScopeKind::CanUse))
    }
}

/// The kind of scope used
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeKind {
    /// Requires parent in path, e.g. `SomeStruct::associated_func()`
    RequiresParent,
    /// Can be used in a `use` statement, e.g. `use a::b::SomeStruct;`
    CanUse,
}

impl TryInto<ScopeKind> for RelationKind {
    type Error = RelationConversionError; // Use the new error type

    fn try_into(self) -> Result<ScopeKind, Self::Error> {
        match self {
            Self::FunctionParameter => Ok(ScopeKind::RequiresParent),
            Self::FunctionReturn => Ok(ScopeKind::RequiresParent),
            Self::StructField => Ok(ScopeKind::RequiresParent),
            Self::Method => Ok(ScopeKind::RequiresParent),
            Self::EnumVariant => Ok(ScopeKind::RequiresParent),
            Self::VariantField => Ok(ScopeKind::RequiresParent),
            Self::ImplementsFor => Err(RelationConversionError::NotApplicable(self)),
            Self::ImplementsTrait => Err(RelationConversionError::NotApplicable(self)),
            Self::Inherits => Err(RelationConversionError::NotApplicable(self)),
            Self::References => Err(RelationConversionError::NotApplicable(self)),
            Self::Contains => Ok(ScopeKind::CanUse),
            Self::Uses => Ok(ScopeKind::CanUse),
            Self::ValueType => Ok(ScopeKind::CanUse),
            Self::MacroUse => Err(RelationConversionError::NotApplicable(self)), // We don't handle macros too much.
            Self::ModuleImports => Ok(ScopeKind::CanUse),
            Self::ResolvesToDefinition => Err(RelationConversionError::NotApplicable(self)),
        }
    }
}
