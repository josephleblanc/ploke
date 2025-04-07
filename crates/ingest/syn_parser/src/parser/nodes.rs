use crate::parser::types::{GenericParamNode, VisibilityKind};
#[cfg(not(feature = "uuid_ids"))]
use crate::TypeId;
#[cfg(feature = "uuid_ids")]
use ploke_core::{NodeId, TrackingHash, TypeId}; // Use new types when feature is enabled
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "uuid_ids"))]
pub type NodeId = usize;

// ANCHOR: ItemFn
// Represents a function definition
impl Visible for ModuleNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    pub parameters: Vec<ParamData>,
    pub return_type: Option<TypeId>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}
//ANCHOR_END: ItemFn
impl Visible for FunctionNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// Represents a parameter in a function
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParamData {
    pub name: Option<String>,
    pub type_id: TypeId, // The ID of the parameter's type
    pub is_mutable: bool,
    pub is_self: bool,
}

// Represents a type definition (struct, enum, type alias, or union)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TypeDefNode {
    Struct(StructNode),
    Enum(EnumNode),
    TypeAlias(TypeAliasNode),
    Union(UnionNode),
}

// ANCHOR: StructNode
// Represents a struct definition
impl Visible for StructNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StructNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>, // Replace Vec<String>
    pub docstring: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}
//ANCHOR_END: StructNode

// Represents an enum definition
impl Visible for EnumNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnumNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    pub variants: Vec<VariantNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}

// ANCHOR: field_node
// Represents a field in a struct
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FieldNode {
    pub id: NodeId,
    pub name: Option<String>,
    pub type_id: TypeId,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
}
//ANCHOR_END: field_node

// Represents a variant in an enum
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VariantNode {
    pub id: NodeId,
    pub name: String,
    pub fields: Vec<FieldNode>,
    pub discriminant: Option<String>,
    pub attributes: Vec<Attribute>,
}

// Represents a type alias (type NewType = OldType)
impl Visible for TypeAliasNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TypeAliasNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}

// Represents a union definition
impl Visible for UnionNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnionNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
    #[cfg(feature = "uuid_ids")]
    pub span: (usize, usize),
}

// ANCHOR: ImplNode
// Represents an implementation block
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImplNode {
    pub id: NodeId,
    pub self_type: TypeId,
    pub span: (usize, usize), // Byte start/end offsets
    pub trait_type: Option<TypeId>,
    pub methods: Vec<FunctionNode>,
    pub generic_params: Vec<GenericParamNode>,
}

impl Visible for ImplNode {
    fn visibility(&self) -> VisibilityKind {
        VisibilityKind::Public
    }

    fn name(&self) -> &str {
        // Placeholder
        // TODO: Think through this and improve it
        "impl block"
    }

    fn id(&self) -> NodeId {
        self.id
    }
}
//ANCHOR_END: ItemImpl

// ANCHOR: TraitNode
// Represents a trait definition
impl Visible for TraitNode {
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> NodeId {
        self.id
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TraitNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    pub methods: Vec<FunctionNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub super_traits: Vec<TypeId>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}
//ANCHOR_END: TraitNode

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleNode {
    pub id: NodeId,
    pub name: String,
    pub path: Vec<String>,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub submodules: Vec<NodeId>,
    pub items: Vec<NodeId>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<NodeId>, // TODO: Confirm if exports need tracking hash? Likely not.
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}

// Represents a constant or static variable
impl Visible for ValueNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ValueNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub kind: ValueKind,
    pub value: Option<String>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}

// Represents a macro definition
impl Visible for MacroNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MacroNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub kind: MacroKind,
    #[cfg(not(feature = "uuid_ids"))]
    pub rules: Vec<MacroRuleNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
}

// Represents a macro rule
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MacroRuleNode {
    pub id: NodeId,
    pub pattern: String,
    pub expansion: String,
}

// Different kinds of macros
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum MacroKind {
    DeclarativeMacro,
    ProcedureMacro { kind: ProcMacroKind },
}

