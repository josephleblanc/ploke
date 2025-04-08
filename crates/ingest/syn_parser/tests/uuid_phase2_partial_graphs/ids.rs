#![cfg(feature = "uuid_ids")] // Gate the whole module

#[cfg(test)]
mod phase2_id_tests {
    use ploke_common::fixtures_crates_dir;
    use ploke_core::{NodeId, TrackingHash, TypeId};
    use std::path::PathBuf;
    use syn_parser::{
        discovery::{run_discovery_phase, DiscoveryOutput},
        parser::{analyze_files_parallel, graph::CodeGraph, nodes::FunctionNode}, // Import necessary items
    };
    use uuid::Uuid;

    // Helper function similar to the one in basic.rs
    fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<CodeGraph, syn::Error>> {
        let crate_path = fixtures_crates_dir().join(fixture_name);
        // Use a dummy project root; discovery only needs crate paths for this setup
        let project_root = fixtures_crates_dir();
        let discovery_output = run_discovery_phase(&project_root, &[crate_path])
            .unwrap_or_else(|e| {
                panic!(
                    "Phase 1 Discovery failed for fixture '{}': {:?}",
                    fixture_name, e
                )
            });
        analyze_files_parallel(&discovery_output, 0) // num_workers often ignored by rayon bridge
    }

    // Helper to find a function node by name in a CodeGraph
    // Note: This is simple and assumes unique function names in the test fixtures for now.
    fn find_function_by_name<'a>(
        graph: &'a CodeGraph,
        name: &str,
    ) -> Option<&'a FunctionNode> {
        graph.functions.iter().find(|f| f.name == name)
    }

    #[test]
    fn test_synthetic_ids_and_hashes_present_simple_crate() {
        let results = run_phase1_phase2("simple_crate");
        assert_eq!(results.len(), 1, "Expected results for 1 file");
        let graph = results[0]
            .as_ref()
            .expect("Parsing failed for simple_crate");

        // Check the 'add' function
        let func = find_function_by_name(graph, "add")
            .expect("Failed to find 'add' function in simple_crate");

        // 1. Verify NodeId is Synthetic
        match func.id {
            NodeId::Synthetic(uuid) => {
                // Optionally check if UUID is non-nil, though matching variant is primary check
                assert!(!uuid.is_nil(), "Synthetic NodeId UUID should not be nil");
            }
            NodeId::Resolved(_) => panic!("Expected NodeId::Synthetic, found NodeId::Resolved"),
        }

        // 2. Verify TrackingHash is Present
        match func.tracking_hash {
            Some(TrackingHash(uuid)) => {
                assert!(
                    !uuid.is_nil(),
                    "TrackingHash UUID should not be nil"
                );
            }
            None => panic!("Expected Some(TrackingHash), found None"),
        }

        // 3. Verify Parameter TypeId is Synthetic
        assert_eq!(
            func.parameters.len(),
            2,
            "Expected 'add' function to have 2 parameters"
        );
        for param in &func.parameters {
            match param.type_id {
                TypeId::Synthetic(uuid) => {
                    assert!(!uuid.is_nil(), "Synthetic TypeId UUID should not be nil");
                }
                TypeId::Resolved(_) => {
                    panic!("Expected TypeId::Synthetic, found TypeId::Resolved")
                }
            }
        }

        // 4. Verify Return TypeId is Synthetic
        match func.return_type {
            Some(type_id) => match type_id {
                TypeId::Synthetic(uuid) => {
                    assert!(!uuid.is_nil(), "Return TypeId UUID should not be nil");
                }
                TypeId::Resolved(_) => {
                    panic!("Expected Return TypeId::Synthetic, found TypeId::Resolved")
                }
            },
            None => panic!("Expected 'add' function to have a return type"),
        }

        // Add checks for other nodes if simple_crate contains them (e.g., modules, structs)
    }

    #[test]
    fn test_synthetic_node_ids_differ_across_crates() {
        // Run on simple_crate
        let results_simple = run_phase1_phase2("simple_crate");
        let graph_simple = results_simple[0].as_ref().expect("Parsing simple_crate failed");
        let func_simple = find_function_by_name(graph_simple, "add")
            .expect("Failed to find 'add' function in simple_crate");
        let simple_id = match func_simple.id {
            NodeId::Synthetic(uuid) => uuid,
            _ => panic!("simple_crate 'add' function ID was not Synthetic"),
        };

        // Run on example_crate (assuming it also has an 'add' function in lib.rs)
        let results_example = run_phase1_phase2("example_crate");
        // Find the graph for lib.rs in example_crate
        let graph_example = results_example
            .iter()
            .find_map(|res| {
                res.as_ref().ok().filter(|g| {
                    // Heuristic: Check if this graph contains the 'add' function.
                    // A better approach might involve checking file paths if VisitorState stored them.
                    find_function_by_name(g, "add").is_some()
                })
            })
            .expect("Could not find graph containing 'add' function in example_crate results");

        let func_example = find_function_by_name(graph_example, "add")
            .expect("Failed to find 'add' function in example_crate");
        let example_id = match func_example.id {
            NodeId::Synthetic(uuid) => uuid,
            _ => panic!("example_crate 'add' function ID was not Synthetic"),
        };

        // Assert the UUIDs are different because the crate_namespace was different
        assert_ne!(
            simple_id, example_id,
            "Synthetic NodeIds for 'add' function should differ between simple_crate and example_crate"
        );
    }

    // TODO: Add tests for TypeId consistency/difference across crates/files
    // TODO: Add tests for TrackingHash consistency/difference
}
