#![cfg(feature = "uuid_ids")] // Gate the whole module

#[cfg(test)]
mod phase2_relation_tests {
    use crate::common::uuid_ids_utils::*;
    use ploke_common::{fixtures_crates_dir, workspace_root};
    use ploke_core::{NodeId, TypeId};
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };
    use syn_parser::{
        discovery::{run_discovery_phase, DiscoveryOutput},
        parser::{
            analyze_files_parallel,
            graph::CodeGraph,
            nodes::{
                FieldNode, FunctionNode, ImplNode, ImportNode, ModuleNode, StructNode, TraitNode,
                TypeDefNode, ValueNode, Visible,
            },
            relations::{GraphId, Relation, RelationKind},
            types::{GenericParamKind, TypeNode},
            visitor::ParsedCodeGraph,
        },
    };
    use uuid::Uuid;

    // --- Test Setup Helpers ---

    // --- Relation Tests ---

    #[test]
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
        let expect_mod_two_id = NodeId::generate_synthetic(
            expect_crate_namespace,
            &expect_mod_two_path,
            &["crate".to_string()],
            "module_two",
            (0, 0),
        );
        let code_graph_with_mod_two_option = code_graphs
            .iter()
            .find(|cg| find_node_id_name(&cg.graph, expect_mod_two_id).is_some());
        assert!(
            code_graph_with_mod_two_option.is_some(),
            "module_two's expected NodeId not found in any CodeGraph:
\tmodule_two_expected_id: {}",
            expect_mod_two_id
        );

        #[cfg(feature = "verbose_debug")]
        print!("Looking for module_two in code_graphs...");
        let mod_two_graph = code_graph_with_mod_two_option.unwrap();
        #[cfg(feature = "verbose_debug")]
        println!(
            "found in code graph with path: {:?}",
            mod_two_graph.file_path,
        );

        let mod_two_func_id = find_node_id_by_path_and_name(
            &mod_two_graph.graph,
            &["crate".to_string(), "module_two".to_string()],
            "mod_two_func",
        ).expect("Failed to find mod_two_func in same code graph as 'module_two' using 'crate' as module path");

        let found = mod_two_graph.graph.relations.iter().find(|r| {
            r.source == GraphId::Node(expect_mod_two_id)
                && r.target == GraphId::Node(mod_two_func_id)
                && r.kind == RelationKind::Contains
        });

        assert!(
            found.is_some(),
            "Did not find Contains relation between module_two -> mod_two_func"
        );
    }
}
