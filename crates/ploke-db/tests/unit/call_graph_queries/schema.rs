use super::*;

#[test]
fn has_call_graph_relations_reports_schema_presence() -> Result<(), DbError> {
    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let empty = Database::new(raw);
    assert!(
        !empty.has_call_graph_relations()?,
        "empty database should not report call-graph relations"
    );

    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        db.has_call_graph_relations()?,
        "full schema should include all call-graph relations"
    );

    Ok(())
}
