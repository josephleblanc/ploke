use cozo::{Db, MemStorage};
use syn_parser::parser::{
    graph::CodeGraph,
    nodes::{AnyNodeId, AsAnyNodeId, ToCozoUuid, TypeDefNode},
    type_nodes::TypeNode,
    types::{GenericParamKind, GenericParamNode, TypeWherePredicate},
};

use crate::{
    error::TransformError,
    schema::edges::{TypeContainsSchema, TypeUseSchema},
};

#[derive(Debug, Clone, Copy)]
enum TypeUseRole {
    FunctionParam,
    FunctionReturn,
    MethodParam,
    MethodReturn,
    FieldType,
    TypeAliasTarget,
    ImplSelf,
    ImplTrait,
    TraitSuper,
    AssociatedTypeBound,
    ConstType,
    StaticType,
    GenericBound,
    GenericParamBound,
    WherePredicateSubject,
    WherePredicateBound,
    WhereGenericParamBound,
}

impl TypeUseRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::FunctionParam => "FunctionParam",
            Self::FunctionReturn => "FunctionReturn",
            Self::MethodParam => "MethodParam",
            Self::MethodReturn => "MethodReturn",
            Self::FieldType => "FieldType",
            Self::TypeAliasTarget => "TypeAliasTarget",
            Self::ImplSelf => "ImplSelf",
            Self::ImplTrait => "ImplTrait",
            Self::TraitSuper => "TraitSuper",
            Self::AssociatedTypeBound => "AssociatedTypeBound",
            Self::ConstType => "ConstType",
            Self::StaticType => "StaticType",
            Self::GenericBound => "GenericBound",
            Self::GenericParamBound => "GenericParamBound",
            Self::WherePredicateSubject => "WherePredicateSubject",
            Self::WherePredicateBound => "WherePredicateBound",
            Self::WhereGenericParamBound => "WhereGenericParamBound",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TypeContainmentKind {
    Argument,
    Element,
    Referenced,
    Pointee,
    TraitBound,
    QualifiedSelf,
    QualifiedTrait,
    Inner,
    FunctionParam,
    FunctionReturn,
}

impl TypeContainmentKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Argument => "Argument",
            Self::Element => "Element",
            Self::Referenced => "Referenced",
            Self::Pointee => "Pointee",
            Self::TraitBound => "TraitBound",
            Self::QualifiedSelf => "QualifiedSelf",
            Self::QualifiedTrait => "QualifiedTrait",
            Self::Inner => "Inner",
            Self::FunctionParam => "FunctionParam",
            Self::FunctionReturn => "FunctionReturn",
        }
    }
}

pub(super) fn transform_type_graph_edges(
    db: &Db<MemStorage>,
    graph: &CodeGraph,
) -> Result<(), TransformError> {
    transform_type_uses(db, graph)?;
    transform_type_contains(db, &graph.type_graph)?;
    Ok(())
}

