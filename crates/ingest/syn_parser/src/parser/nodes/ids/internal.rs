//! Private implementation module for strictly encapsulated typed node identifiers.
//!
//! Defines the ID newtype structs with private fields, implements necessary traits
//! internally, provides restricted constructors, and defines helper types like
//! `AnyNodeId`. Access to the base `NodeId` is confined to this module.

use crate::parser::graph::GraphAccess;
use crate::parser::visitor::VisitorState;

// We will move ID definitions, trait implementations, etc., here later.
use super::*;
use crate::utils::LOG_TARGET_NODE_ID;
use log::debug;
use ploke_core::{NodeId, TypeKind};// Removed IdConversionError import
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::Display;

pub mod test_ids {
    use ploke_core::NodeId;

    use super::*;

    pub trait TestIds: TypedId {
        fn base_tid(&self) -> NodeId;
        fn new_test(id: NodeId) -> Self;
    }


    macro_rules! make_id_testable{
        ($SpecificId:ty) => {
            impl TestIds for $SpecificId {
                #[inline]
                fn base_tid(&self) -> NodeId {
                    self.base_id()
                }
                #[inline]
                fn new_test(id: NodeId) -> Self {
                    Self(id)
                }
            }
        };
    }
make_id_testable!(FunctionNodeId);
make_id_testable!(StructNodeId);
make_id_testable!(EnumNodeId);
make_id_testable!(UnionNodeId);
make_id_testable!(TypeAliasNodeId);
make_id_testable!(TraitNodeId);
make_id_testable!(ImplNodeId);
make_id_testable!(ConstNodeId);
make_id_testable!(StaticNodeId);
make_id_testable!(MacroNodeId);
make_id_testable!(ImportNodeId);
make_id_testable!(ModuleNodeId);
// Associated Items
make_id_testable!(MethodNodeId);
// Secondary Nodes
make_id_testable!(FieldNodeId);
make_id_testable!(VariantNodeId);
make_id_testable!(ParamNodeId);
make_id_testable!(GenericParamNodeId);
// Other IDs
make_id_testable!(ReexportNodeId);
}

// ----- Traits -----

/// Allows retrieving the corresponding `GraphNode` trait object from a graph
/// using a specific typed ID.
///
/// This trait is implemented internally for each specific ID type that represents
/// a node directly stored and retrievable in the `GraphAccess` implementor.
pub(crate) trait TypedNodeIdGet: Copy + private_traits::Sealed {
    // Added Copy bound as IDs are Copy
    // Added Sealed bound to prevent external implementations
    fn get<'g, T: GraphNode>(&self, graph: &'g impl GraphAccess) -> Option<&'g T>;
}

/// Convenience trait to help be more explicit about converting into AnyNodeId.
/// Relies on `Into<AnyNodeId>` being implemented on the base type on a case by case basis.
pub trait AsAnyNodeId
where
    Self: AnyTypedId + Into<AnyNodeId> + Copy,
{
    fn as_any(self) -> AnyNodeId {
        self.into()
    }
}


impl<T> AsAnyNodeId for T where T: AnyTypedId + Into<AnyNodeId> {}

