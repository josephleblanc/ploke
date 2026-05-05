use super::state::VisitorState;
use crate::parser::nodes::GenerateTypeId as _;
#[cfg(feature = "typed_type_graph")]
use crate::parser::nodes::{
    AnyTypeId, ArrayTypeId, FunctionTypeId, ImplTraitTypeId, InferredTypeId, MacroTypeId,
    NamedTypeId, NeverTypeId, ParenTypeId, RawPointerTypeId, ReferenceTypeId, SliceTypeId,
    StructuralTypeId as _, TraitBoundTypeId, TraitObjectTypeId, TraitTypeSourceId, TupleTypeId,
    UnknownTypeId,
};
#[cfg(feature = "typed_type_graph")]
use crate::parser::type_nodes::{
    ArrayTypeNode, FunctionTypeNode, ImplTraitTypeNode, InferredTypeNode, MacroTypeNode,
    NamedTypeNode, NeverTypeNode, ParenTypeNode, RawPointerTypeNode, ReferenceTypeNode,
    SliceTypeNode, TraitBoundTypeNode, TraitObjectTypeNode, TupleTypeNode,
    TypeNode as TypedTypeNode, UnknownTypeNode,
};
use crate::parser::type_slots::{OrdinaryTypeUseId, TraitTypeUseId};
#[cfg(not(feature = "typed_type_graph"))]
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
#[cfg(not(feature = "typed_type_graph"))]
pub(crate) fn get_or_create_type(state: &mut VisitorState, ty: &Type) -> OrdinaryTypeUseId {
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

#[cfg(feature = "typed_type_graph")]
pub(crate) fn get_or_create_type(state: &mut VisitorState, ty: &Type) -> OrdinaryTypeUseId {
    process_typed_type(state, ty)
}

#[cfg(not(feature = "typed_type_graph"))]
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

#[cfg(feature = "typed_type_graph")]
fn insert_typed_type_node(state: &mut VisitorState, type_node: TypedTypeNode) -> AnyTypeId {
    let type_id = type_node.id();
    if state
        .code_graph
        .type_graph
        .iter()
        .any(|tn| tn.id() == type_id)
    {
        return type_id;
    }

    state.code_graph.type_graph.push(type_node);
    type_id
}

#[cfg(feature = "typed_type_graph")]
fn related_base_ids<T>(related_types: &[T]) -> Vec<TypeId>
where
    T: Copy + crate::parser::nodes::StructuralTypeId,
{
    related_types.iter().copied().map(T::base_id).collect()
}

#[cfg(feature = "typed_type_graph")]
fn collect_path_segments_and_type_arguments(
    state: &mut VisitorState,
    path: &Path,
    arguments: &mut Vec<OrdinaryTypeUseId>,
) -> Vec<String> {
    path.segments
        .iter()
        .map(|seg| {
            match &seg.arguments {
                PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) => {
                    for arg in args {
                        match arg {
                            GenericArgument::Type(arg_type) => {
                                arguments.push(get_or_create_type(state, arg_type));
                            }
                            GenericArgument::AssocType(assoc_type) => {
                                arguments.push(get_or_create_type(state, &assoc_type.ty));
                            }
                            _ => {}
                        }
                    }
                }
                PathArguments::Parenthesized(parenthesized) => {
                    for input in &parenthesized.inputs {
                        arguments.push(get_or_create_type(state, input));
                    }
                    if let ReturnType::Type(_, return_ty) = &parenthesized.output {
                        arguments.push(get_or_create_type(state, return_ty));
                    }
                }
                PathArguments::None => {}
            }

            seg.ident.to_string()
        })
        .collect()
}

#[cfg(feature = "typed_type_graph")]
fn trait_source_from_ordinary(type_id: OrdinaryTypeUseId) -> TraitTypeUseId {
    match type_id {
        OrdinaryTypeUseId::Named(id) => TraitTypeSourceId::from(id),
        _ => panic!("type id {type_id} is not admissible in trait position"),
    }
}

