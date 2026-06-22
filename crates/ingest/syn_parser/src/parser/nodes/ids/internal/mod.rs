#![allow(dead_code, reason = "macros make dead_code warnings the worst")]

//! Private implementation module for strictly encapsulated typed node identifiers.
//!
//! This module defines the primitive vertex sets used by the code graph. A raw
//! `NodeId` names an element in the node-id universe, but code outside this
//! layer should work with refined typed IDs or finite unions of those typed IDs.
//! Access to the base `NodeId` is confined to this module so callers cannot
//! silently construct graph facts with endpoints whose admissibility has not
//! been proven.
//!
//! The design rule is set-theoretic:
//!
//! ```text
//! V_node = the universe of node vertices
//!
//! FunctionNodeId ⊆ V_node
//! StructNodeId   ⊆ V_node
//! ModuleNodeId   ⊆ V_node
//!
//! PrimaryNodeId =
//!     FunctionNodeId
//!   ∪ StructNodeId
//!   ∪ EnumNodeId
//!   ∪ ...
//! ```
//!
//! Specific ID newtypes such as `FunctionNodeId` and `StructNodeId` prove
//! membership in one concrete subset of `V_node`. Category enums such as
//! `PrimaryNodeId`, `SecondaryNodeId`, `AssociatedItemNodeId`, and `AnyNodeId`
//! name finite unions of those subsets. They are endpoint sets, not loose
//! classifiers.
//!
//! The major node-id endpoint sets encoded here are:
//!
//! ```text
//! PrimaryNodeId =
//!     FunctionNodeId
//!   ∪ StructNodeId
//!   ∪ EnumNodeId
//!   ∪ UnionNodeId
//!   ∪ TypeAliasNodeId
//!   ∪ TraitNodeId
//!   ∪ ImplNodeId
//!   ∪ ConstNodeId
//!   ∪ StaticNodeId
//!   ∪ MacroNodeId
//!   ∪ ImportNodeId
//!   ∪ ModuleNodeId
//!   ∪ UnresolvedNodeId
//!
//! SecondaryNodeId =
//!     VariantNodeId
//!   ∪ FieldNodeId
//!   ∪ GenericParamNodeId
//!
//! AssociatedItemNodeId =
//!     MethodNodeId
//!   ∪ TypeAliasNodeId
//!   ∪ ConstNodeId
//!
//! GenericParamOwnerId =
//!     FunctionNodeId
//!   ∪ StructNodeId
//!   ∪ EnumNodeId
//!   ∪ UnionNodeId
//!   ∪ TraitNodeId
//!   ∪ ImplNodeId
//!   ∪ MethodNodeId
//!   ∪ TypeAliasNodeId
//!
//! TypeUseOwnerId =
//!     FunctionNodeId
//!   ∪ MethodNodeId
//!   ∪ FieldNodeId
//!   ∪ TypeAliasNodeId
//!   ∪ TraitNodeId
//!   ∪ ImplNodeId
//!   ∪ ConstNodeId
//!   ∪ StaticNodeId
//!   ∪ GenericParamNodeId
//!
//! SelfScopeOwnerId =
//!     StructNodeId
//!   ∪ EnumNodeId
//!   ∪ UnionNodeId
//!   ∪ TraitNodeId
//!   ∪ ImplNodeId
//!
//! AssociatedItemOwnerId = TraitNodeId ∪ ImplNodeId
//! ```
//!
//! Generic parameters have an additional refinement layer. A
//! `GenericParamNodeId` proves only that a generic parameter exists; the
//! refined wrappers prove which Rust generic-parameter kind it has:
//!
//! ```text
//! TypeGenericParamNodeId     ⊆ GenericParamNodeId
//! LifetimeGenericParamNodeId ⊆ GenericParamNodeId
//! ConstGenericParamNodeId    ⊆ GenericParamNodeId
//!
//! AnyGenericParamId =
//!     TypeGenericParamNodeId
//!   ∪ LifetimeGenericParamNodeId
//!   ∪ ConstGenericParamNodeId
//! ```
//!
//! These three refined generic-parameter subsets are intended to be pairwise
//! disjoint because a parsed generic parameter has exactly one
//! `GenericParamKind`:
//!
//! ```text
//! TypeGenericParamNodeId     ∩ LifetimeGenericParamNodeId = ∅
//! TypeGenericParamNodeId     ∩ ConstGenericParamNodeId    = ∅
//! LifetimeGenericParamNodeId ∩ ConstGenericParamNodeId    = ∅
//! ```
//!
//! Other category intersections are intentional and meaningful. Category enums
//! are finite unions, not a global partition:
//!
//! ```text
//! PrimaryNodeId ∩ AssociatedItemNodeId = TypeAliasNodeId ∪ ConstNodeId
//! AssociatedItemOwnerId ⊆ PrimaryNodeId
//! SelfScopeOwnerId      ⊆ PrimaryNodeId
//! ```
//!
//! Subset arrows are encoded as Rust conversion arrows when the value already
//! carries the membership proof. Widening along an inclusion is total and maps
//! to `From`:
//!
//! ```text
//! A ⊆ B
//! A → B
//! impl From<A> for B
//! ```
//!
//! Reverse movement across a subset boundary is partial. It maps to `TryFrom`
//! only when the source value carries enough evidence to prove the narrower
//! membership:
//!
//! ```text
//! B ⇀ A
//! impl TryFrom<B> for A
//! ```
//!
//! When the proof lives in another payload table, the reverse arrow is not a
//! plain conversion from the ID alone. For example, `GenericParamNodeId` does
//! not prove whether the parameter is type, lifetime, or const; the refinement
//! needs the `GenericParamKind` payload:
//!
//! ```text
//! (GenericParamNodeId × GenericParamKind)
//!     → Result<TypeGenericParamNodeId, GenericParamIdRefinementError>
//! ```
//!
//! In code, that is represented by `TypeGenericParamNodeId::try_refine(id,
//! &node.kind)`, not by an unchecked narrowing conversion from
//! `GenericParamNodeId`.
//!
//! A relation variant then denotes an admissible subset of a Cartesian product:
//!
//! ```text
//! Contains      ⊆ ModuleNodeId            × PrimaryNodeId
//! StructField   ⊆ StructNodeId            × FieldNodeId
//! UnionField    ⊆ UnionNodeId             × FieldNodeId
//! DeclaresParam ⊆ GenericParamOwnerId     × AnyGenericParamId
//! TypeBound     ⊆ TypeGenericParamNodeId  × TraitTypeSourceId
//! TypeDefault   ⊆ TypeGenericParamNodeId  × OrdinaryTypeUseId
//! ConstParamType ⊆ ConstGenericParamNodeId × OrdinaryTypeUseId
//! ```
//!
//! In Rust, the endpoint fields encode that product directly:
//!
//! ```ignore
//! Contains {
//!     source: ModuleNodeId,
//!     target: PrimaryNodeId,
//! }
//! ```
//!
//! A graph fact may be constructed only when its endpoints inhabit the sets
//! required by the relation variant. If a raw ID cannot be refined into the
//! required typed ID or category enum, that failure belongs at the construction
//! boundary as an error, unresolved state, or diagnostic. It should not be
//! smuggled into the inner graph as a broad ID plus a later runtime check.
//!
//! The same pattern applies outside the node universe. For example, refined
//! structural type IDs such as `NamedTypeId` prove membership in subsets of the
//! type-id universe, and type-resolution endpoint families should be modeled as
//! finite unions in the same style:
//!
//! ```text
//! OrdinaryTypeRelation ⊆ OrdinaryTypeSourceId × OrdinaryTypeTargetId
//! TraitTypeRelation    ⊆ TraitTypeSourceId    × TraitTypeTargetId
//! ```
//!
//! This is the foundation for correct-by-construction graph primitives:
//! typed IDs define valid vertices, category enums define admissible endpoint
//! sets, and relation variants define valid edges between those sets.

