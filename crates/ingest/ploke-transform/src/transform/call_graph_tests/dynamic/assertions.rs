use super::*;

pub(super) fn assert_dynamic_relation(
    db: &Db<MemStorage>,
    site_id: DataValue,
    target_id: DataValue,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    params.insert("target_id".to_string(), target_id);
    let rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected one persisted {label} dynamic call_relation row"
    );
    assert_dynamic_relation_family(&rows.rows[0]);
    Ok(())
}

pub(super) fn assert_dynamic_candidate_relations(
    db: &Db<MemStorage>,
    site_id: DataValue,
    mut expected_targets: Vec<DataValue>,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    let rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    let mut actual_targets = rows
        .rows
        .iter()
        .map(|row| row[1].clone())
        .collect::<Vec<_>>();
    actual_targets.sort();
    expected_targets.sort();
    assert_eq!(
        actual_targets, expected_targets,
        "{label} dynamic call should persist proven candidates"
    );
    assert_eq!(
        rows.rows.len(),
        2,
        "expected two persisted {label} dynamic candidate rows"
    );
    for row in &rows.rows {
        assert_dynamic_relation_family(row);
    }
    Ok(())
}

fn assert_dynamic_relation_family(row: &[DataValue]) {
    assert_eq!(&row[2], &DataValue::from("DynamicFunction"));
    assert_eq!(&row[3], &DataValue::from("Dynamic"));
    assert_eq!(&row[4], &DataValue::from("Function"));
}

pub(super) fn assert_dynamic_site_path(
    db: &Db<MemStorage>,
    site_id: DataValue,
    expected_path: Option<&[&str]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    let rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(rows.rows.len(), 1);
    assert_eq!(&rows.rows[0][1], &DataValue::from("Dynamic"));
    let expected_path = expected_path
        .map(|path| {
            DataValue::List(
                path.iter()
                    .map(|segment| DataValue::from(*segment))
                    .collect(),
            )
        })
        .unwrap_or(DataValue::Null);
    assert_eq!(&rows.rows[0][2], &expected_path);
    Ok(())
}

pub(super) fn assert_dynamic_status(
    db: &Db<MemStorage>,
    site_id: DataValue,
    expected_status: &str,
    expected_resolution: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    let rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(rows.rows.len(), 1);
    assert_eq!(&rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&rows.rows[0][2], &DataValue::from(expected_status));
    let expected_resolution = expected_resolution
        .map(DataValue::from)
        .unwrap_or(DataValue::Null);
    assert_eq!(&rows.rows[0][3], &expected_resolution);
    Ok(())
}
