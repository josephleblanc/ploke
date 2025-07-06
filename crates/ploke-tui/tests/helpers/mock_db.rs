use std::sync::Arc;
use ploke_db::Database;
use cozo::{Db, MemStorage, ScriptMutability};
use cozo::data::iter::Iter;
use cozo::data::value::DataValue;

pub fn create_mock_db(num_unindexed: usize) -> Arc<Database> {
    let storage = MemStorage::default();
    let db = Arc::new(Database::new(Db::new(storage).unwrap()));
    
    let script = r#"
    ?[id, path, tracking_hash, start_byte, end_byte] <- [
        $unindexed,
    ]

    :create embedding_nodes {
        id => Uuid
    }
    "#;
    
    let mut params = cozo::MapParameter::new();
    let uuids: Vec<DataValue> = (0..num_unindexed)
        .map(|i| DataValue::Uuid(
            uuid::Uuid::from_u128(i as u128).into()
        ))
        .collect();
    
    params.insert("unindexed".into(), DataValue::List(vec![
        DataValue::List(uuids),
        DataValue::List(vec![DataValue::Str("test".into()); num_unindexed]),
        DataValue::List(vec![DataValue::Int(123); num_unindexed]),
        DataValue::List(vec![DataValue::Int(0); num_unindexed]),
        DataValue::List(vec![DataValue::Int(10); num_unindexed]),
    ]));
    
    db.run_script(script, params, ScriptMutability::Mutable).unwrap();
    db
}
