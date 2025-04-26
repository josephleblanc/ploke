use super::nodes::GraphId; // Import GraphId from its new location
use serde::{Deserialize, Serialize};
use thiserror::Error; // Add thiserror import

// Define the new error type
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationConversionError {
    #[error("Relation kind {0:?} is not applicable for ScopeKind conversion")]
    NotApplicable(RelationKind),
}

// ANCHOR: Relation
// Represents a relation between nodes
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Relation {
    pub source: GraphId,
    pub target: GraphId,

    pub kind: RelationKind,
}

impl Relation {
    /// Checks if the relation's source matches the given `GraphId`.
    pub fn matches_source(&self, source_id: GraphId) -> bool {
        self.source == source_id
    }

    /// Checks if the relation's target matches the given `GraphId`.
    pub fn matches_target(&self, target_id: GraphId) -> bool {
        self.target == target_id
    }

    /// Checks if the relation's source and kind match the given values.
    pub fn matches_source_and_kind(&self, source_id: GraphId, kind: RelationKind) -> bool {
        self.source == source_id && self.kind == kind
    }

    /// Checks if the relation's target and kind match the given values.
    pub fn matches_target_and_kind(&self, target_id: GraphId, kind: RelationKind) -> bool {
        self.target == target_id && self.kind == kind
    }

    /// Checks if the relation's source, target, and kind match the given values.
    pub fn matches_source_target_kind(
        &self,
        source_id: GraphId,
        target_id: GraphId,
        kind: RelationKind,
    ) -> bool {
        self.source == source_id && self.target == target_id && self.kind == kind
    }
}

// Different kinds of relations
// TODO: These relations really need to be refactored. We are not taking advantage of type safety
// very well here, and the RelationKind can currently be between any two nodes of any type or
// TypeId, which is just bad.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    //-----------------------------------------------------------------------
    //-----------------Defined in ModuleTree Methods-------------------------
    //-----------------------------------------------------------------------
    // e.g. `pub use some_dep::a::b::ReExportedStruct`--ReExport-->ReExportedStruct defn
    // (currently only targets module in module_tree.rs)
    // ImportNode--------------ReExports-------------> NodeId of reexported item
    ReExports,
    // ModuleNode decl --------CustomPath------------> module defn for `#[path]` attribute
    CustomPath,
    /// Links a module declaration (`mod foo;`) to its definition (the `ModuleNode` for `foo.rs` or
    /// `mod foo { ... }`).
    /// Direction: Declaration `ModuleNode` -> `Definition ModuleNode`.
    // ModuleNode Delc---------ResolvesToDefinition--> ModuleNode definition
    ResolvesToDefinition,
    //-----------------------------------------------------------------------
    //-----------------Defined in visit_item_* methods-----------------------
    //-----------------------------------------------------------------------
    // ModuleNode definition---Contains--------------> all primary nodes (NodeId)
    // (including modules)
    Contains,
    // ModuleNode -------------ModuleImports---------> ImportNode (NodeId)
    // NOTE: all `use` and `pub use` included, not distinguished by relation
    ModuleImports,
    // FunctionNode -----------FunctionParameter-----> TypeId of ParamNode
    // FunctionNode (method) --FunctionParameter-----> TypeId of ParamNode
    FunctionParameter,
    // FunctionNode -----------FunctionReturn--------> TypeId of return type
    FunctionReturn,
    // StructNode/EnumNode ----StructField-----------> StructField (NodeId)
    StructField,
    // (TypeId of struct) -----Method----------------> FunctionNode (NodeId)
    Method,
    // EnumNode ---------------EnumVariant-----------> EnumVariant (NodeId)
    EnumVariant,
    // EnumNode ---------------VariantField----------> named/unnamed VariantNode (NodeId)
    VariantField,
    // ImplNode ---------------ImplementsFor---------> TypeId of `Self` (cannot be known at parse time)
    ImplementsFor,
    // ImplNode ---------------ImplementsTrait-------> TypeId of trait (cannot be known at parse time)
    ImplementsTrait,
    // ValueNode --------------ValueType-------------> TypeId of its own type
    ValueType,
    // ImportNode (Extern) ----Uses------------------> TypeId (honestly not sure about this one)
    Uses,
    Inherits,
    // MacroUse,
    // References, // Not currently used, possibly will use for tracking lifetimes later
    // MacroExpansion,
    // This is outside the scope of this project right now, but if it were to be implemented, it
    // would probably go here.
    // Inherits, // Not currently used, may wish to use this in a different context

    // TODO: Likely will delete this later.
    // Using it currently for testing an implementation of `shortest_public_path` in module_tree.rs
    // ModuleNode --Sibling--> ModuleNode
    Sibling,
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

    /// Checks if this relation kind relates to module structure or definition.
    pub fn is_module_related(self) -> bool {
        matches!(
            self,
            RelationKind::Contains
                | RelationKind::ModuleImports
                | RelationKind::ResolvesToDefinition
                | RelationKind::CustomPath
                | RelationKind::Sibling
        )
    }

    /// Checks if this relation kind relates to type definitions, usage, or implementation.
    pub fn is_type_related(self) -> bool {
        matches!(
            self,
            RelationKind::FunctionParameter
                | RelationKind::FunctionReturn
                | RelationKind::StructField
                | RelationKind::Method // Method is a function related to a type
                | RelationKind::EnumVariant
                | RelationKind::VariantField
                | RelationKind::ImplementsFor
                | RelationKind::ImplementsTrait
                | RelationKind::ValueType
                | RelationKind::Inherits // Assuming Inherits relates types
        )
    }

    /// Checks if this relation kind relates to importing or exporting items.
    pub fn is_import_export_related(self) -> bool {
        matches!(
            self,
            RelationKind::ModuleImports | RelationKind::ReExports | RelationKind::Uses
        )
    }
}

/// Differentiates between a `Relation` that can be used to bring an item directly into scope, e.g.
/// through a `use module_a::SomeStruct`, which can then be freely used inside the module without
/// including the path of the parent, e.g. `let some_struct: SomeStruct = SomeStruct::default();`
/// and an relation that signifies a parent is required in its use, e.g. a parent being a struct
/// and a child being one of its methods.
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
            Self::Contains => Ok(ScopeKind::CanUse),
            Self::Uses => Ok(ScopeKind::CanUse),
            Self::ValueType => Ok(ScopeKind::CanUse),
            Self::ModuleImports => Ok(ScopeKind::CanUse),
            Self::ReExports => Ok(ScopeKind::CanUse),
            Self::FunctionParameter => Ok(ScopeKind::RequiresParent),
            Self::FunctionReturn => Ok(ScopeKind::RequiresParent),
            Self::StructField => Ok(ScopeKind::RequiresParent),
            Self::Method => Ok(ScopeKind::RequiresParent),
            Self::EnumVariant => Ok(ScopeKind::RequiresParent),
            Self::VariantField => Ok(ScopeKind::RequiresParent),
            Self::ImplementsFor => Err(RelationConversionError::NotApplicable(self)),
            Self::ImplementsTrait => Err(RelationConversionError::NotApplicable(self)),
            Self::ResolvesToDefinition => Err(RelationConversionError::NotApplicable(self)),
            Self::Sibling => Err(RelationConversionError::NotApplicable(self)),
            Self::Inherits => Err(RelationConversionError::NotApplicable(self)),
            Self::CustomPath => Ok(ScopeKind::CanUse), // TODO: Revisit this one, not sure.
        }
    }
}
