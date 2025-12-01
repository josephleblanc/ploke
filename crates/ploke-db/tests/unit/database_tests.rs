use cozo::DataValue;
use ploke_db::to_usize;
use ploke_db::Database;
use uuid::Uuid;
use std::collections::BTreeMap;

#[cfg(test)]
#[ignore = "outdated test, not useful"]
fn create_test_db_for_embedding_updates() -> Database {
    let db = Database::init_with_schema().unwrap();

    let function_id = Uuid::new_v4();
    let module_id = Uuid::new_v4();

    // Create mock function data for embedding tests
    let function_script = r#"
        ?[id, tracking_hash, module_id, name, span] <- [
            [
                $function_id,
                $tracking_hash,
                $module_id,
                $name,
                $span
            ]
        ]
        :put function {id, tracking_hash, module_id, name, span}
        "#;

    let mut function_params = BTreeMap::new();
    function_params.insert("function_id".to_string(), DataValue::Uuid(cozo::UuidWrapper(function_id)));
    function_params.insert("tracking_hash".to_string(), DataValue::Uuid(cozo::UuidWrapper(Uuid::new_v4())));
    function_params.insert("module_id".to_string(), DataValue::Uuid(cozo::UuidWrapper(module_id)));
    function_params.insert("name".to_string(), DataValue::Str("test_function".into()));
    function_params.insert("span".to_string(), DataValue::List(vec![DataValue::Num(cozo::Num::Int(0)), DataValue::Num(cozo::Num::Int(100))]));

    db.run_script(
        function_script,
        function_params,
        cozo::ScriptMutability::Mutable,
    ).unwrap();

    let module_script = r#"
        ?[id, path] <- [
            [$module_id, $path]
        ]
        :put module { id, path }
        "#;

    let mut module_params = BTreeMap::new();
    module_params.insert("module_id".to_string(), DataValue::Uuid(cozo::UuidWrapper(module_id)));
    module_params.insert("path".to_string(), DataValue::List(vec![DataValue::Str("crate".into())]));

    db.run_script(
        module_script,
        module_params,
        cozo::ScriptMutability::Mutable,
    ).unwrap();

    db
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_update_embeddings_batch_empty() {
    let db = create_test_db_for_embedding_updates();
    db.update_embeddings_batch(vec![]).unwrap();
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_update_embeddings_batch_single() {
    let db = create_test_db_for_embedding_updates();
    let id = Uuid::new_v4();
    // Use 384-dimensional vector to match schema
    let embedding = vec![0.5f32; 384]; 
    
    db.update_embeddings_batch(vec![(id, embedding)])
        .unwrap();
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_update_embeddings_invalid_input() {
    let db = create_test_db_for_embedding_updates();
    let result = db.update_embeddings_batch(vec![(Uuid::new_v4(), vec![])]);
    
    assert!(
        result.is_err(),
        "Update with invalid vector length should fail"
    );
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_pending_embedding_count() {
    let db = create_test_db_for_embedding_updates();
    let count = db.count_pending_embeddings().unwrap();
    assert!(count > 0, "Expected pending embeddings");
}

#[test]
fn test_into_usize_valid() {
    let mut rows = vec![vec![DataValue::from(42i64)]];
    let row = rows.pop().unwrap();
    let result = to_usize(&row[0]);
    
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_into_usize_invalid() {
    let mut rows = vec![vec![DataValue::Null]];
    let row = rows.pop().unwrap();
    let result = to_usize(&row[0]);
    
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_simple_function_insert() {
    let db = Database::init_with_schema().unwrap();
    let function_id = Uuid::new_v4();
    let module_id = Uuid::new_v4();
    let tracking_hash = Uuid::new_v4();

    let script = r#"
        ?[id, tracking_hash, module_id, name, span] <- [
            [$function_id, $tracking_hash, $module_id, $name, $span]
        ]
        :put function {id, tracking_hash, module_id, name, span}
    "#;

    let mut params = BTreeMap::new();
    params.insert("function_id".to_string(), DataValue::Uuid(cozo::UuidWrapper(function_id)));
    params.insert("tracking_hash".to_string(), DataValue::Uuid(cozo::UuidWrapper(tracking_hash)));
    params.insert("module_id".to_string(), DataValue::Uuid(cozo::UuidWrapper(module_id)));
    params.insert("name".to_string(), DataValue::Str("test_function".into()));
    params.insert("span".to_string(), DataValue::List(vec![DataValue::Num(cozo::Num::Int(0)), DataValue::Num(cozo::Num::Int(100))]));

    let result = db.run_script(script, params, cozo::ScriptMutability::Mutable);
    assert!(result.is_ok(), "Failed to insert function: {:?}", result.err());
}
