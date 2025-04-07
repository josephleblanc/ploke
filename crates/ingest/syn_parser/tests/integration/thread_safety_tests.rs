#[cfg(test)]
#[cfg(not(feature = "uuid_ids"))]
mod tests {
    use syn_parser::parser::graph::CodeGraph;
    use syn_parser::parser::nodes::{
        EnumNode, FieldNode, FunctionNode, ImplNode, MacroNode, ModuleNode, ParameterNode,
        StructNode, TraitNode, TypeAliasNode, UnionNode, ValueNode, VariantNode,
    };
    use syn_parser::parser::types::TypeNode;

    #[test]
    fn test_core_types_are_send_and_sync() {
        // This test statically verifies that core data structures
        // implement Send + Sync traits as required by our conventions
        fn assert_send_sync<T: Send + Sync>() {}

        // Test CodeGraph and its components
        assert_send_sync::<CodeGraph>();

        // Test node types
        assert_send_sync::<FunctionNode>();
        assert_send_sync::<ParameterNode>();
        assert_send_sync::<StructNode>();
        assert_send_sync::<EnumNode>();
        assert_send_sync::<FieldNode>();
        assert_send_sync::<VariantNode>();
        assert_send_sync::<TypeAliasNode>();
        assert_send_sync::<UnionNode>();
        assert_send_sync::<ImplNode>();
        assert_send_sync::<TraitNode>();
        assert_send_sync::<ModuleNode>();
        assert_send_sync::<ValueNode>();
        assert_send_sync::<MacroNode>();

        // Test type system
        assert_send_sync::<TypeNode>();
    }
}
