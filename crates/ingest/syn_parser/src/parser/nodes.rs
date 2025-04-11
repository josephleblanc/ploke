use std::path::PathBuf;

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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FunctionNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    #[cfg(feature = "uuid_ids")]
    pub parameters: Vec<ParamData>,
    #[cfg(not(feature = "uuid_ids"))]
    pub parameters: Vec<ParameterNode>,
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg(feature = "uuid_ids")]
pub struct ParamData {
    pub name: Option<String>,
    pub type_id: TypeId, // The ID of the parameter's type
    pub is_mutable: bool,
    pub is_self: bool,
}

// Represents a parameter in a function
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg(not(feature = "uuid_ids"))]
pub struct ParameterNode {
    pub id: NodeId,
    pub name: Option<String>,
    pub type_id: TypeId,
    pub is_mutable: bool,
    pub is_self: bool,
}

// Represents a type definition (struct, enum, type alias, or union)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TypeDefNode {
    Struct(StructNode),
    Enum(EnumNode),
    TypeAlias(TypeAliasNode),
    Union(UnionNode),
}

impl Visible for TypeDefNode {
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FieldNode {
    pub id: NodeId,
    pub name: Option<String>,
    pub type_id: TypeId,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
}
//ANCHOR_END: field_node

// Represents a variant in an enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

// Your detailed breakdown of the way cozo handles the search is well taken, and makes me want to
// revisit the data structure we are currently using within our `CodeVisitor` struct implementation
// of the `syn::Visit` trait. Let's drill down on the `ModuleNode` as an example of how to
// structure our top-level node items, with an eye toward both how it would affect our
// `visit_item_*` methods, e.g. `visit_item_mod` and toward how we will later need to transform
// these data structures into the database.
// A few additional points to keep in mind:
// 1. We have not yet decided whether the transformation from the parser into the locally embedded,
//    in-memory `cozo::Db` should be capable of going from `ModuleNode` to `cozo` database object
//    and back again (there is a word for this but I forget, please remind me. I think it is the
//    structure-preserving transformation, like isomorphic or homomorphic?)
// 2. The structure of `ModuleNode` was primarily inspired by the `syn::ItemMod` initially, then
//    adjusted to handle the needs of our `syn_parser` crate as it grew. While practical in the
//    sense that this enabled our `syn_parser` project to rapidly develop, the desired end-state
//    for the cozo database is still not well understood by me. In our design, we want to lean into
//    the cozo database representation, but are not clear on how valuable it will be to have a
//    different representation for analysis outside the cozo database. This lack of clarity carries
//    over into the design below. However, I would like to take your example as an opportunity to
//    consider how the data structure might be adjusted to better suit the cozo database
//    downstream, and what the trade-offs might be for our parser implementation.
// 3. Relation vs. ModuleNode field: This is the primary point that should likely be decided upon
//    in the near future to clarify both the responsibility of the parser and and the as-yet
//    undesigned cozo schema. I have a number of questions on this point:
//    - What elements, currently represented as fields in the `ModuleNode` should be represented as
//    a `Relation`?
//    - What should we ensure the `ModuleNode` retains as direct properties of a `cozo`
//    relation/object/node (the language is hard here) such as CozoScript
//    `module_node { id: Uuid, name: String , ..}`
//    - Does the answer to what should be an edge or node property change if we want to do analysis
//    outside of cozo. Why would we want to do analysis outside/inside the cozo framework?
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ModuleNode {
    pub id: NodeId,
    pub name: String,
    pub path: Vec<String>,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<NodeId>, // TODO: Confirm if exports need tracking hash? Likely not.
    #[cfg(feature = "uuid_ids")]
    pub span: (usize, usize), // Add span field
    #[cfg(feature = "uuid_ids")]
    #[cfg_attr(feature = "uuid_ids", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "uuid_ids", serde(default))]
    pub tracking_hash: Option<TrackingHash>,
    #[cfg(feature = "uuid_ids")]
    pub module_def: ModuleDef,
}

// - Simple getters should be named after the field (`items()`, `file_path()`, etc.)
// - Boolean checks should use `is_` or `has_` prefixes
// - Methods returning `Option` should indicate this in the name (like `try_get_items()`)
#[cfg(feature = "uuid_ids")]
impl ModuleNode {
    /// Returns true if this is a file-based module
    pub fn is_file_based(&self) -> bool {
        matches!(self.module_def, ModuleDef::FileBased { .. })
    }

    /// Returns true if this is an inline module
    pub fn is_inline(&self) -> bool {
        matches!(self.module_def, ModuleDef::Inline { .. })
    }

    /// Returns true if this is just a module declaration
    pub fn is_declaration(&self) -> bool {
        matches!(self.module_def, ModuleDef::Declaration { .. })
    }

    /// Returns the items if this is an inline module, None otherwise
    pub fn items(&self) -> Option<&[NodeId]> {
        if let ModuleDef::Inline { items, .. } = &self.module_def {
            Some(items)
        } else {
            None
        }
    }

    /// Returns the file path if this is a file-based module, None otherwise
    pub fn file_path(&self) -> Option<&PathBuf> {
        if let ModuleDef::FileBased { file_path, .. } = &self.module_def {
            Some(file_path)
        } else {
            None
        }
    }

    /// Returns the file attributes if this is a file-based module, None otherwise
    pub fn file_attrs(&self) -> Option<&[Attribute]> {
        if let ModuleDef::FileBased { file_attrs, .. } = &self.module_def {
            Some(file_attrs)
        } else {
            None
        }
    }

    /// Returns the file docs if this is a file-based module, None otherwise
    pub fn file_docs(&self) -> Option<&[String]> {
        if let ModuleDef::FileBased { file_docs, .. } = &self.module_def {
            Some(file_docs)
        } else {
            None
        }
    }

    /// Returns the span if this is an inline module, None otherwise
    pub fn inline_span(&self) -> Option<(usize, usize)> {
        if let ModuleDef::Inline { span, .. } = &self.module_def {
            Some(*span)
        } else {
            None
        }
    }

    /// Returns the declaration span if this is a module declaration, None otherwise
    pub fn declaration_span(&self) -> Option<(usize, usize)> {
        if let ModuleDef::Declaration {
            declaration_span, ..
        } = &self.module_def
        {
            Some(*declaration_span)
        } else {
            None
        }
    }

    /// Returns the resolved definition if this is a module declaration, None otherwise
    pub fn resolved_definition(&self) -> Option<NodeId> {
        if let ModuleDef::Declaration {
            resolved_definition,
            ..
        } = &self.module_def
        {
            *resolved_definition
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ModuleDef {
    /// File-based module (src/module/mod.rs)
    FileBased {
        file_path: PathBuf,
        file_attrs: Vec<Attribute>, // #![...]
        file_docs: Vec<String>,     // //!
    },
    /// Inline module (mod name { ... })
    Inline {
        items: Vec<NodeId>, // References to contained items
        span: (usize, usize),
    },
    /// Declaration only (mod name;)
    Declaration {
        declaration_span: (usize, usize),
        resolved_definition: Option<NodeId>, // Populated during resolution phase
    },
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Attribute {
    pub span: (usize, usize),  // Byte start/end offsets
    pub name: String,          // e.g., "derive", "cfg", "serde"
    pub args: Vec<String>,     // Arguments or parameters of the attribute
    pub value: Option<String>, // Optional value (e.g., for `#[attr = "value"]`)
}
