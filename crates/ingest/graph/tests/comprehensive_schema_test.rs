use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use graph::schema::{create_schema, insert_sample_data};
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
    test_advanced_graph_traversal(&db).expect("Failed to test advanced graph traversal");
    test_vector_similarity_search(&db).expect("Failed to test vector similarity search");
    test_hnsw_graph_walking(&db).expect("Failed to test HNSW graph walking");
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

fn insert_sample_embeddings(db: &Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    // Create a sample embedding vector (384 dimensions)
    // We'll use a simple pattern for the vector values
    let embedding_values = (0..384)
        .map(|i| format!("{:.6}", i as f64 / 384.0))
        .collect::<Vec<String>>()
        .join(", ");
    
    // Insert a sample embedding for a function using raw script
    let script = format!(
        r#"
        ?[id, node_id, node_type, embedding, text_snippet] <- [[
            1, 
            1, 
            "Function", 
            vec([{}]), 
            "fn sample_function(input: String) -> String {{ println!(\"Hello\"); input }}"
        ]] :put code_embeddings
        "#,
        embedding_values
    );

    db.run_script(
        &script,
        BTreeMap::new(),
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
        // AI: Apply this pattern to the other tests that take strings,
        // e.g. SampleTrait, which are failing tests because the output of
        // result.rows[0][0].to_string() for SampleStruct is actually "\"SampleStruct"\"
        result.rows[0][0].get_str(),
        Some("SampleStruct"),
        "Expected struct name to be 'SampleStruct'"
    );
    assert_eq!(
        result.rows[0][1].get_str(),
        Some("SampleTrait"),
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
        result.rows[0][0].get_str(),
        Some("parent_module"),
        "Expected parent module name to be 'parent_module'"
    );
    assert_eq!(
        result.rows[0][1].get_str(),
        Some("child_module"),
        "Expected child module name to be 'child_module'"
    );

    Ok(())
}

// Test vector similarity search using HNSW
fn test_vector_similarity_search(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    // Insert sample embeddings if not already done
    insert_sample_embeddings(db)?;
    
    // Create a query vector using the vec function in CozoScript
    // We'll use the same vector as in our sample data for perfect similarity
    let embedding_values = (0..384)
        .map(|i| format!("{:.6}", i as f64 / 384.0))
        .collect::<Vec<String>>()
        .join(", ");
    
    // Query to find similar code snippets using HNSW index
    let query = format!(
        r#"
        ?[node_type, text_snippet, dist] := 
            ~code_embeddings:vector{{node_type, text_snippet | 
                query: vec([{}]), 
                k: 5, 
                ef: 50,
                bind_distance: dist
            }}
        "#,
        embedding_values
    );
    
    let result = db.run_script(&query, BTreeMap::new(), ScriptMutability::Immutable)?;
    
    #[cfg(feature = "debug")]
    println!("Vector search results: {:?}", result);
    
    // We should have at least one result
    assert!(!result.rows.is_empty(), "Expected at least one vector search result");
    
    // The first result should have a very low distance (close to 0.0)
    // Since we're using the same vector, it should be almost exactly 0.0
    let distance = result.rows[0][2].get_float().unwrap_or(1.0);
    assert!(distance < 0.01, "Expected low distance score, got {}", distance);
    
    Ok(())
}

// Test walking the HNSW graph directly
fn test_hnsw_graph_walking(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    // Insert sample embeddings if not already done
    insert_sample_embeddings(db)?;
    
    // Query to walk the HNSW graph at layer 0
    let query = r#"
        ?[fr_node_type, to_node_type, dist] := 
            *code_embeddings:vector{
                layer: 0, 
                fr_node_type: "Function", 
                to_node_type, 
                dist
            }
        :limit 10
    "#;
    
    let result = db.run_script(query, BTreeMap::new(), ScriptMutability::Immutable)?;
    
    #[cfg(feature = "debug")]
    println!("HNSW graph walking results: {:?}", result);
    
    // The graph might be empty if there's only one node, but the query should succeed
    
    Ok(())
}

// Test advanced graph traversal with recursive queries
fn test_advanced_graph_traversal(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    // First, create a more complex module hierarchy
    // grandparent -> parent -> child
    let grandparent_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(22)),
        ("name".to_string(), DataValue::from("grandparent_module")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("docstring".to_string(), DataValue::from("Grandparent module")),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put modules",
        grandparent_params,
        ScriptMutability::Mutable,
    )?;

    // Create relationship: grandparent contains parent
    let relation_params = BTreeMap::from([
        ("module_id".to_string(), DataValue::from(22)),
        ("related_id".to_string(), DataValue::from(20)), // parent_module
        ("kind".to_string(), DataValue::from("Contains")),
    ]);

    db.run_script(
        "?[module_id, related_id, kind] <- [[$module_id, $related_id, $kind]] :put module_relationships",
        relation_params,
        ScriptMutability::Mutable,
    )?;

    // Recursive query to find all descendants of a module
    let recursive_query = r#"
        descendants[ancestor, descendant] := 
            *modules[ancestor_id, ancestor, _, _],
            *module_relationships[ancestor_id, descendant_id, "Contains"],
            *modules[descendant_id, descendant, _, _]
        
        descendants[ancestor, descendant] := 
            descendants[ancestor, intermediate],
            *modules[intermediate_id, intermediate, _, _],
            *module_relationships[intermediate_id, descendant_id, "Contains"],
            *modules[descendant_id, descendant, _, _]
        
        ?[ancestor, descendant] := descendants[ancestor, descendant]
    "#;

    let params = BTreeMap::from([
        ("ancestor".to_string(), DataValue::from("grandparent_module")),
    ]);

    let result = db.run_script(recursive_query, params, ScriptMutability::Immutable)?;

    #[cfg(feature = "debug")]
    println!("Module descendants: {:?}", result);

    // Should find both parent_module and child_module as descendants
    assert!(result.rows.len() >= 2, "Expected at least 2 descendants");
    
    // Check that both parent and child modules are found
    let mut found_parent = false;
    let mut found_child = false;
    
    for row in &result.rows {
        if row[1].get_str() == Some("parent_module") {
            found_parent = true;
        }
        if row[1].get_str() == Some("child_module") {
            found_child = true;
        }
    }
    
    assert!(found_parent, "Should find parent_module as descendant");
    assert!(found_child, "Should find child_module as descendant");

    Ok(())
}
