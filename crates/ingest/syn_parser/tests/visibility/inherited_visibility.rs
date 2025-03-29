#![cfg(feature = "module_path_tracking")]
use crate::common::{find_function_by_name, find_struct_by_name, parse_fixture};
use syn_parser::parser::nodes::{OutOfScopeReason, VisibilityResult};

mod inherited_items {
    use syn_parser::parser::nodes::Visible;

    use super::*;

    #[test]
    fn private_struct_same_module() {
        let graph = parse_fixture("visibility.rs").expect("Fixture failed to parse");
        let private_struct = find_struct_by_name(&graph, "InheritedStruct").unwrap();
        let context = &["crate".to_owned()];

        println!("\n=== Testing private_struct_same_module ===");
        println!("Struct ID: {}", private_struct.id);
        println!("Struct name: {}", private_struct.name);
        println!("Struct visibility: {:?}", private_struct.visibility());
        println!("Context module: {:?}", context);
        println!(
            "Struct module path: {:?}",
            graph.get_item_module_path(private_struct.id)
        );

        let result = graph.resolve_visibility(private_struct.id, context);
        println!("Visibility result: {:?}", result);

        assert!(
            matches!(result, VisibilityResult::Direct),
            "Private struct should be visible in same module.\n\
             Context: {:?}\n\
             Struct module: {:?}\n\
             Actual result: {:?}",
            context,
            graph.get_item_module_path(private_struct.id),
            result
        );
    }

    #[test]
    fn private_function_cross_module() {
        let graph = parse_fixture("modules.rs").expect(
            "Fixture failed to   
 parse",
        );
        let inner_func = find_function_by_name(&graph, "inner_function").unwrap();

        // Context is outer module trying to access inner module's private  function
        let context = &["crate".to_owned(), "outer".to_owned()];
        let expected_module_path = &["crate".to_owned(), "outer".to_owned(), "inner".to_owned()];

        #[cfg(feature = "verbose_debug")]
        {
            println!("\n=== Testing private_function_cross_module ===");
            println!("Function ID: {}", inner_func.id);
            println!("Function name: {}", inner_func.name);
            println!("Function visibility: {:?}", inner_func.visibility());
            println!("Context module: {:?}", context);
        }

        let actual_module_path = graph.get_item_module_path(inner_func.id);
        #[cfg(feature = "verbose_debug")]
        println!("Function module path: {:?}", actual_module_path);
        assert_eq!(
            actual_module_path, expected_module_path,
            "Function module path mismatch"
        );

        let result = graph.resolve_visibility(inner_func.id, context);
        #[cfg(feature = "verbose_debug")]
        println!("Visibility result: {:?}", result);

        assert!(
            matches!(
                result,
                VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::Private,
                    ..
                }
            ),
            "Private function should be blocked outside module.\n
              Context: {:?}\n
              Function module: {:?}\n
              Actual result: {:?}",
            context,
            actual_module_path,
            result
        );
    }
}
