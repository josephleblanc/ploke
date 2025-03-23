use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use graph::schema::create_schema;
use std::collections::BTreeMap;

#[test]
fn test_comprehensive_schema() {
    // Create an in-memory database
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    create_schema(&db).expect("Failed to create schema");

    // Insert sample data for new relations
    insert_sample_type_alias(&db).expect("Failed to insert type alias");
    insert_sample_union(&db).expect("Failed to insert union");
    insert_sample_value(&db).expect("Failed to insert value");
    insert_sample_macro(&db).expect("Failed to insert macro");
    insert_sample_type_details(&db).expect("Failed to insert type details");
    insert_sample_module_relationship(&db).expect("Failed to insert module relationship");

    // Test complex queries
    test_find_implementations(&db).expect("Failed to test find implementations");
    test_find_type_usages(&db).expect("Failed to test find type usages");
    test_module_hierarchy(&db).expect("Failed to test module hierarchy");
}

fn insert_sample_type_alias(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    let params = BTreeMap::from([
        ("id".to_string(), DataValue::from(10)),
        ("name".to_string(), DataValue::from("StringVec")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("type_id".to_string(), DataValue::from(1)),
        (
            "docstring".to_string(),
            DataValue::from("Type alias for Vec<String>"),
        ),
    ]);

    db.run_script(
        "?[id, name, visibility, type_id, docstring] <- [[$id, $name, $visibility, $type_id, $docstring]] :put type_aliases",
        params,
        ScriptMutability::Mutable,
    )
}

fn insert_sample_union(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    let params = BTreeMap::from([
        ("id".to_string(), DataValue::from(11)),
        ("name".to_string(), DataValue::from("IntOrFloat")),
        ("visibility".to_string(), DataValue::from("Public")),
        (
            "docstring".to_string(),
            DataValue::from("Union of int and float"),
        ),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put unions",
        params,
        ScriptMutability::Mutable,
    )
}

fn insert_sample_value(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    let params = BTreeMap::from([
        ("id".to_string(), DataValue::from(12)),
        ("name".to_string(), DataValue::from("MAX_SIZE")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("type_id".to_string(), DataValue::from(1)),
        ("kind".to_string(), DataValue::from("Constant")),
        ("value".to_string(), DataValue::from("100")),
        (
            "docstring".to_string(),
            DataValue::from("Maximum size constant"),
        ),
    ]);

    db.run_script(
        "?[id, name, visibility, type_id, kind, value, docstring] <- [[$id, $name, $visibility, $type_id, $kind, $value, $docstring]] :put values",
        params,
        ScriptMutability::Mutable,
    )
}

fn insert_sample_macro(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    let params = BTreeMap::from([
        ("id".to_string(), DataValue::from(13)),
        ("name".to_string(), DataValue::from("debug_print")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("kind".to_string(), DataValue::from("DeclarativeMacro")),
        (
            "docstring".to_string(),
            DataValue::from("Debug print macro"),
        ),
        (
            "body".to_string(),
            DataValue::from("println!(\"Debug: {}\", $expr)"),
        ),
    ]);

    db.run_script(
        "?[id, name, visibility, kind, docstring, body] <- [[$id, $name, $visibility, $kind, $docstring, $body]] :put macros",
        params,
        ScriptMutability::Mutable,
    )
}

fn insert_sample_type_details(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    let params = BTreeMap::from([
        ("type_id".to_string(), DataValue::from(1)),
        ("is_mutable".to_string(), DataValue::from(false)),
        ("lifetime".to_string(), DataValue::from("'static")),
        ("abi".to_string(), DataValue::Null),
        ("is_unsafe".to_string(), DataValue::from(false)),
        ("is_extern".to_string(), DataValue::from(false)),
        ("dyn_token".to_string(), DataValue::from(false)),
    ]);

    db.run_script(
        "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
        params,
        ScriptMutability::Mutable,
    )
}

fn insert_sample_module_relationship(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    // First, insert a module
    let module_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(20)),
        ("name".to_string(), DataValue::from("parent_module")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("docstring".to_string(), DataValue::from("Parent module")),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put modules",
        module_params,
        ScriptMutability::Mutable,
    )?;

    // Insert another module
    let submodule_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(21)),
        ("name".to_string(), DataValue::from("child_module")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("docstring".to_string(), DataValue::from("Child module")),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put modules",
        submodule_params,
        ScriptMutability::Mutable,
    )?;

    // Create a relationship between them
    let relation_params = BTreeMap::from([
        ("module_id".to_string(), DataValue::from(20)),
        ("related_id".to_string(), DataValue::from(21)),
        ("kind".to_string(), DataValue::from("Contains")),
    ]);

    db.run_script(
        "?[module_id, related_id, kind] <- [[$module_id, $related_id, $kind]] :put module_relationships",
        relation_params,
        ScriptMutability::Mutable,
    )
}

fn test_find_implementations(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    // Insert a trait
    let trait_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(30)),
        ("name".to_string(), DataValue::from("SampleTrait")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("docstring".to_string(), DataValue::from("A sample trait")),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put traits",
        trait_params,
        ScriptMutability::Mutable,
    )?;

    // Insert an impl
    let impl_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(31)),
        ("self_type_id".to_string(), DataValue::from(2)), // SampleStruct
        ("trait_type_id".to_string(), DataValue::from(30)), // SampleTrait
    ]);

    db.run_script(
        "?[id, self_type_id, trait_type_id] <- [[$id, $self_type_id, $trait_type_id]] :put impls",
        impl_params,
        ScriptMutability::Mutable,
    )?;

    // Query to find all implementations of a trait
    let query = r#"
        ?[struct_name, trait_name] := 
            *traits[trait_id, trait_name, _, _],
            *impls[_, struct_id, trait_id],
            *structs[struct_id, struct_name, _, _]
    "#;

    // Insert sample data for testing
    insert_sample_data(db)?;

    let result = db.run_script(query, BTreeMap::new(), ScriptMutability::Immutable)?;

    #[cfg(feature = "debug")]
    println!("Implementations: {:?}", result);

    assert_eq!(result.rows.len(), 1, "Expected 1 implementation");
    assert_eq!(
        result.rows[0][0].to_string(),
        "SampleStruct",
        "Expected struct name to be 'SampleStruct'"
    );
    assert_eq!(
        result.rows[0][1].to_string(),
        "SampleTrait",
        "Expected trait name to be 'SampleTrait'"
    );

    Ok(())
}

