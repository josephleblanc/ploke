use ploke_core::{NodeId, TypeId}; // Use new types when feature is enabled // Use compat types when feature is disabled

use serde::{Deserialize, Serialize};

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

// ANCHOR: TypeKind_defn
// Different kinds of types
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TypeKind {
    //ANCHOR_END: TypeKind_defn
    Named {
        path: Vec<String>, // Full path segments
        is_fully_qualified: bool,
    },
    Reference {
        lifetime: Option<String>,
        is_mutable: bool,
        // Type being referenced is in related_types[0]
    },
    Slice {
        // Element type is in related_types[0]
    },
    Array {
        // Element type is in related_types[0]
        size: Option<String>,
    },
    Tuple {
        // Element types are in related_types
    },
    // ANCHOR: ExternCrate
    Function {
        // Parameter types are in related_types (except last one)
        // Return type is in related_types[last]
        is_unsafe: bool,
        is_extern: bool,
        abi: Option<String>,
    },
    //ANCHOR_END: ExternCrate
    Never,
    Inferred,
    RawPointer {
        is_mutable: bool,
        // Pointee type is in related_types[0]
    },
    // ANCHOR: TraitObject
    TraitObject {
        // Trait bounds are in related_types
        dyn_token: bool,
    },
    //ANCHOR_END: TraitObject
    // ANCHOR: ImplTrait
    ImplTrait {
        // Trait bounds are in related_types
    },
    //ANCHOR_END: ImplTrait
    Paren {
        // Inner type is in related_types[0]
    },
    // ANCHOR: ItemMacro
    Macro {
        name: String,
        tokens: String,
    },
    //ANCHOR_END: ItemMacro
    Unknown {
        type_str: String,
    },
}

// Represents a generic parameter
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
//  Questions:
//  - What exactly is the `Path` type used in `VisRestricted`?
//  - Can we link the `Path` type to a file and/or span?
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum VisibilityKind {
    Public,
    Crate,
    Restricted(Vec<String>), // Path components of restricted visibility
    Inherited,               // Default visibility
}
