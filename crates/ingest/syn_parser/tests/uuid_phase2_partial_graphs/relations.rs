#![cfg(not(feature = "type_bearing_ids"))]
#[cfg(test)]
mod phase2_relation_tests {
    use crate::common::uuid_ids_utils::*;
    use crate::common::uuid_ids_utils::*;
    use ploke_common::{fixtures_crates_dir, workspace_root};
    use ploke_core::NodeId;
    use syn_parser::{
        discovery::{run_discovery_phase, DiscoveryOutput},
        parser::{
            analyze_files_parallel,
            // Removed GraphId import
            relations::{Relation, RelationKind},
            ParsedCodeGraph,
        },
    };

    // --- Test Setup Helpers ---

    // --- Relation Tests ---

    #[test]
    #[cfg(not(feature = "type_bearing_ids"))]
fn test_contains_relation_example() {
        let crate_name = "example_crate";
        let crate_version = "0.1.0";
        let crate_path = fixtures_crates_dir().join(crate_name);
        // Use workspace root as project root for discovery context
        let project_root = workspace_root();
        let discovery_output: DiscoveryOutput =
            run_discovery_phase(&project_root, &[crate_path.clone()]).unwrap_or_else(|e| {
                panic!(
                    "Phase 1 Discovery failed for fixture '{}': {:?}",
                    crate_name, e
                )
            });
        let code_graphs: Vec<ParsedCodeGraph> = analyze_files_parallel(&discovery_output, 0)
            .iter_mut()
            .map(|res| res.to_owned().unwrap())
            .collect();

        for code_graph in &code_graphs {
            println!("Code graph for file: {:?}", code_graph.file_path.to_str());
            for module in &code_graph.graph.modules {
                println!("{:#?}", module);
            }
        }

        #[cfg(feature = "verbose_debug")]
        {
            println!("{:-^60}", "");
            println!("{:-^60}", "All Relations");
            println!("{:-^60}", "");
            for code_graph in &code_graphs {
                print_all_relations(&code_graph.graph);
            }
            println!("{:-^60}", "");
        }

        let expect_crate_namespace =
            syn_parser::discovery::derive_crate_namespace(crate_name, crate_version);

        // Module -> Function
        // module_two -> mod_two_func

        let expect_mod_two_path = crate_path
            .clone()
            .join("src")
            .join("module_two")
            .join("mod.rs");

        let expected_mod_two_rel_path = &["crate".to_string()];
        // Regenerate ID using ItemKind::Module and None for parent_scope_id
        let expect_mod_two_id = NodeId::generate_synthetic(
            expect_crate_namespace,
            &expect_mod_two_path,
            expected_mod_two_rel_path, // This is the *parent* path for ID generation context
            "module_two",
            ploke_core::ItemKind::Module, // Pass ItemKind::Module
            None,                         // File-level module definition has no parent scope ID
            None, // Assume no relevant file-level #[cfg] in fixture for this test
        );
        let code_graph_with_mod_two_option = code_graphs
            .iter()
            .find(|cg| find_node_id_name(&cg.graph, expect_mod_two_id).is_some());
        assert!(
            code_graph_with_mod_two_option.is_some(),
            "module_two's expected NodeId not found in any CodeGraph:
\tmodule_two_expected_id: {}
derived using:
    expected_crate_namespace:   {}
    expect_mod_two_path:        {:?}
    expected_mod_two_rel_path:  {:?} (Parent Path for ID Gen)
    name:                       {}
    item_kind:                  {:?}
    parent_scope_id:            None
    cfg_bytes:                  None",
            expect_mod_two_id,
            expect_crate_namespace,
            &expect_mod_two_path, // File path
            expected_mod_two_rel_path,
            "module_two",
            ploke_core::ItemKind::Module,
        );

        #[cfg(feature = "verbose_debug")]
        print!("Looking for module_two in code_graphs...");
        let mod_two_graph = code_graph_with_mod_two_option.unwrap();
        #[cfg(feature = "verbose_debug")]
        println!(
            "found in code graph with path: {:?}",
            mod_two_graph.file_path,
        );

        // .expect("Failed to find mod_two_func_id in same code graph as 'module_two' using 'crate' as module path");

        let candidate_rels: Vec<&Relation> = mod_two_graph
            .graph
            .relations
            .iter()
            .filter(|r| {
                r.source == expect_mod_two_id // Use NodeId directly
                    && r.kind == RelationKind::Contains
            })
            .collect();
        let mut candidate_funcs = mod_two_graph.graph.functions.iter().filter(|f| {
            candidate_rels.iter().any(|r| r.target == f.id) // Use NodeId directly
        });
        let debug_candidate_funcs = candidate_funcs.clone().collect::<Vec<_>>();
        let found = candidate_funcs.next();

        let func_node =
            found.expect("Did not find Contains relation between module_two -> mod_two_func");

        assert!(
            found.is_some(),
            "Did not find Contains relation between module_two -> mod_two_func"
        );
        assert!(
            debug_candidate_funcs.len() == 1,
            // candidate_funcs.next().is_none(),
            "Found more than one match of RelationKind::Contains Relation between:\n\
            \tsource module id: {},\n\
            \tfunc module id: {},\n\
            \tnumber of candidate funcs: {},\n\
            relation: {:#?},\n\
            func: {:#?},\n\
            debug_candidate_funcs: {:#?}",
            candidate_rels
                .iter()
                .find(|r| r.source == expect_mod_two_id // Use NodeId directly
                    && r.target == func_node.id // Use NodeId directly
                    && r.kind == RelationKind::Contains)
                .map(|r| r.source) // Source is NodeId
                .unwrap(),
            func_node.id,
            debug_candidate_funcs.len(),
            candidate_rels
                .iter()
                .find(|r| r.source == expect_mod_two_id // Use NodeId directly
                    && r.target == func_node.id // Use NodeId directly
                    && r.kind == RelationKind::Contains)
                .unwrap(),
            func_node,
            debug_candidate_funcs
        );
    }
}