// TODO: Reach true certainty regarding scoping of `NodeId` generation by making this trait
// private.
// Can't keep this completely private, unfortunately. It is a reasonable compromise for now. Maybe
// I'll be able to figure this one out later. The goal would be to prevent all possibility of
// creating new node is within this private crate itself, but I'm not sure that is really possible.
// Perhaps if we used a `#[path}`... we'd need to do the same thing for the `nodes` directory, and
// have it be a sibling or perhaps child of `visitor.rs`. Worth considering. Not today.
pub(in crate::parser) trait GeneratesAnyNodeId {
    /// Helper to generate a synthetic NodeId using the current visitor state.
    /// Uses the last ID pushed onto `current_primary_defn_scope` as the parent scope ID.
    /// Accepts the calculated hash bytes of the effective CFG strings.
    fn generate_synthetic_node_id(
        &self,
        name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> AnyNodeId;
}
impl GeneratesAnyNodeId for VisitorState {
    fn generate_synthetic_node_id(
        &self,
        name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> AnyNodeId {
        // Get the last pushed scope ID as the parent, if available
        let primary_parent_scope_id = self
            .current_primary_defn_scope
            .last()
            .copied()
            .map(|p_id| p_id.base_id());

        debug!(target: LOG_TARGET_NODE_ID,
            "[Visitor generate_synthetic_node_id for '{}' ({:?})]",
            name, item_kind
        );
        debug!(target: LOG_TARGET_NODE_ID, "  crate_namespace: {}", self.crate_namespace);
        debug!(target: LOG_TARGET_NODE_ID, "  file_path: {:?}", self.current_file_path);
        debug!(target: LOG_TARGET_NODE_ID, "  relative_path: {:?}", self.current_module_path);
        debug!(target: LOG_TARGET_NODE_ID, "  item_name: {}", name);
        debug!(target: LOG_TARGET_NODE_ID, "  item_kind: {:?}", item_kind);
        debug!(target: LOG_TARGET_NODE_ID, "  primary_parent_scope_id: {:?}", primary_parent_scope_id);
        debug!(target: LOG_TARGET_NODE_ID, "  cfg_bytes: {:?}", cfg_bytes);

        let node_id = NodeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            &self.current_module_path, // Current module path acts as relative path context
            name,
            item_kind,
            primary_parent_scope_id, // Pass the parent scope ID from the stack
            cfg_bytes,               // Pass the provided CFG bytes
        );

        match item_kind {
            ItemKind::Function => FunctionNodeId(node_id).into(),
            ItemKind::Method => MethodNodeId(node_id).into(),
            ItemKind::Struct => StructNodeId(node_id).into(),
            ItemKind::Enum => EnumNodeId(node_id).into(),
            ItemKind::Union => UnionNodeId(node_id).into(),
            ItemKind::TypeAlias => TypeAliasNodeId(node_id).into(),
            ItemKind::Trait => TraitNodeId(node_id).into(),
            ItemKind::Impl => ImplNodeId(node_id).into(),
            ItemKind::Module => ModuleNodeId(node_id).into(),
            ItemKind::Field => FieldNodeId(node_id).into(),
            ItemKind::Variant => VariantNodeId(node_id).into(),
            ItemKind::GenericParam => GenericParamNodeId(node_id).into(),
            ItemKind::Const => ConstNodeId(node_id).into(),
            ItemKind::Static => StaticNodeId(node_id).into(),
            ItemKind::Macro => MacroNodeId(node_id).into(),
            ItemKind::Import => ImportNodeId(node_id).into(),
            // TODO: Decide what to do about handling ExternCrate. We kind of do want everything to
            // have a NodeId of some kind, and this will do for now, but we also want to
            // distinguish between an ExternCrate statement and something else... probably.
            ItemKind::ExternCrate => ImportNodeId(node_id).into(),
        }
    }
}

pub(in crate::parser) trait GenerateTypeId {
    /// Helper to generate a synthetic NodeId using the current visitor state.
    /// Uses the last ID pushed onto `current_primary_defn_scope` as the parent scope ID.
    /// Accepts the calculated hash bytes of the effective CFG strings.
    fn generate_type_id(&self, type_kind: &TypeKind, related_types: &[ TypeId ]) -> TypeId;
}
impl GenerateTypeId for VisitorState {
    fn generate_type_id(&self, type_kind: &TypeKind, related_types: &[TypeId]) -> TypeId {
        // 2. Get the current parent scope ID from the state.
        //    Assume it's always present because the root module ID is pushed first.
        let parent_scope_id = self.current_primary_defn_scope.iter()
            .copied()
            .map(|pid| pid.as_any())
            .chain(
                self.current_secondary_defn_scope.iter() 
                    .map(|sid| sid.as_any())
            )
            .chain(
                self.current_assoc_defn_scope.iter()
                    .map(|aid| aid.as_any())
            )
            .last().expect(
            "Invalid State: Visitor.self's current_primary_defn_scope should not be empty during type processing",
        );
        // 3. Generate the new Synthetic Type ID using structural info AND parent scope
        
        TypeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            type_kind,                       // Pass the determined TypeKind
            related_types,                   // Pass the determined related TypeIds
            Some(parent_scope_id.base_id()), // Pass the non-optional parent scope ID wrapped in Some
        )
    }
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
/// - `impl TryFrom<EnumName> for SpecificIdType` for each variant, using a specified error type.
/// - `impl Display for EnumName`.
///
/// # Usage
/// ```ignore
/// define_category_enum!(
///     #[doc = "Represents primary node IDs."] // Optional outer attributes
///     PrimaryNodeId, // Enum Name
///     TryFromPrimaryError, // Error type for TryFrom
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
///     TryFromAnyNodeError, // Error type for TryFrom
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
    ($(#[$outer:meta])* $EnumName:ident, $ErrorType:ty, $KindType:ty, [ $( ($Variant:ident, $IdType:ty, $ItemKindVal:expr) ),* $(,)? ] ) => {
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

        // --- Display Implementation ---
        impl std::fmt::Display for $EnumName {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match *self {
                    $(
                        // Include the variant name using stringify! and delegate formatting of the ID
                        $EnumName::$Variant(id) => write!(f,
                            "{}({})",
                            stringify!($Variant),
                            id,
                        ),
                    )*
                }
            }
        }

        impl From<$EnumName> for AnyNodeId {
            #[inline]
            fn from(id: $EnumName) -> Self {
                match id {
                    $(
                        $EnumName::$Variant(typed_id) => AnyNodeId::$Variant(typed_id),
                    )*
                }
            }
        }

        impl AnyTypedId for $EnumName {}

        impl TryFrom<AnyNodeId> for $EnumName {
            type Error = $ErrorType; // Use the provided error type
            fn try_from(value: AnyNodeId) -> Result<Self, Self::Error> {
                match value {
                    $(
                        AnyNodeId::$Variant(id) => Ok($EnumName::$Variant(id)),
                    )*
                    // Instantiate the error type using Default
                    _ => Err(<$ErrorType>::default()),
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


            // Implement TryFrom<$EnumName> for $IdType
            impl TryFrom<$EnumName> for $IdType {
                type Error = $ErrorType; // Use the provided error type
                fn try_from(value: $EnumName) -> Result<Self, Self::Error> {
                    match value {
                        $EnumName::$Variant(id) => Ok(id),
                        // Instantiate the error type using Default
                        _ => Err(<$ErrorType>::default()),
                    }
                }
            }
        
        )*
    };

    // Matcher for enums WITHOUT an associated ItemKind method (like AnyNodeId)
    ($(#[$outer:meta])* $EnumName:ident, $ErrorType:ty, [ $( ($Variant:ident, $IdType:ty) ),* $(,)? ] ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $EnumName {
            $(
                $Variant($IdType),
            )*
        };

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
        };
        // --- Display Implementation ---
        impl std::fmt::Display for $EnumName {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match *self {
                    $(
                        // Include the variant name using stringify! and delegate formatting of the ID
                        $EnumName::$Variant(id) => write!(f,
                            "{}({})",
                            stringify!($Variant),
                            id
                        ),
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

            // Implement TryFrom<$EnumName> for $IdType
            impl TryFrom<$EnumName> for $IdType {
                type Error = $ErrorType; // Use the provided error type
                fn try_from(value: $EnumName) -> Result<Self, Self::Error> {
                    match value {
                        $EnumName::$Variant(id) => Ok(id),
                        // Instantiate the error type using Default
                        _ => Err($ErrorType::default()),
                    }
                }
            }
        )*;
    };
}

///// Macro to implement the `TypedNodeIdGet` trait for a specific ID type.
/////
///// # Usage
///// ```ignore
///// // Implements TypedNodeIdGet for StructNodeId using graph.get_struct()
///// impl_typed_node_id_get!(StructNodeId, get_struct);
///// ```
// macro_rules! impl_typed_node_id_get {
//     ($IdType:ty, $GetterMethod:ident) => {
//         // Implement the private sealing trait first
//         impl private_traits::Sealed for $IdType {}
//         // Implement the getter trait
//         impl TypedNodeIdGet for $IdType {
//             #[inline]
//             fn get<'g>(&self, graph: &'g dyn GraphAccess) -> Option<&'g dyn GraphNode> {
//                 graph
//                     .$GetterMethod(*self)
//                     .map(|node| node as &dyn GraphNode)
//             }
//         }
//     };
// }

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
        // impl std::borrow::Borrow<NodeId> for $NewTypeId {
        //     #[inline]
        //     fn borrow(&self) -> &NodeId {
        //         &self.0
        //     }
        // }
        // //
        // impl AsRef<NodeId> for $NewTypeId {
        //     #[inline]
        //     fn as_ref(&self) -> &NodeId {
        //         &self.0
        //     }
        // }


        // Implement the base TypedId trait for all generated IDs
        // Ensure the TypedId trait is defined in this scope or accessible via path
        impl $crate::parser::nodes::ids::internal::AnyTypedId for $NewTypeId {}
        impl $crate::parser::nodes::ids::internal::TypedId for $NewTypeId {}

        // Implement specified marker traits
        // Ensure marker traits are defined in this scope or accessible via path
        $( $(impl $MarkerTrait for $NewTypeId {})* )?
    };
}

// Now use the *new* internal macro with markers
define_internal_node_id!(
    struct EnumNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct FunctionNodeId {
        markers: [PrimaryNodeIdTrait],
    }
); // For standalone functions
define_internal_node_id!(
    struct MethodNodeId {
        markers: [AssociatedItemNodeIdTrait],
    }
); // For associated functions/methods
define_internal_node_id!(
    struct ImplNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct ImportNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct ModuleNodeId {
        markers: [PrimaryNodeIdTrait],
    }
); // Use the macro now
define_internal_node_id!(
    struct StructNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct TraitNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);
define_internal_node_id!(struct TypeAliasNodeId { markers: [PrimaryNodeIdTrait, AssociatedItemNodeIdTrait] }); // Can be both primary and associated
define_internal_node_id!(
    struct UnionNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);
define_internal_node_id!(struct ConstNodeId { markers: [PrimaryNodeIdTrait, AssociatedItemNodeIdTrait] }); // Can be both primary and associated
define_internal_node_id!(
    struct StaticNodeId {
        markers: [PrimaryNodeIdTrait],
    }
); // Added
define_internal_node_id!(
    struct FieldNodeId {
        markers: [SecondaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct VariantNodeId {
        markers: [SecondaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct ParamNodeId {
        markers: [], // removed SecondaryNodeIdTrait since we are experimenting with not using
    // Nodeid for this
    }
); // For ParamData

// define_internal_node_id!(
//     struct ParamNodeId {
//         markers: [SecondaryNodeIdTrait],
//     }
// ); // For ParamData
define_internal_node_id!(
    struct GenericParamNodeId {
        markers: [SecondaryNodeIdTrait],
    }
);
define_internal_node_id!(
    struct MacroNodeId {
        markers: [PrimaryNodeIdTrait],
    }
);

// For more explicit differntiation within Phase 3 module tree processing
define_internal_node_id!(
    struct ReexportNodeId {
        markers: [],
    }
); // No specific category yet, just TypedId
   // --- Category ID Enums ---


use ploke_core::ItemKind; // Need ItemKind for kind() methods

/// Error type for failed TryFrom<PrimaryNodeId> conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub struct TryFromPrimaryError;

impl std::fmt::Display for TryFromPrimaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrimaryNodeId variant mismatch")
    }
}

impl Default for TryFromPrimaryError {
    fn default() -> Self {
        TryFromPrimaryError
    }
}

/// Error type for failed TryFrom<PrimaryNodeId> conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromSecondaryError;

impl std::fmt::Display for TryFromSecondaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecondaryNodeId variant mismatch")
    }
}
impl std::error::Error for TryFromSecondaryError {}

impl Default for TryFromSecondaryError {
    fn default() -> Self {
        TryFromSecondaryError
    }
}

/// Error type for failed TryFrom<AssociatedItemNodeId> conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromAssociatedItemError;

impl std::fmt::Display for TryFromAssociatedItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AssociatedItemNodeId variant mismatch")
    }
}
impl std::error::Error for TryFromAssociatedItemError {}

