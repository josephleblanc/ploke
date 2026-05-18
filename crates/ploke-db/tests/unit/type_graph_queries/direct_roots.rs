use ploke_core::IdTrait;
use ploke_db::{Database, DbError, TypeUseCoordinate, TypeUseRole, TypeUseRoot};
use syn_parser::parser::{graph::GraphAccess, nodes::AsAnyNodeId};

use super::common::{
    const_id_by_name_in_file_suffix, exactly_one_uuid, field_id_by_owner_index,
    function_id_by_name, function_id_by_name_in_module, function_param_type_by_name,
    function_return_type_by_name_in_module, generic_type_param_id_by_owner_name,
    impl_id_by_trait_and_self_type_names, method_id_by_impl_self_type_name,
    method_id_by_impl_trait_and_self_type_names, setup_typed_fixture_db,
    static_id_by_name_in_file_suffix, struct_id_by_name, struct_id_by_name_in_module,
    trait_id_by_name_in_module, type_alias_row_by_name,
};

fn has_root(
    roots: &[TypeUseRoot],
    owner_id: uuid::Uuid,
    root_type_id: uuid::Uuid,
    role: TypeUseRole,
    coordinate: TypeUseCoordinate,
) -> bool {
    roots.iter().any(|root| {
        root.owner_id == owner_id
            && root.root_type_id == root_type_id
            && root.role == role
            && root.coordinate == coordinate
    })
}

fn assert_exact_root(
    roots: &[TypeUseRoot],
    owner_id: uuid::Uuid,
    root_type_id: uuid::Uuid,
    role: TypeUseRole,
    coordinate: TypeUseCoordinate,
    message: &str,
) {
    let matching = roots
        .iter()
        .filter(|root| {
            root.owner_id == owner_id
                && root.root_type_id == root_type_id
                && root.role == role
                && root.coordinate == coordinate
        })
        .collect::<Vec<_>>();

    assert_eq!(
        matching.len(),
        1,
        "{message}; expected exactly one root for owner {owner_id}, root {root_type_id}, role {role:?}, coordinate {coordinate:?}; roots: {roots:#?}"
    );
}

fn fixture_graph() -> Result<syn_parser::parser::ParsedCodeGraph, DbError> {
    syn_parser::parser::ParsedCodeGraph::merge_new(ploke_test_utils::test_run_phases_and_collect(
        "fixture_nodes",
    ))
    .map_err(|err| DbError::QueryExecution(err.to_string()))
}

fn fixture_method_param_type_by_slot(
    owner_id: uuid::Uuid,
    param_index: u32,
) -> Result<uuid::Uuid, DbError> {
    let graph = fixture_graph()?;
    let method = graph
        .impls()
        .iter()
        .flat_map(|impl_node| impl_node.methods.iter())
        .chain(
            graph
                .traits()
                .iter()
                .flat_map(|trait_node| trait_node.methods.iter()),
        )
        .find(|method| method.id.as_any().uuid() == owner_id)
        .ok_or_else(|| {
            DbError::QueryExecution(format!(
                "fixture method {owner_id} missing from source graph"
            ))
        })?;
    method
        .parameters
        .get(param_index as usize)
        .map(|param| param.type_id.uuid())
        .ok_or_else(|| {
            DbError::QueryExecution(format!(
                "fixture method {owner_id} missing source param slot {param_index}"
            ))
        })
}

fn fixture_method_return_type_by_id(owner_id: uuid::Uuid) -> Result<uuid::Uuid, DbError> {
    let graph = fixture_graph()?;
    let method = graph
        .impls()
        .iter()
        .flat_map(|impl_node| impl_node.methods.iter())
        .chain(
            graph
                .traits()
                .iter()
                .flat_map(|trait_node| trait_node.methods.iter()),
        )
        .find(|method| method.id.as_any().uuid() == owner_id)
        .ok_or_else(|| {
            DbError::QueryExecution(format!(
                "fixture method {owner_id} missing from source graph"
            ))
        })?;
    method
        .return_type
        .map(|type_id| type_id.uuid())
        .ok_or_else(|| {
            DbError::QueryExecution(format!(
                "fixture method {owner_id} missing source return type"
            ))
        })
}

fn impl_self_type_by_id(db: &Database, impl_id: uuid::Uuid) -> Result<uuid::Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[self_type] :=
                *impl {{
                    id: to_uuid("{impl_id}"),
                    self_type @ 'NOW'
                }}"#
        ),
        0,
    )
}

fn impl_trait_type_by_id(db: &Database, impl_id: uuid::Uuid) -> Result<uuid::Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[trait_type] :=
                *impl {{
                    id: to_uuid("{impl_id}"),
                    trait_type @ 'NOW'
                }}"#
        ),
        0,
    )
}

