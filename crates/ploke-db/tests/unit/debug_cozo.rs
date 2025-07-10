use cozo::{DbInstance, ScriptMutability};
use cozo::DataValue;
use std::collections::BTreeMap;

#[tokio::test]
async fn debug_cozo_update_syntax() {
    // Create a simple in-memory database
    let db = DbInstance::new("mem", "", Default::default()).unwrap();
    
    // First, let's create a simple table with id and embedding columns
    let create_script = r#"
    :create test_table {
        id: Uuid
        =>
        embedding: [Float]
    }
    "#;
    
    let result = db.run_script(create_script, Default::default(), ScriptMutability::Mutable);
    println!("Create table result: {:?}", result);
    
    // Insert some test data
    let insert_script = r#"
    ?[id, embedding] <- [
        [to_uuid("11111111-1111-1111-1111-111111111111"), [1.0, 2.0, 3.0]],
        [to_uuid("22222222-2222-2222-2222-222222222222"), [4.0, 5.0, 6.0]]
    ]
    :insert test_table { id, embedding }
    "#;
    
    let result = db.run_script(insert_script, Default::default(), ScriptMutability::Mutable);
    println!("Insert result: {:?}", result);
    
    // Now let's test different ways of structuring the update parameter
    
    // Test 1: Direct literal format (this should work)
    println!("\n=== Test 1: Direct literal format ===");
    let update_literal = r#"
        ?[id, embedding] <- [
            [to_uuid("11111111-1111-1111-1111-111111111111"), [10.0, 20.0, 30.0]]
        ]
        :update test_table { id, embedding }
    "#;
    
    let result = db.run_script(update_literal, Default::default(), ScriptMutability::Mutable);
    println!("Update literal result: {:?}", result);
    
    // Test 2: Using parameter as we currently do
    println!("\n=== Test 2: Using $updates parameter (our current approach) ===");
    let update_param_script = r#"
        ?[id, embedding] <- $updates
        :update test_table { id, embedding }
    "#;
    
    // Create the parameter data structure as we currently do
    let uuid_val = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!("11111111-1111-1111-1111-111111111111")));
    let embedding_val = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(100.0)),
        DataValue::Num(cozo::Num::Float(200.0)),
        DataValue::Num(cozo::Num::Float(300.0)),
    ]);
    
    let updates_data = vec![
        DataValue::List(vec![uuid_val, embedding_val])
    ];
    
    let mut params = BTreeMap::new();
    params.insert("updates".to_string(), DataValue::List(updates_data));
    
    println!("Parameters: {:?}", params);
    
    let result = db.run_script(update_param_script, params, ScriptMutability::Mutable);
    println!("Update param result: {:?}", result);
    
    // Test 3: Let's try a simpler debug - just select the parameter to see what it looks like
    println!("\n=== Test 3: Debug - just select the parameter ===");
    let debug_script = r#"
        ?[id, embedding] <- $updates
        :limit 10
    "#;
    
    let uuid_val2 = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!("22222222-2222-2222-2222-222222222222")));
    let embedding_val2 = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(400.0)),
        DataValue::Num(cozo::Num::Float(500.0)),
        DataValue::Num(cozo::Num::Float(600.0)),
    ]);
    
    let debug_updates = vec![
        DataValue::List(vec![uuid_val2, embedding_val2])
    ];
    
    let mut debug_params = BTreeMap::new();
    debug_params.insert("updates".to_string(), DataValue::List(debug_updates));
    
    let result = db.run_script(debug_script, debug_params, ScriptMutability::Mutable);
    println!("Debug select result: {:?}", result);
    
    // Test 4: Let's also check what the actual table structure looks like
    println!("\n=== Test 4: Check table structure ===");
    let describe_script = r#"
::explain { 
        :put
}
    "#;
    
    let result = db.run_script(describe_script, Default::default(), ScriptMutability::Immutable);
    println!("Describe table result: {:?}", result);
    
    // Test 5: Show current table contents
    println!("\n=== Test 5: Show table contents ===");
    let select_script = r#"
        ?[id, embedding] := *test_table[id, embedding]
    "#;
    
    let result = db.run_script(select_script, Default::default(), ScriptMutability::Immutable);
    println!("Table contents: {:?}", result);
}

// Helper function to test different parameter structures
fn test_different_param_structures() {
    println!("\n=== Testing different parameter structures ===");
    
    // Structure 1: List of [id, embedding] pairs
    let uuid_val = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!("11111111-1111-1111-1111-111111111111")));
    let embedding_val = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(1.0)),
        DataValue::Num(cozo::Num::Float(2.0)),
    ]);
    
    let structure1 = DataValue::List(vec![
        DataValue::List(vec![uuid_val.clone(), embedding_val.clone()])
    ]);
    println!("Structure 1 (our current): {:?}", structure1);
    
    // Structure 2: Maybe it expects named fields?
    // This is probably not right, but let's see
    let structure2 = DataValue::List(vec![
        DataValue::List(vec![
            DataValue::Str("id".into()),
            uuid_val.clone(),
            DataValue::Str("embedding".into()),
            embedding_val.clone(),
        ])
    ]);
    println!("Structure 2 (with field names): {:?}", structure2);
}