impl Default for TryFromAssociatedItemError {
    fn default() -> Self {
        TryFromAssociatedItemError
    }
}

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

// Trait for all typed ids and all typed id categories.
// This helps to ensure all of the following properties hold for all typed ids and categories of
// typed ids.
pub trait AnyTypedId:
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

// Marker traits for the categories of typed ids themselves
pub trait TypedId: AnyTypedId {} // Base trait for all typed IDs - Added common bounds
pub trait CategoricalTypedId: AnyTypedId {}

// --- Marker Traits ---
// Define the marker traits themselves here.
// Implementations are generated by the define_internal_node_id! macro.
// Marker traits for categories of typed ids
pub trait PrimaryNodeIdTrait: AnyTypedId + TryFrom<PrimaryNodeId> + Into<PrimaryNodeId> + Into<AnyNodeId> {
    fn to_pid(self) -> PrimaryNodeId {
        self.into()
    }
} // Marker for primary node IDs
pub trait AssociatedItemNodeIdTrait: AnyTypedId + TryFrom<AssociatedItemNodeId> {} // Marker for associated item IDs
pub trait SecondaryNodeIdTrait: AnyTypedId + TryFrom<SecondaryNodeId> {} // Marker for secondary node IDs (fields, params, etc.)

// impl<T> TryFrom<PrimaryNodeId> for T 
//     where T: PrimaryNodeIdTrait + PrimaryNodeId
// {
//
// }

