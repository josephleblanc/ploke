//! Private implementation module for strictly encapsulated typed node identifiers.
//!
//! Defines the ID newtype structs with private fields, implements necessary traits
//! internally, provides restricted constructors, and defines helper types like
//! `AnyNodeId`. Access to the base `NodeId` is confined to this module.

use crate::parser::graph::GraphNode;

// We will move ID definitions, trait implementations, etc., here later.
use super::*;
use ploke_core::NodeId;
use std::borrow::Borrow;
use std::fmt::Display;

// ----- Traits -----

/// Allows retrieving the corresponding `GraphNode` trait object from a graph
/// using a specific typed ID.
///
/// This trait is implemented internally for each specific ID type that represents
/// a node directly stored and retrievable in the `GraphAccess` implementor.
pub(crate) trait TypedNodeIdGet: Copy + private_traits::Sealed {
    // Added Copy bound as IDs are Copy
    // Added Sealed bound to prevent external implementations
    fn get<'g>(&self, graph: &'g dyn GraphAccess) -> Option<&'g dyn GraphNode>;
}

// ----- Macros -----

/// Macro to generate category enums (like PrimaryNodeId, AnyNodeId) that wrap specific typed IDs.
///
/// Generates:
/// - The enum definition with specified variants.
/// - Standard derives: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord.
/// - `impl EnumName`:
///   - `pub fn base_id(&self) -> NodeId`: Calls `base_id()` on the inner typed ID.
///   - `pub fn kind(&self) -> ItemKind` (optional): Returns the corresponding `ItemKind`.
/// - `impl From<SpecificIdType> for EnumName` for each variant.
///
/// # Usage
/// ```ignore
/// define_category_enum!(
///     #[doc = "Represents primary node IDs."] // Optional outer attributes
///     PrimaryNodeId, // Enum Name
///     ItemKind, // Include kind() method that returns this type
///     [ // List of variants: (VariantName, SpecificIdType, ItemKindValue)
///         (Function, FunctionNodeId, ItemKind::Function),
///         (Struct, StructNodeId, ItemKind::Struct),
///         // ...
///     ]
/// );
///
/// define_category_enum!(
///     AnyNodeId, // Enum Name
///     // No ItemKind specified, so kind() method won't be generated
///     [ // List of variants: (VariantName, SpecificIdType)
///         (Function, FunctionNodeId),
///         (Struct, StructNodeId),
///         // ... *all* specific IDs
///     ]
/// );
/// ```
macro_rules! define_category_enum {
    // Matcher for enums WITH an associated ItemKind method
    ($(#[$outer:meta])* $EnumName:ident, $KindType:ty, [ $( ($Variant:ident, $IdType:ty, $ItemKindVal:expr) ),* $(,)? ] ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $EnumName {
            $(
                $Variant($IdType),
            )*
        }

        impl $EnumName {
            /// Returns the underlying base NodeId using the internal `base_id` method
            /// of the wrapped specific ID type.
            #[inline]
            pub(super) fn base_id(&self) -> NodeId {
                match *self {
                    $(
                        $EnumName::$Variant(id) => id.base_id(),
                    )*
                }
            }

            /// Returns the corresponding ItemKind for this category ID variant.
            #[inline]
            pub fn kind(&self) -> $KindType {
                match *self {
                    $(
                        $EnumName::$Variant(_) => $ItemKindVal,
                    )*
                }
            }
        }

        $(
            impl From<$IdType> for $EnumName {
                #[inline]
                fn from(id: $IdType) -> Self {
                    $EnumName::$Variant(id)
                }
            }
        )* // <-- Add semicolon HERE
    };

    // Matcher for enums WITHOUT an associated ItemKind method (like AnyNodeId)
    ($(#[$outer:meta])* $EnumName:ident, [ $( ($Variant:ident, $IdType:ty) ),* $(,)? ] ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $EnumName {
            $(
                $Variant($IdType),
            )*
        }; // <-- Added semicolon

        impl $EnumName {
            /// Returns the underlying base NodeId using the internal `base_id` method
            /// of the wrapped specific ID type.
            #[inline]
            pub(super) fn base_id(&self) -> NodeId {
                match *self {
                    $(
                        $EnumName::$Variant(id) => id.base_id(),
                    )*
                }
            }
            // No kind() method generated for this variant
        }; // <-- Added semicolon

        $(
            impl From<$IdType> for $EnumName {
                #[inline]
                fn from(id: $IdType) -> Self {
                    $EnumName::$Variant(id)
                }
            }
        )*; // <-- Add semicolon HERE
    };
}