fn test_find_type_usages(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    // Query to find all functions that use a specific type
    let query = r#"
        ?[fn_name, type_str] := 
            *functions[fn_id, fn_name, _, _, _, _],
            *function_params[fn_id, _, _, type_id, _, _],
            *types[type_id, _, type_str]
    "#;

    let result = db.run_script(query, BTreeMap::new(), ScriptMutability::Immutable)?;

    #[cfg(feature = "debug")]
    println!("Type usages: {:?}", result);

    // We should have at least one function using a type
    assert!(!result.rows.is_empty(), "Expected at least one type usage");

    Ok(())
}

fn test_module_hierarchy(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    // Query to find all submodules
    let query = r#"
        ?[parent_name, child_name] := 
            *modules[parent_id, parent_name, _, _],
            *module_relationships[parent_id, child_id, "Contains"],
            *modules[child_id, child_name, _, _]
    "#;

    let result = db.run_script(query, BTreeMap::new(), ScriptMutability::Immutable)?;

    #[cfg(feature = "debug")]
    println!("Module hierarchy: {:?}", result);

    assert_eq!(result.rows.len(), 1, "Expected 1 module relationship");
    assert_eq!(
        result.rows[0][0].to_string(),
        "parent_module",
        "Expected parent module name to be 'parent_module'"
    );
    assert_eq!(
        result.rows[0][1].to_string(),
        "child_module",
        "Expected child module name to be 'child_module'"
    );

    Ok(())
}
