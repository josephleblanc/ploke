#![allow(unused_imports, dead_code)]
use cozo::DataValue;
use ploke_transform::traits::{BatchIntoCozo, IntoCozo};
use syn_parser::parser::nodes::FunctionNode;

mod test_helpers;

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_visibility_conversions() {
    let graph = parse_fixture("visibility.rs").expect("Error parsing fixture visibility.rs");
    let func = graph
        .functions
        .iter()
        .find(|f| f.name == "restricted_fn")
        .unwrap();

    let map = func.clone().into_cozo_map();
    assert_eq!(
        map["visibility"],
        // string
        DataValue::from("Restricted(super)".to_string())
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_conversion() {
    let graph = parse_fixture("functions.rs").expect("Error parsing fixture functions.rs");
    let func = graph
        .functions
        .iter()
        .find(|f| f.name == "regular_function")
        .unwrap();

    let script = func.cozo_insert_script();
    assert!(script.contains(":put functions"));
    assert!(script.contains("regular_function"));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_batch_insert() {
    let graph = parse_fixture("functions.rs").expect("Error parsing fixture functions.rs");
    let functions: Vec<_> = graph.functions.iter().take(2).cloned().collect();
    let script = FunctionNode::cozo_batch_insert_script(&functions);

    assert!(script.contains(":put functions"));
    assert!(script.contains("regular_function"));
    assert!(script.contains("function_with_params"));
}