/// Macro to implement the `TypedNodeIdGet` trait for a specific ID type.
///
/// # Usage
/// ```ignore
/// // Implements TypedNodeIdGet for StructNodeId using graph.get_struct()
/// impl_typed_node_id_get!(StructNodeId, get_struct);
/// ```
macro_rules! impl_typed_node_id_get {
    ($IdType:ty, $GetterMethod:ident) => {
        // Implement the private sealing trait first
        impl private_traits::Sealed for $IdType {}
        // Implement the getter trait
        impl TypedNodeIdGet for $IdType {
            #[inline]
            fn get<'g>(&self, graph: &'g dyn GraphAccess) -> Option<&'g dyn GraphNode> {
                graph
                    .$GetterMethod(*self)
                    .map(|node| node as &dyn GraphNode)
            }
        }
    };
}

// ----- Internal Macro for Typed IDs -----

/// Macro to define a strictly encapsulated newtype wrapper around NodeId.
///
/// Generates:
/// - A public struct `StructName(NodeId)` where the `NodeId` field is private.
/// - Derives: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord.
/// - `impl StructName`:
///   - `pub(in crate::parser::nodes) fn create(NodeId) -> Self`: Restricted constructor.
///   - `pub(super) fn base_id(&self) -> NodeId`: Internal access to the base ID.
/// - `impl Display for StructName` (delegates to inner NodeId).
/// - `impl Borrow<NodeId>` and `impl AsRef<NodeId>` for internal use if needed (though direct access via `base_id` might be preferred).
///
/// # Usage (within this module)
/// ```ignore
/// define_internal_node_id!(
///     #[doc = "Identifier for a function node."]
///     struct FunctionNodeId {
///         markers: [TypedId, PrimaryNodeIdTrait] // Optional list of marker traits
///     }
/// );
/// ```
macro_rules! define_internal_node_id {
    // Matcher with optional markers block
    (
        $(#[$outer:meta])*
        struct $NewTypeId:ident { // Match 'struct Name {'
            $(markers: [$($MarkerTrait:path),*] $(,)? )?
        } // Match the closing '}' brace *after* the optional markers
    ) => { // Start expansion
        $(#[$outer])*
        // The struct is pub, but its field NodeId is private
        // because NodeId itself is not pub in this scope after potential future refactoring
        // or simply because tuple struct fields are private without `pub`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub struct $NewTypeId(NodeId);

        impl $NewTypeId {
            /// Creates a new typed ID. Restricted constructor.
            /// Only code within `crate::parser::nodes` can call this.
            /// Ensures typed IDs are only created alongside actual node construction.
            #[inline]
            pub(in crate::parser::nodes) fn create(id: NodeId) -> Self {
                Self(id)
            }

            /// Get the underlying base NodeId.
            /// Restricted visibility (`pub(super)`) allows access only within the `ids` module.
            /// This is the controlled escape hatch for internal operations like hashing,
            /// indexing in generic maps, or passing context to ploke-core.
            #[inline]
            pub(super) fn base_id(&self) -> NodeId {
                self.0
            }

            // We intentionally DO NOT provide public or pub(crate) `into_inner` or `as_inner`.
            // Access to the base ID outside this module should be impossible.
        }

        impl std::fmt::Display for $NewTypeId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // Delegate to the inner NodeId's Display implementation
                write!(f, "{}", self.0)
            }
        }

        // These Borrow/AsRef impls might be useful for internal generic code
        // within the `ids` module that needs to operate on the base ID without
        // consuming the wrapper. However, calling `base_id()` might be clearer.
        // Keep them commented out unless a clear need arises.
        impl std::borrow::Borrow<NodeId> for $NewTypeId {
            #[inline]
            fn borrow(&self) -> &NodeId {
                &self.0
            }
        }
        //
        impl AsRef<NodeId> for $NewTypeId {
            #[inline]
            fn as_ref(&self) -> &NodeId {
                &self.0
            }
        }
        // AI: Implement `TryFrom<PrimaryNodeId>` for $NewTypeId AI!


        // Implement the base TypedId trait for all generated IDs
        // Ensure the TypedId trait is defined in this scope or accessible via path
        impl $crate::parser::nodes::ids::internal::TypedId for $NewTypeId {}

        // Implement specified marker traits
        // Ensure marker traits are defined in this scope or accessible via path
        $( $(impl $MarkerTrait for $NewTypeId {})* )?
    };
}