// Different kinds of procedural macros
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ProcMacroKind {
    Derive,
    Attribute,
    Function,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ValueKind {
    Constant,
    Static { is_mutable: bool },
}

/// Represents all import/export semantics in the code graph, including:
/// - Regular `use` statements
/// - `pub use` re-exports
/// - Extern crate declarations
/// - Future import-like constructs
///
/// # Key Features
/// - Tracks both source path and visible identifiers
/// - Handles rename semantics (`as` clauses) and glob imports
/// - Preserves span information for error reporting
/// - Distinguishes between import types via `ImportKind`
///
/// # Example: Basic Import
/// ```rust
/// use std::collections::HashMap;
/// ```
/// Produces:
/// ```ignore
/// ImportNode {
///     path: vec!["std", "collections", "HashMap"],
///     visible_name: "HashMap",
///     original_name: None,
///     is_glob: false,
///     kind: ImportKind::UseStatement,
///     ...
/// }
/// ```
///
/// # Example: Renamed Import
/// ```rust
/// use std::collections::{HashMap as Map, BTreeSet};
/// ```
/// Produces two nodes:
/// ```ignore
/// [
///     ImportNode {
///         path: vec!["std", "collections", "HashMap"],
///         visible_name: "Map",
///         original_name: Some("HashMap"),
///         is_glob: false,
///         kind: ImportKind::UseStatement,
///         ...
///     },
///     ImportNode {
///         path: vec!["std", "collections", "BTreeSet"],
///         visible_name: "BTreeSet",
///         original_name: None,
///         is_glob: false,
///         kind: ImportKind::UseStatement,
///         ...
///     }
/// ]
/// ```
///
/// # Example: Re-export
/// ```ignore
/// pub use crate::internal::api as public_api;
/// ```
/// Produces:
/// ```ignore
/// ImportNode {
///     path: vec!["crate", "internal", "api"],
///     visible_name: "public_api",
///     original_name: Some("api"),
///     is_glob: false,
///     kind: ImportKind::UseStatement,
///     ...
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportNode {
    /// Unique identifier for this import in the graph
    pub id: NodeId,

    /// Source code span (byte offsets) of the import statement
    pub span: (usize, usize),

    /// Full path segments in original order (e.g. ["std", "collections", "HashMap"])
    pub path: Vec<String>,

    /// Type of import (regular use, extern crate, etc.)
    pub kind: ImportKind,

    /// Name as brought into scope (accounts for renames via `as`)
    pub visible_name: String,

    /// Original identifier name when renamed (None for direct imports)
    pub original_name: Option<String>,

    /// Whether this is a glob import (`use some::path::*`)
    pub is_glob: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ImportKind {
    ImportNode,
    ExternCrate,
    UseStatement,
}

/// Result of visibility resolution with detailed scoping information
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VisibilityResult {
    /// Directly usable without imports
    Direct,
    /// Needs use statement with given path
    NeedsUse(Vec<String>),
    /// Not accessible with current scope
    OutOfScope {
        /// Why the item isn't accessible
        // reason: OutOfScopeReason,
        /// For pub(in path) cases, shows allowed scopes  
        allowed_scopes: Option<Vec<String>>,
    },
}
/// Trait for nodes that have visibility information                   
pub trait Visible {
    fn visibility(&self) -> VisibilityKind;
    fn name(&self) -> &str;
    fn id(&self) -> NodeId;
}

/// Detailed reasons for out-of-scope items
// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
// pub enum OutOfScopeReason {
//     Private,
//     CrateRestricted,
//     SuperRestricted,
//     WorkspaceHidden, // Reserved for future workspace support
//     CfgGated,        // Reserved for cfg() attributes
// }

// Represent an attribute
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attribute {
    pub span: (usize, usize),  // Byte start/end offsets
    pub name: String,          // e.g., "derive", "cfg", "serde"
    pub args: Vec<String>,     // Arguments or parameters of the attribute
    pub value: Option<String>, // Optional value (e.g., for `#[attr = "value"]`)
}
