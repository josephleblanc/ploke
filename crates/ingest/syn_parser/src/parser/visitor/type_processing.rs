use super::state::VisitorState;
use crate::parser::types::*;
use quote::ToTokens;
use syn::{
    spanned::Spanned, AngleBracketedGenericArguments, GenericArgument, PathArguments, ReturnType,
    Type, TypePath, TypeReference,
};

// Get or create a type ID
pub(crate) fn get_or_create_type(state: &mut VisitorState, ty: &Type) -> TypeId {
    // Convert type to a string representation for caching
    let type_str = ty.to_token_stream().to_string();

    // First check if the type already exists
    if let Some(entry) = state.type_map.get(&type_str) {
        let id = *entry.value();
        drop(entry); // Explicitly drop the reference to release the borrow
        return id;
    }

    // Process the type to get its kind and related types
    let (type_kind, related_types) = process_type(state, ty);

    // Create a new type ID
    let id = state.next_type_id();

    // Insert the new type ID into the map
    state.type_map.insert(type_str.clone(), id);

    // Add the type to the graph
    state.code_graph.type_graph.push(TypeNode {
        id,
        kind: type_kind,
        related_types,
        // span: (ty.span().byte_range().start, ty.span().byte_range().end),
    });

    id
}

// Process a type and get its kind and related types
pub(crate) fn process_type(state: &mut VisitorState, ty: &Type) -> (TypeKind, Vec<TypeId>) {
    let mut related_types = Vec::new();

    match ty {
        Type::Path(TypePath { path, qself }) => {
            let segments: Vec<String> = path
                .segments
                .iter()
                .map(|seg| {
                    // Process generic arguments if any
                    if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        args,
                        ..
                    }) = &seg.arguments
                    {
                        for arg in args {
                            match arg {
                                GenericArgument::Type(arg_type) => {
                                    related_types.push(get_or_create_type(state, arg_type));
                                }
                                GenericArgument::AssocType(assoc_type) => {
                                    related_types.push(get_or_create_type(state, &assoc_type.ty));
                                }
                                // Process other generic arguments if needed
                                _ => {}
                            }
                        }
                    } else if let PathArguments::Parenthesized(parenthesized) = &seg.arguments {
                        // Handle function pointer types like Fn(Args) -> Return
                        for input in &parenthesized.inputs {
                            related_types.push(get_or_create_type(state, input));
                        }
                        if let ReturnType::Type(_, return_ty) = &parenthesized.output {
                            related_types.push(get_or_create_type(state, return_ty));
                        }
                    }

                    seg.ident.to_string()
                })
                .collect();

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
            let elem_id = get_or_create_type(state, elem);
            (
                TypeKind::Reference {
                    lifetime: lifetime.as_ref().map(|lt| lt.ident.to_string()),
                    is_mutable: mutability.is_some(),
                },
                vec![elem_id],
            )
        }
        // ... include all the other type processing cases from the original file
        // This is where most of the code from process_type would go
        _ => {
            // Handle other types or unknown types
            (
                TypeKind::Unknown {
                    type_str: ty.to_token_stream().to_string(),
                },
                Vec::new(),
            )
        }
    }
}
