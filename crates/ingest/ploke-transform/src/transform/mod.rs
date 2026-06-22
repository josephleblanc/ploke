//! Transforms CodeGraph into CozoDB relations

// -- external
use cozo::{DataValue, Db, MemStorage, Num, ScriptMutability};

use crate_context::transform_crate_context;
pub use workspace::transform_parsed_workspace;
// -- from workspace
use syn_parser::ParsedCodeGraph;
use syn_parser::parser::nodes::*;
use syn_parser::parser::types::TypeNode;
use syn_parser::parser::{graph::CodeGraph, nodes::TypeDefNode, types::VisibilityKind};
use syn_parser::resolve::RelationIndexer;
use syn_parser::resolve::call_resolution::resolve_call_relations_after_tree;
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::resolve::type_resolution_v2::resolve_type_relations_after_tree;
use syn_parser::utils::LogStyle;

// ---- local imports ----
// -- error handling --
use crate::error::TransformError;

// -- script creations
// use crate::schema::*;

// -- transforms
use consts::transform_consts;
use edges::transform_call_resolution_report;
use edges::transform_call_site_relations;
use edges::transform_call_sites;
use edges::transform_relations;
use edges::transform_type_relations;
use enums::transform_enums;
use impls::transform_impls;
use imports::transform_imports;
use macros::transform_macros;
use module::transform_modules;
use statics::transform_statics;
use std::collections::BTreeMap;
use structs::transform_structs;
use tracing::instrument;
use traits::transform_traits;
use type_alias::transform_type_aliases;
use type_graph::transform_type_graph_edges;
use type_node::transform_types;
use unions::transform_unions;

mod fields;
mod secondary_nodes;
// -- special case nodes --
pub mod compilation_unit;
pub mod union_crate_masks;
pub use compilation_unit::insert_structural_compilation_unit_slice;
pub use union_crate_masks::transform_union_crate_and_structural_masks;
mod crate_context;
mod workspace;
// -- primary nodes --
mod consts;
mod edges;
mod enums;
mod functions;
mod impls;
mod imports;
mod macros;
mod module;
mod statics;
mod structs;
mod traits;
mod type_alias;
mod unions;

// -- types --
mod type_graph;
mod type_node;

// -- primary node transforms
use functions::transform_functions;

// -- secondary node transformations
use secondary_nodes::{process_attributes, process_generic_params, process_params};

// -- schema
use crate::schema::secondary_nodes::AttributeNodeSchema;

// -- edges

#[deprecated = "Use transform_parsed_graph instead"]
/// Transforms a CodeGraph into CozoDB relations
pub fn transform_code_graph(
    db: &Db<MemStorage>,
    code_graph: CodeGraph,
    tree: &ModuleTree,
    namespace: uuid::Uuid,
) -> Result<(), TransformError> {
    // Transform types
    transform_types(db, code_graph.type_graph)?;

    // Transform functions
    transform_functions(db, code_graph.functions, tree)?;

    // Transform defined types (structs, enums, etc.)
    //  TODO: Refactor CodeGraph to split these nodes into their own collections.
    transform_defined_types(db, code_graph.defined_types)?;

    // Transform traits
    transform_traits(db, code_graph.traits)?;

    // Transform impls
    transform_impls(db, code_graph.impls)?;

    // Transform modules
    transform_modules(db, code_graph.modules, namespace)?;

    // Transform consts
    transform_consts(db, code_graph.consts)?;

    // Transoform statics
    transform_statics(db, code_graph.statics)?;

    // Transform macros
    transform_macros(db, code_graph.macros)?;

    // Transform imports/reexports
    transform_imports(db, code_graph.use_statements)?;

    // Transform relations
    transform_relations(db, code_graph.relations)?;

    Ok(())
}

