#![cfg(feature = "uuid_ids")] // Gate the whole module

#[cfg(test)]
mod determinism_tests {
    use crate::common::run_phase1_phase2; // Assuming this helper exists and works
    use ploke_core::{NodeId, TrackingHash, TypeId};
    use std::collections::{HashMap, HashSet};
    use std::fmt::Debug;
    use syn_parser::parser::{
        graph::CodeGraph,
        nodes::*, // Import all node types
        relations::{GraphId, Relation, RelationKind},
        types::TypeNode,
    };
    use uuid::Uuid;

    #[test]
    fn test_phase2_determinism() {
        let fixture_names = [
            "duplicate_name_fixture_1",
            "duplicate_name_fixture_2",
            "example_crate",
            "file_dir_detection",
            "fixture_attributes",
            "fixture_cyclic_types",
            "fixture_edge_cases",
            "fixture_generics",
            "fixture_macros",
            "fixture_tracking_hash",
            "fixture_types",
            "simple_crate",
            "subdir/duplicate_name_fixture_3",
        ];

        for fixture_name in fixture_names {
            // Run Phase 1+2 on the fixture crate
            println!("Running first analysis of {}...", fixture_name);
            let results1 = run_phase1_phase2(fixture_name);
            println!("Finished first analysis.");

            // Run Phase 1+2 again on the same crate
            println!("Running second analysis of {}...", fixture_name);
            let results2 = run_phase1_phase2(fixture_name);
            println!("Finished second analysis.");

            // Basic check: same number of results (files processed)
            assert_eq!(
                results1.len(),
                results2.len(),
                "Number of processed files should be deterministic. Run 1: {}, Run 2: {}",
                results1.len(),
                results2.len()
            );

            // Compare the results element by element (assuming order is preserved)
            for i in 0..results1.len() {
                let res1 = &results1[i];
                let res2 = &results2[i];

                match (res1, res2) {
                    (Ok(graph1), Ok(graph2)) => {
                        println!("Comparing graphs for file index {}...", i);
                        if let Err(e) = assert_graphs_identical(&graph1.graph, &graph2.graph, i) {
                            // Use assert! with the error message for clear test failure
                            panic!("Graph comparison failed for file index {}: {}", i, e);
                        }
                        println!("Graphs for file index {} are identical.", i);
                    }
                    (Err(e1), Err(e2)) => {
                        // If both runs failed for the same file, check if errors are "similar enough".
                        // Exact error comparison can be brittle. Maybe just log it.
                        // For determinism, ideally, the errors should be identical too.
                        let err_msg1 = e1.to_string();
                        let err_msg2 = e2.to_string();
                        if err_msg1 != err_msg2 {
                            // Use assert! for clear failure
                            panic!(
                                "Error messages differ for file index {}:\nRun 1: {}\nRun 2: {}",
                                i, err_msg1, err_msg2
                            );
                        } else {
                            println!(
                                "Warning: Both runs produced the same error for file index {}: {}",
                                i, err_msg1
                            );
                        }
                    }
                    (Ok(_), Err(e2)) => {
                        panic!(
                        "Mismatch in Result variant for file index {}. Run 1: Ok, Run 2: Err({})",
                        i, e2
                    );
                    }
                    (Err(e1), Ok(_)) => {
                        panic!(
                        "Mismatch in Result variant for file index {}. Run 1: Err({}), Run 2: Ok",
                        i, e1
                    );
                    }
                }
            }
            println!(
                "Phase 2 analysis appears deterministic for {}.",
                fixture_name
            );
        }
    }

