// Import specific typed IDs AND the new category enums
use crate::parser::nodes::{
    AssociatedItemId, EnumNodeId, FieldNodeId, FunctionNodeId, GenericParamNodeId, ImplNodeId,
    ImportNodeId, MacroNodeId, ModuleNodeId, ParamNodeId, PrimaryNodeId, ReexportNodeId,
    StructNodeId, TraitNodeId, TypeAliasNodeId, UnionNodeId, VariantNodeId,
};
use ploke_core::TypeId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::nodes::PrimaryNodeIdTrait;

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
    /// Target: PrimaryNodeId (Restricts target to primary item types)
    Contains {
        source: ModuleNodeId,
        target: PrimaryNodeId,
    },

    /// Module declaration resolves to its definition.
    /// Source: ModuleNodeId (Declaration)
    /// Target: ModuleNodeId (Definition)
    ResolvesToDefinition {
        source: ModuleNodeId,
        target: ModuleNodeId,
    },

    /// Module declaration uses `#[path]` attribute.
    /// Source: ModuleNodeId (Declaration)
    /// Target: ModuleNodeId (Definition pointed to by path)
    CustomPath {
        source: ModuleNodeId,
        target: ModuleNodeId,
    },

    /// Module is a sibling file (e.g., `mod foo;` and `mod bar;` in `lib.rs`).
    /// Used for SPP calculation.
    /// Source: ModuleNodeId
    /// Target: ModuleNodeId
    Sibling {
        source: ModuleNodeId,
        target: ModuleNodeId,
    },

    //-----------------------------------------------------------------------
    // Import/Export Relations (Primarily from ModuleTree & Visitor)
    //-----------------------------------------------------------------------
    /// Module contains an import statement.
    /// Source: ModuleNodeId
    /// Target: ImportNodeId
    ModuleImports {
        source: ModuleNodeId,
        target: ImportNodeId,
    },

    /// An import statement re-exports an item.
    /// Source: ImportNodeId
    /// Target: PrimaryNodeId (Restricts target to primary item types)
    ReExports {
        source: ImportNodeId,
        target: PrimaryNodeId,
    },

    //-----------------------------------------------------------------------
    // Item Composition Relations (Primarily from Visitor)
    //-----------------------------------------------------------------------
    /// Struct contains a field.
    /// Source: StructNodeId
    /// Target: FieldNodeId
    StructField {
        source: StructNodeId,
        target: FieldNodeId,
    },

    /// Union contains a field.
    /// Source: UnionNodeId
    /// Target: FieldNodeId
    UnionField {
        source: UnionNodeId,
        target: FieldNodeId,
    },

    /// Enum variant contains a field (for struct-like variants).
    /// Source: VariantNodeId
    /// Target: FieldNodeId
    VariantField {
        source: VariantNodeId,
        target: FieldNodeId,
    },

    /// Enum contains a variant.
    /// Source: EnumNodeId
    /// Target: VariantNodeId
    EnumVariant {
        source: EnumNodeId,
        target: VariantNodeId,
    },

    /// Impl block contains an associated item (method, type, const).
    /// Source: ImplNodeId
    /// Target: AssociatedItemId (Restricts target to valid associated item types)
    ImplAssociatedItem {
        source: ImplNodeId,
        target: AssociatedItemId,
    },

    /// Trait definition contains an associated item (method, type, const).
    /// Source: TraitNodeId
    /// Target: AssociatedItemId (Restricts target to valid associated item types)
    TraitAssociatedItem {
        source: TraitNodeId,
        target: AssociatedItemId,
    },
}

// pub fn find_ancestor(child: PrimaryNodeId, rels: &[ SyntacticRelation ]) -> ModuleNodeId {
//     rels.iter().find_map(|r| r.contains_target(child).or_else())
//
// }

impl SyntacticRelation {
    // Add helper methods if needed, e.g., getting source/target NodeId generically
    pub fn source_contains(&self, trg: PrimaryNodeId) -> Option<ModuleNodeId> {
        match self {
            Self::Contains { target: t, source } if *t == trg => Some(*source),
            _ => None,
        }
    }
    pub fn contains_target<T: From<PrimaryNodeId>>(&self, src: ModuleNodeId) -> Option<T> {
        match self {
            Self::Contains { source: s, target } if *s == src => Some(T::from(*target)),
            _ => None,
        }
    }
    pub fn resolves_to_defn(&self, decl: ModuleNodeId) -> Option<ModuleNodeId> {
        match self {
            Self::ResolvesToDefinition {
                source: s,
                target: defn,
            } if *s == decl => Some(*defn),
            _ => None,
        }
    }
    pub fn resolved_by_decl(&self, decl: ModuleNodeId) -> Option<ModuleNodeId> {
        match self {
            Self::ResolvesToDefinition { source, target: t } if *t == decl => Some(*source),
            _ => None,
        }
    }
    pub fn mod_tree_parent(&self, child: PrimaryNodeId) -> Option<ModuleNodeId> {
        self.source_contains(child)?;
        child
            .try_into()
            .ok()
            .and_then(|m_child| self.resolved_by_decl(m_child))
    }
    // Implement a `target()` and `source()` method AI!
}
impl std::fmt::Display for SyntacticRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyntacticRelation::Contains { source, target } => {
                write!(f, "Contains({} → {})", source, target)
            }
            SyntacticRelation::ResolvesToDefinition { source, target } => {
                write!(f, "ResolvesToDefinition({} → {})", source, target)
            }
            SyntacticRelation::CustomPath { source, target } => {
                write!(f, "CustomPath({} → {})", source, target)
            }
            SyntacticRelation::Sibling { source, target } => {
                write!(f, "Sibling({} → {})", source, target)
            }
            SyntacticRelation::ModuleImports { source, target } => {
                write!(f, "ModuleImports({} → {})", source, target)
            }
            SyntacticRelation::ReExports { source, target } => {
                write!(f, "ReExports({} → {})", source, target)
            }
            SyntacticRelation::StructField { source, target } => {
                write!(f, "StructField({} → {})", source, target)
            }
            SyntacticRelation::UnionField { source, target } => {
                write!(f, "UnionField({} → {})", source, target)
            }
            SyntacticRelation::VariantField { source, target } => {
                write!(f, "VariantField({} → {})", source, target)
            }
            SyntacticRelation::EnumVariant { source, target } => {
                write!(f, "EnumVariant({} → {})", source, target)
            }
            SyntacticRelation::ImplAssociatedItem { source, target } => {
                write!(f, "ImplAssociatedItem({} → {})", source, target)
            }
            SyntacticRelation::TraitAssociatedItem { source, target } => {
                write!(f, "TraitAssociatedItem({} → {})", source, target)
            }
        }
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
            Self::UnionField { .. } => Ok(ScopeKind::RequiresParent),  // e.g., my_union.field
            Self::VariantField { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyEnum::Variant.field
            Self::EnumVariant { .. } => Ok(ScopeKind::RequiresParent),  // e.g., MyEnum::Variant
            Self::ImplAssociatedItem { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyType::assoc_fn()
            Self::TraitAssociatedItem { .. } => Ok(ScopeKind::RequiresParent), // e.g., MyTrait::assoc_fn()

            // Relations that don't fit the ScopeKind model
            Self::ResolvesToDefinition { .. } => Err(RelationConversionError::NotApplicable(self)),
            Self::Sibling { .. } => Err(RelationConversionError::NotApplicable(self)),
        }
    }
}
