#![cfg(feature = "uuid_ids")] // Gate the whole module

#[cfg(test)]
mod phase2_id_tests {
    use crate::common::{find_function_by_name, run_phase1_phase2};
    use ploke_common::fixtures_crates_dir;
    use ploke_core::{NodeId, TrackingHash, TypeId};
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };
    use syn_parser::{
        discovery::{run_discovery_phase, CrateContext, DiscoveryOutput}, // Import CrateContext
        parser::{
            analyze_files_parallel,
            graph::CodeGraph,
            nodes::{FunctionNode, StructNode, TypeDefNode},
            visitor::ParsedCodeGraph, // Import StructNode, TypeDefNode
        },
    };
    use uuid::Uuid; // Import the helper function

    // Helper function to run Phase 1 on multiple fixtures, then Phase 2
    // Returns results mapped by the original crate root path for easier lookup.
    fn run_phase1_phase2_multi(
        fixture_names: &[&str],
    ) -> HashMap<PathBuf, Vec<Result<ParsedCodeGraph, syn::Error>>> {
        let crate_paths: Vec<PathBuf> = fixture_names
            .iter()
            .map(|name| fixtures_crates_dir().join(name))
            .collect();

        // Use a dummy project root; discovery only needs crate paths for this setup
        let project_root = fixtures_crates_dir();
        let discovery_output = run_discovery_phase(&project_root, &crate_paths)
            .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for fixtures: {:?}", e));

        // Run Phase 2 on the combined output. The results are ordered according to
        // the iteration order of crate_contexts and then files within each context.
        let mut all_results_iter = analyze_files_parallel(&discovery_output, 0).into_iter();

        let mut grouped_results: HashMap<PathBuf, Vec<Result<ParsedCodeGraph, syn::Error>>> =
            HashMap::new();

        // Iterate through the crate contexts in the *same order* discovery likely processed them
        // (assuming BTreeMap iteration order is stable, which it is).
        // We rely on the fact that analyze_files_parallel processes files in the order
        // they appear within each CrateContext, and processes CrateContexts sequentially.
        for crate_context in discovery_output.crate_contexts.values() {
            let crate_path = &crate_context.root_path;
            let num_files_in_crate = crate_context.files.len();
            let mut crate_results = Vec::with_capacity(num_files_in_crate);

            for _ in 0..num_files_in_crate {
                if let Some(result) = all_results_iter.next() {
                    crate_results.push(result);
                } else {
                    panic!(
                        "Mismatch in expected number of results for crate {}",
                        crate_path.display()
                    );
                }
            }
            grouped_results.insert(crate_path.clone(), crate_results);
        }

        // Ensure all results were consumed
        if all_results_iter.next().is_some() {
            panic!("analyze_files_parallel returned more results than expected based on DiscoveryOutput");
        }

        grouped_results
    }

    //Helper to find a node by name (function or struct)
    // WARNING: Brittle method, will fail in any case where names of all `graph.functions` are not
    // unique.
    fn find_node_id_by_name(graph: &CodeGraph, name: &str) -> Option<NodeId> {
        // misleading, there may be two valid nodes with same name
        // e.g.
        //  fn some_func() {}
        //  mod a {
        //      fn some_func() {}
        //  }
        graph
            .functions
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.id)
            .or_else(|| {
                graph.defined_types.iter().find_map(|td| match td {
                    TypeDefNode::Struct(s) if s.name == name => Some(s.id),
                    // Add Enum, Union, TypeAlias if needed
                    _ => None,
                })
            })
        // Add other node types if necessary
    }

    // Helper to find the TypeId of a function's first parameter
    fn find_first_param_type_id(graph: &CodeGraph, func_name: &str) -> Option<TypeId> {
        graph
            .functions
            .iter()
            .find(|f| f.name == func_name)
            .and_then(|f| f.parameters.first())
            .map(|p| p.type_id)
    }

    #[test]
    fn test_synthetic_ids_and_hashes_present_simple_crate() {
        let results = run_phase1_phase2("simple_crate");
        assert_eq!(results.len(), 1, "Expected results for 1 file");
        let graph = &results[0]
            .as_ref()
            .expect("Parsing failed for simple_crate")
            .graph;

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
                assert!(!uuid.is_nil(), "TrackingHash UUID should not be nil");
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
        let graph_simple = &results_simple[0]
            .as_ref()
            .expect("Parsing simple_crate failed")
            .graph;
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
                    find_function_by_name(&g.graph, "add").is_some()
                })
            })
            .expect("Could not find graph containing 'add' function in example_crate results");

        let func_example = find_function_by_name(&graph_example.graph, "add")
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

    #[test]
    fn test_synthetic_ids_differ_across_files_same_crate_name() {
        let fixture_names = [
            "duplicate_name_fixture_1",
            "duplicate_name_fixture_2",
            "subdir/duplicate_name_fixture_3", // Include the one in subdir
        ];
        let results_map = run_phase1_phase2_multi(&fixture_names);

        assert_eq!(
            results_map.len(),
            3,
            "Should have results for 3 fixture paths"
        );

        let mut ids = HashMap::new();

        for name in fixture_names {
            let path = fixtures_crates_dir().join(name);
            let results = results_map
                .get(&path)
                .unwrap_or_else(|| panic!("No results found for path {}", path.display()));
            assert_eq!(results.len(), 1, "Expected 1 result per fixture");
            let graph = results[0]
                .as_ref()
                .unwrap_or_else(|e| panic!("Parsing failed for {}: {:?}", name, e));

            // Get IDs for 'Thing' struct and 'do_thing' function
            let thing_id =
                find_node_id_by_name(&graph.graph, "Thing").expect("Failed to find 'Thing' struct");
            let do_thing_id = find_node_id_by_name(&graph.graph, "do_thing")
                .expect("Failed to find 'do_thing' function");
            let param_type_id = find_first_param_type_id(&graph.graph, "do_thing")
                .expect("Failed to find param type id for 'do_thing'");

            ids.insert(
                name,
                (
                    thing_id,      // NodeId for struct
                    do_thing_id,   // NodeId for function
                    param_type_id, // TypeId for function parameter
                ),
            );
        }

        // Extract IDs for comparison
        let (thing1, fn1, type1) = ids["duplicate_name_fixture_1"];
        let (thing2, fn2, type2) = ids["duplicate_name_fixture_2"];
        let (thing3, fn3, type3) = ids["subdir/duplicate_name_fixture_3"];

        // --- Assertions ---

        // 1. NodeIds for 'Thing' struct should differ due to file path
        assert_ne!(
            thing1, thing2,
            "Thing NodeId should differ between fixture 1 and 2 (different file paths)"
        );
        assert_ne!(
            thing1, thing3,
            "Thing NodeId should differ between fixture 1 and 3 (different file paths)"
        );
        assert_ne!(
            thing2, thing3,
            "Thing NodeId should differ between fixture 2 and 3 (different file paths)"
        );

        // 2. NodeIds for 'do_thing' function should differ
        //    - fn1 vs fn2: Different file path AND different span (due to comment)
        //    - fn1 vs fn3: Different file path (same span)
        //    - fn2 vs fn3: Different file path AND different span
        assert_ne!(
            fn1, fn2,
            "do_thing NodeId should differ between fixture 1 and 2 (path and span)"
        );
        assert_ne!(
            fn1, fn3,
            "do_thing NodeId should differ between fixture 1 and 3 (path)"
        );
        assert_ne!(
            fn2, fn3,
            "do_thing NodeId should differ between fixture 2 and 3 (path and span)"
        );

        // 3. TypeIds for the 'Thing' parameter should differ due to file path context during generation
        assert_ne!(
            type1, type2,
            "Param TypeId should differ between fixture 1 and 2 (different file context)"
        );
        assert_ne!(
            type1, type3,
            "Param TypeId should differ between fixture 1 and 3 (different file context)"
        );
        assert_ne!(
            type2, type3,
            "Param TypeId should differ between fixture 2 and 3 (different file context)"
        );

        // Ensure all IDs are Synthetic
        assert!(matches!(thing1, NodeId::Synthetic(_)));
        assert!(matches!(thing2, NodeId::Synthetic(_)));
        assert!(matches!(thing3, NodeId::Synthetic(_)));
        assert!(matches!(fn1, NodeId::Synthetic(_)));
        assert!(matches!(fn2, NodeId::Synthetic(_)));
        assert!(matches!(fn3, NodeId::Synthetic(_)));
        assert!(matches!(type1, TypeId::Synthetic(_)));
        assert!(matches!(type2, TypeId::Synthetic(_)));
        assert!(matches!(type3, TypeId::Synthetic(_)));
    }

    // TODO: Add tests for TypeId consistency/difference across crates/files
    // TODO: Add tests for TrackingHash consistency/difference
    // TODO: Add tests for items in nested modules
}
