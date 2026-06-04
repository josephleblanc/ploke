use cozo::{DataValue, UuidWrapper, Vector};
use ploke_db::{Database, DbError, to_usize};
use std::collections::BTreeMap;
use uuid::Uuid;

fn fixture_db() -> Database {
    Database::new(ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes").unwrap())
}

fn active_embedding_relation_and_dims(db: &Database) -> (String, usize) {
    db.with_active_set(|set| (set.rel_name.to_string(), set.dims() as usize))
        .unwrap()
}

fn first_unembedded_node_id(db: &Database) -> Uuid {
    db.get_unembedded_node_data(10, 0)
        .unwrap()
        .iter()
        .flat_map(|typed| typed.v.iter())
        .map(|node| node.id)
        .next()
        .expect("fixture_nodes should have at least one unembedded node")
}

fn stored_vector_for_node(db: &Database, relation: &str, node_id: Uuid) -> Vec<f64> {
    let query = format!("?[vector] := *{relation} {{ node_id: $node_id, vector }}");
    let mut params = BTreeMap::new();
    params.insert("node_id".to_string(), DataValue::Uuid(UuidWrapper(node_id)));

    let result = db
        .run_script(&query, params, cozo::ScriptMutability::Immutable)
        .unwrap();
    assert_eq!(
        result.rows.len(),
        1,
        "expected one embedding row for node {node_id} in relation {relation}"
    );

    match &result.rows[0][0] {
        DataValue::List(values) => values
            .iter()
            .map(|value| match value {
                DataValue::Num(cozo::Num::Float(f)) => *f,
                DataValue::Num(cozo::Num::Int(i)) => *i as f64,
                other => panic!("expected numeric vector element, got {other:?}"),
            })
            .collect(),
        DataValue::Vec(Vector::F32(values)) => {
            values.iter().map(|value| f64::from(*value)).collect()
        }
        DataValue::Vec(Vector::F64(values)) => values.iter().copied().collect(),
        other => panic!("expected vector list, got {other:?}"),
    }
}

fn assert_query_execution_contains(err: DbError, expected: &str) {
    match err {
        DbError::QueryExecution(message) => assert!(
            message.contains(expected),
            "expected query execution error containing {expected:?}, got {message:?}"
        ),
        other => panic!("expected DbError::QueryExecution, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "fixture-backed embedding contract"]
async fn test_update_embeddings_batch_empty() {
    let db = fixture_db();
    let before = db.count_pending_embeddings().unwrap();

    db.update_embeddings_batch(vec![]).unwrap();

    assert_eq!(db.count_pending_embeddings().unwrap(), before);
}

#[tokio::test]
#[ignore = "fixture-backed embedding contract"]
async fn test_update_embeddings_batch_single() {
    let db = fixture_db();
    let (relation, dims) = active_embedding_relation_and_dims(&db);
    let node_id = first_unembedded_node_id(&db);
    let before = db.count_pending_embeddings().unwrap();
    let embedding: Vec<f32> = (0..dims).map(|i| i as f32 / dims as f32).collect();

    db.update_embeddings_batch(vec![(node_id, embedding.clone())])
        .unwrap();

    assert_eq!(db.count_pending_embeddings().unwrap(), before - 1);
    let stored = stored_vector_for_node(&db, &relation, node_id);
    assert_eq!(stored.len(), dims);
    for (actual, expected) in stored.iter().zip(embedding.iter()) {
        assert_eq!(*actual, f64::from(*expected));
    }
}

#[tokio::test]
#[ignore = "fixture-backed embedding contract"]
async fn test_update_embeddings_invalid_input() {
    let db = fixture_db();
    let (_, dims) = active_embedding_relation_and_dims(&db);
    let node_id = first_unembedded_node_id(&db);

    let empty_err = db
        .update_embeddings_batch(vec![(node_id, Vec::new())])
        .expect_err("empty embedding vector should be rejected");
    assert_query_execution_contains(empty_err, "Embedding vector must not be empty");

    let wrong_len = if dims == 1 { 2 } else { dims - 1 };
    let shape_err = db
        .update_embeddings_batch(vec![(node_id, vec![0.25; wrong_len])])
        .expect_err("embedding vector with wrong length should be rejected");
    assert_query_execution_contains(shape_err, "does not match active embedding set dimension");
}

#[tokio::test]
#[ignore = "fixture-backed embedding contract"]
async fn test_pending_embedding_count() {
    let db = fixture_db();
    let all_pending = db.count_pending_embeddings().unwrap();
    let non_file_pending = db.count_unembedded_nonfiles().unwrap();
    let file_pending = db.count_unembedded_files().unwrap();

    assert!(all_pending > 0, "fixture should have pending embeddings");
    assert_eq!(all_pending, non_file_pending + file_pending);
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