// Add other category marker traits as needed

/// Private module for the sealing pattern. Prevents external crates or modules
/// from implementing traits intended only for internal ID types (like TypedNodeIdGet).
mod private_traits {
    /// The sealing trait. Cannot be named or implemented outside this module.
    pub(super) trait Sealed {}
}
// --- TypedNodeIdGet Implementations ---
// Primary Nodes
// impl_typed_node_id_get!(FunctionNodeId, get_function);
// impl_typed_node_id_get!(StructNodeId, get_struct);
// impl_typed_node_id_get!(EnumNodeId, get_enum);
// impl_typed_node_id_get!(UnionNodeId, get_union);
// impl_typed_node_id_get!(TypeAliasNodeId, get_type_alias);
// impl_typed_node_id_get!(TraitNodeId, get_trait);
// impl_typed_node_id_get!(ImplNodeId, get_impl);
// impl_typed_node_id_get!(ConstNodeId, get_const);
// impl_typed_node_id_get!(StaticNodeId, get_static);
// impl_typed_node_id_get!(MacroNodeId, get_macro);
// impl_typed_node_id_get!(ImportNodeId, get_import);
// impl_typed_node_id_get!(ModuleNodeId, get_module);

// Associated Items (Methods are retrieved via their parent Impl/Trait, not directly)
// Note: MethodNodeId does *not* get an impl, as there's no `graph.get_method(MethodNodeId)`

