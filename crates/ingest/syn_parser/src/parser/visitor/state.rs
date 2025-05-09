use crate::parser::graph::CodeGraph;
use crate::parser::types::{GenericParamKind, GenericParamNode, VisibilityKind};
use log::debug;
// Removed cfg_expr::Expression import
use ploke_core::ItemKind;
use syn::{FnArg, Generics, Pat, PatIdent, PatType, TypeParam, Visibility};

use super::calculate_cfg_hash_bytes;
use super::type_processing::get_or_create_type; // NEW: Import helper

use {
    crate::parser::nodes::ParamData,
    ploke_core::{NodeId, TrackingHash, TypeId},
    std::path::PathBuf,
    uuid::Uuid,
};
const LOG_TARGET: &str = "node_id";

pub struct VisitorState {
    pub(crate) code_graph: CodeGraph,
    pub(crate) crate_namespace: Uuid, // Namespace for the crate being parsed
    pub(crate) current_file_path: PathBuf, // Path of the file being parsed by this visitor instance
    // --- End Conditional Fields ---

    // Removed type_map cache (Arc<DashMap<String, TypeId>>)

    // TODO: AI comment: Re-evaluate if both current_module_path and current_module are needed.
    // current_module_path seems more aligned with UUID generation needs.
    // USER response: Agreed, should re-evaluate post-refactor of uuid system.
    pub(crate) current_module_path: Vec<String>, // e.g., ["crate", "parser", "visitor"]
    pub(crate) current_module: Vec<String>,      // Stack of module IDs/names? Needs clarification.
    // Stack tracking the NodeId of the current definition scope (e.g., struct, fn, impl, trait)
    pub(crate) current_definition_scope: Vec<NodeId>,
    // --- NEW CFG Tracking Fields ---
    /// The combined raw CFG strings inherited from the current scope (file, module, struct, etc.)
    pub(crate) current_scope_cfgs: Vec<String>,
    /// Stack to save/restore `current_scope_cfgs` when entering/leaving scopes.
    pub(crate) cfg_stack: Vec<Vec<String>>,
}

impl VisitorState {
    // TODO: Update constructor signature when integrating with Phase 1/`analyze_files_parallel`
    // It will need to accept crate_namespace and current_file_path.
    pub(crate) fn new(crate_namespace: Uuid, current_file_path: PathBuf) -> Self {
        Self {
            code_graph: CodeGraph {
                functions: Vec::new(),
                defined_types: Vec::new(),
                type_graph: Vec::new(),
                impls: Vec::new(),
                traits: Vec::new(),
                relations: Vec::new(),
                modules: Vec::new(),
                values: Vec::new(),
                macros: Vec::new(),
                use_statements: Vec::new(),
            },
            // New values needed for Uuid generation of Synthetic NodeId/TypeId variants
            crate_namespace,
            current_file_path,
            // New fields are conditionally compiled out
            // type_map removed
            current_module_path: Vec::new(),
            current_module: Vec::new(),
            current_definition_scope: Vec::new(), // Initialize empty scope stack
            // Initialize new CFG fields
            current_scope_cfgs: Vec::new(),
            cfg_stack: Vec::new(),
        }
    }

