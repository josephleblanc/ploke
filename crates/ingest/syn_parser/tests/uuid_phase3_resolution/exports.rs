//! Tests focused on the `ModuleTree::process_export_rels` function and the
//! resulting `ReExports` relations and `reexport_index`.

#[cfg(test)]
mod export_tests {
    use ploke_core::NodeId; // Removed unused PROJECT_NAMESPACE_UUID
                            // Removed unused HashMap import
    use syn_parser::{
        error::SynParserError,
        parser::{
            graph::{GraphAccess, ParsedCodeGraph},
            nodes::{GraphId, ModuleNode, NodePath},
            relations::{Relation, RelationKind},
            CodeGraph, // Keep CodeGraph
        },
        resolve::module_tree::{ModuleTree, TreeRelation},
    };
    // Removed unused Uuid import

    // Corrected imports for helper functions from common::paranoid and common::uuid_ids_utils
    use crate::common::uuid_ids_utils::find_function_node_paranoid;
    use crate::common::{
        paranoid::find_import_node_paranoid, // Removed find_struct_node_paranoid as unused in this file
        uuid_ids_utils::{
            assert_relation_exists, find_module_node_by_path, run_phases_and_collect,
        }, // Added assert_relation_exists import
    };

    // Helper to merge ParsedCodeGraphs and build/process the ModuleTree
    fn build_tree_and_process_exports(
        fixture_name: &str,
    ) -> Result<(ParsedCodeGraph, ModuleTree), SynParserError> {
        let mut parsed_graphs = run_phases_and_collect(fixture_name);

        // Manually merge ParsedCodeGraphs
        let mut merged_parsed_graph = parsed_graphs.pop().ok_or_else(|| {
            SynParserError::InternalState(format!(
                "No parsed graphs found for fixture: {}",
                fixture_name
            ))
        })?;
        for graph in parsed_graphs {
            merged_parsed_graph.append_all(graph).map_err(|e| *e)?; // Unbox error from append_all
        }

        // Build tree from the fully merged ParsedCodeGraph
        let mut module_tree = merged_parsed_graph.build_module_tree()?;

        // Process exports using the tree and the merged graph data
        module_tree.process_export_rels(&merged_parsed_graph)?; // Pass the merged ParsedCodeGraph

        Ok((merged_parsed_graph, module_tree)) // Return merged graph and tree
    }

    // Helper to find the NodeId of an ImportNode based on its visible name and containing module path
    // Takes ParsedCodeGraph now
    fn find_import_node_id_by_name(
        graph: &ParsedCodeGraph, // Changed to ParsedCodeGraph
        module_path: &[&str],
        visible_name: &str,
    ) -> NodeId {
        // Use find_module_node_by_path which takes &CodeGraph, so access graph.graph
        let module_node = find_module_node_by_path(&graph.graph, module_path)
            .unwrap_or_else(|| panic!("Module not found for path: {:?}", module_path));
        module_node // ModuleNode is from the CodeGraph inside ParsedCodeGraph
            .imports
            .iter()
            .find(|imp| imp.visible_name == visible_name)
            .unwrap_or_else(|| {
                panic!(
                    "ImportNode with visible_name '{}' not found in module {:?}",
                    visible_name, module_path
                )
            })
            .id
    }

