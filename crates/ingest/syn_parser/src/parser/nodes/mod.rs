mod enums;
mod function;
mod impls;
mod import;
mod macros;
mod module;
mod structs;
mod traits;
mod type_alias;
mod union;
mod value;

use std::borrow::Borrow;
use std::fmt::Display;

use crate::{
    error::SynParserError,
    utils::{LogStyle, LogStyleDebug},
};

use super::{graph::GraphAccess, types::VisibilityKind};
use log::debug;
use ploke_core::{NodeId, TypeId};
use serde::{Deserialize, Serialize};

// Re-export all node types from submodules
pub use enums::{EnumNode, VariantNode};
pub use function::{FunctionNode, MethodNode, ParamData}; // Added MethodNode
pub use impls::ImplNode;
pub use import::{ImportKind, ImportNode};
pub use macros::{MacroKind, MacroNode, MacroRuleNode, ProcMacroKind};
pub use module::{ModuleKind, ModuleNode, ModuleNodeId};
pub use structs::{FieldNode, StructNode};
pub use traits::TraitNode;
pub use type_alias::TypeAliasNode;
pub use union::UnionNode;
pub use value::{ConstNode, StaticNode};

// Re-export the generated *NodeInfo structs for internal use within the crate
pub(crate) use enums::{EnumNodeInfo, VariantNodeInfo};
pub(crate) use function::{FunctionNodeInfo, MethodNodeInfo};
pub(crate) use impls::ImplNodeInfo;
pub(crate) use import::ImportNodeInfo;
pub(crate) use macros::MacroNodeInfo;
pub(crate) use module::ModuleNodeInfo;
pub(crate) use structs::{FieldNodeInfo, StructNodeInfo};
pub(crate) use traits::TraitNodeInfo;
pub(crate) use type_alias::TypeAliasNodeInfo;
pub(crate) use union::UnionNodeInfo;
pub(crate) use value::{ConstNodeInfo, StaticNodeInfo};

// ----- utility macro -----
// Differentiators for primary id types
// We don't actualy use these anywhere yet. You can see that `ModuleNodeId` is commented out
// because there is a separate implementation of that. We started using ModuleNodeId during the
// creation of the ModuleTree.
use crate::define_node_id_wrapper;
define_node_id_wrapper!(EnumNodeId);
define_node_id_wrapper!(FunctionNodeId); // For standalone functions
define_node_id_wrapper!(MethodNodeId); // For associated functions/methods
define_node_id_wrapper!(ImplNodeId);
define_node_id_wrapper!(ImportNodeId);
// define_node_id_wrapper!(ModuleNodeId); // Keep commented out if manual impl exists
define_node_id_wrapper!(StructNodeId);
define_node_id_wrapper!(TraitNodeId);
define_node_id_wrapper!(TypeAliasNodeId);
define_node_id_wrapper!(UnionNodeId);
// Removed ValueNodeId
define_node_id_wrapper!(ConstNodeId); // Added
define_node_id_wrapper!(StaticNodeId); // Added
define_node_id_wrapper!(FieldNodeId);
define_node_id_wrapper!(VariantNodeId);
define_node_id_wrapper!(ParamNodeId); // For ParamData
define_node_id_wrapper!(GenericParamNodeId);
define_node_id_wrapper!(MacroNodeId);

// For more explicit differntiation within Phase 3 module tree processing
define_node_id_wrapper!(ReexportNodeId);

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

// AI:

// Maybe seal it for better control
mod private {
    pub trait Sealed {}
}

pub trait TypedNodeIdGet: private::Sealed + Copy + Into<NodeId> {
    // Bounds needed by impls
    type TargetNode: PrimaryNodeMarker + GraphNode + 'static; // The associated node type

    /// Method to perform the lookup, dispatched statically based on `Self` (the ID type)       
    fn get_node<'a>(self, graph: &'a impl GraphAccess) -> Option<&'a Self::TargetNode>;
}
impl private::Sealed for FunctionNodeId {}
impl TypedNodeIdGet for FunctionNodeId {
    type TargetNode = FunctionNode;

    fn get_node<'a>(self, graph: &'a impl GraphAccess) -> Option<&'a Self::TargetNode> {
        graph.get_function(self) // Calls the specific getter
    }
}

impl private::Sealed for StructNodeId {}
impl TypedNodeIdGet for StructNodeId {
    type TargetNode = StructNode;

