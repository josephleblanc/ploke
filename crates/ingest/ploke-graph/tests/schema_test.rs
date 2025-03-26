use crate::test_helpers::insert_visibility;
use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use ploke_graph::schema::{create_schema, insert_sample_data, verify_schema};
use std::collections::BTreeMap;

mod test_helpers;

/// Helper to verify visibility records
fn verify_visibility(
    db: &Db<MemStorage>,
    node_id: i64,
    expected_kind: &str,
    expected_path: Option<Vec<&str>>,
) -> bool {
    let query = match expected_path {
        Some(path) => {
            let path_values: Vec<DataValue> = path.iter().map(|s| DataValue::from(*s)).collect();
            let mut params = BTreeMap::new();
            params.insert("node_id".to_string(), DataValue::from(node_id));
            params.insert("kind".to_string(), DataValue::from(expected_kind));
            params.insert("path".to_string(), DataValue::List(path_values));

            db.run_script(
                r#"
                ?[count(*)] := 
                    *visibility[node_id, kind, path],
                    node_id == $node_id,
                    kind == $kind,
                    path == $path
                "#,
                params,
                ScriptMutability::Immutable,
            )
        }
        None => {
            let mut params = BTreeMap::new();
            params.insert("node_id".to_string(), DataValue::from(node_id));
            params.insert("kind".to_string(), DataValue::from(expected_kind));

            db.run_script(
                r#"
                ?[count(*)] := 
                    *visibility[node_id, kind, path],
                    node_id == $node_id,
                    kind == $kind,
                    path == null
                "#,
                params,
                ScriptMutability::Immutable,
            )
        }
    };

    match query {
        Ok(result) => {
            if result.rows.is_empty() {
                false
            } else {
                let count = result.rows[0][0].get_int().unwrap_or(0);
                count > 0
            }
        }
        Err(e) => {
            eprintln!("Visibility query failed: {:?}", e);
            false
        }
    }
}

#[test]
fn test_schema_creation() {
    // Create an in-memory database
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    create_schema(&db).expect("Failed to create schema");

    // Verify the schema was created by listing relations
    // Clippy is setting off false warnings due to the cfg flag that prints the results, so we have
    // to use this flag
    #[allow(unused_variables)]
    let result = db
        .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to list relations");

    #[cfg(feature = "debug")]
    println!("Relations: {:?}", result);

    // Enhanced visibility relation check
    let result = db
        .run_script(
            "::columns visibility",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to get visibility columns");

    let expected_columns = vec!["node_id", "kind", "path"];
    let actual_columns: Vec<_> = result
        .rows
        .iter()
        .map(|r| r[0].get_str().unwrap())
        .collect();
    assert_eq!(
        actual_columns, expected_columns,
        "Visibility columns mismatch"
    );

    // Test all visibility variants
    let test_cases = vec![
        (100, "public", None),
        (101, "crate", None),
        (102, "restricted", Some(vec!["super"])),
        (103, "restricted", Some(vec!["crate", "module"])),
        (104, "inherited", None),
    ];

    // First insert all test cases
    for (id, kind, path) in &test_cases {
        insert_visibility(&db, *id, kind, path.clone())
            .unwrap_or_else(|_| panic!("Failed to insert visibility {} {}", id, kind));
    }

    // Then verify them in a separate pass
    for (id, kind, path) in test_cases {
        // Retry verification up to 3 times with small delay
        let mut verified = false;
        for _ in 0..3 {
            if verify_visibility(&db, id, kind, path.clone()) {
                verified = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !verified {
            // Print debug info on failure
            let result = db.run_script(
                r#"
                ?[id, kind, path] := *visibility[id, kind, path],
                    id == $node_id
                "#,
                BTreeMap::from([
                    ("node_id".into(), DataValue::from(id))
                ]),
                ScriptMutability::Immutable,
            ).expect("Failed to query visibility for debugging");
            
            let actual_count = result.rows[0][0].get_int().unwrap_or(0);
            panic!(
                "Visibility verification failed for {} {}\n\
                Expected: kind={}, path={:?}\n\
                Actual count: {}\n\
                Full records:\n{:?}",
                id, kind, kind, path, actual_count, result
            );
        }
    }

    // Test visibility index queries
    let result = db
        .run_script(
            r#"
            ?[id, kind] := 
                *visibility[id, kind, _],
                kind == "restricted"
            "#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query restricted visibility");

    assert!(
        result
            .rows
            .iter()
            .any(|r| r[0].get_int() == Some(102) || r[0].get_int() == Some(103)),
        "Expected to find restricted visibility items"
    );

    // Verify we can insert and query sample data
    insert_sample_data(&db).expect("Failed to insert sample data");
    verify_schema(&db).expect("Failed to verify schema");

    // Test a query joining functions and visibility
    let result = db
        .run_script(
            r#"
            ?[fn_name, vis_kind] := 
                *functions[fn_id, fn_name, _, _, _, _],
                *visibility[fn_id, vis_kind, _]
            "#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query functions with visibility");

    assert!(
        !result.rows.is_empty(),
        "Expected functions with visibility"
    );
}

#[test]
fn test_visibility_path_queries() {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    create_schema(&db).expect("Failed to create schema");

    // Insert test data
    insert_visibility(&db, 1, "restricted", Some(vec!["super", "module"]))
        .expect("Failed to insert visibility");
    insert_visibility(&db, 2, "restricted", Some(vec!["crate"]))
        .expect("Failed to insert visibility");

    // Test path membership using list indexing
    let result = db
        .run_script(
            r#"
            ?[node_id] := 
                *visibility[node_id, "restricted", path],
                path != null,
                is_in("super", path)
            "#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query path membership");

    assert_eq!(result.rows.len(), 1, "Expected one match for path membership");
    assert_eq!(
        result.rows[0][0].get_int(),
        Some(1),
        "Expected id 1 for path membership"
    );

    // Test exact path match
    let mut params = BTreeMap::new();
    params.insert(
        "path".to_string(),
        DataValue::List(vec![DataValue::from("crate")]),
    );

    let result = db
        .run_script(
            r#"
            ?[id] := 
                *visibility[id, "restricted", $path]
            "#,
            params,
            ScriptMutability::Immutable,
        )
        .expect("Failed to query exact path match");

    assert_eq!(result.rows.len(), 1, "Expected one exact path match");
    assert_eq!(
        result.rows[0][0].get_int(),
        Some(2),
        "Expected id 2 for exact path match"
    );
}

#[test]
fn test_indices() {
    // Create an in-memory database
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema (which includes indices)
    create_schema(&db).expect("Failed to create schema");

    // Verify indices were created
    let result = db
        .run_script(
            "::indices functions",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");

    #[cfg(feature = "debug")]
    println!("Indices for functions: {:?}", result);
    assert!(
        !result.rows.is_empty(),
        "Expected at least one index for functions"
    );

    // Check relations indices
    let result = db
        .run_script(
            "::indices relations",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");

    #[cfg(feature = "debug")]
    println!("Indices for relations: {:?}", result);
    assert!(
        !result.rows.is_empty(),
        "Expected at least one index for relations"
    );
}