    #[test]
    #[cfg(feature = "reexport")] // Only run when the feature is enabled
    fn test_path_resolution_exports() -> Result<(), SynParserError> {
        let fixture_name = "fixture_path_resolution";
        // Get IDs before building the tree/merging for paranoid helpers
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        let target_func_id = find_function_node_paranoid(
            &pre_merge_graphs, // Use pre-merged graphs
            fixture_name,
            "src/local_mod.rs", // Relative path needed by paranoid helper
            &["crate".into(), "local_mod".into()],
            "local_func",
        )
        .id;
        let target_deep_func_id = find_function_node_paranoid(
            &pre_merge_graphs, // Use pre-merged graphs
            fixture_name,
            "src/local_mod/nested.rs", // Relative path needed
            &["crate".into(), "local_mod".into(), "nested".into()],
            "deep_func",
        )
        .id;

        // Now build the tree and get the merged graph
        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        // 1. Test `pub use local_mod::local_func;`
        let import_node_id =
            find_import_node_id_by_name(&merged_parsed_graph, &["crate"], "local_func");
        let expected_public_path = NodePath::try_from(vec!["crate".into(), "local_func".into()])?;

        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(import_node_id),
            GraphId::Node(target_func_id),
            RelationKind::ReExports,
            "Relation for local_func re-export",
            // Removed duplicate argument
        );
        assert_eq!(
            tree.reexport_index().get(&expected_public_path),
            Some(&target_func_id),
            "Re-export index check for local_func"
        );

        // 2. Test `pub use log::debug as log_debug_reexport;` (External - should error during processing)
        // We expect `process_export_rels` to return an error for external re-exports.
        // Re-run the process and check the error
        let build_result = build_tree_and_process_exports(fixture_name);

        assert!(
            matches!(
                build_result,
                Err(SynParserError::ModuleTreeError(
                    syn_parser::resolve::module_tree::ModuleTreeError::UnresolvedReExportTarget { .. }
                ))
            ),
            "Expected UnresolvedReExportTarget error for external re-export 'log::debug'"
        );
        // Note: We don't have the 'tree' or 'graph' if build_result is Err, so subsequent checks are skipped implicitly.

        // If the build succeeded unexpectedly (it shouldn't due to the error above),
        // we retrieve the graph and tree for the next assertion.
        // This structure allows testing the error case without crashing if it passes unexpectedly.
        if build_result.is_ok() {
            let (merged_parsed_graph_for_deep, tree_for_deep) = build_result.unwrap(); // Safe if is_ok

            // 3. Test `pub use local_mod::nested::deep_func as renamed_deep_func;`
            // target_deep_func_id obtained before merge
            let import_renamed_id = find_import_node_id_by_name(
                &merged_parsed_graph_for_deep,
                &["crate"],
                "renamed_deep_func",
            );
            let expected_renamed_path =
                NodePath::try_from(vec!["crate".into(), "renamed_deep_func".into()])?;

            assert_relation_exists(
                &tree_for_deep.tree_relations(), // Pass tree relations directly
                GraphId::Node(import_renamed_id),
                GraphId::Node(target_deep_func_id),
                RelationKind::ReExports,
                "Relation for renamed_deep_func re-export",
                 // Removed duplicate argument
            );
            assert_eq!(
                tree_for_deep.reexport_index().get(&expected_renamed_path),
                Some(&target_deep_func_id),
                "Re-export index check for renamed_deep_func"
            );
        }

