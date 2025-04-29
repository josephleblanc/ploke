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

use std::fmt::Display;
use std::iter::{Flatten, Map};
use std::marker::PhantomData;
use std::{borrow::Borrow, io::Chain};

use crate::{
    error::SynParserError,
    utils::{LogStyle, LogStyleDebug},
};

// Removed GraphId import from relations
use super::types::VisibilityKind;
use log::debug;
use ploke_core::{ItemKind, NodeId, TypeId};
use serde::{Deserialize, Serialize};
use thiserror::Error; // Add thiserror

// Re-export all node types from submodules
pub use enums::{EnumNode, VariantNode};
pub use function::{FunctionNode, ParamData};
pub use impls::ImplNode;
pub use import::{ImportKind, ImportNode};
pub use macros::{MacroKind, MacroNode, MacroRuleNode, ProcMacroKind};
pub use module::{ModuleKind, ModuleNode, ModuleNodeId};
pub use structs::{FieldNode, StructNode};
pub use traits::TraitNode;
pub use type_alias::TypeAliasNode;
pub use union::UnionNode;
pub use value::{ValueKind, ValueNode};
// ... other re-exports

// ----- utility macro -----
// Differentiators for primary id types
// We don't actualy use these anywhere yet. You can see that `ModuleNodeId` is commented out
// because there is a separate implementation of that. We started using ModuleNodeId during the
// creation of the ModuleTree.
use crate::define_node_id_wrapper;
define_node_id_wrapper!(EnumNodeId);
define_node_id_wrapper!(FunctionNodeId);
define_node_id_wrapper!(ImplNodeId);
define_node_id_wrapper!(ImportNodeId);
// define_node_id_wrapper!(ModuleNodeId);
define_node_id_wrapper!(StructNodeId);
define_node_id_wrapper!(TraitNodeId);
define_node_id_wrapper!(TypeAliasNodeId);
define_node_id_wrapper!(UnionNodeId);
define_node_id_wrapper!(ValueNodeId);
define_node_id_wrapper!(FieldNodeId);
define_node_id_wrapper!(VariantNodeId);
define_node_id_wrapper!(ParamNodeId); // For ParamData
define_node_id_wrapper!(GenericParamNodeId);
define_node_id_wrapper!(MacroNodeId);


// For more explicit differntiation within Phase 3 module tree processing
define_node_id_wrapper!(ReexportNodeId);

// Logging target
const LOG_TARGET_NODE: &str = "node_info"; // Define log target for visibility checks

/// Core trait for all graph nodes
pub trait GraphNode {
    fn id(&self) -> NodeId;
    fn visibility(&self) -> VisibilityKind;
    fn name(&self) -> &str;
    fn cfgs(&self) -> &[String];

    // --- Default implementations for downcasting ---
    fn as_function(&self) -> Option<&FunctionNode> {
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
    // Replaced by following const/static differentiators
    // fn as_value(&self) -> Option<&ValueNode> {
    //     None
    // }
    fn as_value_const(&self) -> Option<&ValueNode> {
        None
    }
    fn as_value_static(&self) -> Option<&ValueNode> {
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
            ItemKind::Function => self.as_function().is_some(),
            ItemKind::Struct => self.as_struct().is_some(),
            ItemKind::Enum => self.as_enum().is_some(),
            ItemKind::Union => self.as_union().is_some(),
            ItemKind::TypeAlias => self.as_type_alias().is_some(),
            ItemKind::Trait => self.as_trait().is_some(),
            ItemKind::Impl => self.as_impl().is_some(),
            ItemKind::Module => self.as_module().is_some(),
            ItemKind::Const => self.as_value_const().is_some(),
            ItemKind::Static => self.as_value_static().is_some(), // Combine Const/Static check
            ItemKind::Macro => self.as_macro().is_some(),
            ItemKind::Import => self.as_import().is_some(),
            ItemKind::ExternCrate => {
                // kind of a hack job. needs cleaner solution
                if self.as_import().is_some() {
                    let extern_crate = self.as_import().unwrap(); // safe due to check above
                    extern_crate.is_extern_crate()
                } else {
                    false
                }
            }

            // ItemKind::Field | ItemKind::Variant | ItemKind::GenericParam | ItemKind::ExternCrate
            // are not directly represented as top-level GraphNode types this way.
            _ => false,
        }
    }

    fn kind(&self) -> ItemKind {
        if self.as_function().is_some() {
            ItemKind::Function
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
            ItemKind::Import
        } else if self.as_value_static().is_some() {
            ItemKind::Static
        } else if self.as_value_const().is_some() {
            ItemKind::Const
        } else {
            // Kind of a hack
            panic!("Unknown TypeKind found.")
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

/// Represents either a Node or a Type in the graph context, used primarily in Relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum GraphId {
    Node(NodeId),
    Type(TypeId),
}

impl GraphId {
    /// Returns a reference to the inner `NodeId` if this is a `Node` variant.
    pub fn as_node_ref(&self) -> Option<&NodeId> {
        match self {
            GraphId::Node(id) => Some(id),
            GraphId::Type(_) => None,
        }
    }

    /// Returns a reference to the inner `TypeId` if this is a `Type` variant.
    pub fn as_type_ref(&self) -> Option<&TypeId> {
        match self {
            GraphId::Type(id) => Some(id),
            _ => None,
        }
    }

    /// Consumes the `GraphId` and returns the inner `NodeId` if it's a `Node` variant.
    pub fn into_node(self) -> Option<NodeId> {
        match self {
            GraphId::Node(id) => Some(id),
            GraphId::Type(_) => None,
        }
    }

    /// Consumes the `GraphId` and returns the inner `TypeId` if it's a `Type` variant.
    pub fn into_type(self) -> Option<TypeId> {
        match self {
            _ => None,
            GraphId::Type(id) => Some(id),
        }
    }
}

impl std::fmt::Display for GraphId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            node_id => write!(f, "GraphID: {}", node_id),
            GraphId::Type(type_id) => write!(f, "GraphID: {}", type_id),
        }
    }
}

/// Error during GraphId conversion.
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)] // GraphId is Copy, Eq
pub enum GraphIdConversionError {
    #[error("Expected GraphId::Node variant, but found Type variant: {0}")]
    ExpectedNode(GraphId),
    #[error("Expected GraphId::Type variant, but found Node variant: {0}")]
    ExpectedType(GraphId),
}

// Removed TryInto<NodeId> for GraphId
// Removed TryInto<TypeId> for GraphId

impl From<NodeId> for GraphId {
    fn from(node_id: NodeId) -> Self {
        GraphId::Node(node_id)
    }
}

// Shared error types
#[derive(Debug, thiserror::Error, Clone, PartialEq)] // Removed Eq because TypeId might not be Eq
pub enum NodeError {
    #[error("Invalid node configuration: {0}")]
    Validation(String),

    #[error("Invalid node converstion from GraphId::Type, must be GraphId::Node: {0}")]
    Conversion(TypeId),
    #[error("Graph ID conversion error: {0}")]
    GraphIdConversion(#[from] GraphIdConversionError), // Add From conversion
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
    fn visibility(&self) -> VisibilityKind {
        match self {
            TypeDefNode::Struct(struct_node) => struct_node.visibility.clone(),
            TypeDefNode::Enum(enum_node) => enum_node.visibility.clone(),
            TypeDefNode::TypeAlias(type_alias_node) => type_alias_node.visibility.clone(),
            TypeDefNode::Union(union_node) => union_node.visibility.clone(),
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