mod call_ids;
mod type_families;
mod type_ids;

use crate::parser::visitor::VisitorState;

// We will move ID definitions, trait implementations, etc., here later.
use super::*;
use crate::parser::types::{GenericParamKind, GenericParamNode};
use crate::utils::{LogStyle, LogStyleDebug};
use cozo::{DataValue, UuidWrapper};
use log::debug;
use ploke_core::{IdTrait, NodeId, TypeKind};
use uuid::Uuid;
// Removed IdConversionError import
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::Display;

pub(in crate::parser) use call_ids::generate_method_call_site_id;
pub use call_ids::{
    AnyCallSiteId, CallBodyOwnerId, CallSiteKind, DynamicCallSiteId, MacroCallSiteId,
    MethodCallSiteId, PathCallSiteId,
};

pub use type_families::{
    AnyTypeId, OrdinaryTypeDefId, OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId,
    TraitTypeSourceId, TraitTypeTargetId, TryFromAnyTypeError, TryFromOrdinaryTypeDefError,
    TryFromOrdinaryTypeSourceError, TryFromOrdinaryTypeTargetError, TryFromOrdinaryTypeUseError,
    TryFromTraitTypeSourceError, TryFromTraitTypeTargetError, TryFromTypeSourceError, TypeSourceId,
};
pub(in crate::parser) use type_ids::StructuralTypeId;
pub use type_ids::{
    ArrayTypeId, FunctionTypeId, ImplTraitTypeId, InferredTypeId, MacroTypeId, NamedTypeId,
    NeverTypeId, ParenTypeId, RawPointerTypeId, ReferenceTypeId, SliceTypeId, TraitBoundTypeId,
    TraitObjectTypeId, TupleTypeId, TypeIdRefinementError, UnknownTypeId,
};

