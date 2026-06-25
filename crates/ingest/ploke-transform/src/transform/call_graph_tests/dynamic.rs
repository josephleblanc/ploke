use super::*;
use syn_parser::parser::nodes::{DynamicCallNode, DynamicCallSiteId, FunctionNodeId};
use syn_parser::resolve::call_resolution::CallResolutionReport;

mod assertions;
mod cases;
mod lookup;

use assertions::*;
use cases::*;

// CALL_GRAPH_GATE:db-projection - dynamic function edges must not be flattened to path functions.
#[cfg(feature = "call_graph")]
#[test]
fn test_call_graph_projection_for_dynamic_function_call() -> Result<(), Box<dyn std::error::Error>>
{
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_call_graph");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let cases = dynamic_projection_cases(&call_report, &merged);

    transform_parsed_graph(&db, merged, &tree)?;

    for case in cases.resolved {
        assert_dynamic_relation(&db, case.site_id.clone(), case.target_id, case.label)?;
        if let Some(path) = case.path {
            assert_dynamic_site_path(&db, case.site_id.clone(), Some(path))?;
        }
        assert_dynamic_status(&db, case.site_id, "Resolved", Some("LocalExact"))?;
    }

    for case in cases.ambiguous {
        assert_dynamic_site_path(&db, case.site_id.clone(), None)?;
        assert_dynamic_status(&db, case.site_id.clone(), "Ambiguous", None)?;
        assert_dynamic_candidate_relations(&db, case.site_id, case.expected_targets, case.label)?;
    }

    Ok(())
}
