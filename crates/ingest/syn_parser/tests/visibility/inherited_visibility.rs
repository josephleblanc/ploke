use crate::common::{find_function_by_name, find_struct_by_name, parse_fixture};
use syn_parser::parser::nodes::{OutOfScopeReason, VisibilityResult};

mod inherited_items {
    use super::*;

    #[test]
    fn private_struct_same_module() {
        let graph = parse_fixture("visibility.rs").expect("Fixture failed to parse");
        let private_struct = find_struct_by_name(&graph, "InheritedStruct").unwrap();
        
        // Should be visible in same module
        let result = graph.resolve_visibility(private_struct.id, &["crate"]);
        assert!(
            matches!(result, VisibilityResult::Direct),
            "Private struct should be visible in same module"
        );
    }

    #[test] 
    fn private_function_cross_module() {
        let graph = parse_fixture("modules.rs").expect("Fixture failed to parse");
        let inner_func = find_function_by_name(&graph, "inner_function").unwrap();
        
        // Should be blocked outside module
        let result = graph.resolve_visibility(inner_func.id, &["crate", "outer"]);
        assert!(
            matches!(
                result,
                VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::Private,
                    ..
                }
            ),
            "Private function should be blocked outside module"
        );
    }
}
