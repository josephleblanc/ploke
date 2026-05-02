use super::state::VisitorState;
use crate::parser::nodes::GenerateTypeId as _;
use crate::parser::types::TypeNode; // Removed unused import: utils::type_to_string
use ploke_core::TypeId;
use ploke_core::TypeKind;
use quote::ToTokens;
use syn::{
    AngleBracketedGenericArguments, GenericArgument, Path, PathArguments, ReturnType, TraitBound,
    Type, TypeParamBound, TypePath, TypeReference,
};

/// Gets or creates a TypeId for a given syn::Type.
/// Ensures that each unique type string within the parsing context maps to one TypeId.
/// If a new TypeId is created, it also processes the type's structure
/// and adds a corresponding TypeNode to the graph.
///
/// # Arguments
/// * `state` - Mutable visitor state containing the type cache and code graph.
/// * `ty` - The syn::Type to get an ID for.
///
/// # Returns
/// The `TypeId` (Synthetic variant in Phase 2) for the given type.
pub(crate) fn get_or_create_type(state: &mut VisitorState, ty: &Type) -> TypeId {
    // --- New Type Processing ---

    // 1. Process the type structure first to get TypeKind and related TypeIds
    //    This handles recursion internally.
    let (type_kind, related_types) = process_type(state, ty);

    // NOTE: within the private module to help prevent the possibility of a given type being
    // generated improperly somewhere. This is more a simple extension of the approach we are
    // currently using for NodeId creation, but it provides some additional safety at little cost
    // and we have a natural extension point if we ever need to make some wrappers around `TypeId`,
    // e.g. if we ever implement lifetime processing or want to distinguish differences in Generic
    // types possibly.
    // 2. Handle TypeId creation
    get_or_create_type_node(state, type_kind, related_types)
}

fn get_or_create_type_node(
    state: &mut VisitorState,
    type_kind: TypeKind,
    related_types: Vec<TypeId>,
) -> TypeId {
    let new_id = state.generate_type_id(&type_kind, &related_types);

    if state.code_graph.type_graph.iter().any(|tn| tn.id == new_id) {
        return new_id;
    }

    let type_node = TypeNode {
        id: new_id,
        kind: type_kind,
        related_types,
    };

    state.code_graph.type_graph.push(type_node);
    new_id
}

fn collect_path_segments_and_related_types(
    state: &mut VisitorState,
    path: &Path,
    related_types: &mut Vec<TypeId>,
) -> Vec<String> {
    path.segments
        .iter()
        .map(|seg| {
            match &seg.arguments {
                PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) => {
                    for arg in args {
                        match arg {
                            GenericArgument::Type(arg_type) => {
                                related_types.push(get_or_create_type(state, arg_type));
                            }
                            GenericArgument::AssocType(assoc_type) => {
                                related_types.push(get_or_create_type(state, &assoc_type.ty));
                            }
                            _ => {}
                        }
                    }
                }
                PathArguments::Parenthesized(parenthesized) => {
                    for input in &parenthesized.inputs {
                        related_types.push(get_or_create_type(state, input));
                    }
                    if let ReturnType::Type(_, return_ty) = &parenthesized.output {
                        related_types.push(get_or_create_type(state, return_ty));
                    }
                }
                PathArguments::None => {}
            }

            seg.ident.to_string()
        })
        .collect()
}

pub(crate) fn get_or_create_trait_bound_type(
    state: &mut VisitorState,
    bound: &TraitBound,
) -> TypeId {
    let mut related_types = Vec::new();
    let path = collect_path_segments_and_related_types(state, &bound.path, &mut related_types);
    let kind = TypeKind::TraitBound {
        path,
        is_fully_qualified: bound.path.leading_colon.is_some(),
    };

    get_or_create_type_node(state, kind, related_types)
}