// NOTE: WIP, turn this into macro for, e.g. FunctionNodeId, StructNodeId, etc
// pub trait HasAnyNodeId {
//     fn any_id(&self) -> AnyNodeId;
// }
//
// impl HasAnyNodeId for FunctionNode {
//     fn any_id(&self) -> AnyNodeId {
//         self.id.as_any()
//     }
// }

// macro_rules! has_any_id {
//     ($node:path) => {
//         impl HasAnyNodeId for $node {
//             fn any_id(&self) -> AnyNodeId {
//                 self.id.as_any()
//             }
//         }
//     };
// }

// has_any_id!(StructNode);

pub trait ToUuidString: TypedId {
    fn to_uuid_string(&self) -> String;
}

impl ToUuidString for FunctionNodeId {
    fn to_uuid_string(&self) -> String {
        self.base_id().uuid().to_string()
    }
}
impl ToUuidString for ModuleNodeId {
    fn to_uuid_string(&self) -> String {
        self.base_id().uuid().to_string()
    }
}

pub trait ToCozoUuid {
    fn to_cozo_uuid(self) -> DataValue;
}

impl ToCozoUuid for AnyNodeId {
    fn to_cozo_uuid(self) -> DataValue {
        match self.base_id() {
            NodeId::Resolved(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
            NodeId::Synthetic(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
        }
    }
}

impl ToCozoUuid for PrimaryNodeId {
    fn to_cozo_uuid(self) -> DataValue {
        DataValue::Uuid(UuidWrapper(self.base_id().uuid()))
    }
}

impl ToCozoUuid for GenericParamNodeId {
    fn to_cozo_uuid(self) -> DataValue {
        DataValue::Uuid(UuidWrapper(self.base_id().uuid()))
    }
}

impl ToCozoUuid for TypeId {
    fn to_cozo_uuid(self) -> DataValue {
        DataValue::Uuid(UuidWrapper(self.uuid()))
    }
}

pub mod test_ids {
    use ploke_core::NodeId;

    use super::*;

    pub trait TestIds: TypedId {
        fn base_tid(&self) -> NodeId;
        fn new_test(id: NodeId) -> Self;
    }

    macro_rules! make_id_testable {
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

    /// Test-only accessors for deterministic call-site ID regeneration.
    ///
    /// Production code should not use this trait as a construction path. It is
    /// exported for integration tests that need to assert the parser and test
    /// fixture computed the same call-site identity from the same structural
    /// inputs.
    pub trait TestCallIds: Copy {
        /// Returns the underlying base call-site ID for exact test comparison.
        fn base_cid(&self) -> ploke_core::CallId;
        /// Constructs a typed call-site ID from a base [`ploke_core::CallId`]
        /// in tests only.
        fn new_call_test(id: ploke_core::CallId) -> Self;
    }

    macro_rules! make_call_id_testable {
        ($SpecificId:ty) => {
            impl TestCallIds for $SpecificId {
                #[inline]
                fn base_cid(&self) -> ploke_core::CallId {
                    use super::call_ids::CallSiteId as _;
                    self.base_id()
                }
                #[inline]
                fn new_call_test(id: ploke_core::CallId) -> Self {
                    Self::create(id)
                }
            }
        };
    }

    make_call_id_testable!(PathCallSiteId);
    make_call_id_testable!(MethodCallSiteId);
    make_call_id_testable!(DynamicCallSiteId);
    make_call_id_testable!(MacroCallSiteId);

    /// Deterministically regenerates a base call-site ID for integration tests.
    ///
    /// This mirrors parser-internal call-site ID generation without making the
    /// production typed-ID constructors public.
    pub fn generate_test_call_id(
        owner: CallBodyOwnerId,
        kind: CallSiteKind,
        discriminator: &str,
        span: (usize, usize),
        cfgs: &[String],
    ) -> ploke_core::CallId {
        super::call_ids::generate_call_id(owner, kind, discriminator, span, cfgs)
    }
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
    /// Uses the last active scope ID from the primary/associated/secondary scope stacks as the
    /// parent scope ID.
    /// Accepts the calculated hash bytes of the effective CFG strings.
    fn generate_synthetic_node_id(
        &self,
        name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> AnyNodeId;

    fn log_id_gen(
        &self,
        name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>,
        parent_scope_id: Option<NodeId>,
    ) {
        if let Ok(debug_target_item) = std::env::var("ID_REGEN_TARGET") {
            if log::log_enabled!(target: LOG_TEST_ID_REGEN, log::Level::Debug)
                && debug_target_item == name
            // allow for filtering by command env variable
            {
                // Check if specific log is enabled
                debug!(target: LOG_TEST_ID_REGEN, "{:=^60}", " VisitorState Id Generation ".log_header());
                debug!(target: LOG_TEST_ID_REGEN,
                    "  Inputs for '{}' ({}):\n    crate_namespace: {}\n    file_path: {}\n    relative_path: {}\n    item_name: {}\n    item_kind: {}\n    parent_scope_id: {}\n    cfg_bytes: {}\n",
                    name.log_name(), // item name being processed by visitor
                    item_kind.log_comment_debug(),
                    self.crate_namespace(),
                    &self.current_file_path().as_os_str().log_comment_debug(),
                    &self.current_module_path().log_path_debug(), // This is the 'relative_path' for the item's ID context
                    name.log_name(),
                    item_kind.log_comment_debug(),
                    parent_scope_id.log_id_debug(), // The actual parent_scope_id used by visitor
                    cfg_bytes.log_comment_debug() // The actual cfg_bytes used by visitor
                );
            }
        }
    }
    fn crate_namespace(&self) -> Uuid;
    fn current_file_path(&self) -> &std::path::Path;
    fn current_module_path(&self) -> &[String];
}

pub const LOG_TEST_ID_REGEN: &str = "test_id_regen";
impl GeneratesAnyNodeId for VisitorState {
    fn generate_synthetic_node_id(
        &self,
        name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> AnyNodeId {
        let parent_scope_id = self
            .current_primary_defn_scope
            .iter()
            .copied()
            .map(|pid| pid.as_any())
            .chain(self.current_assoc_defn_scope.iter().map(|aid| aid.as_any()))
            .chain(
                self.current_secondary_defn_scope
                    .iter()
                    .map(|sid| sid.as_any()),
            )
            .last()
            .map(|id| id.base_id());

        // MODIFIED CONDITION FOR LOGGING:
        self.log_id_gen(name, item_kind, cfg_bytes, parent_scope_id);

        let node_id = NodeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            &self.current_module_path, // Current module path acts as relative path context
            name,
            item_kind,
            parent_scope_id, // Pass the parent scope ID from the active scope chain
            cfg_bytes,       // Pass the provided CFG bytes
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
            ItemKind::Unresolved => {
                // UnresolvedNodeId is created directly without going through the normal
                // generate_synthetic_node_id path since we need special handling for unresolved items
                UnresolvedNodeId(node_id).into()
            }
        }
    }

    fn crate_namespace(&self) -> Uuid {
        self.crate_namespace
    }
    fn current_file_path(&self) -> &std::path::Path {
        &self.current_file_path
    }
    fn current_module_path(&self) -> &[String] {
        &self.current_module_path
    }
}

pub(in crate::parser) trait GenerateTypeId {
    /// Helper to generate a synthetic NodeId using the current visitor state.
    /// Uses the last ID pushed onto `current_primary_defn_scope` as the parent scope ID.
    /// Accepts the calculated hash bytes of the effective CFG strings.
    fn generate_type_id(&self, type_kind: &TypeKind, related_types: &[TypeId]) -> TypeId;
}
impl GenerateTypeId for VisitorState {
    /// Generates the `TypeId::Synthetic` for a parsed item. This `TypeId`, while not necessarily
    /// unique (for example `ExampleStruct(usize, usize)`), it should prevent false positives and
    /// false negatives. A false positive could arise in large part due to renaming of imported
    /// types, or through the use of type aliases. As such, it is necessary to use the `NodeId` of
    /// the parent context in the generation of the `TypeId` here.
    ///
    /// The parent_scope is included as a chain of all of the following:
    ///     - primary scope definition: the immediate primary node (e.g. impl, function, struct,
    ///     etc) parent's NodeId
    ///     - associated scope definition: the immediate associated node parent (if present), e.g. a method
    ///     - secondary scope definition: the immediate secondary node parent (if present), e.g. field
    fn generate_type_id(&self, type_kind: &TypeKind, related_types: &[TypeId]) -> TypeId {
        // 2. Get the current parent scope ID from the state.
        //    Assume it's always present because the root module ID is pushed first.
        let parent_scope_id = self.current_primary_defn_scope.iter()
            .copied()
            .map(|pid| pid.as_any())
            .chain(
                self.current_assoc_defn_scope.iter()
                    .map(|aid| aid.as_any())
            )
            .chain(
                self.current_secondary_defn_scope.iter()
                    .map(|sid| sid.as_any())
            )
            .last().expect(
            "Invalid State: Visitor.self's current_primary_defn_scope should not be empty during type processing",
        );
        // 3. Generate the new Synthetic Type ID using structural info AND parent scope

        TypeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            type_kind,                 // Pass the determined TypeKind
            related_types,             // Pass the determined related TypeIds
            parent_scope_id.base_id(), // NOTE: This used to be an Option for no good reason
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

            pub fn to_cozo_uuid(&self) -> DataValue {
                match self.base_id() {
                    NodeId::Resolved(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
                    NodeId::Synthetic(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
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

        #[allow(clippy::from_over_into)]
        impl Into<DataValue> for $NewTypeId {
            fn into(self) -> DataValue {
                match self.base_id() {
                    NodeId::Resolved(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
                    NodeId::Synthetic(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
                }
            }
        }
        #[allow(clippy::from_over_into)]
        impl Into<DataValue> for &$NewTypeId {
            fn into(self) -> DataValue {
                match self.base_id() {
                    NodeId::Resolved(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
                    NodeId::Synthetic(uuid) => DataValue::Uuid(UuidWrapper(uuid)),
                }
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
// ANCHOR: method_node_id_marker
define_internal_node_id!(
    struct MethodNodeId {
        markers: [AssociatedItemNodeIdTrait],
    }
); // For associated functions/methods
// ANCHOR_END: method_node_id_marker
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
define_internal_node_id!(
    struct UnresolvedNodeId {
        markers: [],
    }
); // For unresolved imports/re-exports - NOT a primary node

impl UnresolvedNodeId {
    /// Creates a new UnresolvedNodeId. This is crate-visible to allow
    /// creation during module tree resolution when an import cannot be resolved.
    #[inline]
    pub(crate) fn new(id: NodeId) -> Self {
        Self(id)
    }
}
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

/// Error type for failed `GenericParamOwnerId` conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromGenericParamOwnerError;

impl std::fmt::Display for TryFromGenericParamOwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GenericParamOwnerId variant mismatch")
    }
}
impl std::error::Error for TryFromGenericParamOwnerError {}

impl Default for TryFromGenericParamOwnerError {
    fn default() -> Self {
        TryFromGenericParamOwnerError
    }
}

/// Error type for failed `TypeUseOwnerId` conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromTypeUseOwnerError;

impl std::fmt::Display for TryFromTypeUseOwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeUseOwnerId variant mismatch")
    }
}
impl std::error::Error for TryFromTypeUseOwnerError {}

impl Default for TryFromTypeUseOwnerError {
    fn default() -> Self {
        TryFromTypeUseOwnerError
    }
}

/// Error type for failed `SelfScopeOwnerId` conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromSelfScopeOwnerError;

impl std::fmt::Display for TryFromSelfScopeOwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SelfScopeOwnerId variant mismatch")
    }
}
impl std::error::Error for TryFromSelfScopeOwnerError {}

impl Default for TryFromSelfScopeOwnerError {
    fn default() -> Self {
        TryFromSelfScopeOwnerError
    }
}

/// Error type for failed `AssociatedItemOwnerId` conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromAssociatedItemOwnerError;

impl std::fmt::Display for TryFromAssociatedItemOwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AssociatedItemOwnerId variant mismatch")
    }
}
impl std::error::Error for TryFromAssociatedItemOwnerError {}

impl Default for TryFromAssociatedItemOwnerError {
    fn default() -> Self {
        TryFromAssociatedItemOwnerError
    }
}

/// Error type for failed `AnyGenericParamId` conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryFromAnyGenericParamError;

impl std::fmt::Display for TryFromAnyGenericParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnyGenericParamId variant mismatch")
    }
}
impl std::error::Error for TryFromAnyGenericParamError {}

impl Default for TryFromAnyGenericParamError {
    fn default() -> Self {
        TryFromAnyGenericParamError
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
pub trait PrimaryNodeIdTrait:
    AnyTypedId + TryFrom<PrimaryNodeId> + Into<PrimaryNodeId> + Into<AnyNodeId>
{
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
        (Unresolved, UnresolvedNodeId, ItemKind::Unresolved),
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

define_category_enum!(
    #[doc = "Represents the ID of any node type that may declare generic parameters."]
    GenericParamOwnerId,
    TryFromGenericParamOwnerError,
    ItemKind,
    [
        (Function, FunctionNodeId, ItemKind::Function),
        (Struct, StructNodeId, ItemKind::Struct),
        (Enum, EnumNodeId, ItemKind::Enum),
        (Union, UnionNodeId, ItemKind::Union),
        (Trait, TraitNodeId, ItemKind::Trait),
        (Impl, ImplNodeId, ItemKind::Impl),
        (Method, MethodNodeId, ItemKind::Method),
        (TypeAlias, TypeAliasNodeId, ItemKind::TypeAlias),
    ]
);

define_category_enum!(
    #[doc = "Represents node IDs whose payload may contain direct structural `TypeId` use-sites."]
    TypeUseOwnerId,
    TryFromTypeUseOwnerError,
    ItemKind,
    [
        (Function, FunctionNodeId, ItemKind::Function),
        (Method, MethodNodeId, ItemKind::Method),
        (Field, FieldNodeId, ItemKind::Field),
        (TypeAlias, TypeAliasNodeId, ItemKind::TypeAlias),
        (Trait, TraitNodeId, ItemKind::Trait),
        (Impl, ImplNodeId, ItemKind::Impl),
        (Const, ConstNodeId, ItemKind::Const),
        (Static, StaticNodeId, ItemKind::Static),
        (GenericParam, GenericParamNodeId, ItemKind::GenericParam),
    ]
);

define_category_enum!(
    #[doc = "Represents node IDs that introduce a Rust `Self` type scope."]
    SelfScopeOwnerId,
    TryFromSelfScopeOwnerError,
    ItemKind,
    [
        (Struct, StructNodeId, ItemKind::Struct),
        (Enum, EnumNodeId, ItemKind::Enum),
        (Union, UnionNodeId, ItemKind::Union),
        (Trait, TraitNodeId, ItemKind::Trait),
        (Impl, ImplNodeId, ItemKind::Impl),
    ]
);

define_category_enum!(
    #[doc = "Represents node IDs that may contain associated items."]
    AssociatedItemOwnerId,
    TryFromAssociatedItemOwnerError,
    ItemKind,
    [
        (Trait, TraitNodeId, ItemKind::Trait),
        (Impl, ImplNodeId, ItemKind::Impl),
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
    // Unresolved nodes (for items that couldn't be resolved, e.g., from include! macros)
    Unresolved(UnresolvedNodeId),
    // Add any other specific ID types here as they are created
}
impl AnyTypedId for AnyNodeId {}

impl AnyNodeId {
    /// Returns the stable UUID backing this node id.
    #[inline]
    pub fn uuid(self) -> Uuid {
        self.base_id().uuid()
    }

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
            // Unresolved nodes
            AnyNodeId::Unresolved(id) => id.base_id(),
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
            // Unresolved nodes
            AnyNodeId::Unresolved(id) => write!(f, "AnyNodeId::Unresolved({})", id),
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
// Unresolved nodes
impl From<UnresolvedNodeId> for AnyNodeId {
    #[inline]
    fn from(id: UnresolvedNodeId) -> Self {
        AnyNodeId::Unresolved(id)
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

fn generic_param_kind_name(kind: &GenericParamKind) -> &'static str {
    match kind {
        GenericParamKind::Type { .. } => "Type",
        GenericParamKind::Lifetime { .. } => "Lifetime",
        GenericParamKind::Const { .. } => "Const",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "cannot refine GenericParamNodeId {generic_param_id} into {expected}; actual GenericParamKind is {found}"
)]
pub struct GenericParamIdRefinementError {
    expected: &'static str,
    found: &'static str,
    generic_param_id: GenericParamNodeId,
}

impl GenericParamIdRefinementError {
    fn new(
        expected: &'static str,
        found: &'static str,
        generic_param_id: GenericParamNodeId,
    ) -> Self {
        Self {
            expected,
            found,
            generic_param_id,
        }
    }
}

macro_rules! define_generic_param_id {
    ($Name:ident, $Variant:ident { .. }) => {
        #[doc = concat!("Refined `GenericParamNodeId` for `GenericParamKind::", stringify!($Variant), "`.")]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
        )]
        pub struct $Name(GenericParamNodeId);

        impl $Name {
            #[inline]
            pub fn matches(kind: &GenericParamKind) -> bool {
                matches!(kind, GenericParamKind::$Variant { .. })
            }

            #[inline]
            pub fn try_refine(
                id: GenericParamNodeId,
                kind: &GenericParamKind,
            ) -> Result<Self, GenericParamIdRefinementError> {
                if Self::matches(kind) {
                    Ok(Self(id))
                } else {
                    Err(GenericParamIdRefinementError::new(
                        stringify!($Name),
                        generic_param_kind_name(kind),
                        id,
                    ))
                }
            }

            #[inline]
            pub fn base_id(self) -> ploke_core::NodeId {
                self.0.base_id()
            }

            #[inline]
            pub fn generic_param_id(self) -> GenericParamNodeId {
                self.0
            }
        }

        impl AnyTypedId for $Name {}
        impl TypedId for $Name {}

        impl From<$Name> for GenericParamNodeId {
            #[inline]
            fn from(id: $Name) -> Self {
                id.0
            }
        }

        impl From<$Name> for AnyNodeId {
            #[inline]
            fn from(id: $Name) -> Self {
                id.0.into()
            }
        }

        impl TryFrom<&GenericParamNode> for $Name {
            type Error = GenericParamIdRefinementError;

            #[inline]
            fn try_from(node: &GenericParamNode) -> Result<Self, Self::Error> {
                Self::try_refine(node.id, &node.kind)
            }
        }

        impl std::fmt::Display for $Name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($Name), self.0)
            }
        }
    };
}

define_generic_param_id!(TypeGenericParamNodeId, Type { .. });
define_generic_param_id!(LifetimeGenericParamNodeId, Lifetime { .. });
define_generic_param_id!(ConstGenericParamNodeId, Const { .. });

macro_rules! define_generic_param_family {
    (
        $(#[$outer:meta])*
        $Family:ident,
        $Error:ty,
        [
            ($FirstVariant:ident, $FirstIdType:ty),
            $(($Variant:ident, $IdType:ty)),+ $(,)?
        ]
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $Family {
            $FirstVariant($FirstIdType),
            $(
                $Variant($IdType),
            )+
        }

        impl $Family {
            #[inline]
            pub fn generic_param_id(self) -> GenericParamNodeId {
                match self {
                    Self::$FirstVariant(id) => id.into(),
                    $(
                        Self::$Variant(id) => id.into(),
                    )+
                }
            }

            #[inline]
            pub fn base_id(self) -> ploke_core::NodeId {
                self.generic_param_id().base_id()
            }

            #[inline]
            pub fn kind_name(&self) -> &'static str {
                match self {
                    Self::$FirstVariant(_) => stringify!($FirstIdType),
                    $(
                        Self::$Variant(_) => stringify!($IdType),
                    )+
                }
            }
        }

        impl AnyTypedId for $Family {}
        impl CategoricalTypedId for $Family {}

        impl From<$Family> for GenericParamNodeId {
            #[inline]
            fn from(id: $Family) -> Self {
                id.generic_param_id()
            }
        }

        impl From<$Family> for AnyNodeId {
            #[inline]
            fn from(id: $Family) -> Self {
                id.generic_param_id().into()
            }
        }

        impl From<$FirstIdType> for $Family {
            #[inline]
            fn from(id: $FirstIdType) -> Self {
                Self::$FirstVariant(id)
            }
        }

        impl TryFrom<$Family> for $FirstIdType {
            type Error = $Error;

            #[inline]
            fn try_from(value: $Family) -> Result<Self, Self::Error> {
                match value {
                    $Family::$FirstVariant(id) => Ok(id),
                    _ => Err(<$Error>::default()),
                }
            }
        }

        $(
            impl From<$IdType> for $Family {
                #[inline]
                fn from(id: $IdType) -> Self {
                    Self::$Variant(id)
                }
            }

            impl TryFrom<$Family> for $IdType {
                type Error = $Error;

                #[inline]
                fn try_from(value: $Family) -> Result<Self, Self::Error> {
                    match value {
                        $Family::$Variant(id) => Ok(id),
                        _ => Err(<$Error>::default()),
                    }
                }
            }
        )+

        impl std::fmt::Display for $Family {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match *self {
                    Self::$FirstVariant(id) => write!(f, "{}({})", stringify!($FirstVariant), id),
                    $(
                        Self::$Variant(id) => write!(f, "{}({})", stringify!($Variant), id),
                    )+
                }
            }
        }
    };
}

define_generic_param_family!(
    /// Represents any refined generic parameter ID.
    ///
    /// Set-theoretically:
    ///
    /// ```text
    /// AnyGenericParamId =
    ///     TypeGenericParamNodeId
    ///   ∪ LifetimeGenericParamNodeId
    ///   ∪ ConstGenericParamNodeId
    /// ```
    AnyGenericParamId,
    TryFromAnyGenericParamError,
    [
        (Type, TypeGenericParamNodeId),
        (Lifetime, LifetimeGenericParamNodeId),
        (Const, ConstGenericParamNodeId),
    ]
);

#[cfg(test)]
mod generic_param_id_tests {
    use super::*;

    fn synthetic_generic_param_id() -> GenericParamNodeId {
        GenericParamNodeId::create(NodeId::Synthetic(Uuid::new_v4()))
    }

    #[test]
    fn any_generic_param_round_trips_refined_generic_ids() {
        let type_param = TypeGenericParamNodeId::try_refine(
            synthetic_generic_param_id(),
            &GenericParamKind::Type {
                name: "T".into(),
                bounds: Vec::new(),
                default: None,
            },
        )
        .expect("type generic should refine");
        let lifetime_param = LifetimeGenericParamNodeId::try_refine(
            synthetic_generic_param_id(),
            &GenericParamKind::Lifetime {
                name: "a".into(),
                bounds: Vec::new(),
            },
        )
        .expect("lifetime generic should refine");

        let type_any = AnyGenericParamId::from(type_param);
        let lifetime_any = AnyGenericParamId::from(lifetime_param);

        assert_eq!(
            TypeGenericParamNodeId::try_from(type_any).expect("type param"),
            type_param
        );
        assert_eq!(
            LifetimeGenericParamNodeId::try_from(lifetime_any).expect("lifetime param"),
            lifetime_param
        );
        assert!(LifetimeGenericParamNodeId::try_from(type_any).is_err());
    }
}

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
// Unresolved nodes
impl_try_from_any_node_id!(UnresolvedNodeId, Unresolved);
