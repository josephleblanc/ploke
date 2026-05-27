use ploke_db::{DbError, TypeRelationKind, TypeTargetPath, TypeUseCoordinate, TypeUseRole};

use super::common::{
    TypePathDepth, assert_type_use_reaches_target, exactly_one_uuid, function_id_by_name,
    function_id_by_name_in_module, function_param_type_by_name,
    generic_type_param_id_by_owner_name, setup_typed_fixture_db, struct_id_by_name,
    trait_id_by_name_in_module,
};

/// Medium resolution contract: direct root resolution should compose owner
/// type-use rows with `type_relation` terminal rows.
///
/// Grounding: the phase3 v2 resolver test already proves the parameter type of
/// `concrete` resolves to the local `T` struct. This DB query should recover
/// the same target through the database graph surface.
#[test]
fn reachable_targets_include_direct_param_struct_target() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "concrete")?;
    let root_type_id = function_param_type_by_name(&db, "concrete", 0)?;
    let target_id = struct_id_by_name(&db, "T")?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;
    let type_use_id = db
        .type_uses_for_owner(owner_id)?
        .into_iter()
        .find(|root| {
            root.role == TypeUseRole::FunctionParam
                && root.coordinate == (TypeUseCoordinate::ParamSlot { param_index: 0 })
        })
        .expect("concrete function param root")
        .id;

    assert!(
        reachable.contains(&TypeTargetPath {
            type_use_id,
            owner_id,
            root_type_id,
            terminal_type_id: root_type_id,
            target_id,
            relation_kind: TypeRelationKind::Ordinary,
            depth: 0,
        }),
        "expected direct function parameter struct target for concrete; reachable: {reachable:#?}"
    );
    Ok(())
}

/// Ambitious traversal contract: trait-position terminals nested inside
/// ordinary root syntax should still reach their trait definitions.
///
/// Grounding: `fixture_types::draw_object` has parameter type `&dyn Drawable`.
/// The owner root is an ordinary reference type, but the terminal trait bound
/// inside the trait object should resolve to the local `Drawable` trait after
/// walking structural containment.
#[test]
fn reachable_targets_walk_reference_trait_object_to_trait_definition() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_types")?;
    let owner_id = function_id_by_name_in_module(&db, &["crate"], "draw_object")?;
    let target_id = trait_id_by_name_in_module(&db, &["crate"], "Drawable")?;

    assert_type_use_reaches_target(
        &db,
        owner_id,
        TypeUseRole::FunctionParam,
        TypeUseCoordinate::ParamSlot { param_index: 0 },
        target_id,
        TypeRelationKind::Trait,
        TypePathDepth::Min(1),
        "expected draw_object parameter to reach Drawable through &dyn Drawable",
    )?;
    Ok(())
}

/// Ambitious graphRAG distance contract: nested type targets should retain
/// depth information.
///
/// `takes_vec_of_t(Vec<T>)` in `fixture_type_resolution_v2` should be closer to
/// the root `Vec<T>` than to the nested `T`. The target struct `T` should be
/// reachable, but at a positive depth.
#[test]
fn reachable_targets_preserve_positive_depth_for_nested_ordinary_type() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "takes_vec_of_t")?;
    let target_id = struct_id_by_name(&db, "T")?;

    assert_type_use_reaches_target(
        &db,
        owner_id,
        TypeUseRole::FunctionParam,
        TypeUseCoordinate::ParamSlot { param_index: 0 },
        target_id,
        TypeRelationKind::Ordinary,
        TypePathDepth::Min(1),
        "expected takes_vec_of_t parameter to reach T through nested Vec<T>",
    )?;
    Ok(())
}

/// Ambitious multi-terminal contract: one owner may reach multiple local type
/// definitions through a single complex root.
///
/// `returns_pair` in `fixture_type_resolution_v2` returns a tuple containing
/// local type definitions. The DB query should keep both targets visible so
/// downstream graphRAG can rank each terminal independently.
#[test]
fn reachable_targets_include_multiple_terminals_from_one_root() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "returns_pair")?;
    let t_id = struct_id_by_name(&db, "T")?;
    let u_id = exactly_one_uuid(
        &db,
        r#"?[id] :=
            *struct { id, name: "U" @ 'NOW' }"#,
        0,
    )?;

    for target_id in [t_id, u_id] {
        assert_type_use_reaches_target(
            &db,
            owner_id,
            TypeUseRole::FunctionReturn,
            TypeUseCoordinate::None,
            target_id,
            TypeRelationKind::Ordinary,
            TypePathDepth::Min(1),
            &format!("expected tuple return traversal to reach target {target_id}"),
        )?;
    }
    Ok(())
}

