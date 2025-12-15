// Import specific typed IDs AND the new category enums
use super::nodes::{AnyNodeId, PrimaryNodeIdTrait};
use crate::parser::nodes::{
    AssociatedItemNodeId, EnumNodeId, FieldNodeId, ImplNodeId, ImportNodeId, ModuleNodeId,
    PrimaryNodeId, StructNodeId, TraitNodeId, UnionNodeId, VariantNodeId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Define the new error type
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationConversionError {
    #[error("Relation kind {0:?} is not applicable for ScopeKind conversion")]
    NotApplicable(SyntacticRelation),
}
/// Represents a type-safe structural or semantic relation between two nodes in the code graph.
/// Each variant enforces the correct NodeId types for its source and target where possible.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyntacticRelation {
    //-----------------------------------------------------------------------
    //                Module Structure & Definition Relations
    //-----------------------------------------------------------------------
    // ----------------------------------
    // ----- Created in CodeVisitor -----
    // ----------------------------------
    /// Module contains another node (function, struct, enum, impl, trait, module, import, etc.).
    /// Created in CodeVisitor
    /// Source: ModuleNodeId
    /// Target: PrimaryNodeId (Restricts target to primary item types)
    // #
    Contains {
        source: ModuleNodeId,
        target: PrimaryNodeId,
    },

    // ---------------------------------
    // ----- Created in ModuleTree -----
    // ---------------------------------
    /// Module declaration resolves to its definition.
    /// Created in CodeVisitor
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
    //                      Import/Export Relations
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
    /// A definition is brought into scope by a `use`/`pub use` site.
    /// Source: PrimaryNodeId (definition)
    /// Target: ImportNodeId (the import/re-export site)
    ImportedBy {
        source: PrimaryNodeId,
        target: ImportNodeId,
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
    /// Target: AssociatedItemNodeId (Restricts target to valid associated item types)
    // NOTE: Note yet implemented (2025-05-02)
    //
    ImplAssociatedItem {
        source: ImplNodeId,
        target: AssociatedItemNodeId,
    },

    /// Trait definition contains an associated item (method, type, const).
    /// Source: TraitNodeId
    /// Target: AssociatedItemNodeId (Restricts target to valid associated item types)
    // NOTE: Note yet implemented (2025-05-02)
    TraitAssociatedItem {
        source: TraitNodeId,
        target: AssociatedItemNodeId,
    },
}

// pub fn find_ancestor(child: PrimaryNodeId, rels: &[ SyntacticRelation ]) -> ModuleNodeId {
//     rels.iter().find_map(|r| r.contains_target(child).or_else())
//
// }

impl SyntacticRelation {
    pub fn source_contains<T: PrimaryNodeIdTrait>(&self, trg: T) -> Option<ModuleNodeId> {
        match self {
            Self::Contains { target: t, source } if *t == trg.to_pid() => Some(*source),
            _ => None,
        }
    }
    pub fn contains_target<T: PrimaryNodeIdTrait>(&self, src: ModuleNodeId) -> Option<T> {
        match self {
            Self::Contains { source: s, target } if *s == src => T::try_from(*target).ok(),
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
    pub fn source_reexports(&self, src: ImportNodeId) -> Option<PrimaryNodeId> {
        match self {
            Self::ReExports { source: s, target } if *s == src => Some(*target),
            _ => None,
        }
    }
    pub fn imported_by(&self, src: PrimaryNodeId) -> Option<ImportNodeId> {
        match self {
            Self::ImportedBy { source: s, target } if *s == src => Some(*target),
            _ => None,
        }
    }
    pub fn target_exported_by(&self, trg: PrimaryNodeId) -> Option<ImportNodeId> {
        match self {
            Self::ReExports { source, target: t } if *t == trg => Some(*source),
            _ => None,
        }
    }
    pub fn union_field(&self, src: UnionNodeId) -> Option<FieldNodeId> {
        match self {
            Self::UnionField { source: s, target } if *s == src => Some(*target),
            _ => None,
        }
    }
    pub fn field_of_union(&self, trg: FieldNodeId) -> Option<UnionNodeId> {
        match self {
            Self::UnionField { source, target: t } if *t == trg => Some(*source),
            _ => None,
        }
    }

    /// Returns the source NodeId of the relation as a generic TypeId.
    pub fn source(&self) -> AnyNodeId {
        match *self {
            SyntacticRelation::Contains { source, .. } => source.into(),
            SyntacticRelation::ResolvesToDefinition { source, .. } => source.into(),
            SyntacticRelation::CustomPath { source, .. } => source.into(),
            SyntacticRelation::Sibling { source, .. } => source.into(),
            SyntacticRelation::ModuleImports { source, .. } => source.into(),
            SyntacticRelation::ReExports { source, .. } => source.into(),
            SyntacticRelation::ImportedBy { source, .. } => source.into(),
            SyntacticRelation::StructField { source, .. } => source.into(),
            SyntacticRelation::UnionField { source, .. } => source.into(),
            SyntacticRelation::VariantField { source, .. } => source.into(),
            SyntacticRelation::EnumVariant { source, .. } => source.into(),
            SyntacticRelation::ImplAssociatedItem { source, .. } => source.into(),
            SyntacticRelation::TraitAssociatedItem { source, .. } => source.into(),
        }
    }

    /// Returns the target NodeId of the relation as a generic TypeId.
    pub fn target(&self) -> AnyNodeId {
        match *self {
            SyntacticRelation::Contains { target, .. } => target.into(),
            SyntacticRelation::ResolvesToDefinition { target, .. } => target.into(),
            SyntacticRelation::CustomPath { target, .. } => target.into(),
            SyntacticRelation::Sibling { target, .. } => target.into(),
            SyntacticRelation::ModuleImports { target, .. } => target.into(),
            SyntacticRelation::ReExports { target, .. } => target.into(),
            SyntacticRelation::ImportedBy { target, .. } => target.into(),
            SyntacticRelation::StructField { target, .. } => target.into(),
            SyntacticRelation::UnionField { target, .. } => target.into(),
            SyntacticRelation::VariantField { target, .. } => target.into(),
            SyntacticRelation::EnumVariant { target, .. } => target.into(),
            SyntacticRelation::ImplAssociatedItem { target, .. } => target.into(),
            SyntacticRelation::TraitAssociatedItem { target, .. } => target.into(),
        }
    }

    pub fn src_eq_trg(&self, r_next: Self) -> bool {
        self.source() == r_next.target()
    }
    pub fn trg_eq_src(&self, r_next: Self) -> bool {
        self.target() == r_next.source()
    }
    pub fn src_eq_src(&self, r_next: Self) -> bool {
        self.source() == r_next.target()
    }
    pub fn trg_eq_trg(&self, r_next: Self) -> bool {
        self.target() == r_next.target()
    }

    /// Formats the name of the `SyntacticRelation` variant name as a string.
    ///
    /// Useful for translating into the database. For now we will follow a convention of the
    /// relation type being represented with the same name case (camel-case) as the original enum.
    pub fn kind_str(&self) -> &'static str {
        match self {
            SyntacticRelation::Contains { .. } => "Contains",
            SyntacticRelation::ResolvesToDefinition { .. } => "ResolvesToDefinition",
            SyntacticRelation::CustomPath { .. } => "CustomPath",
            SyntacticRelation::Sibling { .. } => "Sibling",
            SyntacticRelation::ModuleImports { .. } => "ModuleImports",
            SyntacticRelation::ReExports { .. } => "ReExports",
            SyntacticRelation::ImportedBy { .. } => "ImportedBy",
            SyntacticRelation::StructField { .. } => "StructField",
            SyntacticRelation::UnionField { .. } => "UnionField",
            SyntacticRelation::VariantField { .. } => "VariantField",
            SyntacticRelation::EnumVariant { .. } => "EnumVariant",
            SyntacticRelation::ImplAssociatedItem { .. } => "ImplAssociatedItem",
            SyntacticRelation::TraitAssociatedItem { .. } => "TraitAssociatedItem",
        }
    }

    #[cfg(not(feature = "not_wip_marker"))]
    pub fn traverse_pub(&self, r_next: Self) {
        match *self {
            // e.g. with match arms for chosen traversal
            SyntacticRelation::Contains { source, target: t } if r_next.trg_eq_src(*t) => {
                todo!()
            }
            SyntacticRelation::ResolvesToDefinition { source, target } => todo!(),
            SyntacticRelation::CustomPath { source, target } => todo!(),
            SyntacticRelation::ReExports { source, target } => todo!(),
            SyntacticRelation::ModuleImports { source, target } => todo!(),
            _ => None,
        }
    }

    /// Returns `true` if the syntactic relation is [`Contains`].
    ///
    /// [`Contains`]: SyntacticRelation::Contains
    #[must_use]
    pub fn is_contains(&self) -> bool {
        matches!(self, Self::Contains { .. })
    }
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
            SyntacticRelation::ImportedBy { source, target } => {
                write!(f, "ImportedBy({} → {})", source, target)
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
            Self::ImportedBy { .. } => Ok(ScopeKind::CanUse),
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