    fn get_node<'a>(self, graph: &'a impl GraphAccess) -> Option<&'a Self::TargetNode> {
        graph.get_struct(self) // Calls the specific getter
    }
}
// The above doesn't work because we are trying to keep conversions between the node types like
// ModuleNodeId or FunctionNodeId into NodeId private as far as is possible to leverage
// compile-time guarentees wherever possible. Is there some way we can refine this approach, or
// perhaps locate the implementation in a certain context where the privacy can be managed without
// loosening our desired restrictions on conversions between the primary node ids and NodeId AI?

/// Represents the ID of any node type that can typically be defined directly
/// within a module scope (primary items).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum PrimaryNodeId {
    Function(FunctionNodeId),
    Struct(StructNodeId),
    Enum(EnumNodeId),
    Union(UnionNodeId),
    TypeAlias(TypeAliasNodeId),
    Trait(TraitNodeId),
    Impl(ImplNodeId),
    Const(ConstNodeId),   // Changed from Value
    Static(StaticNodeId), // Added
    Macro(MacroNodeId),
    Import(ImportNodeId),
    Module(ModuleNodeId),
}

impl PrimaryNodeId {
    /// Returns the underlying base NodeId.
    pub fn base_id(&self) -> NodeId {
        match *self {
            PrimaryNodeId::Function(id) => id.into_inner(),
            PrimaryNodeId::Struct(id) => id.into_inner(),
            PrimaryNodeId::Enum(id) => id.into_inner(),
            PrimaryNodeId::Union(id) => id.into_inner(),
            PrimaryNodeId::TypeAlias(id) => id.into_inner(),
            PrimaryNodeId::Trait(id) => id.into_inner(),
            PrimaryNodeId::Impl(id) => id.into_inner(),
            PrimaryNodeId::Const(id) => id.into_inner(), // Changed from Value
            PrimaryNodeId::Static(id) => id.into_inner(), // Added
            PrimaryNodeId::Macro(id) => id.into_inner(),
            PrimaryNodeId::Import(id) => id.into_inner(),
            PrimaryNodeId::Module(id) => id.into_inner(),
        }
    }

    // Optional: Get the ItemKind directly
    pub fn kind(&self) -> ItemKind {
        match self {
            PrimaryNodeId::Function(_) => ItemKind::Function,
            PrimaryNodeId::Struct(_) => ItemKind::Struct,
            PrimaryNodeId::Enum(_) => ItemKind::Enum,
            PrimaryNodeId::Union(_) => ItemKind::Union,
            PrimaryNodeId::TypeAlias(_) => ItemKind::TypeAlias,
            PrimaryNodeId::Trait(_) => ItemKind::Trait,
            PrimaryNodeId::Impl(_) => ItemKind::Impl,
            PrimaryNodeId::Const(_) => ItemKind::Const, // Changed from Value
            PrimaryNodeId::Static(_) => ItemKind::Static, // Added
            PrimaryNodeId::Macro(_) => ItemKind::Macro,
            PrimaryNodeId::Import(_) => ItemKind::Import,
            PrimaryNodeId::Module(_) => ItemKind::Module,
        }
    }
}

/// Represents the ID of any node type that can be an associated item
/// within an `impl` or `trait` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum AssociatedItemId {
    Method(MethodNodeId),       // Associated function/method (changed from Function)
    TypeAlias(TypeAliasNodeId), // Associated type
    Const(ConstNodeId),         // Associated const
}

impl AssociatedItemId {
    /// Returns the underlying base NodeId.
    pub fn base_id(&self) -> NodeId {
        match *self {
            AssociatedItemId::Method(id) => id.into_inner(), // Changed from Function
            AssociatedItemId::TypeAlias(id) => id.into_inner(),
            AssociatedItemId::Const(id) => id.into_inner(),
        }
    }

    // Optional: Get the ItemKind directly
    pub fn kind(&self) -> ItemKind {
        match self {
            AssociatedItemId::Method(_) => ItemKind::Method, // Changed from Function
            AssociatedItemId::TypeAlias(_) => ItemKind::TypeAlias,
            AssociatedItemId::Const(_) => ItemKind::Const,
        }
    }
}

// --- From Implementations for Category Enums ---

