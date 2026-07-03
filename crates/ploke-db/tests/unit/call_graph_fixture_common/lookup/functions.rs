use super::*;

pub(in crate::unit) fn function_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(in crate::unit) fn function_id_by_exact_name(
    db: &Database,
    name: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    exactly_one_uuid_params(
        db,
        r#"?[id] :=
            *function { id, name: $name @ 'NOW' }"#,
        params,
        0,
    )
}

pub(in crate::unit) fn function_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_path_literal(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        ),
        0,
    )
}

pub(in crate::unit) fn closure_owner_for_parent(
    db: &Database,
    parent: Uuid,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, owner_kind, parent_kind, label] :=
            parent = to_uuid("{parent}"),
            *call_body_owner {{
                id,
                owner_kind,
                parent_id: parent,
                parent_kind,
                label @ 'NOW'
            }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one closure call_body_owner row for parent {parent}: {:#?}",
        rows.rows
    );
    assert_eq!(
        data_str(&rows.rows[0][1], "call_body_owner.owner_kind"),
        "Closure"
    );
    assert_eq!(
        data_str(&rows.rows[0][2], "call_body_owner.parent_kind"),
        "Function"
    );
    assert_eq!(
        data_str(&rows.rows[0][3], "call_body_owner.label"),
        "closure"
    );
    to_uuid(&rows.rows[0][0])
}