// Secondary Nodes (Fields, Variants, Params, Generics are part of their parent node, not directly retrieved)
// Note: FieldNodeId, VariantNodeId, ParamNodeId, GenericParamNodeId do *not* get impls.

// Other IDs
// Note: ReexportNodeId does *not* get an impl.

// --- TryFrom Implementations for PrimaryNodeId Variants ---
// These are now generated by the macro define_category_enum!
// Removed manual implementations

// --- Generated Category Enums ---

define_category_enum!(
    #[doc = "Represents the ID of any node type that can typically be defined directly within a module scope (primary items)."]
    PrimaryNodeId,
    TryFromPrimaryError, // Pass the specific error type
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
// Adding this for simplicity, since I think it should work to help us be generic over both the
// category and any elements of the category. Might break though.
impl PrimaryNodeIdTrait for PrimaryNodeId {}

define_category_enum!(
    #[doc = "Represents the ID of any node type that can be defined within a Primary Node and may define items within their owns scope, such as a struct's FieldNode and a Variant's FieldNode's"]
    SecondaryNodeId,
    TryFromSecondaryError, // Pass the specific error type
    ItemKind,
    [
        (Variant, VariantNodeId, ItemKind::Variant),
        (Field, FieldNodeId, ItemKind::Field),
        (GenericParam, GenericParamNodeId, ItemKind::GenericParam),
    ]
);

define_category_enum!(
    #[doc = "Represents the ID of any node type that can be an associated item within an `impl` or `trait` block."]
    AssociatedItemNodeId,
    TryFromAssociatedItemError, // Pass the specific error type
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
impl AnyTypedId for AnyNodeId {}

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

impl Display for AnyNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            // Primary Nodes
            AnyNodeId::Function(id) => write!(f, "AnyNodeId::Function({})", id),
            AnyNodeId::Struct(id) => write!(f, "AnyNodeId::Struct({})", id),
            AnyNodeId::Enum(id) => write!(f, "AnyNodeId::Enum({})", id),
            AnyNodeId::Union(id) => write!(f, "AnyNodeId::Union({})", id),
            AnyNodeId::TypeAlias(id) => write!(f, "AnyNodeId::TypeAlias({})", id),
            AnyNodeId::Trait(id) => write!(f, "AnyNodeId::Trait({})", id),
            AnyNodeId::Impl(id) => write!(f, "AnyNodeId::Impl({})", id),
            AnyNodeId::Const(id) => write!(f, "AnyNodeId::Const({})", id),
            AnyNodeId::Static(id) => write!(f, "AnyNodeId::Static({})", id),
            AnyNodeId::Macro(id) => write!(f, "AnyNodeId::Macro({})", id),
            AnyNodeId::Import(id) => write!(f, "AnyNodeId::Import({})", id),
            AnyNodeId::Module(id) => write!(f, "AnyNodeId::Module({})", id),
            // Associated Items
            AnyNodeId::Method(id) => write!(f, "AnyNodeId::Method({})", id),
            // Secondary Nodes
            AnyNodeId::Field(id) => write!(f, "AnyNodeId::Field({})", id),
            AnyNodeId::Variant(id) => write!(f, "AnyNodeId::Variant({})", id),
            AnyNodeId::Param(id) => write!(f, "AnyNodeId::Param({})", id),
            AnyNodeId::GenericParam(id) => write!(f, "AnyNodeId::GenericParam({})", id),
            // Other IDs
            AnyNodeId::Reexport(id) => write!(f, "AnyNodeId::Reexport({})", id),
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

// --- Error Type for AnyNodeId Conversion ---

/// Error type for failed TryFrom<AnyNodeId> conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnyNodeIdConversionError;

impl std::fmt::Display for AnyNodeIdConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnyNodeId variant mismatch during conversion")
    }
}
impl Error for AnyNodeIdConversionError {}

