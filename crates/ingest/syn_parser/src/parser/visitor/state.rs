use crate::parser::graph::CodeGraph;
use crate::parser::nodes::{Attribute, ImportNode, ModuleNode, ParameterNode, Visible};
use crate::parser::types::{
    GenericParamKind, GenericParamNode, TypeKind, TypeNode, VisibilityKind,
};
use crate::parser::utils::type_to_string;
use quote::ToTokens;
use syn::{FnArg, Generics, Pat, PatIdent, PatType, TypeParam, Visibility};

use dashmap::DashMap;
use std::sync::Arc;

// --- Conditional Imports based on 'uuid_ids' feature ---
#[cfg(feature = "uuid_ids")]
use {
    ploke_core::{LogicalTypeId, NodeId, TrackingHash, TypeId, PROJECT_NAMESPACE_UUID},
    std::path::{Path, PathBuf}, // Needed for new fields
    uuid::Uuid,                 // Needed for new fields
};

#[cfg(not(feature = "uuid_ids"))]
use ploke_core::{NodeId, TypeId}; // Use compat types when feature is disabled
                                  // --- End Conditional Imports ---

pub struct VisitorState {
    pub(crate) code_graph: CodeGraph,

    // --- Conditional Fields based on 'uuid_ids' feature ---
    #[cfg(not(feature = "uuid_ids"))]
    next_node_id: NodeId, // usize counter when UUIDs are off
    #[cfg(not(feature = "uuid_ids"))]
    next_type_id: TypeId, // usize counter when UUIDs are off

    #[cfg(feature = "uuid_ids")]
    pub(crate) crate_namespace: Uuid, // Namespace for the crate being parsed
    #[cfg(feature = "uuid_ids")]
    pub(crate) current_file_path: PathBuf, // Path of the file being parsed by this visitor instance
    // --- End Conditional Fields ---

    // Use DashMap for thread-safe concurrent access to the type cache
    // TypeId here will be usize or the Uuid-based struct depending on the feature flag
    pub(crate) type_map: Arc<DashMap<String, TypeId>>,

    // TODO: AI comment: Re-evaluate if both current_module_path and current_module are needed.
    // current_module_path seems more aligned with UUID generation needs.
    // USER response: Agreed, should re-evaluate post-refactor of uuid system.
    pub(crate) current_module_path: Vec<String>, // e.g., ["crate", "parser", "visitor"]
    pub(crate) current_module: Vec<String>,      // Stack of module IDs/names? Needs clarification.
}

impl VisitorState {
    // TODO: Update constructor signature when integrating with Phase 1/`analyze_files_parallel`
    // It will need to accept crate_namespace and current_file_path.
    #[cfg(feature = "uuid_ids")]
    pub(crate) fn new(crate_namespace: Uuid, current_file_path: PathBuf) -> Self {
        Self {
            code_graph: CodeGraph {
                functions: Vec::new(),
                defined_types: Vec::new(),
                type_graph: Vec::new(),
                impls: Vec::new(),
                traits: Vec::new(),
                private_traits: Vec::new(),
                relations: Vec::new(),
                modules: Vec::new(),
                values: Vec::new(),
                macros: Vec::new(),
                use_statements: Vec::new(),
            },
            // Legacy, not needed
            // Initialize old fields for non-uuid_ids
            // next_node_id: 0,
            // next_type_id: 0,

            // New values needed for Uuid generation of Synthetic NodeId/TypeId variants
            crate_namespace,
            current_file_path,
            // New fields are conditionally compiled out
            type_map: Arc::new(DashMap::new()),
            current_module_path: Vec::new(),
            current_module: Vec::new(),
        }
    }

    // --- Conditional Methods based on 'uuid_ids' feature ---

