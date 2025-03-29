use test_utils::{
    parse_fixture,
    find_function_by_name,
    find_struct_by_name,
    find_module_by_path,
    test_module_path
};
use syn_parser::parser::{
    nodes::{VisibilityResult, OutOfScopeReason},
};

mod fixtures {
    pub const SIMPLE_PUB: &str = "visibility/simple_pub.rs";
    pub const RESTRICTED: &str = "visibility/restricted.rs";
    pub const USE_STATEMENTS: &str = "visibility/use_statements.rs";
    pub const NESTED_MODULES: &str = "visibility/nested_modules.rs";
}

#[test]
fn test_public_items_direct_visibility() {/*...*/}
