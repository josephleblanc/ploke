use super::*;

pub(in crate::unit) fn const_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *const {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(in crate::unit) fn static_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *static {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}
