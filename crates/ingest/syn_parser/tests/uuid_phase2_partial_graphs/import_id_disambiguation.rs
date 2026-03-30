//! Regression: synthetic `ImportNodeId` must differ when the same leaf name is imported from
//! different source paths (see `process_use_tree` full-path `id_key`).

use crate::common::run_phase1_phase2;
use syn_parser::parser::graph::GraphAccess;

#[test]
fn duplicate_leaf_name_different_paths_distinct_import_ids() {
    let results = run_phase1_phase2("fixture_import_duplicate_leaf");
    assert_eq!(
        results.len(),
        1,
        "fixture should expose a single lib.rs graph"
    );
    let parsed = results.into_iter().next().unwrap().expect("parse");
    let graph = &parsed.graph;

    let result_imports: Vec<_> = graph
        .use_statements
        .iter()
        .filter(|i| i.visible_name == "Result" && !i.is_glob)
        .collect();

    assert_eq!(
        result_imports.len(),
        2,
        "expected two `Result` imports from std::io and std::fmt; got {:?}",
        graph
            .use_statements
            .iter()
            .map(|i| (&i.visible_name, &i.source_path))
            .collect::<Vec<_>>()
    );

    assert_ne!(
        result_imports[0].id, result_imports[1].id,
        "ImportNodeId must not collide when only the leaf name matches"
    );

    assert!(
        graph.validate_unique_rels(),
        "relations must remain unique after import ID fix"
    );
}
