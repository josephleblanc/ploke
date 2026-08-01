use std::collections::BTreeSet;

use cozo::DataValue;
use ploke_db::{DbError, to_uuid};

use super::common::setup_typed_fixture_db;

#[test]
fn coordinate_bearing_type_uses_have_exactly_one_coordinate_row() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let type_uses = db.raw_query(r#"?[id, role] := *type_use { id, role @ 'NOW' }"#)?;

    for row in &type_uses.rows {
        let type_use_id = to_uuid(&row[0])?;
        let DataValue::Str(role) = &row[1] else {
            panic!("role should be a string, row: {row:?}");
        };
        let Some(relation) = coordinate_relation_for_role(role) else {
            continue;
        };
        let coordinate_rows = db.raw_query(&format!(
            r#"?[type_use_id] :=
                type_use_id = to_uuid("{type_use_id}"),
                *{relation} {{ type_use_id @ 'NOW' }}"#
        ))?;
        assert_eq!(
            coordinate_rows.rows.len(),
            1,
            "type_use {type_use_id} role {role} should have exactly one coordinate row in {relation}; rows: {:#?}",
            coordinate_rows.rows
        );
    }

    Ok(())
}

#[test]
fn coordinate_rows_never_point_to_missing_type_use() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let type_use_rows = db.raw_query(r#"?[id] := *type_use { id @ 'NOW' }"#)?;
    let type_use_ids = type_use_rows
        .rows
        .iter()
        .map(|row| to_uuid(&row[0]))
        .collect::<Result<BTreeSet<_>, _>>()?;

    for relation in COORDINATE_RELATIONS {
        let coordinate_rows = db.raw_query(&format!(
            r#"?[type_use_id] := *{relation} {{ type_use_id @ 'NOW' }}"#
        ))?;
        for row in &coordinate_rows.rows {
            let type_use_id = to_uuid(&row[0])?;
            assert!(
                type_use_ids.contains(&type_use_id),
                "coordinate row in {relation} points to missing type_use {type_use_id}"
            );
        }
    }

    Ok(())
}

const COORDINATE_RELATIONS: &[&str] = &[
    "type_use_param_slot",
    "type_use_field_slot",
    "type_use_trait_super_slot",
    "type_use_generic_bound_slot",
    "type_use_generic_param_bound_slot",
    "type_use_where_subject_slot",
    "type_use_where_bound_slot",
    "type_use_where_generic_param_bound_slot",
    "type_use_associated_type_bound_slot",
];

fn coordinate_relation_for_role(role: &str) -> Option<&'static str> {
    match role {
        "FunctionParam" | "MethodParam" => Some("type_use_param_slot"),
        "FieldType" => Some("type_use_field_slot"),
        "TraitSuper" => Some("type_use_trait_super_slot"),
        "GenericBound" => Some("type_use_generic_bound_slot"),
        "GenericParamBound" => Some("type_use_generic_param_bound_slot"),
        "WherePredicateSubject" => Some("type_use_where_subject_slot"),
        "WherePredicateBound" => Some("type_use_where_bound_slot"),
        "WhereGenericParamBound" => Some("type_use_where_generic_param_bound_slot"),
        "AssociatedTypeBound" => Some("type_use_associated_type_bound_slot"),
        "FunctionReturn" | "MethodReturn" | "TypeAliasTarget" | "ImplSelf" | "ImplTrait"
        | "ConstType" | "StaticType" => None,
        other => panic!("unexpected type-use role {other}"),
    }
}
