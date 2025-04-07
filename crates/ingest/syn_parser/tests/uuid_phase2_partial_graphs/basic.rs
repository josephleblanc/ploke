#![cfg(feature = "uuid_ids")]

#[cfg(test)]
#[cfg(feature = "uuid_ids")] // Gate the whole module
mod phase2_tests {
    use ploke_common::{fixtures_crates_dir, fixtures_dir}; // Assuming this helper exists
    use ploke_core::TrackingHash;
    use std::path::PathBuf;
    use syn_parser::discovery::{run_discovery_phase, DiscoveryOutput};
    use syn_parser::parser::analyze_files_parallel; // Import TrackingHash if needed for assertions
    use syn_parser::parser::relations::GraphId; // Import UUID versions
    use syn_parser::{CodeGraph, NodeId, TypeId};
    use uuid::Uuid;

    fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<CodeGraph, syn::Error>> {
        let crate_path = fixtures_crates_dir().join(fixture_name);
        let discovery_output = run_discovery_phase(&PathBuf::from("."), &[crate_path]) // Adjust project_root if needed
            .expect("Phase 1 Discovery failed");
        analyze_files_parallel(&discovery_output, 0) // num_workers often ignored by rayon bridge
    }

    #[test]
    fn test_simple_crate_phase2_output() {
        let results = run_phase1_phase2("simple_crate");

        assert_eq!(results.len(), 1, "Expected results for 1 file"); // Assuming simple_crate/src/lib.rs only

        let graph = results[0].as_ref().expect("Parsing failed");

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
