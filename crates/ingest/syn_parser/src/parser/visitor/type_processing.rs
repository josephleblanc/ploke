use super::state::VisitorState;
use crate::parser::{types::TypeNode, utils::type_to_string}; // Includes TypeNode, TypeKind, TypeId (enum)
use ploke_core::TypeId;
use ploke_core::TypeKind;
use quote::ToTokens;
use syn::{
    AngleBracketedGenericArguments, GenericArgument, PathArguments, ReturnType, Type, TypePath,
    TypeReference,
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
// NOTE: Known Issues:
// * It is unclear exactly how the `syn` crate will handle "Self" types, which may not resolve
// as we are hoping (into different types).
// * Possible logical error: Two Structs with `impl` blocks in the same file that both use `self`
// or `Self` types (not sure) may result in the same `type_to_string` and therefore the same
// `TypeId`.
//   * Duplicate TypeId == Bad
pub(crate) fn get_or_create_type(state: &mut VisitorState, ty: &Type) -> TypeId {
    let type_str = type_to_string(ty);
    // --- Cache Check ---
    // Check if the type string already exists in the map
    if let Some(entry) = state.type_map.get(&type_str) {
        let id = *entry.value();
        // Drop the dashmap ref explicitly before returning if needed, though often implicit drop is fine.
        // drop(entry);
        return id;
    }

    // --- New Type Processing ---

    // 1. Generate the new Synthetic Type ID using the string representation
    let new_id =
        TypeId::generate_synthetic(state.crate_namespace, &state.current_file_path, &type_str);

    // 2. **Crucial:** Insert the new ID into the cache *before* recursive processing
    //    to handle potential cycles (e.g., struct Foo(Box<Foo>)).
    state.type_map.insert(type_str.to_string(), new_id);

    // 3. Process the type structure (recursively calls this function for nested types)
    //    This determines the TypeKind and finds related TypeIds.
    let (type_kind, related_types) = process_type(state, ty); // Pass only state and ty

    // 4. Create the TypeNode containing the structural information
    let type_node = TypeNode {
        id: new_id, // The newly generated ID
        kind: type_kind,
        related_types,
        // span: Option? If we want to store the span of the first encounter? Maybe later.
    };

    // 5. Add the new TypeNode to the graph
    state.code_graph.type_graph.push(type_node);

    // 6. Return the newly generated ID
    new_id
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
                                    // Recurse: Get TypeId for the generic argument type
                                    related_types.push(get_or_create_type(state, arg_type));
                                }
                                GenericArgument::AssocType(assoc_type) => {
                                    // Recurse: Get TypeId for the associated type
                                    let assoc_type_ty = &assoc_type.ty;
                                    related_types.push(get_or_create_type(state, assoc_type_ty));
                                }
                                // TODO: Handle Lifetime and Const generic arguments if needed
                                _ => {}
                            }
                        }
                    } else if let PathArguments::Parenthesized(parenthesized) = &seg.arguments {
                        // Handle function pointer types like Fn(Args) -> Return
                        for input in &parenthesized.inputs {
                            // Recurse: Get TypeId for input types
                            related_types.push(get_or_create_type(state, input));
                        }
                        if let ReturnType::Type(_, return_ty) = &parenthesized.output {
                            // Recurse: Get TypeId for return type
                            related_types.push(get_or_create_type(state, return_ty));
                        }
                    }
                    // TODO: Handle `PathArguments::None` if necessary

                    seg.ident.to_string()
                })
                .collect();
            // Removed the problematic `ends_with` check and the associated comments.

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
        // --- Add other Type::* cases here ---
        // e.g., Type::Tuple, Type::Slice, Type::Array, Type::Ptr, etc.
        // Each case should:
        // 1. Identify nested types (like tuple elements, array/slice element type).
        // 2. For each nested type:
        //    a. Call `get_or_create_type()` with the nested type and its string.
        //    b. Push the returned TypeId into `related_types`.
        // 3. Construct the appropriate `TypeKind` variant.
        // 4. Return `(type_kind, related_types)`.

        // --- Fallback Case ---
        _ => {
            // Handle other types or unknown types
            // Use the string representation we already have from the caller
            // (get_or_create_type) if possible, or re-generate if needed.
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
