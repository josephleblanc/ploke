use ploke_core::{NodeId, TypeId, TypeKind}; // Import TypeKind from ploke_core

use serde::{Deserialize, Serialize};

use std::fmt;

// ANCHOR: TypeNode
// Represents a type reference with full metadata
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TypeNode {
    pub id: TypeId,
    pub kind: TypeKind,
    // Reference to related types (e.g., generic arguments)
    pub related_types: Vec<TypeId>,
}
//ANCHOR_END: TypeNode

// TypeKind moved to ploke_core

/// Represents a generic parameter
/// These are appropriate node elements because they are "defined" when used in part of another,
/// e.g. function, definition. Then, we can say that they are "referenced" when used in, e.g. a
/// function call, or as part of another definition like for a struct's fields.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenericParamNode {
    pub id: NodeId,
    pub kind: GenericParamKind,
}

impl GenericParamNode {
    pub fn name_if_type_id(&self, ty_id: TypeId) -> Option<&str> {
        match &self.kind {
            GenericParamKind::Type { name, default, .. } => {
                if let Some(type_id) = r#default {
                    if type_id == &ty_id {
                        Some(name)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            GenericParamKind::Lifetime { .. } => None,
            GenericParamKind::Const { name, type_id } => {
                if type_id == &ty_id {
                    Some(name)
                } else {
                    None
                }
            }
        }
    }
}

// ANCHOR: generic_param_kind
// Different kinds of generic parameters
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum GenericParamKind {
    Type {
        name: String,
        bounds: Vec<TypeId>,
        default: Option<TypeId>,
    },
    Lifetime {
        name: String,
        bounds: Vec<String>,
    },
    Const {
        name: String,
        type_id: TypeId,
    },
}

impl GenericParamKind {
    /// Returns the name of the generic parameter, if applicable.
    pub fn name(&self) -> Option<&str> {
        match self {
            GenericParamKind::Type { name, .. } => Some(name),
            GenericParamKind::Lifetime { name, .. } => Some(name),
            GenericParamKind::Const { name, .. } => Some(name),
        }
    }

    /// Returns the type bounds of the generic parameter, if applicable.
    pub fn bounds(&self) -> Option<&[TypeId]> {
        match self {
            GenericParamKind::Type { bounds, .. } => Some(bounds),
            GenericParamKind::Lifetime { .. } => None, // Lifetimes have string bounds, handled separately if needed
            GenericParamKind::Const { .. } => None,
        }
    }

    /// Returns the lifetime bounds of the generic parameter, if applicable.
    pub fn lifetime_bounds(&self) -> Option<&[String]> {
        match self {
            GenericParamKind::Type { .. } => None,
            GenericParamKind::Lifetime { bounds, .. } => Some(bounds),
            GenericParamKind::Const { .. } => None,
        }
    }

    /// Returns the default type of the generic parameter, if applicable.
    pub fn default(&self) -> Option<&TypeId> {
        match self {
            GenericParamKind::Type { default, .. } => default.as_ref(),
            GenericParamKind::Lifetime { .. } => None,
            GenericParamKind::Const { .. } => None,
        }
    }

    /// Returns the type ID of the const generic parameter, if applicable.
    pub fn const_type_id(&self) -> Option<&TypeId> {
        match self {
            GenericParamKind::Type { .. } => None,
            GenericParamKind::Lifetime { .. } => None,
            GenericParamKind::Const { type_id, .. } => Some(type_id),
        }
    }
}

//ANCHOR_END: generic_param_kind

/// Different kinds of visibility
// TODO: Revisit the design of our visibility parsing.
// It is not clear to me that we are correctly handling visibility. Ideally, we
// should be able to say with certainty that a given node (e.g. FunctionNode,
// StructNode) is visible within a given span (defined as byte start to byte end).
// I have downloaded the repository for `syn`, and the relevant file for
// `Visibility` is:
//  - ~/clones/syn/src/restriction.rs
//  - Contains definition of Visibility
//  - Good jumping off point to find more docs/source describing exactly how visibility is handled,
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum VisibilityKind {
    /// Corresponds to `pub` visibility.
    Public,
    /// Corresponds to `pub(crate)` visibility.
    Crate,
    /// Corresponds to restricted visibility like `pub(in path)` or `pub(super)`.
    /// The `Vec<String>` contains the path segments (e.g., `["super"]` or `["crate", "module"]`).
    /// An empty path `[]` within `Restricted` is technically possible if `syn` parses `pub(in)` without a path,
    /// but typically implies `Public`. We might normalize this later if needed.
    Restricted(Vec<String>),
    /// Corresponds to the absence of an explicit visibility keyword (e.g., `fn foo() {}` inside a module).
    /// The actual visibility depends on the context (private to the module by default).
    Inherited,
}

impl fmt::Display for VisibilityKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VisibilityKind::Public => write!(f, "pub"),
            VisibilityKind::Crate => write!(f, "pub(crate)"),
            VisibilityKind::Restricted(path) => {
                if path.is_empty() {
                    write!(f, "pub")
                } else {
                    write!(f, "pub(in {})", path.join("::"))
                }
            }
            VisibilityKind::Inherited => write!(f, ""), // Empty for inherited
        }
    }
}