impl From<FunctionNodeId> for PrimaryNodeId {
    // Standalone Function
    fn from(id: FunctionNodeId) -> Self {
        PrimaryNodeId::Function(id)
    }
}
impl From<StructNodeId> for PrimaryNodeId {
    fn from(id: StructNodeId) -> Self {
        PrimaryNodeId::Struct(id)
    }
}
impl From<EnumNodeId> for PrimaryNodeId {
    fn from(id: EnumNodeId) -> Self {
        PrimaryNodeId::Enum(id)
    }
}
impl From<UnionNodeId> for PrimaryNodeId {
    fn from(id: UnionNodeId) -> Self {
        PrimaryNodeId::Union(id)
    }
}
impl From<TypeAliasNodeId> for PrimaryNodeId {
    fn from(id: TypeAliasNodeId) -> Self {
        PrimaryNodeId::TypeAlias(id)
    }
}
impl From<TraitNodeId> for PrimaryNodeId {
    fn from(id: TraitNodeId) -> Self {
        PrimaryNodeId::Trait(id)
    }
}
impl From<ImplNodeId> for PrimaryNodeId {
    fn from(id: ImplNodeId) -> Self {
        PrimaryNodeId::Impl(id)
    }
}
// Removed From<ValueNodeId>
impl From<ConstNodeId> for PrimaryNodeId {
    fn from(id: ConstNodeId) -> Self {
        PrimaryNodeId::Const(id)
    }
} // Added
impl From<StaticNodeId> for PrimaryNodeId {
    fn from(id: StaticNodeId) -> Self {
        PrimaryNodeId::Static(id)
    }
} // Added
impl From<MacroNodeId> for PrimaryNodeId {
    fn from(id: MacroNodeId) -> Self {
        PrimaryNodeId::Macro(id)
    }
}
impl From<ImportNodeId> for PrimaryNodeId {
    fn from(id: ImportNodeId) -> Self {
        PrimaryNodeId::Import(id)
    }
}
impl From<ModuleNodeId> for PrimaryNodeId {
    fn from(id: ModuleNodeId) -> Self {
        PrimaryNodeId::Module(id)
    }
}

// Removed From<FunctionNodeId> for AssociatedItemId
impl From<MethodNodeId> for AssociatedItemId {
    // Method
    fn from(id: MethodNodeId) -> Self {
        AssociatedItemId::Method(id)
    }
}
impl From<TypeAliasNodeId> for AssociatedItemId {
    fn from(id: TypeAliasNodeId) -> Self {
        AssociatedItemId::TypeAlias(id)
    }
}
// Removed From<ValueNodeId>
impl From<ConstNodeId> for AssociatedItemId {
    fn from(id: ConstNodeId) -> Self {
        AssociatedItemId::Const(id)
    }
} // Added

// --- Node Struct Definitions ---
// Logging target
const LOG_TARGET_NODE: &str = "node_info"; // Define log target for visibility checks

/// Core trait for all graph nodes
pub trait GraphNode {
    fn id(&self) -> NodeId;
    fn visibility(&self) -> &VisibilityKind;
    fn name(&self) -> &str;
    fn cfgs(&self) -> &[String];

    // --- Default implementations for downcasting ---
    fn as_function(&self) -> Option<&FunctionNode> {
        // Standalone function
        None
    }
    fn as_method(&self) -> Option<&MethodNode> {
        // Associated function/method
        None
    }
    fn as_struct(&self) -> Option<&StructNode> {
        None
    }
    fn as_enum(&self) -> Option<&EnumNode> {
        None
    }
    fn as_union(&self) -> Option<&UnionNode> {
        None
    }
    fn as_type_alias(&self) -> Option<&TypeAliasNode> {
        None
    }
    fn as_trait(&self) -> Option<&TraitNode> {
        None
    }
    fn as_impl(&self) -> Option<&ImplNode> {
        None
    }
    fn as_module(&self) -> Option<&ModuleNode> {
        None
    }
    fn as_const(&self) -> Option<&ConstNode> {
        // Added
        None
    }
    fn as_static(&self) -> Option<&StaticNode> {
        // Added
        None
    }
    fn as_macro(&self) -> Option<&MacroNode> {
        None
    }
    fn as_import(&self) -> Option<&ImportNode> {
        None
    }
    fn kind_matches(&self, kind: ItemKind) -> bool {
        match kind {
            ItemKind::Function => self.as_function().is_some(), // Matches standalone functions
            ItemKind::Method => self.as_method().is_some(), // Matches associated functions/methods
            ItemKind::Struct => self.as_struct().is_some(),
            ItemKind::Enum => self.as_enum().is_some(),
            ItemKind::Union => self.as_union().is_some(),
            ItemKind::TypeAlias => self.as_type_alias().is_some(),
            ItemKind::Trait => self.as_trait().is_some(),
            ItemKind::Impl => self.as_impl().is_some(),
            ItemKind::Module => self.as_module().is_some(),
            ItemKind::Const => self.as_const().is_some(), // Updated
            ItemKind::Static => self.as_static().is_some(), // Updated
            ItemKind::Macro => self.as_macro().is_some(),
            ItemKind::Import => self.as_import().is_some(),
            ItemKind::ExternCrate => {
                // kind of a hack job. needs cleaner solution
                if let Some(import_node) = self.as_import() {
                    // Use if let for safety
                    import_node.is_extern_crate()
                } else {
                    false
                }
            }

            // ItemKind::Field | ItemKind::Variant | ItemKind::GenericParam
            // are not directly represented as top-level GraphNode types this way.
            _ => false,
        }
    }

