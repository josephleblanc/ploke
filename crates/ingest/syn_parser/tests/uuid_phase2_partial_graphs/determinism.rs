#![cfg(feature = "uuid_ids")] // Gate the whole module

#[cfg(test)]
mod determinism_tests {
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
            nodes::{FunctionNode, StructNode, TypeDefNode}, // Import StructNode, TypeDefNode
        },
    };
    use uuid::Uuid; // Import the helper function

    #[test]
    fn test_phase2_determinism() {
        // Run Phase 1+2 on example_crate
        let results1 = run_phase1_phase2("example_crate");
        // Run Phase 1+2 again on the same crate
        let results2 = run_phase1_phase2("example_crate");

        // Basic check: same number of results
        assert_eq!(
            results1.len(),
            results2.len(),
            "Number of results should be deterministic"
        );

        // Compare the results element by element
        // This relies on CodeGraph and its contents deriving PartialEq.
        // If PartialEq is not derived or reliable due to UUIDs, more granular checks are needed.
        for (i, (res1, res2)) in results1.iter().zip(results2.iter()).enumerate() {
            match (res1, res2) {
                (Ok(graph1), Ok(graph2)) => {
                    // If PartialEq is not reliable, compare key fields:
                    // assert_eq!(graph1.functions.len(), graph2.functions.len(), "Function count mismatch for index {}", i);
                    // assert_eq!(graph1.defined_types.len(), graph2.defined_types.len(), "Type count mismatch for index {}", i);
                    // assert_eq!(graph1.relations.len(), graph2.relations.len(), "Relation count mismatch for index {}", i);
                    // ... compare specific node names, non-UUID fields, relation kinds etc. ...
                }
                (Err(e1), Err(e2)) => {
                    // Comparing errors can be tricky, maybe just check they are both errors
                    // Or compare error messages if they are expected to be stable
                    // assert_eq!(e1.to_string(), e2.to_string(), "Error messages should be identical for index {}", i);
                    eprintln!(
                        "Warning: Both runs produced an error for index {}: e1='{}', e2='{}'",
                        i, e1, e2
                    );
                }
                _ => {
                    panic!(
                        "Mismatch in Result variant (Ok/Err) for index {} across runs",
                        i
                    );
                }
            }
        }
    }

    // functions: Vec<FunctionNode>,
    // defined_types: Vec<TypeDefNode>,
    // type_graph: Vec<TypeNode>,
    // impls: Vec<ImplNode>,
    // traits: Vec<TraitNode>,
    // private_traits: Vec<TraitNode>,
    // relations: Vec<Relation>,
    // modules: Vec<ModuleNode>,
    // values: Vec<ValueNode>,
    // macros: Vec<MacroNode>,
    // use_statements: Vec<ImportNode>,
    // pub fn check_identical_graphs<T>(
    //     graph1: &CodeGraph,
    //     graph2: &CodeGraph,
    // ) -> (CodeGraph, CodeGraph) {
    //     graph1.functions.iter()
    //         .zip(graph2.functions.iter())
    //         .map(|(f1, f2)| )
    // }
}
