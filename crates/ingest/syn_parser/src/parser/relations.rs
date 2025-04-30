use crate::parser::nodes::{
    EnumNodeId, FieldNodeId, FunctionNodeId, GenericParamNodeId, ImplNodeId, ImportNodeId,
    MacroNodeId, ModuleNodeId, ParamNodeId, ReexportNodeId, StructNodeId, TraitNodeId,
    TypeAliasNodeId, UnionNodeId, ValueNodeId, VariantNodeId,
};
use ploke_core::{NodeId, TypeId}; // Keep NodeId for generic targets
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Define the new error type
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationConversionError {
    #[error("Relation kind {0:?} is not applicable for ScopeKind conversion")]
    NotApplicable(SyntacticRelation), // Use the new enum type
}


// ANCHOR: Relation
// Removed original Relation struct and RelationKind enum.

/// Represents a type-safe structural or semantic relation between two nodes in the code graph.
/// Each variant enforces the correct NodeId types for its source and target where possible.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyntacticRelation {
    //-----------------------------------------------------------------------
    // Module Structure & Definition Relations (Primarily from ModuleTree)
    //-----------------------------------------------------------------------
    /// Module contains another node (function, struct, enum, impl, trait, module, import, etc.).
    /// Source: ModuleNodeId
    /// Target: NodeId (can be any node type contained within)
    Contains { source: ModuleNodeId, target: NodeId },

    /// Module declaration resolves to its definition.
    /// Source: ModuleNodeId (Declaration)
    /// Target: ModuleNodeId (Definition)
    ResolvesToDefinition { source: ModuleNodeId, target: ModuleNodeId },

    /// Module declaration uses `#[path]` attribute.
    /// Source: ModuleNodeId (Declaration)
    /// Target: ModuleNodeId (Definition pointed to by path)
    CustomPath { source: ModuleNodeId, target: ModuleNodeId },

    /// Module is a sibling file (e.g., `mod foo;` and `mod bar;` in `lib.rs`).
    /// Used for SPP calculation.
    /// Source: ModuleNodeId
    /// Target: ModuleNodeId
    Sibling { source: ModuleNodeId, target: ModuleNodeId },

    //-----------------------------------------------------------------------
    // Import/Export Relations (Primarily from ModuleTree & Visitor)
    //-----------------------------------------------------------------------
    /// Module contains an import statement.
    /// Source: ModuleNodeId
    /// Target: ImportNodeId
    ModuleImports { source: ModuleNodeId, target: ImportNodeId },

    /// An import statement re-exports an item.
    /// Source: ImportNodeId
    /// Target: NodeId (The actual item being re-exported)
    ReExports { source: ImportNodeId, target: NodeId },

    //-----------------------------------------------------------------------
    // Item Composition Relations (Primarily from Visitor)
    //-----------------------------------------------------------------------
    /// Struct contains a field.
    /// Source: StructNodeId
    /// Target: FieldNodeId
    StructField { source: StructNodeId, target: FieldNodeId },

    /// Union contains a field.
    /// Source: UnionNodeId
    /// Target: FieldNodeId
    UnionField { source: UnionNodeId, target: FieldNodeId },

    /// Enum variant contains a field (for struct-like variants).
    /// Source: VariantNodeId
    /// Target: FieldNodeId
    VariantField { source: VariantNodeId, target: FieldNodeId },

    /// Enum contains a variant.
    /// Source: EnumNodeId
    /// Target: VariantNodeId
    EnumVariant { source: EnumNodeId, target: VariantNodeId },

    /// Impl block contains an associated item (method, type, const).
    /// Source: ImplNodeId
    /// Target: NodeId (FunctionNodeId, TypeAliasNodeId, or ValueNodeId)
    ImplAssociatedItem { source: ImplNodeId, target: NodeId },

    /// Trait definition contains an associated item (method, type, const).
    /// Source: TraitNodeId
    /// Target: NodeId (FunctionNodeId, TypeAliasNodeId, or ValueNodeId)
    TraitAssociatedItem { source: TraitNodeId, target: NodeId },
}

impl SyntacticRelation {
    // Add helper methods if needed, e.g., getting source/target NodeId generically
    pub fn source_node_id(&self) -> NodeId {
        match *self {
            SyntacticRelation::Contains { source, .. } => source.into_inner(),
            SyntacticRelation::ResolvesToDefinition { source, .. } => source.into_inner(),
            SyntacticRelation::CustomPath { source, .. } => source.into_inner(),
            SyntacticRelation::Sibling { source, .. } => source.into_inner(),
            SyntacticRelation::ModuleImports { source, .. } => source.into_inner(),
            SyntacticRelation::ReExports { source, .. } => source.into_inner(),
            SyntacticRelation::StructField { source, .. } => source.into_inner(),
            SyntacticRelation::UnionField { source, .. } => source.into_inner(),
            SyntacticRelation::VariantField { source, .. } => source.into_inner(),
            SyntacticRelation::EnumVariant { source, .. } => source.into_inner(),
            SyntacticRelation::ImplAssociatedItem { source, .. } => source.into_inner(),
            SyntacticRelation::TraitAssociatedItem { source, .. } => source.into_inner(),
        }
    }

    pub fn target_node_id(&self) -> NodeId {
        match *self {
            SyntacticRelation::Contains { target, .. } => target, // Already NodeId
            SyntacticRelation::ResolvesToDefinition { target, .. } => target.into_inner(),
            SyntacticRelation::CustomPath { target, .. } => target.into_inner(),
            SyntacticRelation::Sibling { target, .. } => target.into_inner(),
            SyntacticRelation::ModuleImports { target, .. } => target.into_inner(),
            SyntacticRelation::ReExports { target, .. } => target, // Already NodeId
            SyntacticRelation::StructField { target, .. } => target.into_inner(),
            SyntacticRelation::UnionField { target, .. } => target.into_inner(),
            SyntacticRelation::VariantField { target, .. } => target.into_inner(),
            SyntacticRelation::EnumVariant { target, .. } => target.into_inner(),
            SyntacticRelation::ImplAssociatedItem { target, .. } => target, // Already NodeId
            SyntacticRelation::TraitAssociatedItem { target, .. } => target, // Already NodeId
        }
    }

    // Add other potential helpers: is_module_related, is_import_export_related, etc.
    // based on matching the variants.
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

impl TryInto<ScopeKind> for SyntacticRelation {
    type Error = RelationConversionError; // Use the new error type

    fn try_into(self) -> Result<ScopeKind, Self::Error> {
        match self {
            // Relations that allow the target to be used directly in the source scope
            Self::Contains { .. } => Ok(ScopeKind::CanUse),
            Self::ModuleImports { .. } => Ok(ScopeKind::CanUse),
            Self::ReExports { .. } => Ok(ScopeKind::CanUse),
            Self::CustomPath { .. } => Ok(ScopeKind::CanUse), // Module linked via path is usable

            // Relations where the target requires the source as a parent/qualifier
            Self::StructField { .. } => Ok(ScopeKind::RequiresParent), // e.g., my_struct.field
            Self::UnionField { .. } => Ok(ScopeKind::RequiresParent), // e.g., my_union.field
            Self::VariantField { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyEnum::Variant.field
            Self::EnumVariant { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyEnum::Variant
            Self::ImplAssociatedItem { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyType::assoc_fn()
            Self::TraitAssociatedItem { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyTrait::assoc_fn()

            // Relations that don't fit the ScopeKind model
            Self::ResolvesToDefinition { .. } => Err(RelationConversionError::NotApplicable(self)),
            Self::Sibling { .. } => Err(RelationConversionError::NotApplicable(self)),
        }
    }
}
