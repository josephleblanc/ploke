use std::path::PathBuf;

use crate::parser::types::{GenericParamNode, VisibilityKind};
use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// Removed cfg_expr::Expression import

// ANCHOR: ItemFn
// Represents a function definition
impl GraphNode for ModuleNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}
//ANCHOR_END: ItemFn
impl GraphNode for FunctionNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }
}

// Represents a parameter in a function
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ParamData {
    pub name: Option<String>,
    pub type_id: TypeId, // The ID of the parameter's type
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

// ANCHOR: StructNode
// Represents a struct definition
impl GraphNode for StructNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
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
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}
//ANCHOR_END: StructNode

// Represents an enum definition
impl GraphNode for EnumNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
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
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
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
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
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
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

// Represents a type alias (type NewType = OldType)
impl GraphNode for TypeAliasNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
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
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

// Represents a union definition
impl GraphNode for UnionNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
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
    pub tracking_hash: Option<TrackingHash>,
    pub span: (usize, usize),
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
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
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl GraphNode for ImplNode {
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
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }
}
//ANCHOR_END: ItemImpl

// ANCHOR: TraitNode
// Represents a trait definition
impl GraphNode for TraitNode {
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> NodeId {
        self.id
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
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
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}
//ANCHOR_END: TraitNode

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ModuleNode {
    pub id: NodeId,
    pub name: String,
    pub path: Vec<String>,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>, // Attributes on the `mod foo { ... }` item itself
    pub docstring: Option<String>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<NodeId>, // TODO: Confirm if exports need tracking hash? Likely not.
    pub span: (usize, usize), // Add span field
    pub tracking_hash: Option<TrackingHash>,
    pub module_def: ModuleDef,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item (`#[cfg] mod foo;` or `#[cfg] mod foo {}`)
}

impl ModuleNode {
    /// Definition path to file as it would be called by a `use` statement,
    /// Examples:
    ///     module declaration in project/main.rs
    ///         "mod module_one;" -> ["crate", "module_one"]
    ///     file module:
    ///         project/module_one/mod.rs -> ["crate", "module_one"]
    ///     in-line module in project/module_one/mod.rs
    ///         `mod module_two {}` -> ["crate", "module_one", "module_two"]
    pub fn defn_path(&self) -> Vec<String> {
        let path = self.path.clone();
        path.to_vec().push(self.name.clone());
        path
    }

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
        match &self.module_def {
            ModuleDef::Inline { items, .. } => Some(items),
            ModuleDef::FileBased { items, .. } => Some(items),
            ModuleDef::Declaration { .. } => None,
        }
        // if let ModuleDef::Inline { items, .. } = &self.module_def {
        //     Some(items)
        // } else {
        //     None
        // }
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
    pub fn file_docs(&self) -> Option<&String> {
        if let ModuleDef::FileBased { file_docs, .. } = &self.module_def {
            // Want to return the reference to the inner type, not Option (using .as_ref())
            file_docs.as_ref()
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
        items: Vec<NodeId>, // Probably temporary while gaining confidence in Relation::Contains
        file_path: PathBuf,
        file_attrs: Vec<Attribute>, // Non-CFG #![...] attributes
        file_docs: Option<String>,  // //!
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
impl GraphNode for ValueNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
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
    pub span: (usize, usize),
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

// Represents a macro definition
impl GraphNode for MacroNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn cfgs(&self) -> &[String] {
        &self.cfgs // Simply return a slice reference to the stored cfgs
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MacroNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Add span field
    pub visibility: VisibilityKind,
    pub kind: MacroKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
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

    /// Whether this is a 'self' import, e.g. `std::fs::{self}`
    pub is_self_import: bool,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl ImportNode {
    pub fn path(&self) -> &[String] {
        &self.path
    }
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
/// Trait for nodes that have visibility and CFG information
pub trait GraphNode {
    fn visibility(&self) -> VisibilityKind;
    fn name(&self) -> &str;
    fn id(&self) -> NodeId;
    /// Returns the raw CFG strings directly attached to this node item.
    fn cfgs(&self) -> &[String];
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
