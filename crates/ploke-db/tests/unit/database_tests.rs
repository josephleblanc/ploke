use cozo::DataValue;
use ploke_db::create_index;
#[cfg(feature = "multi_embedding")]
use ploke_db::multi_embedding::VECTOR_DIMENSION_SPECS;
use ploke_db::to_usize;
use ploke_db::Database;
use ploke_db::NodeType;
use ploke_core::EmbeddingModelId;
use std::collections::BTreeMap;
use uuid::Uuid;

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
    function_params.insert(
        "function_id".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(function_id)),
    );
    function_params.insert(
        "tracking_hash".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(Uuid::new_v4())),
    );
    function_params.insert(
        "module_id".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(module_id)),
    );
    function_params.insert("name".to_string(), DataValue::Str("test_function".into()));
    function_params.insert(
        "span".to_string(),
        DataValue::List(vec![
            DataValue::Num(cozo::Num::Int(0)),
            DataValue::Num(cozo::Num::Int(100)),
        ]),
    );

    db.run_script(
        function_script,
        function_params,
        cozo::ScriptMutability::Mutable,
    )
    .unwrap();

    let module_script = r#"
        ?[id, path] <- [
            [$module_id, $path]
        ]
        :put module { id, path }
        "#;

    let mut module_params = BTreeMap::new();
    module_params.insert(
        "module_id".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(module_id)),
    );
    module_params.insert(
        "path".to_string(),
        DataValue::List(vec![DataValue::Str("crate".into())]),
    );

    db.run_script(
        module_script,
        module_params,
        cozo::ScriptMutability::Mutable,
    )
    .unwrap();

    db
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_update_embeddings_batch_empty() {
    let db = create_test_db_for_embedding_updates();
    db.update_embeddings_batch(vec![]).await.unwrap();
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
async fn test_update_embeddings_batch_single() {
    let db = create_test_db_for_embedding_updates();
    let id = Uuid::new_v4();
    // Use 384-dimensional vector to match schema
    let embedding = vec![0.5f32; 384];

    #[cfg(not(feature = "multi_embedding"))]
    db.update_embeddings_batch(vec![(id, embedding)])
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "outdated test, needs update"]
#[cfg(not(feature = "multi_embedding"))]
async fn test_update_embeddings_invalid_input() {
    let db = create_test_db_for_embedding_updates();
    let result = db
        .update_embeddings_batch(vec![(Uuid::new_v4(), vec![])])
        .await;

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

/// Mirrors the example in `Database::clear_relations` to ensure we keep the contract
/// that user relations are removed while system relations remain.
#[tokio::test]
async fn clear_relations_removes_user_relations() {
    let db = Database::init_with_schema().expect("failed to init schema");
    let before = db.relations_vec().expect("failed to read relations");
    assert!(
        before.iter().any(|name| !name.contains(':')),
        "fixture should include user-defined relations: {before:?}"
    );

    db.clear_relations()
        .await
        .expect("clear_relations should succeed");

    let after = db
        .relations_vec()
        .expect("failed to read relations after clear");
    assert!(
        after.iter().all(|name| name.contains(':')),
        "user relations should be removed, remaining: {after:?}"
    );
}

/// Verifies that `clear_hnsw_idx` removes every `:<relation>:hnsw_idx` entry so
/// we can safely rebuild indices during database maintenance.
#[tokio::test]
async fn clear_hnsw_idx_drops_all_indices() {
    let db = Database::init_with_schema().expect("failed to init schema");
    #[cfg(feature = "multi_embedding")]
    let hnsw_model_info = VECTOR_DIMENSION_SPECS[0].clone();
    #[cfg(feature = "multi_embedding")]
    create_index(&db, NodeType::Function, hnsw_model_info).expect("failed to create hnsw index");
    #[cfg(not(feature = "multi_embedding"))]
    create_index(&db, NodeType::Function).expect("failed to create hnsw index");

    let relations = db.relations_vec().expect("failed to read relations");
    assert!(
        relations.iter().any(|name| name.ends_with(":hnsw_idx")),
        "expected at least one hnsw index relation, got: {relations:?}"
    );

    db.clear_hnsw_idx()
        .await
        .expect("clear_hnsw_idx should succeed");

    let remaining = db
        .relations_vec()
        .expect("failed to read relations after drop attempt");
    assert!(
        remaining.iter().all(|name| !name.ends_with(":hnsw_idx")),
        "expected all hnsw indices removed, remaining: {remaining:?}"
    );
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
    params.insert(
        "function_id".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(function_id)),
    );
    params.insert(
        "tracking_hash".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(tracking_hash)),
    );
    params.insert(
        "module_id".to_string(),
        DataValue::Uuid(cozo::UuidWrapper(module_id)),
    );
    params.insert("name".to_string(), DataValue::Str("test_function".into()));
    params.insert(
        "span".to_string(),
        DataValue::List(vec![
            DataValue::Num(cozo::Num::Int(0)),
            DataValue::Num(cozo::Num::Int(100)),
        ]),
    );

    let result = db.run_script(script, params, cozo::ScriptMutability::Mutable);
    assert!(
        result.is_ok(),
        "Failed to insert function: {:?}",
        result.err()
    );
}