// Now use the *new* internal macro with markers
define_internal_node_id!(
    struct EnumNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct FunctionNodeId {
        markers: [PrimaryNodeIdTrait],
    }
); // For standalone functions
define_internal_node_id!(
    struct MethodNodeId {
        markers: [AssociatedItemIdTrait], // Removed trailing comma
    }
); // For associated functions/methods
define_internal_node_id!(
    struct ImplNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct ImportNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct ModuleNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
); // Use the macro now
define_internal_node_id!(
    struct StructNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct TraitNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(struct TypeAliasNodeId { markers: [PrimaryNodeIdTrait, AssociatedItemIdTrait] }); // Can be both primary and associated
define_internal_node_id!(
    struct UnionNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);
// Removed ValueNodeId
define_internal_node_id!(struct ConstNodeId { markers: [PrimaryNodeIdTrait, AssociatedItemIdTrait] }); // Can be both primary and associated
define_internal_node_id!(
    struct StaticNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
); // Added
define_internal_node_id!(
    struct FieldNodeId {
        markers: [SecondaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct VariantNodeId {
        markers: [SecondaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct ParamNodeId {
        markers: [SecondaryNodeIdTrait], // Removed trailing comma
    }
); // For ParamData
define_internal_node_id!(
    struct GenericParamNodeId {
        markers: [SecondaryNodeIdTrait], // Removed trailing comma
    }
);
define_internal_node_id!(
    struct MacroNodeId {
        markers: [PrimaryNodeIdTrait], // Removed trailing comma
    }
);

// For more explicit differntiation within Phase 3 module tree processing
define_internal_node_id!(struct ReexportNodeId { markers: [] }); // No specific category yet, just TypedId
                                                                 // --- Category ID Enums ---

use ploke_core::ItemKind; // Need ItemKind for kind() methods

pub trait PrimaryNodeMarker {}

impl PrimaryNodeMarker for FunctionNode {}
impl PrimaryNodeMarker for StructNode {}
impl PrimaryNodeMarker for UnionNode {}
impl PrimaryNodeMarker for EnumNode {}
impl PrimaryNodeMarker for TypeAliasNode {}
impl PrimaryNodeMarker for TraitNode {}
impl PrimaryNodeMarker for ImplNode {}
impl PrimaryNodeMarker for ConstNode {}
impl PrimaryNodeMarker for StaticNode {}
impl PrimaryNodeMarker for MacroNode {}
impl PrimaryNodeMarker for ImportNode {}
impl PrimaryNodeMarker for ModuleNode {}

// --- Marker Traits ---
// Define the marker traits themselves here.
// Implementations are generated by the define_internal_node_id! macro.
pub trait TypedId:
    Copy
    + std::fmt::Debug
    + std::hash::Hash
    + Eq
    + Ord
    + Serialize
    + for<'a> Deserialize<'a>
    + Send
    + Sync
{
} // Base trait for all typed IDs - Added common bounds
pub trait PrimaryNodeIdTrait: TypedId {} // Marker for primary node IDs
pub trait AssociatedItemIdTrait: TypedId {} // Marker for associated item IDs
pub trait SecondaryNodeIdTrait: TypedId {} // Marker for secondary node IDs (fields, params, etc.)

// Add other category marker traits as needed

/// Private module for the sealing pattern. Prevents external crates or modules                    
/// from implementing traits intended only for internal ID types (like TypedNodeIdGet).            
mod private_traits {
    /// The sealing trait. Cannot be named or implemented outside this module.                     
    pub(super) trait Sealed {}
}
// --- TypedNodeIdGet Implementations ---
// Primary Nodes
impl_typed_node_id_get!(FunctionNodeId, get_function);
impl_typed_node_id_get!(StructNodeId, get_struct);
impl_typed_node_id_get!(EnumNodeId, get_enum);
impl_typed_node_id_get!(UnionNodeId, get_union);
impl_typed_node_id_get!(TypeAliasNodeId, get_type_alias);
impl_typed_node_id_get!(TraitNodeId, get_trait);
impl_typed_node_id_get!(ImplNodeId, get_impl);
impl_typed_node_id_get!(ConstNodeId, get_const);
impl_typed_node_id_get!(StaticNodeId, get_static);
impl_typed_node_id_get!(MacroNodeId, get_macro);
impl_typed_node_id_get!(ImportNodeId, get_import);
impl_typed_node_id_get!(ModuleNodeId, get_module);

// Associated Items (Methods are retrieved via their parent Impl/Trait, not directly)
// Note: MethodNodeId does *not* get an impl, as there's no `graph.get_method(MethodNodeId)`

// Secondary Nodes (Fields, Variants, Params, Generics are part of their parent node, not directly retrieved)
// Note: FieldNodeId, VariantNodeId, ParamNodeId, GenericParamNodeId do *not* get impls.

// Other IDs
// Note: ReexportNodeId does *not* get an impl.

// --- Generated Category Enums ---

define_category_enum!(
    #[doc = "Represents the ID of any node type that can typically be defined directly within a module scope (primary items)."]
    PrimaryNodeId,
    ItemKind,
    [
        (Function, FunctionNodeId, ItemKind::Function),
        (Struct, StructNodeId, ItemKind::Struct),
        (Enum, EnumNodeId, ItemKind::Enum),
        (Union, UnionNodeId, ItemKind::Union),
        (TypeAlias, TypeAliasNodeId, ItemKind::TypeAlias),
        (Trait, TraitNodeId, ItemKind::Trait),
        (Impl, ImplNodeId, ItemKind::Impl),
        (Const, ConstNodeId, ItemKind::Const),
        (Static, StaticNodeId, ItemKind::Static),
        (Macro, MacroNodeId, ItemKind::Macro),
        (Import, ImportNodeId, ItemKind::Import),
        (Module, ModuleNodeId, ItemKind::Module),
    ]
);

define_category_enum!(
    #[doc = "Represents the ID of any node type that can be an associated item within an `impl` or `trait` block."]
    AssociatedItemId,
    ItemKind,
    [
        (Method, MethodNodeId, ItemKind::Method),
        (TypeAlias, TypeAliasNodeId, ItemKind::TypeAlias), // Associated types use TypeAliasNodeId
        (Const, ConstNodeId, ItemKind::Const),             // Associated consts use ConstNodeId
    ]
);

// --- Manually Defined AnyNodeId ---

/// Represents the ID of *any* node type in the graph. Used as a key for heterogeneous storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum AnyNodeId {
    // Primary Nodes
    Function(FunctionNodeId),
    Struct(StructNodeId),
    Enum(EnumNodeId),
    Union(UnionNodeId),
    TypeAlias(TypeAliasNodeId),
    Trait(TraitNodeId),
    Impl(ImplNodeId),
    Const(ConstNodeId),
    Static(StaticNodeId),
    Macro(MacroNodeId),
    Import(ImportNodeId),
    Module(ModuleNodeId),
    // Associated Items (using their specific IDs)
    Method(MethodNodeId),
    // Secondary Nodes
    Field(FieldNodeId),
    Variant(VariantNodeId),
    Param(ParamNodeId),
    GenericParam(GenericParamNodeId),
    // Other IDs
    Reexport(ReexportNodeId),
    // Add any other specific ID types here as they are created
}

impl AnyNodeId {
    /// Returns the underlying base NodeId using the internal `base_id` method
    /// of the wrapped specific ID type.
    #[inline]
    pub(super) fn base_id(&self) -> NodeId {
        match *self {
            // Primary Nodes
            AnyNodeId::Function(id) => id.base_id(),
            AnyNodeId::Struct(id) => id.base_id(),
            AnyNodeId::Enum(id) => id.base_id(),
            AnyNodeId::Union(id) => id.base_id(),
            AnyNodeId::TypeAlias(id) => id.base_id(),
            AnyNodeId::Trait(id) => id.base_id(),
            AnyNodeId::Impl(id) => id.base_id(),
            AnyNodeId::Const(id) => id.base_id(),
            AnyNodeId::Static(id) => id.base_id(),
            AnyNodeId::Macro(id) => id.base_id(),
            AnyNodeId::Import(id) => id.base_id(),
            AnyNodeId::Module(id) => id.base_id(),
            // Associated Items
            AnyNodeId::Method(id) => id.base_id(),
            // Secondary Nodes
            AnyNodeId::Field(id) => id.base_id(),
            AnyNodeId::Variant(id) => id.base_id(),
            AnyNodeId::Param(id) => id.base_id(),
            AnyNodeId::GenericParam(id) => id.base_id(),
            // Other IDs
            AnyNodeId::Reexport(id) => id.base_id(),
        }
    }
}

// --- From Implementations for AnyNodeId ---

// Primary Nodes
impl From<FunctionNodeId> for AnyNodeId {
    #[inline]
    fn from(id: FunctionNodeId) -> Self {
        AnyNodeId::Function(id)
    }
}
impl From<StructNodeId> for AnyNodeId {
    #[inline]
    fn from(id: StructNodeId) -> Self {
        AnyNodeId::Struct(id)
    }
}
impl From<EnumNodeId> for AnyNodeId {
    #[inline]
    fn from(id: EnumNodeId) -> Self {
        AnyNodeId::Enum(id)
    }
}
impl From<UnionNodeId> for AnyNodeId {
    #[inline]
    fn from(id: UnionNodeId) -> Self {
        AnyNodeId::Union(id)
    }
}
impl From<TypeAliasNodeId> for AnyNodeId {
    #[inline]
    fn from(id: TypeAliasNodeId) -> Self {
        AnyNodeId::TypeAlias(id)
    }
}
impl From<TraitNodeId> for AnyNodeId {
    #[inline]
    fn from(id: TraitNodeId) -> Self {
        AnyNodeId::Trait(id)
    }
}
impl From<ImplNodeId> for AnyNodeId {
    #[inline]
    fn from(id: ImplNodeId) -> Self {
        AnyNodeId::Impl(id)
    }
}
impl From<ConstNodeId> for AnyNodeId {
    #[inline]
    fn from(id: ConstNodeId) -> Self {
        AnyNodeId::Const(id)
    }
}
impl From<StaticNodeId> for AnyNodeId {
    #[inline]
    fn from(id: StaticNodeId) -> Self {
        AnyNodeId::Static(id)
    }
}
impl From<MacroNodeId> for AnyNodeId {
    #[inline]
    fn from(id: MacroNodeId) -> Self {
        AnyNodeId::Macro(id)
    }
}
impl From<ImportNodeId> for AnyNodeId {
    #[inline]
    fn from(id: ImportNodeId) -> Self {
        AnyNodeId::Import(id)
    }
}
impl From<ModuleNodeId> for AnyNodeId {
    #[inline]
    fn from(id: ModuleNodeId) -> Self {
        AnyNodeId::Module(id)
    }
}
// Associated Items
impl From<MethodNodeId> for AnyNodeId {
    #[inline]
    fn from(id: MethodNodeId) -> Self {
        AnyNodeId::Method(id)
    }
}
// Secondary Nodes
impl From<FieldNodeId> for AnyNodeId {
    #[inline]
    fn from(id: FieldNodeId) -> Self {
        AnyNodeId::Field(id)
    }
}
impl From<VariantNodeId> for AnyNodeId {
    #[inline]
    fn from(id: VariantNodeId) -> Self {
        AnyNodeId::Variant(id)
    }
}
impl From<ParamNodeId> for AnyNodeId {
    #[inline]
    fn from(id: ParamNodeId) -> Self {
        AnyNodeId::Param(id)
    }
}
impl From<GenericParamNodeId> for AnyNodeId {
    #[inline]
    fn from(id: GenericParamNodeId) -> Self {
        AnyNodeId::GenericParam(id)
    }
}
// Other IDs
impl From<ReexportNodeId> for AnyNodeId {
    #[inline]
    fn from(id: ReexportNodeId) -> Self {
        AnyNodeId::Reexport(id)
    }
}

// --- Node Struct Definitions ---
// Logging target
