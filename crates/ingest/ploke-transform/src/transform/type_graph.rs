use cozo::{DataValue, Db, MemStorage, UuidWrapper};
use ploke_core::PROJECT_NAMESPACE_UUID;
use syn_parser::parser::{
    graph::CodeGraph,
    nodes::{AnyNodeId, AsAnyNodeId, ToCozoUuid, TypeDefNode},
    type_nodes::TypeNode,
    types::{GenericParamKind, GenericParamNode, TypeWherePredicate},
};
use uuid::Uuid;

use crate::{
    error::TransformError,
    schema::edges::{TypeContainsSchema, TypeUseSchema},
};

// Cross-pipeline coverage note:
// `ploke_test_utils::type_shape_matrix` names the corpus-backed TypeNode
// structures that must survive this transform into `type_use`, `type_contains`,
// and `type_relation` rows before DB/RAG/TUI tests consume them. Keep new
// containment kinds or root roles reflected there instead of adding ad hoc
// downstream-only expectations.

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

#[derive(Debug, Clone)]
struct TypeUseRecord {
    id: Uuid,
    owner_id: Uuid,
    root_type_id: Uuid,
    role: TypeUseRole,
    coordinate: TypeUseCoordinateRecord,
}

#[derive(Debug, Clone)]
enum TypeUseCoordinateRecord {
    None,
    ParamSlot {
        param_index: usize,
    },
    FieldSlot {
        field_index: usize,
    },
    TraitSuperSlot {
        supertrait_index: usize,
    },
    GenericBoundSlot {
        generic_param_index: usize,
        bound_index: usize,
    },
    GenericParamBoundSlot {
        containing_owner_id: Uuid,
        generic_param_index: usize,
        bound_index: usize,
    },
    WhereSubjectSlot {
        predicate_index: usize,
    },
    WhereBoundSlot {
        predicate_index: usize,
        bound_index: usize,
    },
    WhereGenericParamBoundSlot {
        containing_owner_id: Uuid,
        predicate_index: usize,
        bound_index: usize,
    },
    AssociatedTypeBoundSlot {
        associated_type_index: usize,
        associated_type_name: String,
        bound_index: usize,
    },
}

