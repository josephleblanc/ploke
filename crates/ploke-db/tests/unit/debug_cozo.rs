use cozo::DataValue;
use cozo::{DbInstance, ScriptMutability};
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
        [to_uuid("33333333-3333-3333-3333-333333333333"), [4.0, 5.0, 6.0]]
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

    let result = db.run_script(
        update_literal,
        Default::default(),
        ScriptMutability::Mutable,
    );
    println!("Update literal result: {:?}", result);

    // Test 2: Using parameter as we currently do
    println!("\n=== Test 2: Using $updates parameter (our current approach) ===");
    let update_param_script = r#"
        ?[id, embedding] <- $updates
        :update test_table { id, embedding }
    "#;

    // Create the parameter data structure as we currently do
    let uuid_val = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!(
        "11111111-1111-1111-1111-111111111111"
    )));
    let embedding_val = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(100.0)),
        DataValue::Num(cozo::Num::Float(200.0)),
        DataValue::Num(cozo::Num::Float(300.0)),
    ]);

    let updates_data = vec![DataValue::List(vec![uuid_val, embedding_val])];

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

    let uuid_val2 = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!(
        "22222222-2222-2222-2222-222222222222"
    )));
    let embedding_val2 = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(400.0)),
        DataValue::Num(cozo::Num::Float(500.0)),
        DataValue::Num(cozo::Num::Float(600.0)),
    ]);

    let debug_updates = vec![DataValue::List(vec![uuid_val2, embedding_val2])];

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

    let result = db.run_script(
        describe_script,
        Default::default(),
        ScriptMutability::Immutable,
    );
    println!("Describe table result: {:?}", result);

    // Test 5: Show current table contents
    println!("\n=== Test 5: Show table contents ===");
    let select_script = r#"
        ?[id, embedding] := *test_table[id, embedding]
    "#;

    let result = db.run_script(
        select_script,
        Default::default(),
        ScriptMutability::Immutable,
    );
    println!("Table contents: {:?}", result);

    println!("\n=== Test 6: Use conditional update ===");
    let update_conditional = r#"
{:create _test {a}}

%loop
    %if { len[count(x)] := *_test[x]; ?[x] := len[z], x = z >= 10 }
        %then %return _test
    %end
    { ?[a] := a = rand_uuid_v1(); :put _test {a} }
%end
    "#;
    let result = db.run_script(
        update_conditional,
        Default::default(),
        ScriptMutability::Mutable,
    );
    println!("Update literal result: {:?}", result);

    println!("\n=== Test 7: Use conditional update realistic ===");
    // {:create _test {id, embedding}}
    let update_conditional_realistic = r#"
{
    ?[test_id, test_embedding] <- [[to_uuid("55555555-5555-5555-5555-555555555555"), [50.0, 20.0, 30.0]]] 
    :replace _test {test_id, test_embedding} 
} 

%if { 
        ?[id, embedding] := *_test {test_id: id, test_embedding: embedding}, *test_table { id, embedding }
    }
    %then {
        ?[id, embedding] <- [
            [to_uuid("55555555-5555-5555-5555-555555555555"), [50.0, 20.0, 30.0]]
        ]
        :update test_table { id, embedding }
    }
%end
    "#;
    let result = db.run_script(
        update_conditional_realistic,
        Default::default(),
        ScriptMutability::Mutable,
    );
    println!("Update literal result: {:?}", result);

    let update_param_script2 = r#"
{
    ?[test_id, test_embedding] <- $updates 
    :replace _test {test_id, test_embedding} 
} 
{ 
    ?[id, embedding] := *_test{test_id: id, test_embedding: embedding}, *test_table{id}
    :update test_table {id, embedding}
}
"#;

    // Create the parameter data structure as we currently do
    let uuid_val = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!(
        "11111111-1111-1111-1111-111111111111"
    )));
    let embedding_val = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(1.0)),
        DataValue::Num(cozo::Num::Float(2.0)),
        DataValue::Num(cozo::Num::Float(3.0)),
    ]);
    let uuid_val2 = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!(
        "22222222-2222-2222-2222-222222222222"
    )));
    let embedding_val2 = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(9.0)),
        DataValue::Num(cozo::Num::Float(8.0)),
        DataValue::Num(cozo::Num::Float(7.0)),
    ]);
    let uuid_val3 = DataValue::Uuid(cozo::UuidWrapper(uuid::uuid!(
        "99999999-9999-9999-9999-999999999999"
    )));
    let embedding_val3 = DataValue::List(vec![
        DataValue::Num(cozo::Num::Float(10.0)),
        DataValue::Num(cozo::Num::Float(10.0)),
        DataValue::Num(cozo::Num::Float(1.0)),
    ]);

    let updates_data = vec![
        DataValue::List(vec![uuid_val, embedding_val]),
        DataValue::List(vec![uuid_val2, embedding_val2]),
        DataValue::List(vec![uuid_val3, embedding_val3]),
    ];

    let mut params = BTreeMap::new();
    params.insert("updates".to_string(), DataValue::List(updates_data));

    println!("Parameters: {:?}", params);

    let result = db.run_script(update_param_script2, params, ScriptMutability::Mutable);
    println!("Update param result: {:?}", result);

    let check_val_script = "?[id, embedding] := *test_table {id, embedding}";
    let result = db.run_script(
        check_val_script,
        BTreeMap::new(),
        ScriptMutability::Immutable,
    );
    println!("Update param result: {:?}", result);
}

// Helper function to test different parameter structures
