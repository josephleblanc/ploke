mod consts;
mod enums;
mod function;
mod impls;
mod import;
mod macros;
mod module;
mod statics;
mod structs;
mod traits;
mod type_alias;
mod union;
// private ids and methods here
mod ids;
// ----- ids public re-exports -----
// Does not directly expose any direct access to NodeId
pub use ids::*;
// -----------------------------
use std::borrow::Borrow;
use std::fmt::Display;

pub use super::graph::GraphNode;

use crate::error::SynParserError;

use super::types::VisibilityKind;
use ploke_core::{ItemKind, TypeId};
use serde::{Deserialize, Serialize};

// Re-export all node types from submodules
pub use consts::ConstNode;
pub use enums::{EnumNode, VariantNode};
pub use function::{FunctionNode, MethodNode, ParamData}; // Added MethodNode
pub use impls::ImplNode;
pub use import::{ImportKind, ImportNode};
pub use macros::{MacroKind, MacroNode, ProcMacroKind};
pub use module::{ModDisc, ModuleKind, ModuleNode};
pub use statics::StaticNode;
pub use structs::{FieldNode, StructNode};
pub use traits::TraitNode;
pub use type_alias::TypeAliasNode;
pub use union::UnionNode;

// test structures generated with proc macros:
// This might be kind of a dirty way to do it, but I just hope it works:
pub use consts::ExpectedConstNode;
pub use statics::ExpectedStaticNode;

// Re-export the generated *NodeInfo structs for internal use within the crate
// NOTE: 2025-05-02:
// - Deleted all other *NodeInfo types, since we now use ids/internal.rs for NodeId gen through
// visitor methods.
// - Re: ModuleNodeInfo Still using this in the special case of creating the root module. We may
// want to refactor the way the root module is created in `visitor/mod.rs`. Leaving it here for
// now.
pub(crate) use module::ModuleNodeInfo;

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
    #[cfg(not(feature = "type_bearing_ids"))]
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
    // pub span: (usize, usize),  // Byte start/end offsets
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

    fn any_id(&self) -> AnyNodeId {
        match self {
            TypeDefNode::Struct(struct_node) => struct_node.any_id(),
            TypeDefNode::Enum(enum_node) => enum_node.any_id(),
            TypeDefNode::TypeAlias(type_alias_node) => type_alias_node.any_id(),
            TypeDefNode::Union(union_node) => union_node.any_id(),
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

pub(crate) trait HasKind {
    fn has_kind(&self) -> ItemKind;
}