fn field_type_by_id(db: &Database, field_id: uuid::Uuid) -> Result<uuid::Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[type_id] :=
                *field {{
                    id: to_uuid("{field_id}"),
                    type_id @ 'NOW'
                }}"#
        ),
        0,
    )
}

fn fixture_trait_super_type_by_slot(
    owner_id: uuid::Uuid,
    supertrait_index: u32,
) -> Result<uuid::Uuid, DbError> {
    let graph = fixture_graph()?;
    let trait_node = graph
        .traits()
        .iter()
        .find(|trait_node| trait_node.id.as_any().uuid() == owner_id)
        .ok_or_else(|| {
            DbError::QueryExecution(format!(
                "fixture trait {owner_id} missing from source graph"
            ))
        })?;
    trait_node
        .super_traits
        .get(supertrait_index as usize)
        .map(|type_id| type_id.uuid())
        .ok_or_else(|| {
            DbError::QueryExecution(format!(
                "fixture trait {owner_id} missing source supertrait slot {supertrait_index}"
            ))
        })
}

fn const_type_by_id(db: &Database, const_id: uuid::Uuid) -> Result<uuid::Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[ty_id] :=
                *const {{
                    id: to_uuid("{const_id}"),
                    ty_id @ 'NOW'
                }}"#
        ),
        0,
    )
}

fn static_type_by_id(db: &Database, static_id: uuid::Uuid) -> Result<uuid::Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[ty_id] :=
                *static {{
                    id: to_uuid("{static_id}"),
                    ty_id @ 'NOW'
                }}"#
        ),
        0,
    )
}

/// Elementary contract: the DB API should expose direct owner-to-root type-use
/// rows before any structural traversal or terminal resolution is involved.
///
/// Grounding: `uuid_phase3_resolution/type_relations_v2.rs` already asserts
/// that `fixture_type_resolution_v2::concrete` resolves its parameter type to
/// the local `T` struct. This test starts one layer lower in Cozo: the function
/// owner should first expose its parameter slot's root structural type ID.
#[test]
fn function_param_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "concrete")?;
    let root_type_id = function_param_type_by_name(&db, "concrete", 0)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        has_root(
            &roots,
            owner_id,
            root_type_id,
            TypeUseRole::FunctionParam,
            TypeUseCoordinate::ParamSlot { param_index: 0 },
        ),
        "expected function parameter root type use for concrete; roots: {roots:#?}"
    );
    Ok(())
}

/// Direct root contract for function returns.
///
/// Grounding: `fixture_types::apply_op` returns `i32`; regardless of whether
/// that terminal resolves to a local code item, the owner-to-root type-use row
/// should exist for graph traversal and exact root matching.
#[test]
fn function_return_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_types")?;
    let owner_id = function_id_by_name_in_module(&db, &["crate"], "apply_op")?;
    let root_type_id = function_return_type_by_name_in_module(&db, &["crate"], "apply_op")?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        has_root(
            &roots,
            owner_id,
            root_type_id,
            TypeUseRole::FunctionReturn,
            TypeUseCoordinate::None,
        ),
        "expected function return root type use for apply_op; roots: {roots:#?}"
    );
    Ok(())
}

/// Direct root contract for type aliases.
///
/// This pins the boundary where a type alias item is connected to the root
/// structural type it names. Deeper argument traversal belongs to
/// `type_contains`, not to this direct owner/root edge.
#[test]
fn type_alias_target_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (owner_id, root_type_id) = type_alias_row_by_name(&db, "Mapping")?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        has_root(
            &roots,
            owner_id,
            root_type_id,
            TypeUseRole::TypeAliasTarget,
            TypeUseCoordinate::None,
        ),
        "expected type alias root type use for Mapping; roots: {roots:#?}"
    );
    Ok(())
}

/// Generic declaration bounds should be queryable both from the containing item
/// and from the precise generic parameter declaration.
#[test]
fn generic_bound_roots_are_queryable_from_item_and_param_owners() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "LocallyBound")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    let owner_bound = owner_roots
        .iter()
        .find(|root| root.role == TypeUseRole::GenericBound)
        .cloned()
        .unwrap_or_else(|| {
            panic!("expected LocallyBound to expose a GenericBound root; roots: {owner_roots:#?}")
        });

    assert!(
        has_root(
            &generic_param_roots,
            generic_param_id,
            owner_bound.root_type_id,
            TypeUseRole::GenericParamBound,
            TypeUseCoordinate::GenericParamBoundSlot {
                containing_owner_id: owner_id,
                generic_param_index: 0,
                bound_index: 0,
            },
        ),
        "expected LocallyBound::T to expose the same bound root as GenericParamBound; roots: {generic_param_roots:#?}"
    );

    Ok(())
}

