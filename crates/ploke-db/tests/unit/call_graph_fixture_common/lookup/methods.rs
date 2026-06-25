use super::*;

pub(in crate::unit) fn method_id_by_impl_self_type_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(in crate::unit) fn method_owner_is_inherent_impl(
    db: &Database,
    method_id: Uuid,
    self_type_name: &str,
) -> Result<bool, DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "method_id".to_string(),
        DataValue::Uuid(UuidWrapper(method_id)),
    );
    params.insert(
        "self_type_name".to_string(),
        DataValue::from(self_type_name),
    );

    let self_rows = db.raw_query_params(
        r#"?[impl_id] :=
            *method { id: $method_id, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: self_type_id,
                role: "ImplSelf" @ 'NOW'
            },
            *type_relation {
                source_id: self_type_id,
                target_id: self_target_id,
                relation_kind: "Ordinary" @ 'NOW'
            },
            *struct { id: self_target_id, name: $self_type_name @ 'NOW' }"#,
        params.clone(),
    )?;
    let trait_rows = db.raw_query_params(
        r#"?[trait_type_id] :=
            *method { id: $method_id, owner_id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: trait_type_id,
                role: "ImplTrait" @ 'NOW'
            }"#,
        params,
    )?;

    Ok(self_rows.rows.len() == 1 && trait_rows.rows.is_empty())
}

pub(in crate::unit) fn method_id_by_impl_self_type_exact_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "self_type_name".to_string(),
        DataValue::from(self_type_name),
    );
    params.insert("method_name".to_string(), DataValue::from(method_name));
    exactly_one_uuid_params(
        db,
        r#"?[method_id] :=
            *method { id: method_id, name: $method_name, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: self_type_id,
                role: "ImplSelf" @ 'NOW'
            },
            *type_relation {
                source_id: self_type_id,
                target_id: self_target_id,
                relation_kind: "Ordinary" @ 'NOW'
            },
            *struct { id: self_target_id, name: $self_type_name @ 'NOW' }"#,
        params,
        0,
    )
}

pub(in crate::unit) fn method_id_by_impl_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(in crate::unit) fn method_id_by_impl_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(in crate::unit) fn method_id_by_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: trait_id @ 'NOW' }},
                *trait {{ id: trait_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}
