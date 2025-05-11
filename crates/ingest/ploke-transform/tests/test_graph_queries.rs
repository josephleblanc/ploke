//! Tests for graph queries and traversals

use crate::test_helpers::setup_test_db;
use cozo::{DataValue, ScriptMutability};
use ploke_transform::schema::insert_sample_data;
use std::collections::BTreeMap;

mod test_helpers;

#[allow(dead_code)]
fn insert_sample_union(db: &cozo::Db<cozo::MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
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

#[allow(dead_code)]
fn insert_sample_value(db: &cozo::Db<cozo::MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
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

#[allow(dead_code)]
fn insert_sample_macro(db: &cozo::Db<cozo::MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
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

#[allow(dead_code)]
fn insert_sample_type_details(
    db: &cozo::Db<cozo::MemStorage>,
) -> Result<cozo::NamedRows, cozo::Error> {
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

#[allow(dead_code)]
fn insert_sample_module_relationship(
    db: &cozo::Db<cozo::MemStorage>,
) -> Result<cozo::NamedRows, cozo::Error> {
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

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_find_implementations() {
    let db = setup_test_db();

    // Insert sample data
    insert_sample_data(&db).expect("Failed to insert sample data");

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
    )
    .expect("Failed to insert trait");

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
    )
    .expect("Failed to insert impl");

    // Query to find all implementations of a trait
    let query = r#"
        ?[struct_name, trait_name] := 
            *traits[trait_id, trait_name, _, _],
            *impls[_, struct_id, trait_id],
            *structs[struct_id, struct_name, _, _]
    "#;

    let result = db
        .run_script(query, BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to query implementations");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Implementations", &result);

    assert_eq!(result.rows.len(), 1, "Expected 1 implementation");
    assert_eq!(
        result.rows[0][0].get_str(),
        Some("SampleStruct"),
        "Expected struct name to be 'SampleStruct'"
    );
    assert_eq!(
        result.rows[0][1].get_str(),
        Some("SampleTrait"),
        "Expected trait name to be 'SampleTrait'"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_find_type_usages() {
    let db = setup_test_db();

    // Insert sample data
    insert_sample_data(&db).expect("Failed to insert sample data");

    // Query to find all functions that use a specific type
    let query = r#"
        ?[fn_name, type_str] := 
            *functions[fn_id, fn_name, _, _, _],
            *function_params[fn_id, _, _, type_id, _, _],
            *types[type_id, _, type_str]
    "#;

    let result = db
        .run_script(query, BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to query type usages");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Type usages", &result);

    // We should have at least one function using a type
    assert!(!result.rows.is_empty(), "Expected at least one type usage");
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_hierarchy() {
    let db = setup_test_db();

    // Insert module relationship
    insert_sample_module_relationship(&db).expect("Failed to insert module relationship");

    // Query to find all submodules
    let query = r#"
        ?[parent_name, child_name] := 
            *modules[parent_id, parent_name, _, _],
            *module_relationships[parent_id, child_id, "Contains"],
            *modules[child_id, child_name, _, _]
    "#;

    let result = db
        .run_script(query, BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to query module hierarchy");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Module hierarchy", &result);

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
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_advanced_graph_traversal() {
    let db = setup_test_db();

    // Insert module relationships
    insert_sample_module_relationship(&db).expect("Failed to insert module relationship");

    // Create a more complex module hierarchy
    // grandparent -> parent -> child
    let grandparent_params = BTreeMap::from([
        ("id".to_string(), DataValue::from(22)),
        ("name".to_string(), DataValue::from("grandparent_module")),
        ("visibility".to_string(), DataValue::from("Public")),
        (
            "docstring".to_string(),
            DataValue::from("Grandparent module"),
        ),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put modules",
        grandparent_params,
        ScriptMutability::Mutable,
    ).expect("Failed to insert grandparent module");

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
    ).expect("Failed to insert grandparent-parent relationship");

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

    let params = BTreeMap::from([(
        "ancestor".to_string(),
        DataValue::from("grandparent_module"),
    )]);

    let result = db
        .run_script(recursive_query, params, ScriptMutability::Immutable)
        .expect("Failed to execute recursive query");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Module descendants", &result);

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
}