/// Associated type bounds should expose a direct trait-position root from the
/// containing trait. Precise associated type item ownership is a separate
/// future surface; this pins the current GraphRAG-friendly owner path.
#[test]
fn associated_type_bound_root_is_queryable_from_containing_trait() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = trait_id_by_name_in_module(&db, &["crate"], "LocalAssocBound")?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::AssociatedTypeBound
                && root.coordinate
                    == (TypeUseCoordinate::AssociatedTypeBoundSlot {
                        associated_type_index: 0,
                        associated_type_name: "Output".to_string(),
                        bound_index: 0,
                    })
        }),
        "expected LocalAssocBound to expose an AssociatedTypeBound root; roots: {roots:#?}"
    );

    Ok(())
}

/// Where predicates should keep the bounded subject separate from the trait
/// bounds. Direct generic-param subjects additionally expose a derived
/// generic-param-owned bound root.
#[test]
fn where_predicate_roots_are_queryable_from_item_and_param_owners() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereLocal")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    assert!(
        owner_roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateSubject
                && root.coordinate == (TypeUseCoordinate::WhereSubjectSlot { predicate_index: 0 })
        }),
        "expected WhereLocal to expose a WherePredicateSubject root; roots: {owner_roots:#?}"
    );

    let owner_bound = owner_roots
        .iter()
        .find(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateBound
                && root.coordinate
                    == (TypeUseCoordinate::WhereBoundSlot {
                        predicate_index: 0,
                        bound_index: 0,
                    })
        })
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "expected WhereLocal to expose a WherePredicateBound root; roots: {owner_roots:#?}"
            )
        });

    assert!(
        has_root(
            &generic_param_roots,
            generic_param_id,
            owner_bound.root_type_id,
            TypeUseRole::WhereGenericParamBound,
            TypeUseCoordinate::WhereGenericParamBoundSlot {
                containing_owner_id: owner_id,
                predicate_index: 0,
                bound_index: 0,
            },
        ),
        "expected WhereLocal::T to expose the same bound root as WhereGenericParamBound; roots: {generic_param_roots:#?}"
    );

    Ok(())
}

/// Multi-bound where predicates should expose one owner root for each bound,
/// while preserving each distinct bound root for the direct generic parameter.
#[test]
fn multi_bound_where_predicate_roots_are_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereMultiBound")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    for bound_index in [0, 1] {
        let owner_bound = owner_roots
            .iter()
            .find(|root| {
                root.owner_id == owner_id
                    && root.role == TypeUseRole::WherePredicateBound
                    && root.coordinate
                        == (TypeUseCoordinate::WhereBoundSlot {
                            predicate_index: 0,
                            bound_index,
                        })
            })
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "expected WhereMultiBound owner bound at coordinate (0, {bound_index}); roots: {owner_roots:#?}"
                )
            });

        assert!(
            generic_param_roots.iter().any(|root| {
                root.owner_id == generic_param_id
                    && root.root_type_id == owner_bound.root_type_id
                    && root.role == TypeUseRole::WhereGenericParamBound
                    && root.coordinate
                        == (TypeUseCoordinate::WhereGenericParamBoundSlot {
                            containing_owner_id: owner_id,
                            predicate_index: 0,
                            bound_index,
                        })
            }),
            "expected WhereMultiBound::T to expose owner bound root at coordinate (0, {bound_index}); roots: {generic_param_roots:#?}"
        );
    }

    Ok(())
}

/// Repeated predicates over the same direct subject need distinguishable
/// generic-param-owned roots.
#[test]
fn repeated_subject_where_predicate_param_roots_have_distinct_slots() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereRepeatedSubject")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    for predicate_index in [0, 1] {
        assert!(
            owner_roots.iter().any(|root| {
                root.owner_id == owner_id
                    && root.role == TypeUseRole::WherePredicateSubject
                    && root.coordinate == (TypeUseCoordinate::WhereSubjectSlot { predicate_index })
            }),
            "expected WhereRepeatedSubject subject predicate {predicate_index}; roots: {owner_roots:#?}"
        );
        assert!(
            owner_roots.iter().any(|root| {
                root.owner_id == owner_id
                    && root.role == TypeUseRole::WherePredicateBound
                    && root.coordinate
                        == (TypeUseCoordinate::WhereBoundSlot {
                            predicate_index,
                            bound_index: 0,
                        })
            }),
            "expected WhereRepeatedSubject bound predicate {predicate_index}; roots: {owner_roots:#?}"
        );
        assert!(
            generic_param_roots.iter().any(|root| {
                root.owner_id == generic_param_id
                    && root.role == TypeUseRole::WhereGenericParamBound
                    && root.coordinate
                        == (TypeUseCoordinate::WhereGenericParamBoundSlot {
                            containing_owner_id: owner_id,
                            predicate_index,
                            bound_index: 0,
                        })
            }),
            "expected WhereRepeatedSubject::T generic-param bound predicate {predicate_index}; roots: {generic_param_roots:#?}"
        );
    }

    Ok(())
}