#[cfg(feature = "typed_type_graph")]
pub(crate) fn get_or_create_trait_type(state: &mut VisitorState, ty: &Type) -> TraitTypeUseId {
    trait_source_from_ordinary(get_or_create_type(state, ty))
}

#[cfg(not(feature = "typed_type_graph"))]
pub(crate) fn get_or_create_trait_type(state: &mut VisitorState, ty: &Type) -> TraitTypeUseId {
    get_or_create_type(state, ty)
}

#[cfg(not(feature = "typed_type_graph"))]
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

#[cfg(not(feature = "typed_type_graph"))]
pub(crate) fn get_or_create_trait_bound_type(
    state: &mut VisitorState,
    bound: &TraitBound,
) -> TraitTypeUseId {
    let mut related_types = Vec::new();
    let path = collect_path_segments_and_related_types(state, &bound.path, &mut related_types);
    let kind = TypeKind::TraitBound {
        path,
        is_fully_qualified: bound.path.leading_colon.is_some(),
    };

    get_or_create_type_node(state, kind, related_types)
}

#[cfg(feature = "typed_type_graph")]
pub(crate) fn get_or_create_trait_bound_type(
    state: &mut VisitorState,
    bound: &TraitBound,
) -> TraitTypeUseId {
    let mut arguments = Vec::new();
    let path = collect_path_segments_and_type_arguments(state, &bound.path, &mut arguments);
    let kind = TypeKind::TraitBound {
        path: path.clone(),
        is_fully_qualified: bound.path.leading_colon.is_some(),
    };
    let related = related_base_ids(&arguments);
    let id = TraitBoundTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
        .expect("trait-bound TypeKind must refine to TraitBoundTypeId");

    let node = TraitBoundTypeNode {
        id,
        path,
        is_fully_qualified: bound.path.leading_colon.is_some(),
        arguments,
    };
    let _ = insert_typed_type_node(state, node.into());
    TraitTypeSourceId::from(id)
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
#[cfg(not(feature = "typed_type_graph"))]
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

#[cfg(feature = "typed_type_graph")]
fn process_typed_type(state: &mut VisitorState, ty: &Type) -> OrdinaryTypeUseId {
    match ty {
        Type::Path(TypePath { path, qself }) => {
            let mut arguments = Vec::new();
            let segments = collect_path_segments_and_type_arguments(state, path, &mut arguments);
            let kind = TypeKind::Named {
                path: segments.clone(),
                is_fully_qualified: qself.is_some(),
            };
            let related = related_base_ids(&arguments);
            let id = NamedTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("named TypeKind must refine to NamedTypeId");

            let _ = insert_typed_type_node(
                state,
                NamedTypeNode {
                    id,
                    path: segments,
                    is_fully_qualified: qself.is_some(),
                    arguments,
                }
                .into(),
            );
            id.into()
        }
        Type::Reference(TypeReference {
            elem,
            lifetime,
            mutability,
            ..
        }) => {
            let referenced = get_or_create_type(state, elem);
            let kind = TypeKind::Reference {
                lifetime: lifetime.as_ref().map(|lt| lt.ident.to_string()),
                is_mutable: mutability.is_some(),
            };
            let related = [referenced.base_id()];
            let id = ReferenceTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("reference TypeKind must refine to ReferenceTypeId");

            let _ = insert_typed_type_node(
                state,
                ReferenceTypeNode {
                    id,
                    lifetime: lifetime.as_ref().map(|lt| lt.ident.to_string()),
                    is_mutable: mutability.is_some(),
                    referenced,
                }
                .into(),
            );
            id.into()
        }
        Type::Slice(type_slice) => {
            let element = get_or_create_type(state, &type_slice.elem);
            let kind = TypeKind::Slice {};
            let related = [element.base_id()];
            let id = SliceTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("slice TypeKind must refine to SliceTypeId");
            let _ = insert_typed_type_node(state, SliceTypeNode { id, element }.into());
            id.into()
        }
        Type::Array(type_array) => {
            let element = get_or_create_type(state, &type_array.elem);
            let size = Some(type_array.len.to_token_stream().to_string());
            let kind = TypeKind::Array { size: size.clone() };
            let related = [element.base_id()];
            let id = ArrayTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("array TypeKind must refine to ArrayTypeId");
            let _ = insert_typed_type_node(state, ArrayTypeNode { id, element, size }.into());
            id.into()
        }
        Type::Tuple(type_tuple) => {
            let elements: Vec<_> = type_tuple
                .elems
                .iter()
                .map(|elem| get_or_create_type(state, elem))
                .collect();
            let kind = TypeKind::Tuple {};
            let related = related_base_ids(&elements);
            let id = TupleTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("tuple TypeKind must refine to TupleTypeId");
            let _ = insert_typed_type_node(state, TupleTypeNode { id, elements }.into());
            id.into()
        }
        Type::BareFn(type_bare_fn) => {
            let parameters: Vec<_> = type_bare_fn
                .inputs
                .iter()
                .map(|input| get_or_create_type(state, &input.ty))
                .collect();
            let return_type = match &type_bare_fn.output {
                ReturnType::Type(_, return_ty) => Some(get_or_create_type(state, return_ty)),
                ReturnType::Default => None,
            };
            let kind = TypeKind::Function {
                is_unsafe: type_bare_fn.unsafety.is_some(),
                is_extern: type_bare_fn.abi.is_some(),
                abi: type_bare_fn
                    .abi
                    .as_ref()
                    .and_then(|abi| abi.name.as_ref().map(|name| name.value())),
            };
            let related: Vec<_> = parameters
                .iter()
                .copied()
                .chain(return_type)
                .map(|id| id.base_id())
                .collect();
            let id = FunctionTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("function TypeKind must refine to FunctionTypeId");
            let _ = insert_typed_type_node(
                state,
                FunctionTypeNode {
                    id,
                    parameters,
                    return_type,
                    is_unsafe: type_bare_fn.unsafety.is_some(),
                    is_extern: type_bare_fn.abi.is_some(),
                    abi: type_bare_fn
                        .abi
                        .as_ref()
                        .and_then(|abi| abi.name.as_ref().map(|name| name.value())),
                }
                .into(),
            );
            id.into()
        }
        Type::Never(_) => {
            let kind = TypeKind::Never;
            let id = NeverTypeId::try_refine(state.generate_type_id(&kind, &[]), &kind)
                .expect("never TypeKind must refine to NeverTypeId");
            let _ = insert_typed_type_node(state, NeverTypeNode { id }.into());
            id.into()
        }
        Type::Infer(_) => {
            let kind = TypeKind::Inferred;
            let id = InferredTypeId::try_refine(state.generate_type_id(&kind, &[]), &kind)
                .expect("inferred TypeKind must refine to InferredTypeId");
            let _ = insert_typed_type_node(state, InferredTypeNode { id }.into());
            id.into()
        }
        Type::Ptr(type_ptr) => {
            let pointee = get_or_create_type(state, &type_ptr.elem);
            let kind = TypeKind::RawPointer {
                is_mutable: type_ptr.mutability.is_some(),
            };
            let related = [pointee.base_id()];
            let id = RawPointerTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("raw pointer TypeKind must refine to RawPointerTypeId");
            let _ = insert_typed_type_node(
                state,
                RawPointerTypeNode {
                    id,
                    is_mutable: type_ptr.mutability.is_some(),
                    pointee,
                }
                .into(),
            );
            id.into()
        }
        Type::TraitObject(type_trait_object) => {
            let bounds: Vec<_> = type_trait_object
                .bounds
                .iter()
                .filter_map(|bound| match bound {
                    TypeParamBound::Trait(trait_bound) => {
                        Some(get_or_create_trait_bound_type(state, trait_bound))
                    }
                    TypeParamBound::Lifetime(_) => None,
                    _ => None,
                })
                .collect();
            let kind = TypeKind::TraitObject {
                dyn_token: type_trait_object.dyn_token.is_some(),
            };
            let related = related_base_ids(&bounds);
            let id = TraitObjectTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("trait object TypeKind must refine to TraitObjectTypeId");
            let _ = insert_typed_type_node(
                state,
                TraitObjectTypeNode {
                    id,
                    dyn_token: type_trait_object.dyn_token.is_some(),
                    bounds,
                }
                .into(),
            );
            id.into()
        }
        Type::ImplTrait(type_impl_trait) => {
            let bounds: Vec<_> = type_impl_trait
                .bounds
                .iter()
                .filter_map(|bound| match bound {
                    TypeParamBound::Trait(trait_bound) => {
                        Some(get_or_create_trait_bound_type(state, trait_bound))
                    }
                    TypeParamBound::Lifetime(_) => None,
                    _ => None,
                })
                .collect();
            let kind = TypeKind::ImplTrait {};
            let related = related_base_ids(&bounds);
            let id = ImplTraitTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("impl Trait TypeKind must refine to ImplTraitTypeId");
            let _ = insert_typed_type_node(state, ImplTraitTypeNode { id, bounds }.into());
            id.into()
        }
        Type::Paren(type_paren) => {
            let inner = get_or_create_type(state, &type_paren.elem);
            let kind = TypeKind::Paren {};
            let related = [inner.base_id()];
            let id = ParenTypeId::try_refine(state.generate_type_id(&kind, &related), &kind)
                .expect("paren TypeKind must refine to ParenTypeId");
            let _ = insert_typed_type_node(state, ParenTypeNode { id, inner }.into());
            id.into()
        }
        Type::Macro(type_macro) => {
            let name = type_macro.mac.path.to_token_stream().to_string();
            let tokens = type_macro.mac.tokens.to_string();
            let kind = TypeKind::Macro {
                name: name.clone(),
                tokens: tokens.clone(),
            };
            let id = MacroTypeId::try_refine(state.generate_type_id(&kind, &[]), &kind)
                .expect("macro TypeKind must refine to MacroTypeId");
            let _ = insert_typed_type_node(state, MacroTypeNode { id, name, tokens }.into());
            id.into()
        }
        Type::Group(type_group) => process_typed_type(state, &type_group.elem),
        _ => {
            let type_str = ty.to_token_stream().to_string();
            let kind = TypeKind::Unknown {
                type_str: type_str.clone(),
            };
            let id = UnknownTypeId::try_refine(state.generate_type_id(&kind, &[]), &kind)
                .expect("unknown TypeKind must refine to UnknownTypeId");
            let _ = insert_typed_type_node(state, UnknownTypeNode { id, type_str }.into());
            id.into()
        }
    }
}

#[cfg(all(test, feature = "typed_type_graph"))]
mod typed_tests {
    use super::*;
    use crate::discovery::{CrateContext, Dependencies, DevDependencies, Features};
    use crate::parser::nodes::{GeneratesAnyNodeId as _, ModuleNodeId};
    use ploke_core::ItemKind;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_state() -> VisitorState {
        let namespace = Uuid::new_v4();
        let root = PathBuf::from("/tmp/typed-type-graph-test");
        let context = CrateContext {
            name: "typed_type_graph_test".into(),
            version: "0.0.0".into(),
            namespace,
            root_path: root.clone(),
            files: vec![root.join("src/lib.rs")],
            targets: Vec::new(),
            features: Features::default(),
            dependencies: Dependencies::default(),
            dev_dependencies: DevDependencies::default(),
            workspace_path: None,
        };
        let mut state = VisitorState::new(namespace, root.join("src/lib.rs"), &context);
        let module_any = state.generate_synthetic_node_id("crate", ItemKind::Module, None);
        let module_id: ModuleNodeId = module_any.try_into().expect("module id");
        state.current_primary_defn_scope.push(module_id.into());
        state
    }

    #[test]
    fn constructs_typed_named_type_with_typed_argument() {
        let mut state = test_state();
        let ty: Type = syn::parse_str("Vec<Result<String, Error>>").expect("type syntax");

        let root_id = get_or_create_type(&mut state, &ty);
        let root = state
            .code_graph
            .type_graph
            .iter()
            .find(|node| node.id() == root_id.into())
            .expect("root type node");

        let TypedTypeNode::Named(named) = root else {
            panic!("expected named root type");
        };
        assert_eq!(named.path, vec!["Vec"]);
        assert_eq!(named.arguments.len(), 1);
        assert_eq!(state.code_graph.type_graph.len(), 4);
    }

    #[test]
    fn constructs_reference_node_with_typed_child_id() {
        let mut state = test_state();
        let ty: Type = syn::parse_str("&'a mut Foo").expect("type syntax");

        let root_id = get_or_create_type(&mut state, &ty);
        let root = state
            .code_graph
            .type_graph
            .iter()
            .find(|node| node.id() == root_id.into())
            .expect("root type node");

        let TypedTypeNode::Reference(reference) = root else {
            panic!("expected reference root type");
        };
        assert_eq!(reference.lifetime.as_deref(), Some("a"));
        assert!(reference.is_mutable);
        assert!(matches!(reference.referenced, OrdinaryTypeUseId::Named(_)));
    }

    #[test]
    fn trait_object_stores_narrow_children_and_widens_for_traversal() {
        let mut state = test_state();
        let ty: Type = syn::parse_str("dyn Iterator<Item = Vec<Foo>> + Send").expect("type syntax");

        // This example has three different syntactic positions:
        //
        // - `dyn Iterator<Item = Vec<Foo>> + Send` is the whole type expression.
        //   Syn parses it as `Type::TraitObject`, so it is an ordinary type use
        //   at the parser-node slot boundary.
        //
        // - `Iterator<Item = Vec<Foo>>` and `Send` are bounds inside that trait
        //   object. Syn parses them as `TypeParamBound::Trait`, so they inhabit
        //   the trait-position family.
        //
        // - `Vec<Foo>` is the associated type value for `Iterator::Item`. Syn
        //   parses that value as ordinary `Type::Path` syntax, so it inhabits
        //   the ordinary type-use family even though it is nested under a trait
        //   bound.
        let root_id = get_or_create_type(&mut state, &ty);
        let root = state
            .code_graph
            .type_graph
            .iter()
            .find(|node| node.id() == root_id.into())
            .expect("root type node");

        let TypedTypeNode::TraitObject(trait_object) = root else {
            panic!("expected trait object root type");
        };
        assert_eq!(trait_object.bounds.len(), 2);

        // The trait object stores the two bounds in the narrow trait-position
        // family. These are not ordinary type-use roots, even though the
        // containing `dyn ...` expression is an ordinary type use.
        assert!(
            trait_object
                .bounds
                .iter()
                .all(|bound| matches!(bound, TraitTypeSourceId::TraitBound(_)))
        );

        // Find the `Iterator<Item = Vec<Foo>>` bound so we can inspect the
        // nested associated type value. This checks the distinction between
        // the trait-position bound itself and the ordinary type syntax inside
        // its generic/associated arguments.
        let iterator_bound =
            trait_object
                .bounds
                .iter()
                .copied()
                .find_map(|bound| match bound {
                    TraitTypeSourceId::TraitBound(id) => state
                        .code_graph
                        .type_graph
                        .iter()
                        .find_map(|node| match node {
                            TypedTypeNode::TraitBound(node)
                                if node.id == id && node.path == ["Iterator"] =>
                            {
                                Some(node)
                            }
                            _ => None,
                        }),
                    _ => None,
                })
                .expect("Iterator trait bound node");

        assert_eq!(iterator_bound.arguments.len(), 1);

        // `Vec<Foo>` is stored as `OrdinaryTypeUseId::Named(_)`, not as a trait
        // use. The fact that it appears inside `Iterator<...>` does not make
        // it a trait-position bound; the syntactic position of the argument
        // controls the family.
        assert!(matches!(
            iterator_bound.arguments[0],
            OrdinaryTypeUseId::Named(_)
        ));

        // Traversal is the first point where these narrow child families are
        // widened into `AnyTypeId`. For the root trait-object node, that means
        // its two stored `TraitTypeSourceId` bounds project to
        // `AnyTypeId::TraitBound(_)` children.
        let widened_children: Vec<_> = root.child_type_ids().collect();
        assert!(
            widened_children
                .iter()
                .all(|child| matches!(child, AnyTypeId::TraitBound(_)))
        );
    }
}
