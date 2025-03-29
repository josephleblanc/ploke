use syn_parser::parser::nodes::VisibilityResult;
use syn_parser::parser::visitor::analyze_code;
use std::path::Path;

fn parse_fixture(fixture_name: &str) -> syn_parser::CodeGraph {
    let path = Path::new("tests/fixtures").join(fixture_name);
    analyze_code(&path).unwrap()
}

fn find_function_by_name(graph: &syn_parser::CodeGraph, name: &str) -> syn_parser::parser::nodes::NodeId {
    graph.functions.iter().find(|f| f.name == name).map(|f| f.id).unwrap()
}

fn find_struct_by_name(graph: &syn_parser::CodeGraph, name: &str) -> syn_parser::parser::nodes::NodeId {
    graph.defined_types.iter().find_map(|t| match t {
        syn_parser::parser::nodes::TypeDefNode::Struct(s) if s.name == name => Some(s.id),
        _ => None
    }).unwrap()
}

fn test_module_path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| s.to_string()).collect()
}

mod fixtures {
    pub const SIMPLE_PUB: &str = "visibility/simple_pub.rs";
    pub const RESTRICTED: &str = "visibility/restricted.rs"; 
    pub const USE_STATEMENTS: &str = "visibility/use_statements.rs";
    pub const NESTED_MODULES: &str = "visibility/nested_modules.rs";
}

#[test]
fn test_public_items_direct_visibility() {
    let graph = parse_fixture(fixtures::SIMPLE_PUB).unwrap();

    // Test public function visibility
    let pub_func_id = find_function_by_name(&graph, "public_function").unwrap();
    assert!(matches!(
        graph.resolve_visibility(pub_func_id, &test_module_path(&["crate"])),
        VisibilityResult::Direct
    ));

    // Test public struct visibility
    let pub_struct_id = find_struct_by_name(&graph, "PublicStruct").unwrap();
    assert!(matches!(
        graph.resolve_visibility(pub_struct_id, &test_module_path(&["crate", "other_module"])),
        VisibilityResult::Direct
    ));

    // Test nested public module visibility
    let pub_mod_id = find_module_by_path(&graph, &test_module_path(&["crate", "public_module"])).unwrap();
    assert!(matches!(
        graph.resolve_visibility(pub_mod_id, &test_module_path(&["crate", "unrelated_module"])),
        VisibilityResult::Direct
    ));

    // Test nested public function visibility
    let nested_func_id = find_function_by_name(&graph, "nested_public").unwrap();
    assert!(matches!(
        graph.resolve_visibility(nested_func_id, &test_module_path(&["crate"])),
        VisibilityResult::Direct
    ));
}