fn transform_type_uses(db: &Db<MemStorage>, graph: &CodeGraph) -> Result<(), TransformError> {
    for function in &graph.functions {
        transform_generic_bound_type_uses(db, function.id.as_any(), &function.generic_params)?;
        transform_where_predicate_type_uses(
            db,
            function.id.as_any(),
            &function.generic_params,
            &function.where_predicates,
            &graph.type_graph,
        )?;
        for (idx, param) in function.parameters.iter().enumerate() {
            insert_type_use(
                db,
                function.id,
                param.type_id,
                TypeUseRole::FunctionParam,
                Some(idx),
            )?;
        }
        if let Some(return_type) = function.return_type {
            insert_type_use(
                db,
                function.id,
                return_type,
                TypeUseRole::FunctionReturn,
                None,
            )?;
        }
    }

    for defined_type in &graph.defined_types {
        match defined_type {
            TypeDefNode::Struct(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                transform_where_predicate_type_uses(
                    db,
                    node.id.as_any(),
                    &node.generic_params,
                    &node.where_predicates,
                    &graph.type_graph,
                )?;
                for (idx, field) in node.fields.iter().enumerate() {
                    insert_type_use(
                        db,
                        field.id,
                        field.type_id,
                        TypeUseRole::FieldType,
                        Some(idx),
                    )?;
                }
            }
            TypeDefNode::Enum(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                transform_where_predicate_type_uses(
                    db,
                    node.id.as_any(),
                    &node.generic_params,
                    &node.where_predicates,
                    &graph.type_graph,
                )?;
                for variant in &node.variants {
                    for (idx, field) in variant.fields.iter().enumerate() {
                        insert_type_use(
                            db,
                            field.id,
                            field.type_id,
                            TypeUseRole::FieldType,
                            Some(idx),
                        )?;
                    }
                }
            }
            TypeDefNode::TypeAlias(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                transform_where_predicate_type_uses(
                    db,
                    node.id.as_any(),
                    &node.generic_params,
                    &node.where_predicates,
                    &graph.type_graph,
                )?;
                insert_type_use(
                    db,
                    node.id,
                    node.type_id,
                    TypeUseRole::TypeAliasTarget,
                    None,
                )?;
            }
            TypeDefNode::Union(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                transform_where_predicate_type_uses(
                    db,
                    node.id.as_any(),
                    &node.generic_params,
                    &node.where_predicates,
                    &graph.type_graph,
                )?;
                for (idx, field) in node.fields.iter().enumerate() {
                    insert_type_use(
                        db,
                        field.id,
                        field.type_id,
                        TypeUseRole::FieldType,
                        Some(idx),
                    )?;
                }
            }
        }
    }

    for trait_node in &graph.traits {
        transform_generic_bound_type_uses(db, trait_node.id.as_any(), &trait_node.generic_params)?;
        transform_where_predicate_type_uses(
            db,
            trait_node.id.as_any(),
            &trait_node.generic_params,
            &trait_node.where_predicates,
            &graph.type_graph,
        )?;
        for (idx, super_trait) in trait_node.super_traits.iter().copied().enumerate() {
            insert_type_use(
                db,
                trait_node.id,
                super_trait,
                TypeUseRole::TraitSuper,
                Some(idx),
            )?;
        }
        for (idx, bound) in trait_node
            .associated_type_bounds
            .iter()
            .copied()
            .enumerate()
        {
            insert_type_use(
                db,
                trait_node.id,
                bound,
                TypeUseRole::AssociatedTypeBound,
                Some(idx),
            )?;
        }
        for method in &trait_node.methods {
            transform_method_type_uses(db, method, &graph.type_graph)?;
        }
    }

    for impl_node in &graph.impls {
        transform_generic_bound_type_uses(db, impl_node.id.as_any(), &impl_node.generic_params)?;
        transform_where_predicate_type_uses(
            db,
            impl_node.id.as_any(),
            &impl_node.generic_params,
            &impl_node.where_predicates,
            &graph.type_graph,
        )?;
        insert_type_use(
            db,
            impl_node.id,
            impl_node.self_type,
            TypeUseRole::ImplSelf,
            None,
        )?;
        if let Some(trait_type) = impl_node.trait_type {
            insert_type_use(db, impl_node.id, trait_type, TypeUseRole::ImplTrait, None)?;
        }
        for method in &impl_node.methods {
            transform_method_type_uses(db, method, &graph.type_graph)?;
        }
    }

    for const_node in &graph.consts {
        insert_type_use(
            db,
            const_node.id,
            const_node.type_id,
            TypeUseRole::ConstType,
            None,
        )?;
    }

    for static_node in &graph.statics {
        insert_type_use(
            db,
            static_node.id,
            static_node.type_id,
            TypeUseRole::StaticType,
            None,
        )?;
    }

    Ok(())
}