/// Composite where-predicate subjects are queryable as subjects, but should not
/// be collapsed into a direct generic-param-owned bound root.
#[test]
fn composite_where_predicate_subject_is_not_a_generic_param_bound() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereComposite")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    assert!(
        owner_roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateSubject
                && root.coordinate == (TypeUseCoordinate::WhereSubjectSlot { predicate_index: 0 })
        }),
        "expected WhereComposite to expose a WherePredicateSubject root; roots: {owner_roots:#?}"
    );
    assert!(
        owner_roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateBound
                && root.coordinate
                    == (TypeUseCoordinate::WhereBoundSlot {
                        predicate_index: 0,
                        bound_index: 0,
                    })
        }),
        "expected WhereComposite to expose a WherePredicateBound root; roots: {owner_roots:#?}"
    );
    assert!(
        generic_param_roots
            .iter()
            .all(|root| root.role != TypeUseRole::WhereGenericParamBound),
        "composite subject Vec<T> should not be collapsed into a WhereGenericParamBound root; roots: {generic_param_roots:#?}"
    );

    Ok(())
}

#[test]
fn method_param_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let owner_id = method_id_by_impl_trait_and_self_type_names(
        &db,
        "GenericTrait",
        "GenericStruct",
        "generic_trait_method",
    )?;
    let root_type_id = fixture_method_param_type_by_slot(owner_id, 1)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        root_type_id,
        TypeUseRole::MethodParam,
        TypeUseCoordinate::ParamSlot { param_index: 1 },
        "expected GenericTrait for GenericStruct::generic_trait_method value parameter root",
    );
    Ok(())
}

#[test]
fn method_return_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let owner_id = method_id_by_impl_self_type_name(&db, "SimpleStruct", "new")?;
    let root_type_id = fixture_method_return_type_by_id(owner_id)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        root_type_id,
        TypeUseRole::MethodReturn,
        TypeUseCoordinate::None,
        "expected SimpleStruct::new return root",
    );
    Ok(())
}

#[test]
fn field_type_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let struct_id = struct_id_by_name_in_module(&db, &["crate", "structs"], "SampleStruct")?;
    let owner_id = field_id_by_owner_index(&db, struct_id, 0)?;
    let root_type_id = field_type_by_id(&db, owner_id)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        root_type_id,
        TypeUseRole::FieldType,
        TypeUseCoordinate::FieldSlot { field_index: 0 },
        "expected SampleStruct field 0 root",
    );
    Ok(())
}

#[test]
fn impl_self_and_trait_root_type_uses_are_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let owner_id = impl_id_by_trait_and_self_type_names(&db, "SimpleTrait", "SimpleStruct")?;
    let self_root_type_id = impl_self_type_by_id(&db, owner_id)?;
    let trait_root_type_id = impl_trait_type_by_id(&db, owner_id)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        self_root_type_id,
        TypeUseRole::ImplSelf,
        TypeUseCoordinate::None,
        "expected impl SimpleTrait for SimpleStruct self root",
    );
    assert_exact_root(
        &roots,
        owner_id,
        trait_root_type_id,
        TypeUseRole::ImplTrait,
        TypeUseCoordinate::None,
        "expected impl SimpleTrait for SimpleStruct trait root",
    );
    Ok(())
}

#[test]
fn trait_super_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let owner_id = trait_id_by_name_in_module(&db, &["crate", "traits"], "SuperTrait")?;
    let root_type_id = fixture_trait_super_type_by_slot(owner_id, 0)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        root_type_id,
        TypeUseRole::TraitSuper,
        TypeUseCoordinate::TraitSuperSlot {
            supertrait_index: 0,
        },
        "expected SuperTrait supertrait root",
    );
    Ok(())
}

#[test]
fn const_type_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let owner_id = const_id_by_name_in_file_suffix(&db, "src/const_static.rs", "STRUCT_CONST")?;
    let root_type_id = const_type_by_id(&db, owner_id)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        root_type_id,
        TypeUseRole::ConstType,
        TypeUseCoordinate::None,
        "expected STRUCT_CONST type root",
    );
    Ok(())
}

#[test]
fn static_type_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let owner_id =
        static_id_by_name_in_file_suffix(&db, "src/const_static.rs", "TOP_LEVEL_COUNTER")?;
    let root_type_id = static_type_by_id(&db, owner_id)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert_exact_root(
        &roots,
        owner_id,
        root_type_id,
        TypeUseRole::StaticType,
        TypeUseCoordinate::None,
        "expected TOP_LEVEL_COUNTER type root",
    );
    Ok(())
}
