#![cfg(feature = "module_path_tracking")]
use crate::common::find_function_by_name;
use crate::common::find_module_by_path;
use crate::common::find_struct_by_name;
use crate::common::parse_fixture;
use syn_parser::parser::nodes::VisibilityResult;

fn test_module_path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_public_items_direct_visibility_complicated() {
    // 1. Parse with explicit path handling
    let graph = parse_fixture("simple_pub.rs").expect(
        "Failed to parse simple_pub.rs - file missing or inval 
 syntax",
    );

    // 2. Debug output if module not found
    let pub_mod_id = find_module_by_path(&graph, &test_module_path(&["crate", "public_module"]))
        .unwrap_or_else(|| {
            eprintln!("All modules in graph:");
            for m in &graph.modules {
                // #[cfg(feature = "module_path_tracking")]
                eprintln!("- {} (path: {:?})", m.name, m.path);
                // #[cfg(not(feature = "module_path_tracking"))]
                // eprintln!("- {}", m.name);
            }
            panic!("public_module not found");
        });

    // 3. Direct assertion with debug info
    let result = graph.resolve_visibility(
        pub_mod_id,
        &test_module_path(&["crate", "unrelated_module"]),
    );

    assert!(
        matches!(result, VisibilityResult::Direct),
        "Expected Direct visibility, got {:?}",
        result
    );
}

#[test]
fn test_public_items_direct_visibility() {
    let graph = parse_fixture("simple_pub.rs").unwrap();

    // Test public function visibility
    let pub_func_id = find_function_by_name(&graph, "public_function").unwrap();
    assert!(matches!(
        graph.resolve_visibility(pub_func_id.id, &test_module_path(&["crate"])),
        VisibilityResult::Direct
    ));

    // Test public struct visibility
    let pub_struct_id = find_struct_by_name(&graph, "PublicStruct").unwrap();
    assert!(matches!(
        graph.resolve_visibility(
            pub_struct_id.id,
            &test_module_path(&["crate", "other_module"])
        ),
        VisibilityResult::Direct
    ));

    // Test nested public module visibility
    let pub_mod_id =
        find_module_by_path(&graph, &test_module_path(&["crate", "public_module"])).unwrap();
    assert!(matches!(
        graph.resolve_visibility(
            pub_mod_id,
            &test_module_path(&["crate", "unrelated_module"])
        ),
        VisibilityResult::Direct
    ));

    // Test nested public function visibility
    let nested_func_id = find_function_by_name(&graph, "nested_public").unwrap();
    assert!(matches!(
        graph.resolve_visibility(nested_func_id.id, &test_module_path(&["crate"])),
        VisibilityResult::Direct
    ));
}
