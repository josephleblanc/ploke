use crate::parser::types::{GenericParamNode, TypeId, VisibilityKind};

use serde::{Deserialize, Serialize};

// Unique ID for a node in the graph
pub type NodeId = usize;

// ANCHOR: ItemFn
// Represents a function definition
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionNode {
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
    pub visibility: VisibilityKind,
    pub parameters: Vec<ParameterNode>,
    pub return_type: Option<TypeId>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
}
//ANCHOR_END: ItemFn

// Represents a parameter in a function
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParameterNode {
    pub id: NodeId,
    pub name: Option<String>,
    pub type_id: TypeId,
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
}
//ANCHOR_END: StructNode

// Represents an enum definition
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
}

// Represents a union definition
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnionNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
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
//ANCHOR_END: ItemImpl

// ANCHOR: TraitNode
// Represents a trait definition
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
}
//ANCHOR_END: TraitNode

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleNode {
    pub id: NodeId,
    pub name: String,
    #[cfg(feature = "module_path_tracking")]
    pub path: Vec<String>,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub submodules: Vec<NodeId>,
    pub items: Vec<NodeId>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<NodeId>,
}

// Represents a constant or static variable
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
}

// Represents a macro definition
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MacroNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub kind: MacroKind,
    pub rules: Vec<MacroRuleNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
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

// Represents a module
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportNode {
    pub id: NodeId,
    pub span: (usize, usize), // Byte start/end offsets
    pub path: Vec<String>,
    pub kind: ImportKind,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ImportKind {
    UseStatement,
    ExternCrate,
}

/// Represents a Rust `use` statement's semantic meaning in the graph.
/// 
/// # Current Implementation Notes
/// - Tracks raw path segments exactly as written in source
/// - Preserves original spans for error reporting
/// - Aliases are normalized (handles `as` clauses)
/// - Does NOT yet handle macro expansions (pending Phase 4)
///
/// # Example
/// ```rust
/// use std::collections::{HashMap as Map, BTreeSet};
/// ```
/// Produces two UseStatement nodes:
/// ```ignore
/// [
///     UseStatement {
///         path: vec!["std", "collections", "HashMap"],
///         visible_name: "Map",
///         original_name: Some("HashMap"),
///         is_glob: false,
///         span: (start_byte, end_byte)
///     },
///     UseStatement {
///         path: vec!["std", "collections", "BTreeSet"],
///         visible_name: "BTreeSet",
///         original_name: None,
///         is_glob: false,
///         span: (start_byte, end_byte)
///     }
/// ]
/// ```
#[cfg(feature = "use_statement_tracking")]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UseStatement {
    /// Full path segments in original order (e.g. ["std", "collections", "HashMap"])
    pub path: Vec<String>,

    /// Final name as brought into scope (handles renames)
    /// For `use foo::bar as baz` this would be "baz"
    pub visible_name: String,

    /// Original name if renamed (for `use x::y as z` this is Some("y"
    /// None for non-renamed imports                                   
    pub original_name: Option<String>,

    /// Whether this is a glob import
    pub is_glob: bool,

    /// Span of entire use statement for potential future reference
    pub span: (usize, usize),
}

/// Result of visibility resolution with detailed scoping information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum VisibilityResult {
    /// Directly usable without imports
    Direct,
    /// Needs use statement with given path
    NeedsUse(Vec<String>),
    /// Not accessible with current scope
    OutOfScope {
        /// Why the item isn't accessible
        reason: OutOfScopeReason,
        /// For pub(in path) cases, shows allowed scopes  
        allowed_scopes: Option<Vec<String>>
    }
}

/// Detailed reasons for out-of-scope items
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OutOfScopeReason {
    Private,
    CrateRestricted,
    SuperRestricted,
    WorkspaceHidden, // Reserved for future workspace support
    CfgGated,       // Reserved for cfg() attributes
}

// Represent an attribute
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attribute {
    pub span: (usize, usize),  // Byte start/end offsets
    pub name: String,          // e.g., "derive", "cfg", "serde"
    pub args: Vec<String>,     // Arguments or parameters of the attribute
    pub value: Option<String>, // Optional value (e.g., for `#[attr = "value"]`)
}