    /// Helper to generate a synthetic NodeId using the current visitor state.
    /// Uses the last ID pushed onto `current_definition_scope` as the parent scope ID.
    /// Accepts the calculated hash bytes of the effective CFG strings.
    pub(crate) fn generate_synthetic_node_id(
        &self,
        name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> NodeId {
        // Get the last pushed scope ID as the parent, if available
        let parent_scope_id = self.current_definition_scope.last().copied();

        debug!(target: LOG_TARGET,
            "[Visitor generate_synthetic_node_id for '{}' ({:?})]",
            name, item_kind
        );
        debug!(target: LOG_TARGET, "  crate_namespace: {}", self.crate_namespace);
        debug!(target: LOG_TARGET, "  file_path: {:?}", self.current_file_path);
        debug!(target: LOG_TARGET, "  relative_path: {:?}", self.current_module_path);
        debug!(target: LOG_TARGET, "  item_name: {}", name);
        debug!(target: LOG_TARGET, "  item_kind: {:?}", item_kind);
        debug!(target: LOG_TARGET, "  parent_scope_id: {:?}", parent_scope_id);
        debug!(target: LOG_TARGET, "  cfg_bytes: {:?}", cfg_bytes);

        NodeId::generate_synthetic(
            self.crate_namespace,
            &self.current_file_path,
            &self.current_module_path, // Current module path acts as relative path context
            name,
            item_kind,
            parent_scope_id, // Pass the parent scope ID from the stack
            cfg_bytes,       // Pass the provided CFG bytes
        )
    }

    pub(crate) fn generate_tracking_hash(
        &self,
        item_tokens: &proc_macro2::TokenStream,
    ) -> TrackingHash {
        // Directly call the core generation function using state context
        TrackingHash::generate(self.crate_namespace, &self.current_file_path, item_tokens)
    }

    // --- End Conditional Methods ---

    pub(crate) fn convert_visibility(&self, vis: &syn::Visibility) -> VisibilityKind {
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

    /// Processes a function argument (`syn::FnArg`) to extract its metadata
    /// and the TypeId of its type. Does NOT generate a NodeId for the parameter itself.
    ///
    /// # Arguments
    /// * `state` - Mutable visitor state.
    /// * `arg` - The syn::FnArg to process.
    ///
    /// # Returns
    /// An `Option<ParamData>` containing parameter name, TypeId, mutability, and self status.
    pub(crate) fn process_fn_arg(&mut self, arg: &FnArg) -> Option<ParamData> {
        match arg {
            FnArg::Typed(PatType { pat, ty, .. }) => {
                // Get the TypeId for the parameter's type
                let type_id = get_or_create_type(self, ty);

                // Extract parameter name and mutability from the pattern
                let (name, is_mutable) = match &**pat {
                    Pat::Ident(PatIdent {
                        ident, mutability, ..
                    }) => (Some(ident.to_string()), mutability.is_some()),
                    _ => (None, false), // Handle other patterns like tuple/struct destructuring if needed
                };

                Some(ParamData {
                    name,
                    type_id,
                    is_mutable,
                    is_self: false,
                })
            }
            FnArg::Receiver(receiver) => {
                // The receiver's type (`Self`, `Box<Self>`, etc.)
                let receiver_type = &receiver.ty;
                // Get the TypeId for the receiver's type
                let type_id = get_or_create_type(self, receiver_type);

                Some(ParamData {
                    name: Some("self".to_string()),
                    type_id,
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
                        .filter_map(|bound| self.process_type_bound(bound))
                        .collect();

                    let default_type = default.as_ref().map(|expr| {
                        // Use get_or_create_type for default types
                        get_or_create_type(self, expr)
                    });

                    // Calculate CFG hash based on the *current scope* where the generic is defined
                    let generic_cfg_bytes = calculate_cfg_hash_bytes(&self.current_scope_cfgs);

                    // Generate ID for the generic parameter node, pass ItemKind::GenericParam and cfg_bytes
                    let param_node_id = self.generate_synthetic_node_id(
                        &format!("generic_type_{}", ident), // Use a distinct name format
                        ItemKind::GenericParam,
                        generic_cfg_bytes.as_deref(), // Pass calculated bytes
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

                    // Calculate CFG hash based on the *current scope*
                    let generic_cfg_bytes = calculate_cfg_hash_bytes(&self.current_scope_cfgs);

                    // Generate ID for the generic parameter node, pass ItemKind::GenericParam and cfg_bytes
                    let param_node_id = self.generate_synthetic_node_id(
                        &format!("generic_lifetime_{}", lifetime_def.lifetime.ident), // Use a distinct name format
                        ItemKind::GenericParam,
                        generic_cfg_bytes.as_deref(), // Pass calculated bytes
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

                    // Calculate CFG hash based on the *current scope*
                    let generic_cfg_bytes = calculate_cfg_hash_bytes(&self.current_scope_cfgs);

                    // Generate ID for the generic parameter node, pass ItemKind::GenericParam and cfg_bytes
                    let param_node_id = self.generate_synthetic_node_id(
                        &format!("generic_const_{}", const_param.ident), // Use a distinct name format
                        ItemKind::GenericParam,
                        generic_cfg_bytes.as_deref(), // Pass calculated bytes
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

    /// Process type bounds for generics
    // Only handles trait bounds for now
    fn process_type_bound(&mut self, bound: &syn::TypeParamBound) -> Option<TypeId> {
        match bound {
            syn::TypeParamBound::Trait(trait_bound) => {
                let type_id = get_or_create_type(
                    self,
                    &syn::Type::Path(syn::TypePath {
                        qself: None,
                        path: trait_bound.path.clone(),
                    }),
                );
                Some(type_id)
            }
            // TODO: How should lifetime bounds be represented in the type graph?
            // For now, create a placeholder type ID. Revisit during Phase 3 resolution.
            // USER note: Not handling lifetimes for now, we don't need that granular of
            // information for first implementation of RAG, maybe later for static analysis
            //
            // syn::TypeParamBound::Lifetime(lt) => {
            //     // Create a synthetic type for the lifetime bound
            //     let type_id = self.generate_synthetic_node_id("lifetime_bound", lt); // Placeholder
            //
            //     self.code_graph.type_graph.push(TypeNode {
            //         id: type_id,
            //         kind: TypeKind::Named {
            //             // Or a new TypeKind::LifetimeBound?
            //             path: vec!["lifetime".to_string()],
            //             is_fully_qualified: false,
            //         },
            //         related_types: Vec::new(),
            //     });
            //     type_id
            // }

            // Handle `Verbatim` or future variants if necessary
            _ => {
                None
                // Possible option for handling this, but not a good one. We don't want to clutter
                // up our parser with unprocessed and unknown type bounds.
                // let type_id = self.generate_synthetic_type_id("unknown_type_bound"); // Placeholder
                // type_id
            }
        }
    }

    fn process_lifetime_bound(&mut self, bound: &syn::Lifetime) -> String {
        bound.ident.to_string()
    }
    // Move extract_docstring and extract_attributes to attribute_processing.rs
}