/// Transforms a CodeGraph into CozoDB relations, inserts into the cozo database
#[instrument(skip_all)]
pub fn transform_parsed_graph(
    db: &Db<MemStorage>,
    parsed_graph: ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<(), TransformError> {
    let type_relation_report =
        resolve_type_relations_after_tree(&parsed_graph, tree).map_err(|err| {
            TransformError::Transformation(format!("typed type relation resolution failed: {err}"))
        })?;
    let call_resolution_report =
        resolve_call_relations_after_tree(&parsed_graph, tree).map_err(|err| {
            TransformError::Transformation(format!("typed call relation resolution failed: {err}"))
        })?;

    let code_graph = parsed_graph.graph;
    let crate_context = parsed_graph
        .crate_context
        .expect("Invariant: All Code Graphs must have a Crate Context");

    tracing::trace!("{}: Starting", "type_graph_edges".log_step());
    transform_type_graph_edges(db, &code_graph)?;
    tracing::trace!("{}: Starting", "types".log_step());
    transform_types(db, code_graph.type_graph)?;
    tracing::trace!("{}: Starting", "functions".log_step());
    transform_functions(db, code_graph.functions, tree)?;

    tracing::trace!("{}: Starting", "defined_types".log_step());
    transform_defined_types(db, code_graph.defined_types)?;

    tracing::trace!("{}: Starting", "traits".log_step());
    transform_traits(db, code_graph.traits)?;
    tracing::trace!("{}: Starting", "impls".log_step());
    transform_impls(db, code_graph.impls)?;
    tracing::trace!("{}: Starting", "modules".log_step());
    transform_modules(db, code_graph.modules, crate_context.namespace)?;
    tracing::trace!("{}: Starting", "consts".log_step());
    transform_consts(db, code_graph.consts)?;
    tracing::trace!("{}: Starting", "statics".log_step());
    transform_statics(db, code_graph.statics)?;
    tracing::trace!("{}: Starting", "macros".log_step());
    transform_macros(db, code_graph.macros)?;
    tracing::trace!("{}: Starting", "imports".log_step());
    transform_imports(db, code_graph.use_statements)?;
    tracing::trace!("{}: Starting", "relations".log_step());
    transform_relations(db, code_graph.relations)?;
    tracing::trace!("{}: Starting", "type_relations".log_step());
    transform_type_relations(db, &type_relation_report)?;
    tracing::trace!("{}: Starting", "call_sites".log_step());
    transform_call_sites(db, &code_graph.call_sites)?;
    tracing::trace!("{}: Starting", "call_site_relations".log_step());
    transform_call_site_relations(db, &code_graph.call_site_relations)?;
    tracing::trace!("{}: Starting", "call_resolution".log_step());
    transform_call_resolution_report(db, &call_resolution_report)?;

    tracing::trace!("{}: Starting", "crate_context".log_step());
    transform_crate_context(db, crate_context)?;

    Ok(())
}

#[instrument(skip_all)]
fn transform_defined_types(
    db: &Db<MemStorage>,
    defined_types: Vec<TypeDefNode>,
) -> Result<(), TransformError> {
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut type_aliases = Vec::new();
    let mut unions = Vec::new();

    for defined_type in defined_types.into_iter() {
        match defined_type {
            TypeDefNode::Struct(sn) => structs.push(sn),
            TypeDefNode::Enum(en) => enums.push(en),
            TypeDefNode::TypeAlias(tn) => type_aliases.push(tn),
            TypeDefNode::Union(un) => unions.push(un),
        }
    }
    transform_structs(db, structs)?;
    transform_enums(db, enums)?;
    transform_type_aliases(db, type_aliases)?;
    transform_unions(db, unions)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use cozo::{DataValue, Db, MemStorage, ScriptMutability};
    use ploke_test_utils::test_run_phases_and_collect;
    use std::collections::BTreeMap;
    use syn_parser::parser::ParsedCodeGraph;
    use syn_parser::parser::graph::GraphAccess;
    use syn_parser::parser::nodes::{AnyCallSiteId, CallBodyOwnerId, CallNode, ToCozoUuid};
    use syn_parser::parser::relations::{CallRelation, CallResolutionStatus};
    use syn_parser::resolve::call_resolution::resolve_call_relations_after_tree;

    use crate::{error::TransformError, schema::create_schema_all};

    use super::transform_parsed_graph;

    #[test]
    fn test_insert_all() -> Result<(), TransformError> {
        // initialize db
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        // create and insert schema for all nodes
        create_schema_all(&db)?;

        // run the parser
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        // merge results from all files
        let mut merged =
            ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        // build module tree
        let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
            tracing::error!(target: "transform_function",
                "Error building tree: {}",
                e
            );
            panic!()
        });

        transform_parsed_graph(&db, merged, &tree)?;

        let resolved_type_rows = db.run_script(
            r#"?[source_id, target_id] :=
                *type_relation {
                    source_id,
                    target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }"#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )?;
        assert!(
            !resolved_type_rows.rows.is_empty(),
            "expected resolved type relation edges"
        );

        Ok(())
    }

    #[test]
    fn test_call_graph_projection_for_resolved_path_call() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_schema_all(&db)?;

        let successful_graphs = test_run_phases_and_collect("fixture_path_resolution");
        let mut merged =
            ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
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
                CallRelation::Function { source, target } => Some((source, target)),
                CallRelation::Method { .. } => None,
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

    #[test]
    fn test_call_graph_projection_for_method_edge_and_unsupported_path_call()
    -> Result<(), Box<dyn std::error::Error>> {
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_schema_all(&db)?;

        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let mut merged =
            ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
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
                CallRelation::Function { .. } => None,
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
                CallResolutionStatus::Unsupported { source }
                    if *source == AnyCallSiteId::Path(pathbuf_call_id)
            )),
            "PathBuf::new() should be Unsupported before DB projection"
        );
        assert!(
            call_report.relations.iter().all(|relation| {
                !matches!(relation, CallRelation::Function { source, .. } if *source == pathbuf_call_id)
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
            &DataValue::from("Unsupported")
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
            "unsupported PathBuf::new() should not have a persisted call_relation row"
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
}