    fn kind(&self) -> ItemKind {
        // Check for Method first as it might overlap with Function if not careful
        if self.as_method().is_some() {
            ItemKind::Method // Method is more specific
        } else if self.as_function().is_some() {
            ItemKind::Function // Standalone function
        } else if self.as_struct().is_some() {
            ItemKind::Struct
        } else if self.as_enum().is_some() {
            ItemKind::Enum
        } else if self.as_union().is_some() {
            ItemKind::Union
        } else if self.as_type_alias().is_some() {
            ItemKind::TypeAlias
        } else if self.as_trait().is_some() {
            ItemKind::Trait
        } else if self.as_impl().is_some() {
            ItemKind::Impl
        } else if self.as_module().is_some() {
            ItemKind::Module
        } else if self.as_macro().is_some() {
            ItemKind::Macro
        } else if self.as_import().is_some() {
            // Check for extern crate specifically within import
            if self.kind_matches(ItemKind::ExternCrate) {
                ItemKind::ExternCrate
            } else {
                ItemKind::Import
            }
        } else if self.as_static().is_some() {
            // Updated check order
            ItemKind::Static
        } else if self.as_const().is_some() {
            // Updated check order
            ItemKind::Const
        } else {
            // This panic indicates a GraphNode implementation is missing a corresponding
            // 'as_xxx' method or the kind() logic here is incomplete.
            panic!(
                "Unknown GraphNode kind encountered. Name: {}, ID: {}",
                self.name(),
                self.id()
            )
        }

        // ItemKind::Field | ItemKind::Variant | ItemKind::GenericParam | ItemKind::ExternCrate
        // are not directly represented as top-level GraphNode types this way.
    }

    fn log_node_debug(&self) {
        debug!(target: LOG_TARGET_NODE,
            "{} {: <12} {: <20} | {: <12} | {: <15}",
            "NodeInfo".log_header(),
            self.name().log_name(),
            self.id().to_string().log_id(),
            self.kind().log_vis_debug(),
            self.visibility().log_name_debug(),
        );
    }

    fn log_node_error(&self) {
        log::error!(target: LOG_TARGET_NODE,
            "{} {} {: <12} {: <20} | {: <12} | {: <15}",
            "ERROR".log_error(),
            "NodeInfo".log_header(),
            self.name().log_name(),
            self.id().to_string().log_id(),
            self.kind().log_vis_debug(),
            self.visibility().log_name_debug(),
        );
    }

    // Add others like VariantNode, FieldNode if they implement GraphNode directly
}

// Shared error types
#[derive(Debug, thiserror::Error, Clone, PartialEq)] // Removed Eq because TypeId might not be Eq
pub enum NodeError {
    #[error("Invalid node configuration: {0}")]
    Validation(String),

    #[error("Invalid node converstion from TypeId, expected NodeId: {0}")] // Updated error message
    Conversion(TypeId),
    // Removed GraphIdConversion variant
}

/// Full module path name,
/// e.g. for an item in project/src/a/mod.rs mod b { fn func() {} }
///     ["project", "a", "b", "func"]
/// May be composed of relative or absolute elements, e.g. "super", "crate"
/// Glob imports are included.
/// Will not contain "self" (already resolved in Phase 2 processing)
///     - see `visit_item_use` method of `CodeGraph` in code_graph.rs for details on resolution of
///     `syn::UseTree` into ImportNode.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodePath(Vec<String>);

impl Display for NodePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("::"))
    }
}

impl NodePath {
    pub fn new(segments: Vec<String>) -> Result<Self, SynParserError> {
        if segments.is_empty() {
            // The `?` operator won't work here directly as we need to construct the error first.
            // We return the specific SynParserError variant directly.
            return Err(SynParserError::NodeValidation("Empty module path".into()));
        }
        Ok(Self(segments))
    }

    /// Creates a new `NodePath` without checking if the segments are empty.
    /// Use with caution, only when the input is guaranteed to be non-empty.
    ///
    /// # Panics
    /// Panics if the input `segments` vector is empty.
    pub fn new_unchecked(segments: Vec<String>) -> Self {
        if segments.is_empty() {
            panic!("NodePath::new_unchecked called with empty segments. This indicates an internal error.");
        }
        Self(segments)
    }