/// Generic declaration bounds should participate in ordinary reachability from
/// both graphRAG-friendly item owners and precise generic-param owners.
#[test]
fn reachable_targets_include_generic_declaration_bound_trait() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "LocallyBound")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;
    let trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;

    assert_type_use_reaches_target(
        &db,
        owner_id,
        TypeUseRole::GenericBound,
        TypeUseCoordinate::GenericBoundSlot {
            generic_param_index: 0,
            bound_index: 0,
        },
        trait_id,
        TypeRelationKind::Trait,
        TypePathDepth::Exact(0),
        "expected LocallyBound to reach LocalTrait through generic declaration bound",
    )?;
    assert_type_use_reaches_target(
        &db,
        generic_param_id,
        TypeUseRole::GenericParamBound,
        TypeUseCoordinate::GenericParamBoundSlot {
            containing_owner_id: owner_id,
            generic_param_index: 0,
            bound_index: 0,
        },
        trait_id,
        TypeRelationKind::Trait,
        TypePathDepth::Exact(0),
        "expected LocallyBound::T to reach LocalTrait through generic declaration bound",
    )?;

    Ok(())
}

/// Where-clause bounds should compose with `type_relation` from both the
/// containing item and direct generic-param owner when the predicate subject is
/// exactly that local type parameter.
#[test]
fn reachable_targets_include_where_direct_type_param_bound_trait() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereLocal")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;
    let trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;

    assert_type_use_reaches_target(
        &db,
        owner_id,
        TypeUseRole::WherePredicateBound,
        TypeUseCoordinate::WhereBoundSlot {
            predicate_index: 0,
            bound_index: 0,
        },
        trait_id,
        TypeRelationKind::Trait,
        TypePathDepth::Exact(0),
        "expected WhereLocal to reach LocalTrait through where-clause bound",
    )?;
    assert_type_use_reaches_target(
        &db,
        generic_param_id,
        TypeUseRole::WhereGenericParamBound,
        TypeUseCoordinate::WhereGenericParamBoundSlot {
            containing_owner_id: owner_id,
            predicate_index: 0,
            bound_index: 0,
        },
        trait_id,
        TypeRelationKind::Trait,
        TypePathDepth::Exact(0),
        "expected WhereLocal::T to reach LocalTrait through where-clause bound",
    )?;

    Ok(())
}

/// Multiple trait bounds in one where predicate should each be independently
/// reachable from both the containing owner and the direct generic-param owner.
#[test]
fn reachable_targets_include_each_multi_bound_where_trait() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereMultiBound")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;
    let local_trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;
    let extra_trait_id = trait_id_by_name_in_module(&db, &["crate"], "ExtraTrait")?;

    for (bound_index, target_id, target_name) in [
        (0, local_trait_id, "LocalTrait"),
        (1, extra_trait_id, "ExtraTrait"),
    ] {
        assert_type_use_reaches_target(
            &db,
            owner_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinate::WhereBoundSlot {
                predicate_index: 0,
                bound_index,
            },
            target_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            &format!("expected WhereMultiBound to reach {target_name}"),
        )?;
        assert_type_use_reaches_target(
            &db,
            generic_param_id,
            TypeUseRole::WhereGenericParamBound,
            TypeUseCoordinate::WhereGenericParamBoundSlot {
                containing_owner_id: owner_id,
                predicate_index: 0,
                bound_index,
            },
            target_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            &format!("expected WhereMultiBound::T to reach {target_name}"),
        )?;
    }

    Ok(())
}

/// Repeated where predicates on the same subject should keep each predicate's
/// bound independently reachable through the generic-param owner.
#[test]
fn reachable_targets_include_repeated_subject_where_traits() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereRepeatedSubject")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;
    let local_trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;
    let extra_trait_id = trait_id_by_name_in_module(&db, &["crate"], "ExtraTrait")?;

    for (predicate_index, target_id, target_name) in [
        (0, local_trait_id, "LocalTrait"),
        (1, extra_trait_id, "ExtraTrait"),
    ] {
        assert_type_use_reaches_target(
            &db,
            owner_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinate::WhereBoundSlot {
                predicate_index,
                bound_index: 0,
            },
            target_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            &format!("expected WhereRepeatedSubject to reach {target_name}"),
        )?;
        assert_type_use_reaches_target(
            &db,
            generic_param_id,
            TypeUseRole::WhereGenericParamBound,
            TypeUseCoordinate::WhereGenericParamBoundSlot {
                containing_owner_id: owner_id,
                predicate_index,
                bound_index: 0,
            },
            target_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            &format!("expected WhereRepeatedSubject::T to reach {target_name}"),
        )?;
    }

    Ok(())
}

/// Composite where-clause subjects should expose their nested type terminals
/// without pretending the whole predicate is a direct bound on `T`.
#[test]
fn reachable_targets_include_where_composite_subject_nested_type_param() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereComposite")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    assert_type_use_reaches_target(
        &db,
        owner_id,
        TypeUseRole::WherePredicateSubject,
        TypeUseCoordinate::WhereSubjectSlot { predicate_index: 0 },
        generic_param_id,
        TypeRelationKind::Ordinary,
        TypePathDepth::Min(1),
        "expected WhereComposite subject Vec<T> to reach nested generic param T",
    )?;

    Ok(())
}