    // These methods only exist when using usize IDs
    #[cfg(not(feature = "uuid_ids"))]
    pub(crate) fn next_node_id(&mut self) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    #[cfg(not(feature = "uuid_ids"))]
    pub(crate) fn next_type_id(&mut self) -> TypeId {
        let id = self.next_type_id;
        self.next_type_id += 1;
        id
    }
    pub(crate) fn generate_synthetic_node_id(&self, name: &str, span: (usize, usize)) -> NodeId {
        NodeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            &self.current_module_path,
            name,
            span,
        )
    }

    pub(crate) fn generate_synthetic_type_id(&self, ty: &syn::Type) {
        TypeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            type_to_string(ty),
        )
    }

    // --- End Conditional Methods ---

    pub(crate) fn convert_visibility(&self, vis: &Visibility) -> VisibilityKind {
        match vis {
            Visibility::Public(_) => VisibilityKind::Public,
            Visibility::Restricted(restricted) => {
                let path = restricted
                    .path
                    .segments
                    .iter()
                    .map(|seg| seg.ident.to_string())
                    .collect();
                VisibilityKind::Restricted(path)
            }
            // Changed handling of inherited visibility
            Visibility::Inherited => VisibilityKind::Inherited,
        }
    }

    // Process a function parameter
    pub(crate) fn process_fn_arg(&mut self, arg: &FnArg) -> Option<ParameterNode> {
        match arg {
            FnArg::Typed(PatType { pat, ty, .. }) => {
                let type_id = super::type_processing::get_or_create_type(self, ty);

                // Extract parameter name and mutability
                let (name, is_mutable) = match &**pat {
                    Pat::Ident(PatIdent {
                        ident, mutability, ..
                    }) => (Some(ident.to_string()), mutability.is_some()),
                    _ => (None, false), // Handle other patterns if necessary
                };

                // Generate ID based on feature flag
                #[cfg(not(feature = "uuid_ids"))]
                let id = self.next_node_id();
                #[cfg(feature = "uuid_ids")]
                let id: NodeId = self.generate_synthetic_node_id(
                    &format!(
                        "param_{}",
                        name.as_deref().unwrap_or("unnamed") // Simple context for param
                    ), // TODO: Need span info here! Pass it down or get from `arg`.
                       // (0, 0), // Placeholder span
                       // USER comment: Wouldn't the relative module path work just as well here
                       // instead of span? Are they both essentially the same or is there an advantage
                       // to one over the other?
                );

                Some(ParameterNode {
                    id,
                    name,
                    type_id,
                    is_mutable,
                    is_self: false,
                })
            }
            FnArg::Receiver(receiver) => {
                // Create a special self type
                // Use get_or_create_type for consistency, even for "Self"
                // This might require adjustments in get_or_create_type to handle "Self" specifically
                // or we create a dedicated "Self" type node. Let's try creating it directly for now.

                #[cfg(not(feature = "uuid_ids"))]
                let self_type_id = self.next_type_id();
                #[cfg(feature = "uuid_ids")]
                let self_type_id = self.generate_synthetic_type_id("Self"); // Generate ID for Self type

                let mut related_types = Vec::new();

                // If we have an explicit type for self (e.g., `self: Box<Self>`), process it
                let ty_ref: &syn::Type = &receiver.ty;
                let inner_type_id = super::type_processing::get_or_create_type(self, ty_ref);
                related_types.push(inner_type_id);

                self.code_graph.type_graph.push(TypeNode {
                    id: self_type_id,
                    kind: TypeKind::Named {
                        path: vec!["Self".to_string()],
                        is_fully_qualified: false,
                    },
                    related_types,
                });

                // Generate ID for the 'self' parameter node itself
                #[cfg(not(feature = "uuid_ids"))]
                let param_id = self.next_node_id();
                #[cfg(feature = "uuid_ids")]
                let param_id = self.generate_synthetic_node_id(
                    "param_self",
                    // TODO: Get span from receiver if possible
                    (0, 0), // Placeholder span
                );

                Some(ParameterNode {
                    id: param_id,
                    name: Some("self".to_string()),
                    type_id: self_type_id,
                    is_mutable: receiver.mutability.is_some(),
                    is_self: true,
                })
            }
        }
    }

    // Process generic parameters
    pub(crate) fn process_generics(&mut self, generics: &Generics) -> Vec<GenericParamNode> {
        let mut params = Vec::new();

        for param in &generics.params {
            match param {
                syn::GenericParam::Type(TypeParam {
                    ident,
                    bounds,
                    default,
                    ..
                }) => {
                    let bounds: Vec<TypeId> = bounds
                        .iter()
                        .map(|bound| self.process_type_bound(bound))
                        .collect();

                    let default_type = default.as_ref().map(|expr| {
                        // Use get_or_create_type for default types
                        super::type_processing::get_or_create_type(self, expr)
                    });

                    // Generate ID for the generic parameter node
                    #[cfg(not(feature = "uuid_ids"))]
                    let param_node_id = self.next_node_id();
                    #[cfg(feature = "uuid_ids")]
                    let param_node_id = self.generate_synthetic_node_id(
                        &format!("generic_type_{}", ident),
                        // TODO: Get span from TypeParam if possible
                        (0, 0), // Placeholder span
                    );

                    params.push(GenericParamNode {
                        id: param_node_id,
                        kind: GenericParamKind::Type {
                            name: ident.to_string(),
                            bounds,
                            default: default_type,
                        },
                    });
                }
                syn::GenericParam::Lifetime(lifetime_def) => {
                    let bounds: Vec<String> = lifetime_def
                        .bounds
                        .iter()
                        .map(|bound| self.process_lifetime_bound(bound))
                        .collect();

                    // Generate ID for the generic parameter node
                    #[cfg(not(feature = "uuid_ids"))]
                    let param_node_id = self.next_node_id();
                    #[cfg(feature = "uuid_ids")]
                    let param_node_id = self.generate_synthetic_node_id(
                        &format!("generic_lifetime_{}", lifetime_def.lifetime.ident),
                        // TODO: Get span from LifetimeDef if possible
                        (0, 0), // Placeholder span
                    );

                    params.push(GenericParamNode {
                        id: param_node_id,
                        kind: GenericParamKind::Lifetime {
                            name: lifetime_def.lifetime.ident.to_string(),
                            bounds,
                        },
                    });
                }
                syn::GenericParam::Const(const_param) => {
                    let type_id = super::type_processing::get_or_create_type(self, &const_param.ty);

                    // Generate ID for the generic parameter node
                    #[cfg(not(feature = "uuid_ids"))]
                    let param_node_id = self.next_node_id();
                    #[cfg(feature = "uuid_ids")]
                    let param_node_id = self.generate_synthetic_node_id(
                        &format!("generic_const_{}", const_param.ident),
                        // TODO: Get span from ConstParam if possible
                        (0, 0), // Placeholder span
                    );

                    params.push(GenericParamNode {
                        id: param_node_id,
                        kind: GenericParamKind::Const {
                            name: const_param.ident.to_string(),
                            type_id,
                        },
                    });
                }
            }
        }

        params
    }

    fn process_type_bound(&mut self, bound: &syn::TypeParamBound) -> TypeId {
        match bound {
            syn::TypeParamBound::Trait(trait_bound) => super::type_processing::get_or_create_type(
                self,
                &syn::Type::Path(syn::TypePath {
                    qself: None,
                    path: trait_bound.path.clone(),
                }),
            ),
            // TODO: How should lifetime bounds be represented in the type graph?
            // For now, create a placeholder type ID. Revisit during Phase 3 resolution.
            syn::TypeParamBound::Lifetime(_) => {
                // Create a synthetic type for the lifetime bound
                #[cfg(not(feature = "uuid_ids"))]
                let type_id = self.next_type_id();
                #[cfg(feature = "uuid_ids")]
                let type_id = self.generate_synthetic("lifetime_bound"); // Placeholder

                self.code_graph.type_graph.push(TypeNode {
                    id: type_id,
                    kind: TypeKind::Named {
                        // Or a new TypeKind::LifetimeBound?
                        path: vec!["lifetime".to_string()],
                        is_fully_qualified: false,
                    },
                    related_types: Vec::new(),
                });
                type_id
            }
            // Handle `Verbatim` or future variants if necessary
            _ => {
                #[cfg(not(feature = "uuid_ids"))]
                let type_id = self.next_type_id();
                #[cfg(feature = "uuid_ids")]
                let type_id = self.generate_synthetic_type_id("unknown_type_bound"); // Placeholder
                type_id
            }
        }
    }

    fn process_lifetime_bound(&mut self, bound: &syn::Lifetime) -> String {
        bound.ident.to_string()
    }
    // Move extract_docstring and extract_attributes to attribute_processing.rs
}
