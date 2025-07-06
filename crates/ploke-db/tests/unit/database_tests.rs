use cozo::DataValue;
use ploke_db::to_usize;
use ploke_db::Database;
use ploke_db::DbError;
use uuid::Uuid;

#[cfg(test)]
fn create_test_db_for_embedding_updates() -> Database {
    let db = Database::init_with_schema().unwrap();
    
    // Create mock function data for embedding tests
    let function_script = r#"
        ?[id, tracking_hash, module_id, name, span] <- [
            [
                $function_id, 
                'test_hash', 
                $module_id,
                'test_function',
                [0, 100]
            ]
        ]
        :put functions {
            id, tracking_hash, 
            module_id, name, span
        }
        "#;
    
    let module_script = r#"
        ?[id, path] <- [
            [$module_id, ['crate']]
        ]
        :put modules { id, path }
        "#;
    
    let function_id = Uuid::new_v4();
    let module_id = Uuid::new_v4();
    
    db.raw_query(&function_script.replace(
        "$function_id", 
        &function_id.simple().to_string()
    )).unwrap();
    
    db.raw_query(&module_script.replace(
        "$module_id", 
        &module_id.simple().to_string()
    )).unwrap();
    
    db
}

#[tokio::test]
async fn test_update_embeddings_batch_empty() {
    let db = create_test_db_for_embedding_updates();
    db.update_embeddings_batch(vec![]).await.unwrap();
}

#[tokio::test]
async fn test_update_embeddings_batch_single() {
    let db = create_test_db_for_embedding_updates();
    let id = Uuid::new_v4();
    let embedding = vec![0.5f32, -0.25f32, 1.0f32];
    
    db.update_embeddings_batch(vec![(id, embedding)])
        .await
        .unwrap();
}

#[tokio::test]
async fn test_update_embeddings_invalid_input() {
    let db = create_test_db_for_embedding_updates();
    let result = db.update_embeddings_batch(vec![(Uuid::new_v4(), vec![])]).await;
    
    assert!(
        result.is_err(),
        "Update with invalid vector length should fail"
    );
}

#[tokio::test]
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
