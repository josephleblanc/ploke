use cozo::{Db, MemStorage};
use syn_parser::parser::{
    graph::CodeGraph,
    nodes::{AnyNodeId, AsAnyNodeId, ToCozoUuid, TypeDefNode},
    type_nodes::TypeNode,
    types::{GenericParamKind, GenericParamNode},
};

use crate::{
    error::TransformError,
    schema::edges::{TypeContainsSchema, TypeUseSchema},
};

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
        for (idx, param) in function.parameters.iter().enumerate() {
            insert_type_use(db, function.id, param.type_id, "FunctionParam", Some(idx))?;
        }
        if let Some(return_type) = function.return_type {
            insert_type_use(db, function.id, return_type, "FunctionReturn", None)?;
        }
    }

    for defined_type in &graph.defined_types {
        match defined_type {
            TypeDefNode::Struct(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                for (idx, field) in node.fields.iter().enumerate() {
                    insert_type_use(db, field.id, field.type_id, "FieldType", Some(idx))?;
                }
            }
            TypeDefNode::Enum(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                for variant in &node.variants {
                    for (idx, field) in variant.fields.iter().enumerate() {
                        insert_type_use(db, field.id, field.type_id, "FieldType", Some(idx))?;
                    }
                }
            }
            TypeDefNode::TypeAlias(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                insert_type_use(db, node.id, node.type_id, "TypeAliasTarget", None)?;
            }
            TypeDefNode::Union(node) => {
                transform_generic_bound_type_uses(db, node.id.as_any(), &node.generic_params)?;
                for (idx, field) in node.fields.iter().enumerate() {
                    insert_type_use(db, field.id, field.type_id, "FieldType", Some(idx))?;
                }
            }
        }
    }

    for trait_node in &graph.traits {
        transform_generic_bound_type_uses(db, trait_node.id.as_any(), &trait_node.generic_params)?;
        for (idx, super_trait) in trait_node.super_traits.iter().copied().enumerate() {
            insert_type_use(db, trait_node.id, super_trait, "TraitSuper", Some(idx))?;
        }
        for method in &trait_node.methods {
            transform_method_type_uses(db, method)?;
        }
    }

    for impl_node in &graph.impls {
        transform_generic_bound_type_uses(db, impl_node.id.as_any(), &impl_node.generic_params)?;
        insert_type_use(db, impl_node.id, impl_node.self_type, "ImplSelf", None)?;
        if let Some(trait_type) = impl_node.trait_type {
            insert_type_use(db, impl_node.id, trait_type, "ImplTrait", None)?;
        }
        for method in &impl_node.methods {
            transform_method_type_uses(db, method)?;
        }
    }

    for const_node in &graph.consts {
        insert_type_use(db, const_node.id, const_node.type_id, "ConstType", None)?;
    }

    for static_node in &graph.statics {
        insert_type_use(db, static_node.id, static_node.type_id, "StaticType", None)?;
    }

    Ok(())
}

fn transform_method_type_uses(
    db: &Db<MemStorage>,
    method: &syn_parser::parser::nodes::MethodNode,
) -> Result<(), TransformError> {
    transform_generic_bound_type_uses(db, method.id.as_any(), &method.generic_params)?;
    for (idx, param) in method.parameters.iter().enumerate() {
        insert_type_use(db, method.id, param.type_id, "MethodParam", Some(idx))?;
    }
    if let Some(return_type) = method.return_type {
        insert_type_use(db, method.id, return_type, "MethodReturn", None)?;
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
            insert_type_use(db, owner_id, bound, "GenericBound", Some(owner_slot_index))?;
            insert_type_use(
                db,
                generic_param.id,
                bound,
                "GenericParamBound",
                Some(bound_idx),
            )?;
            owner_slot_index += 1;
        }
    }
    Ok(())
}

fn insert_type_use(
    db: &Db<MemStorage>,
    owner_id: impl Into<AnyNodeId>,
    root_type_id: impl ToCozoUuid,
    role: &'static str,
    slot_index: Option<usize>,
) -> Result<(), TransformError> {
    TypeUseSchema::insert_relation(
        db,
        owner_id.into().to_cozo_uuid(),
        root_type_id.to_cozo_uuid(),
        role,
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
                for (idx, child) in node.arguments.iter().copied().enumerate() {
                    insert_type_contains(db, node.id, child, "Argument", Some(idx))?;
                }
            }
            TypeNode::Reference(node) => {
                insert_type_contains(db, node.id, node.referenced, "Referenced", None)?;
            }
            TypeNode::Slice(node) => {
                insert_type_contains(db, node.id, node.element, "Element", None)?;
            }
            TypeNode::Array(node) => {
                insert_type_contains(db, node.id, node.element, "Element", None)?;
            }
            TypeNode::Tuple(node) => {
                for (idx, child) in node.elements.iter().copied().enumerate() {
                    insert_type_contains(db, node.id, child, "Element", Some(idx))?;
                }
            }
            TypeNode::Function(node) => {
                for (idx, child) in node.parameters.iter().copied().enumerate() {
                    insert_type_contains(db, node.id, child, "FunctionParam", Some(idx))?;
                }
                if let Some(return_type) = node.return_type {
                    insert_type_contains(db, node.id, return_type, "FunctionReturn", None)?;
                }
            }
            TypeNode::Never(_)
            | TypeNode::Inferred(_)
            | TypeNode::Macro(_)
            | TypeNode::Unknown(_) => {}
            TypeNode::RawPointer(node) => {
                insert_type_contains(db, node.id, node.pointee, "Pointee", None)?;
            }
            TypeNode::TraitObject(node) => {
                for (idx, child) in node.bounds.iter().copied().enumerate() {
                    insert_type_contains(db, node.id, child, "TraitBound", Some(idx))?;
                }
            }
            TypeNode::ImplTrait(node) => {
                for (idx, child) in node.bounds.iter().copied().enumerate() {
                    insert_type_contains(db, node.id, child, "TraitBound", Some(idx))?;
                }
            }
            TypeNode::TraitBound(node) => {
                for (idx, child) in node.arguments.iter().copied().enumerate() {
                    insert_type_contains(db, node.id, child, "Argument", Some(idx))?;
                }
            }
            TypeNode::Paren(node) => {
                insert_type_contains(db, node.id, node.inner, "Inner", None)?;
            }
        }
    }
    Ok(())
}

fn insert_type_contains(
    db: &Db<MemStorage>,
    parent_type_id: impl ToCozoUuid,
    child_type_id: impl ToCozoUuid,
    kind: &'static str,
    position: Option<usize>,
) -> Result<(), TransformError> {
    TypeContainsSchema::insert_relation(
        db,
        parent_type_id.to_cozo_uuid(),
        child_type_id.to_cozo_uuid(),
        kind,
        position,
    )
}
