use cozo::{DataValue, UuidWrapper};
use ploke_db::{Database, DbError, TypeRelationKind, TypeUseRole};
use uuid::Uuid;

use super::common::{
    function_id_by_name, function_param_type_by_name, setup_typed_fixture_db, struct_id_by_name,
    trait_id_by_name_in_module,
};

#[test]
fn owner_reachability_excludes_ordinary_relation_to_trait_target() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "concrete")?;
    let source_id = function_param_type_by_name(&db, "concrete", 0)?;
    let invalid_target_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;

    insert_invalid_type_relation(
        &db,
        source_id,
        invalid_target_id,
        "Ordinary",
        "Named",
        "Trait",
    )?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;
    assert!(
        reachable.iter().all(|path| {
            !(path.owner_id == owner_id
                && path.root_type_id == source_id
                && path.terminal_type_id == source_id
                && path.target_id == invalid_target_id
                && path.relation_kind == TypeRelationKind::Ordinary)
        }),
        "invalid Ordinary relation to a trait target should not be surfaced; reachable: {reachable:#?}"
    );

    let owners = db.type_owners_for_target(invalid_target_id)?;
    assert!(
        owners.iter().all(|path| {
            !(path.owner_id == owner_id
                && path.root_type_id == source_id
                && path.terminal_type_id == source_id
                && path.relation_kind == TypeRelationKind::Ordinary)
        }),
        "invalid Ordinary relation to a trait target should not be surfaced by target query; owners: {owners:#?}"
    );

    Ok(())
}

#[test]
fn owner_reachability_excludes_trait_relation_to_ordinary_target() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "concrete")?;
    let source_id = function_param_type_by_name(&db, "concrete", 0)?;
    let invalid_target_id = struct_id_by_name(&db, "T")?;

    insert_invalid_type_relation(
        &db,
        source_id,
        invalid_target_id,
        "Trait",
        "Named",
        "Struct",
    )?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;
    assert!(
        reachable.iter().all(|path| {
            !(path.owner_id == owner_id
                && path.root_type_id == source_id
                && path.terminal_type_id == source_id
                && path.target_id == invalid_target_id
                && path.relation_kind == TypeRelationKind::Trait)
        }),
        "invalid Trait relation to an ordinary target should not be surfaced; reachable: {reachable:#?}"
    );

    let owners = db.type_owners_for_target(invalid_target_id)?;
    assert!(
        owners.iter().all(|path| {
            !(path.owner_id == owner_id
                && path.root_type_id == source_id
                && path.terminal_type_id == source_id
                && path.relation_kind == TypeRelationKind::Trait)
        }),
        "invalid Trait relation to an ordinary target should not be surfaced by target query; owners: {owners:#?}"
    );

    Ok(())
}

#[test]
fn owner_reachability_excludes_ordinary_relation_from_trait_bound_source() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = trait_id_by_name_in_module(&db, &["crate"], "LocalAssocBound")?;
    let source_id = associated_type_bound_root(&db, owner_id)?;
    let invalid_target_id = struct_id_by_name(&db, "T")?;

    insert_invalid_type_relation(
        &db,
        source_id,
        invalid_target_id,
        "Ordinary",
        "TraitBound",
        "Struct",
    )?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;
    assert!(
        reachable.iter().all(|path| {
            !(path.owner_id == owner_id
                && path.root_type_id == source_id
                && path.terminal_type_id == source_id
                && path.target_id == invalid_target_id
                && path.relation_kind == TypeRelationKind::Ordinary)
        }),
        "invalid Ordinary relation from a trait-bound source should not be surfaced; reachable: {reachable:#?}"
    );

    Ok(())
}

fn associated_type_bound_root(db: &Database, owner_id: Uuid) -> Result<Uuid, DbError> {
    let roots = db.type_uses_for_owner(owner_id)?;
    let root = roots
        .iter()
        .find(|root| root.role == TypeUseRole::AssociatedTypeBound && root.slot_index == Some(0))
        .copied()
        .unwrap_or_else(|| {
            panic!("expected AssociatedTypeBound root for owner {owner_id}; roots: {roots:#?}")
        });
    Ok(root.root_type_id)
}

fn insert_invalid_type_relation(
    db: &Database,
    source_id: Uuid,
    target_id: Uuid,
    relation_kind: &'static str,
    source_kind: &'static str,
    target_kind: &'static str,
) -> Result<(), DbError> {
    let params = [
        (
            "source_id".to_string(),
            DataValue::Uuid(UuidWrapper(source_id)),
        ),
        (
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        ),
        ("relation_kind".to_string(), DataValue::from(relation_kind)),
        ("source_kind".to_string(), DataValue::from(source_kind)),
        ("target_kind".to_string(), DataValue::from(target_kind)),
    ]
    .into_iter()
    .collect();

    db.raw_query_mut_params(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind, at] :=
            source_id = $source_id,
            target_id = $target_id,
            relation_kind = $relation_kind,
            source_kind = $source_kind,
            target_kind = $target_kind,
            at = 'ASSERT'
        :put type_relation {
            source_id,
            target_id,
            relation_kind,
            source_kind,
            target_kind,
            at
        }"#,
        params,
    )?;

    Ok(())
}
