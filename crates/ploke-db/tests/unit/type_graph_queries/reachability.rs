use ploke_db::{DbError, TypeRelationKind, TypeTargetPath};

use super::common::{
    exactly_one_uuid, function_id_by_name, function_id_by_name_in_module,
    function_param_type_by_name, setup_typed_fixture_db, struct_id_by_name,
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

    assert!(
        reachable.contains(&TypeTargetPath {
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

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;

    assert!(
        reachable.iter().any(|path| {
            path.owner_id == owner_id
                && path.target_id == target_id
                && path.relation_kind == TypeRelationKind::Trait
                && path.depth >= 1
        }),
        "expected draw_object parameter to reach Drawable through &dyn Drawable; reachable: {reachable:#?}"
    );
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

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;

    assert!(
        reachable.iter().any(|path| {
            path.owner_id == owner_id
                && path.target_id == target_id
                && path.relation_kind == TypeRelationKind::Ordinary
                && path.depth > 0
        }),
        "expected nested Vec<T> traversal to reach T at positive depth; reachable: {reachable:#?}"
    );
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

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;

    for target_id in [t_id, u_id] {
        assert!(
            reachable.iter().any(|path| {
                path.owner_id == owner_id
                    && path.target_id == target_id
                    && path.relation_kind == TypeRelationKind::Ordinary
                    && path.depth > 0
            }),
            "expected tuple return traversal to reach target {target_id}; reachable: {reachable:#?}"
        );
    }
    Ok(())
}