    /// Compares two CodeGraphs assuming they originate from the *same file*
    /// and checks for exact equality of all components, including synthetic IDs.
    /// Returns Ok(()) if identical, Err(String) describing the first difference.
    fn assert_graphs_identical(
        graph1: &CodeGraph,
        graph2: &CodeGraph,
        file_index: usize,
    ) -> Result<(), String> {
        // --- Helper Macro for Comparisons ---
        macro_rules! compare_vecs_by_id {
            ($field:ident, $id_type:ty, $node_type:ty, $get_id:expr) => {
                // Compare counts first
                if graph1.$field.len() != graph2.$field.len() {
                    return Err(format!(
                        "File {}: Mismatched count for '{}'. Graph1: {}, Graph2: {}",
                        file_index, stringify!($field), graph1.$field.len(), graph2.$field.len()
                    ));
                }

                // Build HashMaps keyed by ID
                let map1: HashMap<$id_type, &$node_type> = graph1.$field.iter().map(|n| ($get_id(n), n)).collect();
                let map2: HashMap<$id_type, &$node_type> = graph2.$field.iter().map(|n| ($get_id(n), n)).collect();

                // Check if all IDs from graph1 exist in graph2 and nodes are equal
                for (id1, node1) in map1.iter() {
                    match map2.get(id1) {
                        Some(node2) => {
                            // Use PartialEq derived for the node types
                            if node1 != node2 {
                                return Err(format!(
                                    "File {}: Mismatched node content for ID {:?} in '{}'.\nGraph1: {:?}\nGraph2: {:?}",
                                    file_index, id1, stringify!($field), node1, node2
                                ));
                            }
                        }
                        None => {
                            return Err(format!(
                                "File {}: ID {:?} from graph1 missing in graph2 for '{}'.",
                                file_index, id1, stringify!($field)
                            ));
                        }
                    }
                }

                // Optional: Check if graph2 has extra IDs (should be caught by length check, but good sanity check)
                if map1.len() != map2.len() {
                     return Err(format!(
                        "File {}: Mismatched ID sets for '{}' (lengths matched but IDs differ).",
                        file_index, stringify!($field)
                    ));
                }
            };
        }

        // --- Compare Node Collections ---
        compare_vecs_by_id!(functions, NodeId, FunctionNode, |n: &FunctionNode| n.id);
        compare_vecs_by_id!(
            defined_types,
            NodeId,
            TypeDefNode,
            |n: &TypeDefNode| match n {
                TypeDefNode::Struct(s) => s.id,
                TypeDefNode::Enum(e) => e.id,
                TypeDefNode::TypeAlias(t) => t.id,
                TypeDefNode::Union(u) => u.id,
            }
        );
        compare_vecs_by_id!(impls, NodeId, ImplNode, |n: &ImplNode| n.id);
        compare_vecs_by_id!(traits, NodeId, TraitNode, |n: &TraitNode| n.id);
        compare_vecs_by_id!(private_traits, NodeId, TraitNode, |n: &TraitNode| n.id);
        compare_vecs_by_id!(modules, NodeId, ModuleNode, |n: &ModuleNode| n.id);
        compare_vecs_by_id!(values, NodeId, ValueNode, |n: &ValueNode| n.id);
        compare_vecs_by_id!(macros, NodeId, MacroNode, |n: &MacroNode| n.id);
        compare_vecs_by_id!(use_statements, NodeId, ImportNode, |n: &ImportNode| n.id);

        // --- Compare Type Graph ---
        compare_vecs_by_id!(type_graph, TypeId, TypeNode, |n: &TypeNode| n.id);

        // --- Compare Relations ---
        // Relations don't have their own ID, so compare sets directly
        let relations1_set: HashSet<_> = graph1.relations.iter().cloned().collect();
        let relations2_set: HashSet<_> = graph2.relations.iter().cloned().collect();

        if relations1_set != relations2_set {
            // Find differences for better error message (optional but helpful)
            let diff1 = relations1_set
                .difference(&relations2_set)
                .collect::<Vec<_>>();
            let diff2 = relations2_set
                .difference(&relations1_set)
                .collect::<Vec<_>>();
            return Err(format!(
                "File {}: Mismatched relations.\nOnly in Graph1: {:?}\nOnly in Graph2: {:?}",
                file_index, diff1, diff2
            ));
        }

        Ok(())
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