        Ok(())
    }

    #[test]
    #[cfg(feature = "reexport")]
    fn test_spp_no_cfg_multi_step_chain() -> Result<(), SynParserError> {
        let fixture_name = "fixture_spp_edge_cases_no_cfg";
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        let original_item_a_id = find_function_node_paranoid(
            &pre_merge_graphs,
            fixture_name,
            "src/lib.rs", // Defined inline in lib.rs
            &["crate".into(), "chain_a".into()],
            "item_a",
        )
        .id;

        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        // Target: `item_c` which is a re-export of `chain_a::item_a`
        // original_item_a_id obtained before merge

        // Find the final ImportNode at the crate root named "item_c"
        let final_import_id =
            find_import_node_id_by_name(&merged_parsed_graph, &["crate"], "item_c");
        let expected_public_path = NodePath::try_from(vec!["crate".into(), "item_c".into()])?;

        // Assert the ReExports relation links the final import to the original item
        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(final_import_id),
            GraphId::Node(original_item_a_id),
            RelationKind::ReExports,
            "Relation for item_c -> item_a",
             // Removed duplicate argument
        );

        // Assert the reexport_index maps the public path to the original item
        assert_eq!(
            tree.reexport_index().get(&expected_public_path),
            Some(&original_item_a_id),
            "Re-export index check for item_c"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "reexport")]
    fn test_spp_no_cfg_multi_rename() -> Result<(), SynParserError> {
        let fixture_name = "fixture_spp_edge_cases_no_cfg";
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        // Target: `final_renamed_item` -> ... -> `rename_source::multi_rename_item`
        let original_item_id = find_function_node_paranoid(
            &pre_merge_graphs,
            fixture_name,
            "src/lib.rs", // Defined inline
            &["crate".into(), "rename_source".into()],
            "multi_rename_item",
        )
        .id;

        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        let final_import_id =
            find_import_node_id_by_name(&merged_parsed_graph, &["crate"], "final_renamed_item");
        let expected_public_path =
            NodePath::try_from(vec!["crate".into(), "final_renamed_item".into()])?;

        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(final_import_id),
            GraphId::Node(original_item_id),
            RelationKind::ReExports,
            "Relation for final_renamed_item -> multi_rename_item",
             // Removed duplicate argument
        );
        assert_eq!(
            tree.reexport_index().get(&expected_public_path),
            Some(&original_item_id),
            "Re-export index check for final_renamed_item"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "reexport")]
    fn test_spp_no_cfg_branching() -> Result<(), SynParserError> {
        let fixture_name = "fixture_spp_edge_cases_no_cfg";
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        // Target: `branch_item` re-exported as `item_via_a` and `item_via_b`
        let original_item_id = find_function_node_paranoid(
            &pre_merge_graphs,
            fixture_name,
            "src/lib.rs", // Defined inline
            &["crate".into(), "branch_source".into()],
            "branch_item",
        )
        .id;

        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        // Check item_via_a
        let import_a_id =
            find_import_node_id_by_name(&merged_parsed_graph, &["crate"], "item_via_a");
        let path_a = NodePath::try_from(vec!["crate".into(), "item_via_a".into()])?;
        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(import_a_id),
            GraphId::Node(original_item_id),
            RelationKind::ReExports,
            "Relation for item_via_a -> branch_item",
             // Removed duplicate argument
        );
        assert_eq!(
            tree.reexport_index().get(&path_a),
            Some(&original_item_id),
            "Re-export index check for item_via_a"
        );

        // Check item_via_b
        let import_b_id =
            find_import_node_id_by_name(&merged_parsed_graph, &["crate"], "item_via_b");
        let path_b = NodePath::try_from(vec!["crate".into(), "item_via_b".into()])?;
        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(import_b_id),
            GraphId::Node(original_item_id),
            RelationKind::ReExports,
            "Relation for item_via_b -> branch_item",
             // Removed duplicate argument
        );
        assert_eq!(
            tree.reexport_index().get(&path_b),
            Some(&original_item_id),
            "Re-export index check for item_via_b"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "reexport")]
    #[ignore = "Relative path resolution (super::, self::) not yet implemented in resolve_single_export"]
    fn test_spp_no_cfg_relative_super() -> Result<(), SynParserError> {
        let fixture_name = "fixture_spp_edge_cases_no_cfg";
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        // Target: `relative::inner::reexport_super` -> `super::item_in_relative` -> `relative::item_in_relative`
        let original_item_id = find_function_node_paranoid(
            &pre_merge_graphs,
            fixture_name,
            "src/lib.rs", // Defined inline
            &["crate".into(), "relative".into()],
            "item_in_relative",
        )
        .id;

        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        // The ImportNode is inside `relative::inner` and named `reexport_super`
        let import_id = find_import_node_id_by_name(
            &merged_parsed_graph,
            &["crate", "relative", "inner"],
            "reexport_super",
        );
        // The public path is where the item is defined/re-exported publicly
        let expected_public_path = NodePath::try_from(vec![
            "crate".into(),
            "relative".into(),
            "inner".into(),
            "reexport_super".into(), // Assuming it's public via the module path
        ])?;

        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(import_id),
            GraphId::Node(original_item_id),
            RelationKind::ReExports,
            "Relation for reexport_super -> item_in_relative",
             // Removed duplicate argument
        );
        // Check reexport_index - NOTE: This path might differ depending on SPP logic vs. canonical path logic
        // For now, let's assume the index uses the path where the re-export makes it visible.
        assert_eq!(
            tree.reexport_index().get(&expected_public_path),
            Some(&original_item_id),
            "Re-export index check for reexport_super"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "reexport")]
    #[ignore = "Relative path resolution (super::, self::) not yet implemented in resolve_single_export"]
    fn test_spp_no_cfg_relative_self() -> Result<(), SynParserError> {
        let fixture_name = "fixture_spp_edge_cases_no_cfg";
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        // Target: `relative::reexport_self` -> `self::inner::item_in_inner` -> `relative::inner::item_in_inner`
        let original_item_id = find_function_node_paranoid(
            &pre_merge_graphs,
            fixture_name,
            "src/lib.rs", // Defined inline
            &["crate".into(), "relative".into(), "inner".into()],
            "item_in_inner",
        )
        .id;

        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        // The ImportNode is inside `relative` and named `reexport_self`
        let import_id = find_import_node_id_by_name(
            &merged_parsed_graph,
            &["crate", "relative"],
            "reexport_self",
        );
        // The public path is where the item is defined/re-exported publicly
        let expected_public_path = NodePath::try_from(vec![
            "crate".into(),
            "relative".into(),
            "reexport_self".into(),
        ])?;

        assert_relation_exists(
            &tree.tree_relations(), // Pass tree relations directly
            GraphId::Node(import_id),
            GraphId::Node(original_item_id),
            RelationKind::ReExports,
            "Relation for reexport_self -> item_in_inner",
             // Removed duplicate argument
        );
        assert_eq!(
            tree.reexport_index().get(&expected_public_path),
            Some(&original_item_id),
            "Re-export index check for reexport_self"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "reexport")]
    #[ignore = "CFG attribute handling not implemented in process_export_rels"]
    fn test_spp_cfg_limitation() -> Result<(), SynParserError> {
        let fixture_name = "fixture_spp_edge_cases"; // Use the CFG version
        let pre_merge_graphs = run_phases_and_collect(fixture_name);
        // Target: `glob_item_cfg_a` (re-exported via glob from `glob_target`)
        // We expect this re-export *not* to be processed because of the CFG.
        let target_item_id = find_function_node_paranoid(
            &pre_merge_graphs,
            fixture_name,
            "src/glob_target/mod.rs", // Defined in file
            &["crate".into(), "glob_target".into()],
            "glob_item_cfg_a",
        )
        .id;

        let (merged_parsed_graph, tree) = build_tree_and_process_exports(fixture_name)?;

        // The public path *would be* ["crate", "glob_item_cfg_a"] if processed
        let expected_public_path =
            NodePath::try_from(vec!["crate".into(), "glob_item_cfg_a".into()])?;

        // Assert the relation DOES NOT exist
        let relation_exists = tree.tree_relations().iter().any(|tr| {
            tr.relation().kind == RelationKind::ReExports
                && tr.relation().target == GraphId::Node(target_item_id)
            // We don't know the ImportNode ID easily for glob, so check target only
        });
        assert!(
            !relation_exists,
            "ReExports relation for cfg-gated item 'glob_item_cfg_a' should NOT exist"
        );

        // Assert the item is NOT in the reexport_index
        assert!(
            tree.reexport_index().get(&expected_public_path).is_none(),
            "Re-export index should NOT contain cfg-gated item 'glob_item_cfg_a'"
        );

        Ok(())
    }

    // TODO: Add test for conflicting re-exports (requires fixture modification or new fixture)
    // #[test]
    // #[cfg(feature = "reexport")]
    // fn test_conflicting_reexports() { ... }
}