// Process a type and get its kind and related types
/// Processes the structure of a syn::Type to determine its TypeKind
/// and recursively find the TypeIds of any nested types.
///
/// # Arguments
/// * `state` - Mutable visitor state.
/// * `ty` - The syn::Type to process.
///
/// # Returns
/// A tuple containing the `TypeKind` and a `Vec<TypeId>` of related types.
pub(crate) fn process_type(state: &mut VisitorState, ty: &Type) -> (TypeKind, Vec<TypeId>) {
    let mut related_types = Vec::new();

    match ty {
        Type::Path(TypePath { path, qself }) => {
            let segments = collect_path_segments_and_related_types(state, path, &mut related_types);

            (
                TypeKind::Named {
                    path: segments,
                    is_fully_qualified: qself.is_some(),
                },
                related_types,
            )
        }
        Type::Reference(TypeReference {
            elem,
            lifetime,
            mutability,
            ..
        }) => {
            // Recurse: Get TypeId for the referenced element type
            let elem_id = get_or_create_type(state, elem);
            related_types.push(elem_id); // Store the element's TypeId

            (
                TypeKind::Reference {
                    lifetime: lifetime.as_ref().map(|lt| lt.ident.to_string()),
                    is_mutable: mutability.is_some(),
                },
                related_types, // Contains only elem_id
            )
        }
        Type::Slice(type_slice) => {
            related_types.push(get_or_create_type(state, &type_slice.elem));
            (TypeKind::Slice {}, related_types)
        }
        Type::Array(type_array) => {
            related_types.push(get_or_create_type(state, &type_array.elem));
            (
                TypeKind::Array {
                    size: Some(type_array.len.to_token_stream().to_string()),
                },
                related_types,
            )
        }
        Type::Tuple(type_tuple) => {
            related_types.extend(
                type_tuple
                    .elems
                    .iter()
                    .map(|elem| get_or_create_type(state, elem)),
            );
            (TypeKind::Tuple {}, related_types)
        }
        Type::BareFn(type_bare_fn) => {
            related_types.extend(
                type_bare_fn
                    .inputs
                    .iter()
                    .map(|input| get_or_create_type(state, &input.ty)),
            );
            if let ReturnType::Type(_, return_ty) = &type_bare_fn.output {
                related_types.push(get_or_create_type(state, return_ty));
            }

            (
                TypeKind::Function {
                    is_unsafe: type_bare_fn.unsafety.is_some(),
                    is_extern: type_bare_fn.abi.is_some(),
                    abi: type_bare_fn
                        .abi
                        .as_ref()
                        .and_then(|abi| abi.name.as_ref().map(|name| name.value())),
                },
                related_types,
            )
        }
        Type::Never(_) => (TypeKind::Never, related_types),
        Type::Infer(_) => (TypeKind::Inferred, related_types),
        Type::Ptr(type_ptr) => {
            related_types.push(get_or_create_type(state, &type_ptr.elem));
            (
                TypeKind::RawPointer {
                    is_mutable: type_ptr.mutability.is_some(),
                },
                related_types,
            )
        }
        Type::TraitObject(type_trait_object) => {
            related_types.extend(
                type_trait_object
                    .bounds
                    .iter()
                    .filter_map(|bound| match bound {
                        TypeParamBound::Trait(trait_bound) => {
                            Some(get_or_create_trait_bound_type(state, trait_bound))
                        }
                        TypeParamBound::Lifetime(_) => None,
                        _ => None,
                    }),
            );

            (
                TypeKind::TraitObject {
                    dyn_token: type_trait_object.dyn_token.is_some(),
                },
                related_types,
            )
        }
        Type::ImplTrait(type_impl_trait) => {
            related_types.extend(
                type_impl_trait
                    .bounds
                    .iter()
                    .filter_map(|bound| match bound {
                        TypeParamBound::Trait(trait_bound) => {
                            Some(get_or_create_trait_bound_type(state, trait_bound))
                        }
                        TypeParamBound::Lifetime(_) => None,
                        _ => None,
                    }),
            );

            (TypeKind::ImplTrait {}, related_types)
        }
        Type::Paren(type_paren) => {
            related_types.push(get_or_create_type(state, &type_paren.elem));
            (TypeKind::Paren {}, related_types)
        }
        Type::Macro(type_macro) => (
            TypeKind::Macro {
                name: type_macro.mac.path.to_token_stream().to_string(),
                tokens: type_macro.mac.tokens.to_string(),
            },
            related_types,
        ),
        Type::Group(type_group) => process_type(state, &type_group.elem),
        _ => {
            // Handle other types or unknown types
            // Use the string representation we already have from the caller
            // (get_or_create_type) if possible, or re-generate if needed.
            // TODO: Distinguish between "unknown" and "unsupported" types
            let fallback_str = ty.to_token_stream().to_string();
            (
                TypeKind::Unknown {
                    type_str: fallback_str,
                },
                Vec::new(), // No known related types
            )
        }
    }
}
