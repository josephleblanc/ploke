#![cfg(feature = "module_path_tracking")]
use crate::common::{find_function_by_name, find_struct_by_name, parse_fixture};
use syn_parser::parser::nodes::{OutOfScopeReason, VisibilityResult};

mod public_items {
    use super::*;

    #[test]
    fn public_struct_same_module() {
        let graph = parse_fixture("visibility.rs").expect("Fixture failed to parse");
        let public_struct = find_struct_by_name(&graph, "PublicStruct").unwrap();

        let result = graph.resolve_visibility(public_struct.id, &["crate".to_owned()]);
        assert!(
            matches!(result, VisibilityResult::Direct),
            "Public struct should be directly visible"
        );
    }

    #[test]
    fn public_function_cross_module() {
        let graph = parse_fixture("modules.rs").expect("Fixture failed to parse");
        let outer_func = find_function_by_name(&graph, "outer_function").unwrap();

        let result =
            graph.resolve_visibility(outer_func.id, &["crate".to_owned(), "unrelated".to_owned()]);
        assert!(
            matches!(result, VisibilityResult::Direct),
            "Public function should be visible across modules"
        );
    }
}
