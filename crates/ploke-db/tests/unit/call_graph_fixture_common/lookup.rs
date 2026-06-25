use super::*;

mod functions;
mod methods;
mod types;
mod values;

pub(in crate::unit) use functions::*;
pub(in crate::unit) use methods::*;
pub(in crate::unit) use types::*;
pub(in crate::unit) use values::*;

fn exactly_one_uuid(db: &Database, script: &str, column: usize) -> Result<Uuid, DbError> {
    let rows = db.raw_query(script)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

fn exactly_one_uuid_params(
    db: &Database,
    script: &str,
    params: BTreeMap<String, DataValue>,
    column: usize,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query_params(script, params)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

fn cozo_path_literal(segments: &[&str]) -> String {
    let joined = segments
        .iter()
        .map(|segment| format!(r#""{segment}""#))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{joined}]")
}