    pub fn as_segments(&self) -> &[String] {
        &self.0
    }

    pub fn with_name<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a u8> {
        self.0
            .iter()
            .map(|string| string.as_bytes())
            .flatten()
            .chain(name.as_bytes())
    }

    // Conversion helpers
    pub fn from_str_path(path: &str) -> Self {
        Self(path.split("::").map(|s| s.to_string()).collect())
    }

    // Compare with any string-like iterator
    pub fn matches<'a, I>(&self, other: I) -> bool
    where
        I: Iterator<Item = &'a str>,
    {
        self.0.iter().map(|s| s.as_str()).eq(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_paths() {
        let path = NodePath::new(vec!["crate".into(), "mod_a".into()]).unwrap();
        assert!(path.matches(["crate", "mod_a"].into_iter()));
        assert!(!path.matches(["mod_a"].into_iter()));
    }
}

// Add these trait implementations
impl AsRef<[String]> for NodePath {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

// Allow HashMap<&NodePath, V> to be queried with &[String]
impl Borrow<[String]> for NodePath {
    fn borrow(&self) -> &[String] {
        &self.0
    }
}

// Implement TryFrom for fallible conversion from Vec<String>
impl TryFrom<Vec<String>> for NodePath {
    type Error = SynParserError; // The error type is SynParserError

    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        // Call the fallible `new` method and propagate its error using `?`
        NodePath::new(value)
    }
}

// Represent an attribute
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub span: (usize, usize),  // Byte start/end offsets
    pub name: String,          // e.g., "derive", "cfg", "serde"
    pub args: Vec<String>,     // Arguments or parameters of the attribute
    pub value: Option<String>, // Optional value (e.g., for `#[attr = "value"]`)
}

pub fn extract_attr_path_value(attrs: &[Attribute]) -> Option<&str> {
    attrs
        .iter()
        .find(|attr| attr.name == "path" && attr.value.is_some())
        .and_then(|attr| attr.value.as_deref())
}

pub trait HasAttributes {
    fn attributes(&self) -> &[Attribute];
}

pub fn extract_path_attr_from_node<T: HasAttributes>(node: &T) -> Option<&str> {
    extract_attr_path_value(node.attributes())
}

// Represents a type definition (struct, enum, type alias, or union)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TypeDefNode {
    Struct(StructNode),
    Enum(EnumNode),
    TypeAlias(TypeAliasNode),
    Union(UnionNode),
}

impl GraphNode for TypeDefNode {
    fn visibility(&self) -> &VisibilityKind {
        match self {
            TypeDefNode::Struct(struct_node) => &struct_node.visibility,
            TypeDefNode::Enum(enum_node) => &enum_node.visibility,
            TypeDefNode::TypeAlias(type_alias_node) => &type_alias_node.visibility,
            TypeDefNode::Union(union_node) => &union_node.visibility,
        }
    }

    fn name(&self) -> &str {
        match self {
            TypeDefNode::Struct(struct_node) => struct_node.name(),
            TypeDefNode::Enum(enum_node) => enum_node.name(),
            TypeDefNode::TypeAlias(type_alias_node) => type_alias_node.name(),
            TypeDefNode::Union(union_node) => union_node.name(),
        }
    }

    fn id(&self) -> NodeId {
        match self {
            TypeDefNode::Struct(struct_node) => struct_node.id(),
            TypeDefNode::Enum(enum_node) => enum_node.id(),
            TypeDefNode::TypeAlias(type_alias_node) => type_alias_node.id(),
            TypeDefNode::Union(union_node) => union_node.id(),
        }
    }
    fn cfgs(&self) -> &[String] {
        match self {
            TypeDefNode::Struct(n) => n.cfgs(),
            TypeDefNode::Enum(n) => n.cfgs(),
            TypeDefNode::TypeAlias(n) => n.cfgs(),
            TypeDefNode::Union(n) => n.cfgs(),
        }
    }

    // Delegate downcasting methods
    fn as_struct(&self) -> Option<&StructNode> {
        match self {
            TypeDefNode::Struct(s) => Some(s),
            _ => None,
        }
    }

    fn as_enum(&self) -> Option<&EnumNode> {
        match self {
            TypeDefNode::Enum(e) => Some(e),
            _ => None,
        }
    }

    fn as_type_alias(&self) -> Option<&TypeAliasNode> {
        match self {
            TypeDefNode::TypeAlias(t) => Some(t),
            _ => None,
        }
    }

    fn as_union(&self) -> Option<&UnionNode> {
        match self {
            TypeDefNode::Union(u) => Some(u),
            _ => None,
        }
    }
}
