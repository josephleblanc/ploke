use super::nodes::GraphId;
use ploke_core::{NodeId, TypeId}; // Import NodeId and TypeId
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Define the new error type
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationConversionError {
    #[error("Relation kind {0:?} is not applicable for ScopeKind conversion")]
    NotApplicable(RelationKind),
}

/// Represents a relationship where a Node is linked to a Type.
/// These are typically stored directly on the Node struct rather than as separate Relation entries.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeTypeRelationKind {
    /// FunctionNode -----------FunctionParameter-----> TypeId of ParamNode
    FunctionParameter,
    /// FunctionNode -----------FunctionReturn--------> TypeId of return type
    FunctionReturn,
    /// ImplNode ---------------ImplementsFor---------> TypeId of `Self`
    ImplementsFor,
    /// ImplNode ---------------ImplementsTrait-------> TypeId of trait
    ImplementsTrait,
    /// ValueNode --------------ValueType-------------> TypeId of its own type
    ValueType,
    /// TraitNode --------------Inherits--------------> TypeId of supertrait
    Inherits,
    /// ImportNode (Extern) ----Uses------------------> TypeId (representing the crate type?)
    /// TODO: Revisit if 'Uses' for extern crate makes sense or should be removed.
    Uses,
    /// TypeAliasNode ----------Aliases---------------> TypeId of the target type
    Aliases, // Added for TypeAliasNode -> TypeId link
}

// ANCHOR: Relation
/// Represents a structural or semantic relation *between two Nodes* in the code graph.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Relation {
    pub source: NodeId, // Changed from GraphId
    pub target: NodeId, // Changed from GraphId

    pub kind: RelationKind,
}

impl Relation {
    /// Checks if the relation's source matches the given `NodeId`.
    pub fn matches_source(&self, source_id: NodeId) -> bool {
        self.source == source_id
    }

    // Removed matches_source_node (redundant)
    // Removed matches_source_type

    /// Checks if the relation's target matches the given `NodeId`.
    pub fn matches_target(&self, target_id: NodeId) -> bool {
        self.target == target_id
    }

    // Removed matches_target_node (redundant)
    // Removed matches_target_type

    /// Checks if the relation's source and kind match the given values.
    pub fn matches_source_and_kind(&self, source_id: NodeId, kind: RelationKind) -> bool {
        self.source == source_id && self.kind == kind
    }

    /// Checks if the relation's target and kind match the given values.
    pub fn matches_target_and_kind(&self, target_id: NodeId, kind: RelationKind) -> bool {
        self.target == target_id && self.kind == kind
    }

    /// Checks if the relation's source, target, and kind match the given values.
    pub fn matches_source_target_kind(
        &self,
        source_id: NodeId,
        target_id: NodeId,
        kind: RelationKind,
    ) -> bool {
        self.source == source_id && self.target == target_id && self.kind == kind
    }
}

// Different kinds of relations
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    //-----------------------------------------------------------------------
    //-----------------Defined in ModuleTree Methods-------------------------
    //-----------------------------------------------------------------------
    // e.g. `pub use some_dep::a::b::ReExportedStruct`--ReExport-->ReExportedStruct defn
    // (currently only targets module in module_tree.rs)
    // ImportNode--------------ReExports-------------> NodeId of reexported item
    // The NodeId of the ReExported item might be another re-export.
    // We need a new Relation to represent that connection, but it will be in a different set of
    // logical relations, whereas all of these relations are meant to be syntactically accurate.
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
    // (including other modules, functions, types, etc.)
    Contains,
    // ModuleNode -------------ModuleImports---------> ImportNode (NodeId)
    // NOTE: all `use` and `pub use` included, not distinguished by relation
    ModuleImports,
    // StructNode/EnumNode/UnionNode ----Field---> FieldNode (NodeId)
    // Used for fields within structs, unions, and enum variants.
    Field, // Consolidated StructField and VariantField
    // EnumNode ---------------EnumVariant-----------> VariantNode (NodeId)
    EnumVariant,
    // ImplNode/TraitNode -----AssociatedItem--------> FunctionNode/TypeAliasNode/ConstNode (NodeId)
    // Represents items defined within an impl or trait block (methods, associated types, consts).
    AssociatedItem, // Consolidated Method, TraitAssociatedItem, ImplAssociatedItem

    // MacroUse, // Example: FunctionNode --MacroUse--> MacroNode
    // References, // Example: FunctionNode --References--> StructNode (if function body uses it)
    // MacroExpansion, // Example: MacroInvocationNode --MacroExpansion--> GeneratedNode(s)
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

    /// Checks if this relation kind relates to importing or exporting items.
    pub fn is_import_export_related(self) -> bool {
        matches!(self, RelationKind::ModuleImports | RelationKind::ReExports)
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
            // Relations that allow the target to be used directly in the source scope
            Self::Contains => Ok(ScopeKind::CanUse),
            Self::ModuleImports => Ok(ScopeKind::CanUse),
            Self::ReExports => Ok(ScopeKind::CanUse),
            Self::CustomPath => Ok(ScopeKind::CanUse), // Module linked via path is usable

            // Relations where the target requires the source as a parent/qualifier
            Self::Field => Ok(ScopeKind::RequiresParent), // e.g., my_struct.field
            Self::EnumVariant => Ok(ScopeKind::RequiresParent), // e.g., MyEnum::Variant
            Self::AssociatedItem => Ok(ScopeKind::RequiresParent), // e.g., MyType::assoc_fn()

            // Relations that don't fit the ScopeKind model
            Self::ResolvesToDefinition => Err(RelationConversionError::NotApplicable(self)),
            Self::Sibling => Err(RelationConversionError::NotApplicable(self)),
        }
    }
}