impl TypeUseRecord {
    fn function_param(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        param_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::FunctionParam,
            TypeUseCoordinateRecord::ParamSlot { param_index },
        )
    }

    fn function_return(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::FunctionReturn,
            TypeUseCoordinateRecord::None,
        )
    }

    fn method_param(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        param_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::MethodParam,
            TypeUseCoordinateRecord::ParamSlot { param_index },
        )
    }

    fn method_return(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::MethodReturn,
            TypeUseCoordinateRecord::None,
        )
    }

    fn field_type(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        field_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::FieldType,
            TypeUseCoordinateRecord::FieldSlot { field_index },
        )
    }

    fn type_alias_target(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::TypeAliasTarget,
            TypeUseCoordinateRecord::None,
        )
    }

    fn impl_self(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::ImplSelf,
            TypeUseCoordinateRecord::None,
        )
    }

    fn impl_trait(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::ImplTrait,
            TypeUseCoordinateRecord::None,
        )
    }

    fn trait_super(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        supertrait_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::TraitSuper,
            TypeUseCoordinateRecord::TraitSuperSlot { supertrait_index },
        )
    }

    fn associated_type_bound(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        associated_type_index: usize,
        associated_type_name: impl Into<String>,
        bound_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::AssociatedTypeBound,
            TypeUseCoordinateRecord::AssociatedTypeBoundSlot {
                associated_type_index,
                associated_type_name: associated_type_name.into(),
                bound_index,
            },
        )
    }

    fn const_type(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::ConstType,
            TypeUseCoordinateRecord::None,
        )
    }

    fn static_type(owner_id: impl Into<AnyNodeId>, root_type_id: impl ToCozoUuid) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::StaticType,
            TypeUseCoordinateRecord::None,
        )
    }

    fn generic_bound(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        generic_param_index: usize,
        bound_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::GenericBound,
            TypeUseCoordinateRecord::GenericBoundSlot {
                generic_param_index,
                bound_index,
            },
        )
    }

    fn generic_param_bound(
        generic_param_id: impl Into<AnyNodeId>,
        containing_owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        generic_param_index: usize,
        bound_index: usize,
    ) -> Self {
        let containing_owner_id = any_node_uuid(containing_owner_id.into());
        Self::new(
            generic_param_id,
            root_type_id,
            TypeUseRole::GenericParamBound,
            TypeUseCoordinateRecord::GenericParamBoundSlot {
                containing_owner_id,
                generic_param_index,
                bound_index,
            },
        )
    }

    fn where_predicate_subject(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        predicate_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::WherePredicateSubject,
            TypeUseCoordinateRecord::WhereSubjectSlot { predicate_index },
        )
    }

    fn where_predicate_bound(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        predicate_index: usize,
        bound_index: usize,
    ) -> Self {
        Self::new(
            owner_id,
            root_type_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinateRecord::WhereBoundSlot {
                predicate_index,
                bound_index,
            },
        )
    }

    fn where_generic_param_bound(
        generic_param_id: impl Into<AnyNodeId>,
        containing_owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        predicate_index: usize,
        bound_index: usize,
    ) -> Self {
        let containing_owner_id = any_node_uuid(containing_owner_id.into());
        Self::new(
            generic_param_id,
            root_type_id,
            TypeUseRole::WhereGenericParamBound,
            TypeUseCoordinateRecord::WhereGenericParamBoundSlot {
                containing_owner_id,
                predicate_index,
                bound_index,
            },
        )
    }

    fn new(
        owner_id: impl Into<AnyNodeId>,
        root_type_id: impl ToCozoUuid,
        role: TypeUseRole,
        coordinate: TypeUseCoordinateRecord,
    ) -> Self {
        let owner_id = any_node_uuid(owner_id.into());
        let root_type_id = cozo_uuid(root_type_id.to_cozo_uuid());
        let id = deterministic_type_use_id(owner_id, root_type_id, role, &coordinate);
        Self {
            id,
            owner_id,
            root_type_id,
            role,
            coordinate,
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
                TypeUseRecord::function_param(function.id, param.type_id, idx),
            )?;
        }
        if let Some(return_type) = function.return_type {
            insert_type_use(db, TypeUseRecord::function_return(function.id, return_type))?;
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
                    insert_type_use(db, TypeUseRecord::field_type(field.id, field.type_id, idx))?;
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
                            TypeUseRecord::field_type(field.id, field.type_id, idx),
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
                insert_type_use(db, TypeUseRecord::type_alias_target(node.id, node.type_id))?;
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
                    insert_type_use(db, TypeUseRecord::field_type(field.id, field.type_id, idx))?;
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
                TypeUseRecord::trait_super(trait_node.id, super_trait, idx),
            )?;
        }
        for bound in &trait_node.associated_type_bounds {
            insert_type_use(
                db,
                TypeUseRecord::associated_type_bound(
                    trait_node.id,
                    bound.bound_type_id,
                    bound.associated_type_index,
                    &bound.associated_type_name,
                    bound.bound_index,
                ),
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
            TypeUseRecord::impl_self(impl_node.id, impl_node.self_type),
        )?;
        if let Some(trait_type) = impl_node.trait_type {
            insert_type_use(db, TypeUseRecord::impl_trait(impl_node.id, trait_type))?;
        }
        for method in &impl_node.methods {
            transform_method_type_uses(db, method, &graph.type_graph)?;
        }
    }

    for const_node in &graph.consts {
        insert_type_use(
            db,
            TypeUseRecord::const_type(const_node.id, const_node.type_id),
        )?;
    }

    for static_node in &graph.statics {
        insert_type_use(
            db,
            TypeUseRecord::static_type(static_node.id, static_node.type_id),
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
            TypeUseRecord::method_param(method.id, param.type_id, idx),
        )?;
    }
    if let Some(return_type) = method.return_type {
        insert_type_use(db, TypeUseRecord::method_return(method.id, return_type))?;
    }
    Ok(())
}

