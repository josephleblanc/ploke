use cozo::DataValue;
use cozo::{Db, MemStorage, ScriptMutability};
use ploke_test_utils::test_run_phases_and_collect;
use std::collections::BTreeMap;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{
    AnyCallSiteId, CallBodyOwnerId, CallNode, DynamicCallCallee, ToCozoUuid,
};
use syn_parser::parser::relations::{CallRelation, CallResolutionStatus};
use syn_parser::resolve::call_resolution::resolve_call_relations_after_tree;

use crate::schema::create_schema_all;

use super::transform_parsed_graph;

#[test]
fn test_call_graph_projection_for_const_initializer_call() -> Result<(), Box<dyn std::error::Error>>
{
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_nodes");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let (call_site_id, target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::Function { source, target } => {
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == AnyCallSiteId::Path(source))?;
                let CallNode::PathCall(path_call) = call else {
                    return None;
                };
                if path_call.path == ["five"]
                    && matches!(path_call.owner, CallBodyOwnerId::Const(_))
                {
                    Some((source, target))
                } else {
                    None
                }
            }
            CallRelation::DynamicFunction { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_nodes should resolve const FN_CALL_CONST initializer five()");
    let call_site = merged
        .call_sites()
        .iter()
        .find(|call| call.id() == AnyCallSiteId::Path(call_site_id))
        .expect("resolved const call source should have a call-site row");
    let owner_id = match call_site.owner() {
        CallBodyOwnerId::Const(id) => {
            let value: DataValue = id.into();
            value
        }
        other => panic!("five() initializer call should be const-owned, got {other:?}"),
    };
    let call_site_db_id = call_site_id.to_cozo_uuid();
    let target_db_id: DataValue = target_id.into();

    transform_parsed_graph(&db, merged, &tree)?;

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), owner_id);
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    let edge_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $owner_id,
            target_id = $call_site_id,
            *call_site_edge{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        edge_rows.rows.len(),
        1,
        "expected one persisted const BodyContainsCall row"
    );
    assert_eq!(&edge_rows.rows[0][2], &DataValue::from("BodyContainsCall"));
    assert_eq!(&edge_rows.rows[0][3], &DataValue::from("Const"));
    assert_eq!(&edge_rows.rows[0][4], &DataValue::from("Path"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    params.insert("target_id".to_string(), target_db_id);
    let relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        relation_rows.rows.len(),
        1,
        "expected one persisted const initializer call_relation row"
    );
    assert_eq!(&relation_rows.rows[0][2], &DataValue::from("Function"));
    assert_eq!(&relation_rows.rows[0][3], &DataValue::from("Path"));
    assert_eq!(&relation_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id);
    let status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(status_rows.rows.len(), 1);
    assert_eq!(&status_rows.rows[0][1], &DataValue::from("Path"));
    assert_eq!(&status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(&status_rows.rows[0][3], &DataValue::from("LocalExact"));

    Ok(())
}

#[test]
fn test_call_graph_projection_for_resolved_path_call() -> Result<(), Box<dyn std::error::Error>> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_path_resolution");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let resolved_function_edge = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::Function { source, target } => {
                let source_any = AnyCallSiteId::Path(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::PathCall(path_call)
                        if path_call.path == ["super", "restricted_func"] =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::DynamicFunction { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_path_resolution should have a resolved local function call edge");
    let (call_site_id, target_function_id) = resolved_function_edge;
    let call_site_any = AnyCallSiteId::Path(call_site_id);
    let call_site = merged
        .call_sites()
        .iter()
        .find(|call| call.id() == call_site_any)
        .expect("resolved call edge source should have a structural call-site row");
    let CallNode::PathCall(path_call) = call_site else {
        panic!("resolved function edge source should be a PathCall");
    };
    assert_eq!(path_call.path, ["super", "restricted_func"]);
    let owner_id = match call_site.owner() {
        CallBodyOwnerId::Function(id) => {
            let value: DataValue = id.into();
            value
        }
        CallBodyOwnerId::Method(id) => {
            let value: DataValue = id.into();
            value
        }
        CallBodyOwnerId::Const(id) => {
            let value: DataValue = id.into();
            value
        }
        CallBodyOwnerId::Static(id) => {
            let value: DataValue = id.into();
            value
        }
    };
    let call_site_db_id = call_site_id.to_cozo_uuid();
    let target_db_id: DataValue = target_function_id.into();

    transform_parsed_graph(&db, merged, &tree)?;

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    params.insert("target_id".to_string(), target_db_id.clone());
    let call_relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        call_relation_rows.rows.len(),
        1,
        "expected exactly one persisted call_relation row for super::restricted_func()"
    );
    assert_eq!(&call_relation_rows.rows[0][2], &DataValue::from("Function"));
    assert_eq!(&call_relation_rows.rows[0][3], &DataValue::from("Path"));
    assert_eq!(&call_relation_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    params.insert("owner_id".to_string(), owner_id.clone());
    let call_site_rows = db.run_script(
        r#"?[id, owner_id, call_kind, path] :=
            id = $call_site_id,
            owner_id = $owner_id,
            *call_site{id, owner_id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        call_site_rows.rows.len(),
        1,
        "expected exactly one persisted call_site row for super::restricted_func()"
    );
    assert_eq!(&call_site_rows.rows[0][2], &DataValue::from("Path"));
    assert_eq!(
        &call_site_rows.rows[0][3],
        &DataValue::List(vec![
            DataValue::from("super"),
            DataValue::from("restricted_func"),
        ])
    );

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), owner_id);
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    let call_site_edge_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $owner_id,
            target_id = $call_site_id,
            *call_site_edge{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        call_site_edge_rows.rows.len(),
        1,
        "expected exactly one persisted BodyContainsCall row"
    );
    assert_eq!(
        &call_site_edge_rows.rows[0][2],
        &DataValue::from("BodyContainsCall")
    );
    assert_eq!(
        &call_site_edge_rows.rows[0][3],
        &DataValue::from("Function")
    );
    assert_eq!(&call_site_edge_rows.rows[0][4], &DataValue::from("Path"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id);
    let status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        status_rows.rows.len(),
        1,
        "expected exactly one persisted call_resolution_status row"
    );
    assert_eq!(&status_rows.rows[0][1], &DataValue::from("Path"));
    assert_eq!(&status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(&status_rows.rows[0][3], &DataValue::from("LocalExact"));

    Ok(())
}

mod dynamic;

#[test]
fn test_call_graph_projection_for_method_edge_and_external_path_call()
-> Result<(), Box<dyn std::error::Error>> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_nodes");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let (method_call_site_id, target_method_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::Method { source, target } => Some((source, target)),
            CallRelation::Function { .. }
            | CallRelation::DynamicFunction { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_nodes should have a resolved self.private_method() call edge");
    let method_call_any = AnyCallSiteId::Method(method_call_site_id);
    let method_call = merged
        .call_sites()
        .iter()
        .find(|call| call.id() == method_call_any)
        .expect("resolved method edge source should have a structural call-site row");
    let CallNode::MethodCall(method_call_node) = method_call else {
        panic!("resolved method edge source should be a MethodCall");
    };
    assert_eq!(method_call_node.method_name, "private_method");

    let pathbuf_call = merged
        .call_sites()
        .iter()
        .find_map(|call| match call {
            CallNode::PathCall(path_call) if path_call.path == ["PathBuf", "new"] => {
                Some(path_call)
            }
            _ => None,
        })
        .expect("fixture_nodes should include PathBuf::new() call site");
    let pathbuf_call_id = pathbuf_call.id;
    assert!(
        call_report.statuses.iter().any(|status| matches!(
            status,
            CallResolutionStatus::External { source }
                if *source == AnyCallSiteId::Path(pathbuf_call_id)
        )),
        "PathBuf::new() should be External before DB projection"
    );
    assert!(
        call_report.relations.iter().all(|relation| match relation {
            CallRelation::Function { source, .. }
            | CallRelation::AssociatedFunction { source, .. } => *source != pathbuf_call_id,
            CallRelation::DynamicFunction { .. } => true,
            CallRelation::TupleStructConstructor { source, .. }
            | CallRelation::EnumVariantConstructor { source, .. } => *source != pathbuf_call_id,
            CallRelation::Method { .. } => true,
        }),
        "PathBuf::new() should not have a fabricated call_relation before DB projection"
    );

    let method_call_db_id = method_call_site_id.to_cozo_uuid();
    let target_method_db_id: DataValue = target_method_id.into();
    let pathbuf_call_db_id = pathbuf_call_id.to_cozo_uuid();

    transform_parsed_graph(&db, merged, &tree)?;

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), method_call_db_id.clone());
    params.insert("target_id".to_string(), target_method_db_id);
    let method_relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        method_relation_rows.rows.len(),
        1,
        "expected exactly one persisted method call_relation row"
    );
    assert_eq!(&method_relation_rows.rows[0][2], &DataValue::from("Method"));
    assert_eq!(&method_relation_rows.rows[0][3], &DataValue::from("Method"));
    assert_eq!(&method_relation_rows.rows[0][4], &DataValue::from("Method"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), method_call_db_id);
    let method_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(method_status_rows.rows.len(), 1);
    assert_eq!(&method_status_rows.rows[0][1], &DataValue::from("Method"));
    assert_eq!(&method_status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(
        &method_status_rows.rows[0][3],
        &DataValue::from("LocalExact")
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), pathbuf_call_db_id.clone());
    let pathbuf_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(pathbuf_status_rows.rows.len(), 1);
    assert_eq!(&pathbuf_status_rows.rows[0][1], &DataValue::from("Path"));
    assert_eq!(
        &pathbuf_status_rows.rows[0][2],
        &DataValue::from("External")
    );
    assert_eq!(&pathbuf_status_rows.rows[0][3], &DataValue::Null);

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), pathbuf_call_db_id.clone());
    let pathbuf_relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind] :=
            source_id = $call_site_id,
            *call_relation{source_id, target_id, relation_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        pathbuf_relation_rows.rows.len(),
        0,
        "external PathBuf::new() should not have a persisted call_relation row"
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), pathbuf_call_db_id);
    let pathbuf_call_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(pathbuf_call_site_rows.rows.len(), 1);
    assert_eq!(&pathbuf_call_site_rows.rows[0][1], &DataValue::from("Path"));
    assert_eq!(
        &pathbuf_call_site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("PathBuf"), DataValue::from("new")])
    );

    Ok(())
}
