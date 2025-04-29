#[cfg(test)]
mod phase2_tests {
    // Assuming this helper exists
    use ploke_core::TrackingHash;
    // Import TrackingHash if needed for assertions
    use syn_parser::{parser::nodes::GraphId, NodeId};

    use crate::common::run_phase1_phase2;

    #[test]
    fn test_simple_crate_phase2_output() {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();
        let results = run_phase1_phase2("simple_crate");

        assert_eq!(results.len(), 1, "Expected results for 1 file"); // Assuming simple_crate/src/lib.rs only

        let graph = &results[0].as_ref().expect("Parsing failed").graph;

        // Assertions on graph content
        assert!(!graph.functions.is_empty(), "Should find functions");

        // Check function node ID
        let func = &graph.functions[0]; // Assuming at least one function
        assert!(
            matches!(func.id, NodeId::Synthetic(_)),
            "Function ID should be Synthetic"
        );

        // Check tracking hash
        assert!(
            matches!(func.tracking_hash, Some(TrackingHash(_))),
            "Function should have TrackingHash"
        );

        // Check parameter TypeId (if applicable)
        // assert!(matches!(func.parameters[0].type_id, TypeId::Synthetic(_)), "Param TypeID should be Synthetic");

        // Check relations
        assert!(!graph.relations.is_empty(), "Should have relations");
        let relation = &graph.relations[0]; // Example: Check first relation
        assert!(
            matches!(relation.source, GraphId::Node(NodeId::Synthetic(_))),
            "Relation source should be Synthetic Node"
        );
        // Add checks for target (Node or Type)

        // Add more assertions for structs, types, modules etc.
    }

    // Add more tests: determinism, multi-file crates, error handling...
}