fn transform_method_type_uses(
    db: &Db<MemStorage>,
    method: &syn_parser::parser::nodes::MethodNode,
    type_graph: &[TypeNode],
) -> Result<(), TransformError> {
    transform_generic_bound_type_uses(db, method.id.as_any(), &method.generic_params)?;
    transform_where_predicate_type_uses(
        db,
        method.id.as_any(),
        &method.generic_params,
        &method.where_predicates,
        type_graph,
    )?;
    for (idx, param) in method.parameters.iter().enumerate() {
        insert_type_use(
            db,
            method.id,
            param.type_id,
            TypeUseRole::MethodParam,
            Some(idx),
        )?;
    }
    if let Some(return_type) = method.return_type {
        insert_type_use(db, method.id, return_type, TypeUseRole::MethodReturn, None)?;
    }
    Ok(())
}

fn transform_generic_bound_type_uses(
    db: &Db<MemStorage>,
    owner_id: AnyNodeId,
    generic_params: &[GenericParamNode],
) -> Result<(), TransformError> {
    let mut owner_slot_index = 0;
    for generic_param in generic_params {
        let GenericParamKind::Type { bounds, .. } = &generic_param.kind else {
            continue;
        };

        for (bound_idx, bound) in bounds.iter().copied().enumerate() {
            insert_type_use(
                db,
                owner_id,
                bound,
                TypeUseRole::GenericBound,
                Some(owner_slot_index),
            )?;
            insert_type_use(
                db,
                generic_param.id,
                bound,
                TypeUseRole::GenericParamBound,
                Some(bound_idx),
            )?;
            owner_slot_index += 1;
        }
    }
    Ok(())
}

fn transform_where_predicate_type_uses(
    db: &Db<MemStorage>,
    owner_id: AnyNodeId,
    generic_params: &[GenericParamNode],
    where_predicates: &[TypeWherePredicate],
    type_graph: &[TypeNode],
) -> Result<(), TransformError> {
    let mut owner_bound_slot_index = 0;
    for (predicate_idx, predicate) in where_predicates.iter().enumerate() {
        insert_type_use(
            db,
            owner_id,
            predicate.subject,
            TypeUseRole::WherePredicateSubject,
            Some(predicate_idx),
        )?;

        let direct_type_param_owner =
            direct_type_param_subject(predicate.subject, generic_params, type_graph);

        for (bound_idx, bound) in predicate.bounds.iter().copied().enumerate() {
            insert_type_use(
                db,
                owner_id,
                bound,
                TypeUseRole::WherePredicateBound,
                Some(owner_bound_slot_index),
            )?;
            if let Some(generic_param) = direct_type_param_owner {
                insert_type_use(
                    db,
                    generic_param.id,
                    bound,
                    TypeUseRole::WhereGenericParamBound,
                    Some(bound_idx),
                )?;
            }
            owner_bound_slot_index += 1;
        }
    }
    Ok(())
}

fn direct_type_param_subject<'a>(
    subject: syn_parser::parser::type_slots::OrdinaryTypeUseId,
    generic_params: &'a [GenericParamNode],
    type_graph: &[TypeNode],
) -> Option<&'a GenericParamNode> {
    let syn_parser::parser::type_slots::OrdinaryTypeUseId::Named(subject_id) = subject else {
        return None;
    };

    let Some(TypeNode::Named(subject_node)) = type_graph
        .iter()
        .find(|type_node| matches!(type_node, TypeNode::Named(node) if node.id == subject_id))
    else {
        return None;
    };

    if subject_node.is_fully_qualified
        || !subject_node.arguments.is_empty()
        || subject_node.qualified_self.is_some()
        || subject_node.qualified_trait.is_some()
        || subject_node.path.len() != 1
    {
        return None;
    }

    generic_params.iter().find(|param| {
        matches!(
            &param.kind,
            GenericParamKind::Type { name, .. } if name == &subject_node.path[0]
        )
    })
}

fn insert_type_use(
    db: &Db<MemStorage>,
    owner_id: impl Into<AnyNodeId>,
    root_type_id: impl ToCozoUuid,
    role: TypeUseRole,
    slot_index: Option<usize>,
) -> Result<(), TransformError> {
    TypeUseSchema::insert_relation(
        db,
        owner_id.into().to_cozo_uuid(),
        root_type_id.to_cozo_uuid(),
        role.as_str(),
        slot_index,
    )
}