// --- TryFrom<AnyNodeId> Implementations for Specific IDs ---

macro_rules! impl_try_from_any_node_id {
    ($SpecificId:ty, $Variant:ident) => {
        impl TryFrom<AnyNodeId> for $SpecificId {
            type Error = AnyNodeIdConversionError; // Use the new error type
            #[inline]
            fn try_from(value: AnyNodeId) -> Result<Self, Self::Error> {
                match value {
                    AnyNodeId::$Variant(id) => Ok(id),
                    _ => Err(AnyNodeIdConversionError), // Return the new error type
                }
            }
        }
    };
}

// Primary Nodes
impl_try_from_any_node_id!(FunctionNodeId, Function);
impl_try_from_any_node_id!(StructNodeId, Struct);
impl_try_from_any_node_id!(EnumNodeId, Enum);
impl_try_from_any_node_id!(UnionNodeId, Union);
impl_try_from_any_node_id!(TypeAliasNodeId, TypeAlias);
impl_try_from_any_node_id!(TraitNodeId, Trait);
impl_try_from_any_node_id!(ImplNodeId, Impl);
impl_try_from_any_node_id!(ConstNodeId, Const);
impl_try_from_any_node_id!(StaticNodeId, Static);
impl_try_from_any_node_id!(MacroNodeId, Macro);
impl_try_from_any_node_id!(ImportNodeId, Import);
impl_try_from_any_node_id!(ModuleNodeId, Module);
// Associated Items
impl_try_from_any_node_id!(MethodNodeId, Method);
// Secondary Nodes
impl_try_from_any_node_id!(FieldNodeId, Field);
impl_try_from_any_node_id!(VariantNodeId, Variant);
impl_try_from_any_node_id!(ParamNodeId, Param);
impl_try_from_any_node_id!(GenericParamNodeId, GenericParam);
// Other IDs
impl_try_from_any_node_id!(ReexportNodeId, Reexport);
