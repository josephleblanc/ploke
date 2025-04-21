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

use crate::error::SynParserError;

// Removed GraphId import from relations
use super::types::VisibilityKind;
use ploke_core::{NodeId, TypeId};
use serde::{Deserialize, Serialize};
use thiserror::Error; // Add thiserror

// Re-export all node types from submodules
pub use enums::{EnumNode, VariantNode};
pub use function::{FunctionNode, ParamData};
pub use impls::ImplNode;
pub use import::{ImportKind, ImportNode};
pub use macros::{MacroKind, MacroNode, MacroRuleNode, ProcMacroKind};
pub use module::{ModuleDef, ModuleNode};
pub use structs::{FieldNode, StructNode};
pub use traits::TraitNode;
pub use type_alias::TypeAliasNode;
pub use union::UnionNode;
pub use value::{ValueKind, ValueNode};
// ... other re-exports

/// Core trait for all graph nodes
pub trait GraphNode {
    fn id(&self) -> NodeId;
    fn visibility(&self) -> VisibilityKind;
    fn name(&self) -> &str;
    fn cfgs(&self) -> &[String];
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
            GraphId::Node(_) => None,
            GraphId::Type(id) => Some(id),
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
            GraphId::Node(_) => None,
            GraphId::Type(id) => Some(id),
        }
    }
}

impl std::fmt::Display for GraphId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphId::Node(node_id) => write!(f, "GraphID: {}", node_id),
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

impl TryInto<NodeId> for GraphId {
    type Error = GraphIdConversionError;

    fn try_into(self) -> Result<NodeId, Self::Error> {
        match self {
            GraphId::Node(id) => Ok(id),
            GraphId::Type(_) => Err(GraphIdConversionError::ExpectedNode(self)),
        }
    }
}

impl TryInto<TypeId> for GraphId {
    type Error = GraphIdConversionError;

    fn try_into(self) -> Result<TypeId, Self::Error> {
        match self {
            GraphId::Node(_) => Err(GraphIdConversionError::ExpectedType(self)),
            GraphId::Type(id) => Ok(id),
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ModuleNodeId(NodeId);
impl ModuleNodeId {
    /// Create from raw NodeId
    pub fn new(id: NodeId) -> Self {
        Self(id)
    }

    /// Get inner NodeId
    pub fn into_inner(self) -> NodeId {
        self.0
    }

    /// Get reference to inner NodeId
    pub fn as_inner(&self) -> &NodeId {
        &self.0
    }

    /// Converts this ModuleNodeId into a GraphId::Node.
    pub fn to_graph_id(self) -> GraphId {
        GraphId::Node(self.0)
    }
}

impl std::fmt::Display for ModuleNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the inner NodeId's Display implementation
        write!(f, "{}", self.0)
    }
}

impl TryFrom<GraphId> for ModuleNodeId {
    type Error = NodeError;

    fn try_from(value: GraphId) -> Result<Self, Self::Error> {
        match value {
            GraphId::Node(id) => Ok(ModuleNodeId::new(id)),
            GraphId::Type(id) => Err(NodeError::Conversion(id)),
        }
    }
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

    pub fn as_segments(&self) -> &[String] {
        &self.0
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
}
