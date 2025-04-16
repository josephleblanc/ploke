use super::state::VisitorState;
use crate::parser::types::TypeNode; // Removed unused import: utils::type_to_string
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
    // --- New Type Processing ---

    // 1. Process the type structure first to get TypeKind and related TypeIds
    //    This handles recursion internally.
    let (type_kind, related_types) = process_type(state, ty);

    // 2. Get the current parent scope ID from the state
    let parent_scope_id = state.current_definition_scope.last().copied();

    // 3. Generate the new Synthetic Type ID using structural info AND parent scope
    let new_id = TypeId::generate_synthetic(
        state.crate_namespace,
        &state.current_file_path,
        &type_kind,     // Pass the determined TypeKind
        &related_types, // Pass the determined related TypeIds
        parent_scope_id, // Pass the parent scope ID
    );

    // 4. Check if a TypeNode with this ID already exists (handles recursion/cycles)
    //    We avoid adding duplicate TypeNodes.
    // NOTE: Might be slightly more efficient to reverse the iter here. Try benchmarking someday.
    if state.code_graph.type_graph.iter().any(|tn| tn.id == new_id) {
        return new_id; // Already processed and added due to recursion
    }

    // 5. Create the TypeNode containing the structural information if it's new
    let type_node = TypeNode {
        id: new_id, // The newly generated ID
        kind: type_kind,
        related_types,
        // span: Option? If we want to store the span of the first encounter? Maybe later.
    };

    // 6. Add the new TypeNode to the graph
    state.code_graph.type_graph.push(type_node);

    // 7. Return the newly generated ID
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
            // Check if it's a simple path like "Self" or "T" (potential generic/self)
            // For now, treat these like any other named path for TypeKind generation.
            // Contextual disambiguation is deferred.
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
