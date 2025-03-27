//! Schema definitions for the CozoDB database

use cozo::DataValue;
use std::collections::BTreeMap;

/// Creates all relations needed for the code graph
pub fn create_schema(db: &cozo::Db<cozo::MemStorage>) -> Result<(), cozo::Error> {
    // Core Node Relations
    db.run_script(
        r#"
        :create functions {
            id: Int => 
            name: String,
            return_type_id: Int?,
            docstring: String?,
            body: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create visibility {
            node_id: Int =>
            kind: String,
            path: [String]?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create structs {
            id: Int =>
            name: String,
            visibility: String,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create enums {
            id: Int =>
            name: String,
            visibility: String,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create traits {
            id: Int =>
            name: String,
            visibility: String,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create impls {
            id: Int =>
            self_type_id: Int,
            trait_type_id: Int?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create modules {
            id: Int =>
            name: String,
            visibility: String,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create types {
            id: Int =>
            kind: String,
            type_str: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create type_aliases {
            id: Int =>
            name: String,
            visibility: String,
            type_id: Int,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create unions {
            id: Int =>
            name: String,
            visibility: String,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create values {
            id: Int =>
            name: String,
            visibility: String,
            type_id: Int,
            kind: String,
            value: String?,
            docstring: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create macros {
            id: Int =>
            name: String,
            visibility: String,
            kind: String,
            docstring: String?,
            body: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    // Relationship Relations
    db.run_script(
        r#"
        :create relations {
            source_id: Int,
            target_id: Int,
            kind: String =>
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create module_relationships {
            module_id: Int,
            related_id: Int,
            kind: String =>
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    // Detail Relations
    db.run_script(
        r#"
        :create function_params {
            function_id: Int,
            param_index: Int =>
            param_name: String?,
            type_id: Int,
            is_mutable: Bool,
            is_self: Bool
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create struct_fields {
            struct_id: Int,
            field_index: Int =>
            field_name: String?,
            type_id: Int,
            visibility: String
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create enum_variants {
            enum_id: Int,
            variant_index: Int =>
            variant_name: String,
            discriminant: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create type_relations {
            type_id: Int,
            related_index: Int =>
            related_type_id: Int
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create generic_params {
            owner_id: Int,
            param_index: Int =>
            kind: String,
            name: String,
            type_id: Int?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create attributes {
            owner_id: Int,
            attr_index: Int =>
            name: String,
            value: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        r#"
        :create type_details {
            type_id: Int =>
            is_mutable: Bool?,
            lifetime: String?,
            abi: String?,
            is_unsafe: Bool?,
            is_extern: Bool?,
            dyn_token: Bool?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    // Add vector embedding support
    db.run_script(
        r#"
        :create code_embeddings {
            id: Int =>
            node_id: Int,
            node_type: String,
            embedding: <F32; 384>,
            text_snippet: String?
        }
        "#,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    // Create indices for performance
    create_indices(db)?;

    // Create visibility index
    db.run_script(
        "::index create visibility:by_kind_path {kind, path, node_id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    Ok(())
}

/// Creates indices for the relations
fn create_indices(db: &cozo::Db<cozo::MemStorage>) -> Result<(), cozo::Error> {
    // Indices for core node relations
    db.run_script(
        "::index create functions:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create type_aliases:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create unions:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create values:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create macros:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create structs:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create enums:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create traits:by_name {name, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create types:by_kind {kind, id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    // Indices for relationships
    db.run_script(
        "::index create relations:by_target {target_id, kind, source_id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create relations:by_kind {kind, source_id, target_id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create module_relationships:by_kind {kind, module_id, related_id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    db.run_script(
        "::index create type_details:by_type {type_id}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    // Create HNSW vector index for embeddings
    db.run_script(
        "::hnsw create code_embeddings:vector {dim: 384, m: 16, ef_construction: 100, fields: [embedding], distance: L2}",
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;

    Ok(())
}

/// Inserts sample data into the database for testing
pub fn insert_sample_data(db: &cozo::Db<cozo::MemStorage>) -> Result<(), cozo::Error> {
    // Insert a sample type
    let type_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(1)),
        ("kind".to_string(), DataValue::from("Named")),
        ("type_str".to_string(), DataValue::from("String")),
    ]);

    db.run_script(
        "?[id, kind, type_str] <- [[$id, $kind, $type_str]] :put types",
        type_params,
        cozo::ScriptMutability::Mutable,
    )
    .expect("failed to put sample type");

    // First insert visibility (must come before function)
    let visibility_params = BTreeMap::from([
        ("node_id".to_string(), DataValue::from(1)),
        ("kind".to_string(), DataValue::from("public")),
        ("path".to_string(), DataValue::Null),
    ]);
    db.run_script(
        "?[node_id, kind, path] <- [[$node_id, $kind, $path]] :put visibility",
        visibility_params,
        cozo::ScriptMutability::Mutable,
    )
    .expect("failed to put sample visibility");

    // Insert a sample function
    let function_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(1)),
        ("name".to_string(), DataValue::from("sample_function")),
        ("visibility_kind".to_string(), DataValue::from("public")),
        ("visibility_path".to_string(), DataValue::Null),
        ("return_type_id".to_string(), DataValue::from(1)),
        (
            "docstring".to_string(),
            DataValue::from("A sample function"),
        ),
        ("body".to_string(), DataValue::from("println!(\"Hello\");")),
    ]);

    db.run_script(
        "?[id, name, visibility_kind, visibility_path, return_type_id, docstring, body] <- 
            [[
                $id, 
                $name, 
                $visibility_kind, 
                $visibility_path, 
                $return_type_id, 
                $docstring, 
                $body
            ]] 
            :put functions",
        function_params,
        cozo::ScriptMutability::Mutable,
    )
    .expect("failed to put sample visibility");

    // Insert a function parameter
    let param_params = BTreeMap::from([
        ("function_id".to_string(), DataValue::from(1)),
        ("param_index".to_string(), DataValue::from(0)),
        ("param_name".to_string(), DataValue::from("input")),
        ("type_id".to_string(), DataValue::from(1)),
        ("is_mutable".to_string(), DataValue::from(false)),
        ("is_self".to_string(), DataValue::from(false)),
    ]);

    db.run_script(
        "?[function_id, param_index, param_name, type_id, is_mutable, is_self] <- [[$function_id, $param_index, $param_name, $type_id, $is_mutable, $is_self]] :put function_params",
        param_params,
        cozo::ScriptMutability::Mutable,
    )?;

    // Insert a sample struct
    let struct_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(2)),
        ("name".to_string(), DataValue::from("SampleStruct")),
        ("visibility".to_string(), DataValue::from("public")),
        ("docstring".to_string(), DataValue::from("A sample struct")),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put structs",
        struct_params,
        cozo::ScriptMutability::Mutable,
    )?;

    // Insert a sample relation
    let relation_params = BTreeMap::from([
        ("source_id".to_string(), DataValue::from(1)),
        ("target_id".to_string(), DataValue::from(2)),
        ("kind".to_string(), DataValue::from("References")),
    ]);

    db.run_script(
        "?[source_id, target_id, kind] <- [[$source_id, $target_id, $kind]] :put relations",
        relation_params,
        cozo::ScriptMutability::Mutable,
    )?;

    Ok(())
}

/// Queries the database to verify the schema is working correctly
pub fn verify_schema(db: &cozo::Db<cozo::MemStorage>) -> Result<(), cozo::Error> {
    let function_query = "?[id, name, visibility] := *functions[id, name, visibility, _, _, _]";
    let struct_query = "?[id, name, visibility] := *structs[id, name, visibility, _]";
    let relations_query = "?[source_id, target_id, kind] := *relations[source_id, target_id, kind]";
    let joined_query = r#"
        ?[fn_name, struct_name] := 
            *functions[fn_id, fn_name, _, _, _, _],
            *relations[fn_id, struct_id, "References"],
            *structs[struct_id, struct_name, _, _]
        "#;

    // Query all functions
    #[cfg(feature = "debug")]
    pre_test_message(function_query, "Functions");

    #[allow(unused_variables)]
    let functions = db.run_script(
        function_query,
        BTreeMap::new(),
        cozo::ScriptMutability::Immutable,
    )?;
    #[cfg(feature = "debug")]
    post_test_message(functions, "Functions");

    // Query all structs
    #[cfg(feature = "debug")]
    pre_test_message(struct_query, "Structs");
    #[allow(unused_variables)]
    let structs = db.run_script(
        struct_query,
        BTreeMap::new(),
        cozo::ScriptMutability::Immutable,
    )?;
    #[cfg(feature = "debug")]
    post_test_message(structs, "Structs");

    // Query relations
    #[cfg(feature = "debug")]
    pre_test_message(relations_query, "Relations");
    #[allow(unused_variables)]
    let relations = db.run_script(
        relations_query,
        BTreeMap::new(),
        cozo::ScriptMutability::Immutable,
    )?;
    #[cfg(feature = "debug")]
    post_test_message(relations, "Relations");

    // Query with a join
    #[cfg(feature = "debug")]
    pre_test_message(joined_query, "Joined");
    #[allow(unused_variables)]
    let joined = db.run_script(
        joined_query,
        BTreeMap::new(),
        cozo::ScriptMutability::Immutable,
    )?;
    #[cfg(feature = "debug")]
    post_test_message(joined, "Joined");

    Ok(())
}

#[cfg(feature = "debug")]
fn post_test_message(named_rows: cozo::NamedRows, column: &str) {
    println!("success!");
    println!("{:-<10}  \n{column}: {:?}\n{:->10}", "", named_rows, "");
}

#[cfg(feature = "debug")]
fn pre_test_message(query: &str, column: &str) {
    println!("\n{:-<3}> {column} Query: \"{}\"", "", query);
    print!("{:->5}> Attempting to query {:.<60}", "", column);
}