fn transform_generic_bound_type_uses(
    db: &Db<MemStorage>,
    owner_id: AnyNodeId,
    generic_params: &[GenericParamNode],
) -> Result<(), TransformError> {
    for (generic_param_index, generic_param) in generic_params.iter().enumerate() {
        let GenericParamKind::Type { bounds, .. } = &generic_param.kind else {
            continue;
        };

        for (bound_idx, bound) in bounds.iter().copied().enumerate() {
            insert_type_use(
                db,
                TypeUseRecord::generic_bound(owner_id, bound, generic_param_index, bound_idx),
            )?;
            insert_type_use(
                db,
                TypeUseRecord::generic_param_bound(
                    generic_param.id,
                    owner_id,
                    bound,
                    generic_param_index,
                    bound_idx,
                ),
            )?;
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
    for (predicate_idx, predicate) in where_predicates.iter().enumerate() {
        insert_type_use(
            db,
            TypeUseRecord::where_predicate_subject(owner_id, predicate.subject, predicate_idx),
        )?;

        let direct_type_param_owner =
            direct_type_param_subject(predicate.subject, generic_params, type_graph);

        for (bound_idx, bound) in predicate.bounds.iter().copied().enumerate() {
            insert_type_use(
                db,
                TypeUseRecord::where_predicate_bound(owner_id, bound, predicate_idx, bound_idx),
            )?;
            if let Some(generic_param) = direct_type_param_owner {
                insert_type_use(
                    db,
                    TypeUseRecord::where_generic_param_bound(
                        generic_param.id,
                        owner_id,
                        bound,
                        predicate_idx,
                        bound_idx,
                    ),
                )?;
            }
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

fn insert_type_use(db: &Db<MemStorage>, record: TypeUseRecord) -> Result<(), TransformError> {
    TypeUseSchema::insert_relation(
        db,
        DataValue::Uuid(UuidWrapper(record.id)),
        DataValue::Uuid(UuidWrapper(record.owner_id)),
        DataValue::Uuid(UuidWrapper(record.root_type_id)),
        record.role.as_str(),
    )?;

    let type_use_id = DataValue::Uuid(UuidWrapper(record.id));
    match record.coordinate {
        TypeUseCoordinateRecord::None => {}
        TypeUseCoordinateRecord::ParamSlot { param_index } => {
            TypeUseSchema::insert_param_slot(db, type_use_id, param_index)?;
        }
        TypeUseCoordinateRecord::FieldSlot { field_index } => {
            TypeUseSchema::insert_field_slot(db, type_use_id, field_index)?;
        }
        TypeUseCoordinateRecord::TraitSuperSlot { supertrait_index } => {
            TypeUseSchema::insert_trait_super_slot(db, type_use_id, supertrait_index)?;
        }
        TypeUseCoordinateRecord::GenericBoundSlot {
            generic_param_index,
            bound_index,
        } => {
            TypeUseSchema::insert_generic_bound_slot(
                db,
                type_use_id,
                generic_param_index,
                bound_index,
            )?;
        }
        TypeUseCoordinateRecord::GenericParamBoundSlot {
            containing_owner_id,
            generic_param_index,
            bound_index,
        } => {
            TypeUseSchema::insert_generic_param_bound_slot(
                db,
                type_use_id,
                DataValue::Uuid(UuidWrapper(containing_owner_id)),
                generic_param_index,
                bound_index,
            )?;
        }
        TypeUseCoordinateRecord::WhereSubjectSlot { predicate_index } => {
            TypeUseSchema::insert_where_subject_slot(db, type_use_id, predicate_index)?;
        }
        TypeUseCoordinateRecord::WhereBoundSlot {
            predicate_index,
            bound_index,
        } => {
            TypeUseSchema::insert_where_bound_slot(db, type_use_id, predicate_index, bound_index)?;
        }
        TypeUseCoordinateRecord::WhereGenericParamBoundSlot {
            containing_owner_id,
            predicate_index,
            bound_index,
        } => {
            TypeUseSchema::insert_where_generic_param_bound_slot(
                db,
                type_use_id,
                DataValue::Uuid(UuidWrapper(containing_owner_id)),
                predicate_index,
                bound_index,
            )?;
        }
        TypeUseCoordinateRecord::AssociatedTypeBoundSlot {
            associated_type_index,
            associated_type_name,
            bound_index,
        } => {
            TypeUseSchema::insert_associated_type_bound_slot(
                db,
                type_use_id,
                associated_type_index,
                &associated_type_name,
                bound_index,
            )?;
        }
    }

    Ok(())
}

fn any_node_uuid(id: AnyNodeId) -> Uuid {
    cozo_uuid(id.to_cozo_uuid())
}

fn cozo_uuid(value: DataValue) -> Uuid {
    let DataValue::Uuid(UuidWrapper(uuid)) = value else {
        panic!("type-use IDs must lower to Cozo UUIDs, got {value:?}");
    };
    uuid
}

fn deterministic_type_use_id(
    owner_id: Uuid,
    root_type_id: Uuid,
    role: TypeUseRole,
    coordinate: &TypeUseCoordinateRecord,
) -> Uuid {
    let payload = format!(
        "type_use:v1:{owner_id}:{root_type_id}:{}:{}",
        role.as_str(),
        coordinate.identity_fragment()
    );
    Uuid::new_v5(&PROJECT_NAMESPACE_UUID, payload.as_bytes())
}

impl TypeUseCoordinateRecord {
    fn identity_fragment(&self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::ParamSlot { param_index } => format!("param:{param_index}"),
            Self::FieldSlot { field_index } => format!("field:{field_index}"),
            Self::TraitSuperSlot { supertrait_index } => {
                format!("trait-super:{supertrait_index}")
            }
            Self::GenericBoundSlot {
                generic_param_index,
                bound_index,
            } => format!("generic-bound:{generic_param_index}:{bound_index}"),
            Self::GenericParamBoundSlot {
                containing_owner_id,
                generic_param_index,
                bound_index,
            } => {
                format!(
                    "generic-param-bound:{containing_owner_id}:{generic_param_index}:{bound_index}"
                )
            }
            Self::WhereSubjectSlot { predicate_index } => {
                format!("where-subject:{predicate_index}")
            }
            Self::WhereBoundSlot {
                predicate_index,
                bound_index,
            } => format!("where-bound:{predicate_index}:{bound_index}"),
            Self::WhereGenericParamBoundSlot {
                containing_owner_id,
                predicate_index,
                bound_index,
            } => format!(
                "where-generic-param-bound:{containing_owner_id}:{predicate_index}:{bound_index}"
            ),
            Self::AssociatedTypeBoundSlot {
                associated_type_index,
                associated_type_name,
                bound_index,
            } => format!(
                "associated-type-bound:{associated_type_index}:{associated_type_name}:{bound_index}"
            ),
        }
    }
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
