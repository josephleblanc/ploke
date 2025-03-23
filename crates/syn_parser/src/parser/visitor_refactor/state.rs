use crate::parser::graph::CodeGraph;
use crate::parser::nodes::*;
use crate::parser::relations::*;
use crate::parser::types::*;
use quote::ToTokens;
use std::collections::HashMap;
use syn::{FnArg, Generics, Pat, PatIdent, PatType, ReturnType, Type, TypeParam, Visibility};

pub struct VisitorState {
    pub(crate) code_graph: CodeGraph,
    next_node_id: NodeId,
    next_type_id: TypeId,
    // Maps existing types to their IDs to avoid duplication
    pub(crate) type_map: HashMap<String, TypeId>,
}

impl VisitorState {
    pub(crate) fn new() -> Self {
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
            },
            next_node_id: 0,
            next_type_id: 0,
            type_map: HashMap::new(),
        }
    }

    pub(crate) fn next_node_id(&mut self) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    pub(crate) fn next_type_id(&mut self) -> TypeId {
        let id = self.next_type_id;
        self.next_type_id += 1;
        id
    }

    // Move the get_or_create_type method to type_processing.rs
    // Move the process_type method to type_processing.rs

    // Convert syn::Visibility to our VisibilityKind
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
            // Private visibility shows up as Inherited in syn, which in Rust means
            // visibility is limited to the current module and its descendants
            Visibility::Inherited => VisibilityKind::Restricted(vec!["super".to_string()]),
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
                    _ => (None, false),
                };

                Some(ParameterNode {
                    id: self.next_node_id(),
                    name,
                    type_id,
                    is_mutable,
                    is_self: false,
                })
            }
            FnArg::Receiver(receiver) => {
                // Create a special self type
                let self_type_id = self.next_type_id();
                let mut related_types = Vec::new();

                // If we have an explicit type for self, include it
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

                Some(ParameterNode {
                    id: self.next_node_id(),
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
                        let path = expr.to_token_stream().to_string();
                        if let Some(&id) = self.type_map.get(&path) {
                            id
                        } else {
                            let id = self.next_type_id();
                            super::type_processing::get_or_create_type(self, expr);
                            id
                        }
                    });

                    params.push(GenericParamNode {
                        id: self.next_node_id(),
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

                    params.push(GenericParamNode {
                        id: self.next_node_id(),
                        kind: GenericParamKind::Lifetime {
                            name: lifetime_def.lifetime.ident.to_string(),
                            bounds,
                        },
                    });
                }
                syn::GenericParam::Const(const_param) => {
                    let type_id = super::type_processing::get_or_create_type(self, &const_param.ty);

                    params.push(GenericParamNode {
                        id: self.next_node_id(),
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
            syn::TypeParamBound::Lifetime(_) => {
                // Create a synthetic type for the lifetime bound
                let type_id = self.next_type_id();
                self.code_graph.type_graph.push(TypeNode {
                    id: type_id,
                    kind: TypeKind::Named {
                        path: vec!["lifetime".to_string()],
                        is_fully_qualified: false,
                    },
                    related_types: Vec::new(),
                });
                type_id
            }
            _ => self.next_type_id(),
        }
    }

    fn process_lifetime_bound(&mut self, bound: &syn::Lifetime) -> String {
        bound.ident.to_string()
    }

    // Move extract_docstring and extract_attributes to attribute_processing.rs
}