fn transform_type_contains(
    db: &Db<MemStorage>,
    type_nodes: &[TypeNode],
) -> Result<(), TransformError> {
    for type_node in type_nodes {
        match type_node {
            TypeNode::Named(node) => {
                if let Some(qualified_self) = node.qualified_self {
                    insert_type_contains(
                        db,
                        node.id,
                        qualified_self,
                        TypeContainmentKind::QualifiedSelf,
                        None,
                    )?;
                }
                if let Some(qualified_trait) = node.qualified_trait {
                    insert_type_contains(
                        db,
                        node.id,
                        qualified_trait,
                        TypeContainmentKind::QualifiedTrait,
                        None,
                    )?;
                }
                for (idx, child) in node.arguments.iter().copied().enumerate() {
                    insert_type_contains(
                        db,
                        node.id,
                        child,
                        TypeContainmentKind::Argument,
                        Some(idx),
                    )?;
                }
            }
            TypeNode::Reference(node) => {
                insert_type_contains(
                    db,
                    node.id,
                    node.referenced,
                    TypeContainmentKind::Referenced,
                    None,
                )?;
            }
            TypeNode::Slice(node) => {
                insert_type_contains(
                    db,
                    node.id,
                    node.element,
                    TypeContainmentKind::Element,
                    None,
                )?;
            }
            TypeNode::Array(node) => {
                insert_type_contains(
                    db,
                    node.id,
                    node.element,
                    TypeContainmentKind::Element,
                    None,
                )?;
            }
            TypeNode::Tuple(node) => {
                for (idx, child) in node.elements.iter().copied().enumerate() {
                    insert_type_contains(
                        db,
                        node.id,
                        child,
                        TypeContainmentKind::Element,
                        Some(idx),
                    )?;
                }
            }
            TypeNode::Function(node) => {
                for (idx, child) in node.parameters.iter().copied().enumerate() {
                    insert_type_contains(
                        db,
                        node.id,
                        child,
                        TypeContainmentKind::FunctionParam,
                        Some(idx),
                    )?;
                }
                if let Some(return_type) = node.return_type {
                    insert_type_contains(
                        db,
                        node.id,
                        return_type,
                        TypeContainmentKind::FunctionReturn,
                        None,
                    )?;
                }
            }
            TypeNode::Never(_)
            | TypeNode::Inferred(_)
            | TypeNode::Macro(_)
            | TypeNode::Unknown(_) => {}
            TypeNode::RawPointer(node) => {
                insert_type_contains(
                    db,
                    node.id,
                    node.pointee,
                    TypeContainmentKind::Pointee,
                    None,
                )?;
            }
            TypeNode::TraitObject(node) => {
                for (idx, child) in node.bounds.iter().copied().enumerate() {
                    insert_type_contains(
                        db,
                        node.id,
                        child,
                        TypeContainmentKind::TraitBound,
                        Some(idx),
                    )?;
                }
            }
            TypeNode::ImplTrait(node) => {
                for (idx, child) in node.bounds.iter().copied().enumerate() {
                    insert_type_contains(
                        db,
                        node.id,
                        child,
                        TypeContainmentKind::TraitBound,
                        Some(idx),
                    )?;
                }
            }
            TypeNode::TraitBound(node) => {
                for (idx, child) in node.arguments.iter().copied().enumerate() {
                    insert_type_contains(
                        db,
                        node.id,
                        child,
                        TypeContainmentKind::Argument,
                        Some(idx),
                    )?;
                }
            }
            TypeNode::Paren(node) => {
                insert_type_contains(db, node.id, node.inner, TypeContainmentKind::Inner, None)?;
            }
        }
    }
    Ok(())
}

fn insert_type_contains(
    db: &Db<MemStorage>,
    parent_type_id: impl ToCozoUuid,
    child_type_id: impl ToCozoUuid,
    kind: TypeContainmentKind,
    position: Option<usize>,
) -> Result<(), TransformError> {
    TypeContainsSchema::insert_relation(
        db,
        parent_type_id.to_cozo_uuid(),
        child_type_id.to_cozo_uuid(),
        kind.as_str(),
        position,
    )
}
