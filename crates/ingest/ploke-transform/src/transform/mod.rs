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
#[cfg(feature = "call_graph")]
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
#[cfg(feature = "call_graph")]
use edges::transform_call_resolution_report;
#[cfg(feature = "call_graph")]
use edges::transform_call_site_relations;
#[cfg(feature = "call_graph")]
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
    // CALL_GRAPH_GATE:db-projection - keep call graph DB facts out of the default transform path
    // until registered backup fixtures have been regenerated with call graph relations.
    #[cfg(feature = "call_graph")]
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
    // CALL_GRAPH_GATE:db-projection - feature-enabled tests keep strict call graph projection
    // assertions while default DB imports remain compatible with current registered fixtures.
    #[cfg(feature = "call_graph")]
    {
        tracing::trace!("{}: Starting", "call_sites".log_step());
        transform_call_sites(db, &code_graph.call_sites)?;
        tracing::trace!("{}: Starting", "call_site_relations".log_step());
        transform_call_site_relations(db, &code_graph.call_site_relations)?;
        tracing::trace!("{}: Starting", "call_resolution".log_step());
        transform_call_resolution_report(db, &call_resolution_report)?;
    }

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
    #[cfg(feature = "call_graph")]
    use cozo::DataValue;
    use cozo::{Db, MemStorage, ScriptMutability};
    use ploke_test_utils::test_run_phases_and_collect;
    use std::collections::BTreeMap;
    use syn_parser::parser::ParsedCodeGraph;
    #[cfg(feature = "call_graph")]
    use syn_parser::parser::graph::GraphAccess;
    #[cfg(feature = "call_graph")]
    use syn_parser::parser::nodes::{
        AnyCallSiteId, CallBodyOwnerId, CallNode, DynamicCallCallee, ToCozoUuid,
    };
    #[cfg(feature = "call_graph")]
    use syn_parser::parser::relations::{CallRelation, CallResolutionStatus};
    #[cfg(feature = "call_graph")]
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

    // CALL_GRAPH_GATE:db-projection - const initializer owners must persist with their owner kind.
    #[cfg(feature = "call_graph")]
    #[test]
    fn test_call_graph_projection_for_const_initializer_call()
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

    // CALL_GRAPH_GATE:db-projection - strict projection assertion for the feature-enabled DB slice.
    #[cfg(feature = "call_graph")]
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

    // CALL_GRAPH_GATE:db-projection - dynamic function edges must not be flattened to path functions.
    #[cfg(feature = "call_graph")]
    #[test]
    fn test_call_graph_projection_for_dynamic_function_call()
    -> Result<(), Box<dyn std::error::Error>> {
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_schema_all(&db)?;

        let successful_graphs = test_run_phases_and_collect("fixture_call_graph");
        let mut merged =
            ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
        let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
            tracing::error!(target: "transform_function", "Error building tree: {}", e);
            panic!()
        });

        let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
        let dynamic_relation_targets = |site_id| {
            call_report
                .relations
                .iter()
                .copied()
                .filter_map(|relation| match relation {
                    CallRelation::DynamicFunction { source, target } if source == site_id => {
                        Some(target.into())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let (call_site_id, target_function_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::Path { path }
                                    if path.as_slice() == ["local_target"]
                            ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect("fixture_call_graph should resolve (local_target)() as DynamicFunction");
        let call_site_db_id = call_site_id.to_cozo_uuid();
        let target_db_id: DataValue = target_function_id.into();
        let (cast_site_id, cast_target_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::FnPointerCastPath { path }
                                    if path.as_slice() == ["local_target"]
                            ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect(
                "fixture_call_graph should resolve (local_target as fn() -> i32)() as DynamicFunction",
            );
        let cast_site_db = cast_site_id.to_cozo_uuid();
        let cast_target_db: DataValue = cast_target_id.into();
        let (binding_cast_site_id, binding_cast_target_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::FnPointerCastInitializedLocalBinding {
                                    path,
                                    init_path,
                                } if path.as_slice() == ["f"]
                                    && init_path.as_slice() == ["local_target"]
                            ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect("fixture_call_graph should resolve (f as fn() -> i32)() as DynamicFunction");
        let binding_cast_site_db = binding_cast_site_id.to_cozo_uuid();
        let binding_cast_target_db: DataValue = binding_cast_target_id.into();
        let (deref_site_id, deref_target_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::DereferencedInitializedLocalBinding {
                                    path,
                                    init_path,
                                } if path.as_slice() == ["f"]
                                    && init_path.as_slice() == ["local_target"]
                            ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect("fixture_call_graph should resolve (*f)() as DynamicFunction");
        let deref_site_db = deref_site_id.to_cozo_uuid();
        let deref_target_db: DataValue = deref_target_id.into();
        let (block_site_id, block_target_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if dynamic_call.span == (13065, 13085)
                                && matches!(
                                    &dynamic_call.callee,
                                    DynamicCallCallee::Path { path }
                                        if path.as_slice() == ["local_target"]
                                ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect("fixture_call_graph should resolve ({ local_target })() as DynamicFunction");
        let block_site_db = block_site_id.to_cozo_uuid();
        let block_target_db: DataValue = block_target_id.into();
        let (branch_site_id, branch_target_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if dynamic_call.span == (13188, 13238)
                                && matches!(
                                    &dynamic_call.callee,
                                    DynamicCallCallee::IfBranchPaths { paths }
                                        if paths.as_slice()
                                            == [vec!["local_target".to_string()], vec!["local_target".to_string()]]
                                ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect(
                "fixture_call_graph should resolve if same-branch dynamic call as DynamicFunction",
            );
        let branch_site_db = branch_site_id.to_cozo_uuid();
        let branch_target_db: DataValue = branch_target_id.into();
        let branch_ambiguous_site_id = merged
            .call_sites()
            .iter()
            .find_map(|call| match call {
                CallNode::DynamicCall(dynamic_call)
                    if dynamic_call.span == (13306, 13356)
                        && matches!(
                            &dynamic_call.callee,
                            DynamicCallCallee::IfBranchPaths { paths }
                                if paths.as_slice()
                                    == [vec!["local_target".to_string()], vec!["other_target".to_string()]]
                        ) =>
                {
                    Some(dynamic_call.id)
                }
                _ => None,
            })
            .expect("fixture_call_graph should record ambiguous if-branch dynamic call site");
        let branch_ambiguous_targets = dynamic_relation_targets(branch_ambiguous_site_id);
        let branch_ambiguous_site_db = branch_ambiguous_site_id.to_cozo_uuid();
        let (match_site_id, match_target_id) = call_report
            .relations
            .iter()
            .copied()
            .find_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } => {
                    let source_any = AnyCallSiteId::Dynamic(source);
                    let call = merged
                        .call_sites()
                        .iter()
                        .find(|call| call.id() == source_any)?;
                    match call {
                        CallNode::DynamicCall(dynamic_call)
                            if dynamic_call.span == (13422, 13505)
                                && matches!(
                                    &dynamic_call.callee,
                                    DynamicCallCallee::MatchArmPaths { paths }
                                        if paths.as_slice()
                                            == [vec!["local_target".to_string()], vec!["local_target".to_string()]]
                                ) =>
                        {
                            Some((source, target))
                        }
                        _ => None,
                    }
                }
                CallRelation::Function { .. }
                | CallRelation::Method { .. }
                | CallRelation::AssociatedFunction { .. }
                | CallRelation::TupleStructConstructor { .. }
                | CallRelation::EnumVariantConstructor { .. } => None,
            })
            .expect(
                "fixture_call_graph should resolve match same-arm dynamic call as DynamicFunction",
            );
        let match_site_db = match_site_id.to_cozo_uuid();
        let match_target_db: DataValue = match_target_id.into();
        let match_ambiguous_site_id = merged
            .call_sites()
            .iter()
            .find_map(|call| match call {
                CallNode::DynamicCall(dynamic_call)
                    if dynamic_call.span == (13576, 13659)
                        && matches!(
                            &dynamic_call.callee,
                            DynamicCallCallee::MatchArmPaths { paths }
                                if paths.as_slice()
                                    == [vec!["local_target".to_string()], vec!["other_target".to_string()]]
                        ) =>
                {
                    Some(dynamic_call.id)
                }
                _ => None,
            })
            .expect("fixture_call_graph should record ambiguous match-arm dynamic call site");
        let match_ambiguous_targets = dynamic_relation_targets(match_ambiguous_site_id);
        let match_ambiguous_site_db = match_ambiguous_site_id.to_cozo_uuid();

        transform_parsed_graph(&db, merged, &tree)?;

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
            "expected one persisted dynamic function call_relation row"
        );
        assert_eq!(
            &relation_rows.rows[0][2],
            &DataValue::from("DynamicFunction")
        );
        assert_eq!(&relation_rows.rows[0][3], &DataValue::from("Dynamic"));
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
        assert_eq!(&status_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&status_rows.rows[0][2], &DataValue::from("Resolved"));
        assert_eq!(&status_rows.rows[0][3], &DataValue::from("LocalExact"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), cast_site_db.clone());
        params.insert("target_id".to_string(), cast_target_db);
        let cast_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                target_id = $target_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            cast_rows.rows.len(),
            1,
            "expected one persisted function-pointer cast dynamic call_relation row"
        );
        assert_eq!(&cast_rows.rows[0][2], &DataValue::from("DynamicFunction"));
        assert_eq!(&cast_rows.rows[0][3], &DataValue::from("Dynamic"));
        assert_eq!(&cast_rows.rows[0][4], &DataValue::from("Function"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), cast_site_db.clone());
        let site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(site_rows.rows.len(), 1);
        assert_eq!(&site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(
            &site_rows.rows[0][2],
            &DataValue::List(vec![DataValue::from("local_target")])
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), cast_site_db);
        let cast_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(cast_status_rows.rows.len(), 1);
        assert_eq!(&cast_status_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&cast_status_rows.rows[0][2], &DataValue::from("Resolved"));
        assert_eq!(&cast_status_rows.rows[0][3], &DataValue::from("LocalExact"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), binding_cast_site_db.clone());
        params.insert("target_id".to_string(), binding_cast_target_db);
        let binding_cast_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                target_id = $target_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            binding_cast_rows.rows.len(),
            1,
            "expected one persisted initialized function-pointer cast dynamic call_relation row"
        );
        assert_eq!(
            &binding_cast_rows.rows[0][2],
            &DataValue::from("DynamicFunction")
        );
        assert_eq!(&binding_cast_rows.rows[0][3], &DataValue::from("Dynamic"));
        assert_eq!(&binding_cast_rows.rows[0][4], &DataValue::from("Function"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), binding_cast_site_db.clone());
        let binding_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(binding_site_rows.rows.len(), 1);
        assert_eq!(&binding_site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(
            &binding_site_rows.rows[0][2],
            &DataValue::List(vec![DataValue::from("f")])
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), binding_cast_site_db);
        let binding_cast_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(binding_cast_status_rows.rows.len(), 1);
        assert_eq!(
            &binding_cast_status_rows.rows[0][1],
            &DataValue::from("Dynamic")
        );
        assert_eq!(
            &binding_cast_status_rows.rows[0][2],
            &DataValue::from("Resolved")
        );
        assert_eq!(
            &binding_cast_status_rows.rows[0][3],
            &DataValue::from("LocalExact")
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), deref_site_db.clone());
        params.insert("target_id".to_string(), deref_target_db);
        let deref_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                target_id = $target_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            deref_rows.rows.len(),
            1,
            "expected one persisted dereferenced function-pointer dynamic call_relation row"
        );
        assert_eq!(&deref_rows.rows[0][2], &DataValue::from("DynamicFunction"));
        assert_eq!(&deref_rows.rows[0][3], &DataValue::from("Dynamic"));
        assert_eq!(&deref_rows.rows[0][4], &DataValue::from("Function"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), deref_site_db.clone());
        let deref_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(deref_site_rows.rows.len(), 1);
        assert_eq!(&deref_site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(
            &deref_site_rows.rows[0][2],
            &DataValue::List(vec![DataValue::from("f")])
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), deref_site_db);
        let deref_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(deref_status_rows.rows.len(), 1);
        assert_eq!(&deref_status_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&deref_status_rows.rows[0][2], &DataValue::from("Resolved"));
        assert_eq!(
            &deref_status_rows.rows[0][3],
            &DataValue::from("LocalExact")
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), block_site_db.clone());
        params.insert("target_id".to_string(), block_target_db);
        let block_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                target_id = $target_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            block_rows.rows.len(),
            1,
            "expected one persisted block-path dynamic call_relation row"
        );
        assert_eq!(&block_rows.rows[0][2], &DataValue::from("DynamicFunction"));
        assert_eq!(&block_rows.rows[0][3], &DataValue::from("Dynamic"));
        assert_eq!(&block_rows.rows[0][4], &DataValue::from("Function"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), block_site_db.clone());
        let block_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(block_site_rows.rows.len(), 1);
        assert_eq!(&block_site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(
            &block_site_rows.rows[0][2],
            &DataValue::List(vec![DataValue::from("local_target")])
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), block_site_db);
        let block_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(block_status_rows.rows.len(), 1);
        assert_eq!(&block_status_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&block_status_rows.rows[0][2], &DataValue::from("Resolved"));
        assert_eq!(
            &block_status_rows.rows[0][3],
            &DataValue::from("LocalExact")
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), branch_site_db.clone());
        params.insert("target_id".to_string(), branch_target_db);
        let branch_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                target_id = $target_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            branch_rows.rows.len(),
            1,
            "expected one persisted if-branch dynamic call_relation row"
        );
        assert_eq!(&branch_rows.rows[0][2], &DataValue::from("DynamicFunction"));
        assert_eq!(&branch_rows.rows[0][3], &DataValue::from("Dynamic"));
        assert_eq!(&branch_rows.rows[0][4], &DataValue::from("Function"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), branch_site_db.clone());
        let branch_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(branch_site_rows.rows.len(), 1);
        assert_eq!(&branch_site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(
            &branch_site_rows.rows[0][2],
            &DataValue::List(vec![DataValue::from("local_target")])
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), branch_site_db);
        let branch_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(branch_status_rows.rows.len(), 1);
        assert_eq!(&branch_status_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&branch_status_rows.rows[0][2], &DataValue::from("Resolved"));
        assert_eq!(
            &branch_status_rows.rows[0][3],
            &DataValue::from("LocalExact")
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), branch_ambiguous_site_db.clone());
        let ambiguous_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(ambiguous_site_rows.rows.len(), 1);
        assert_eq!(&ambiguous_site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&ambiguous_site_rows.rows[0][2], &DataValue::Null);

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), branch_ambiguous_site_db.clone());
        let ambiguous_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(ambiguous_status_rows.rows.len(), 1);
        assert_eq!(
            &ambiguous_status_rows.rows[0][1],
            &DataValue::from("Dynamic")
        );
        assert_eq!(
            &ambiguous_status_rows.rows[0][2],
            &DataValue::from("Ambiguous")
        );
        assert_eq!(&ambiguous_status_rows.rows[0][3], &DataValue::Null);

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), branch_ambiguous_site_db);
        let ambiguous_relation_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        let mut actual_branch_targets = ambiguous_relation_rows
            .rows
            .iter()
            .map(|row| row[1].clone())
            .collect::<Vec<_>>();
        let mut expected_branch_targets = branch_ambiguous_targets;
        actual_branch_targets.sort();
        expected_branch_targets.sort();
        assert_eq!(
            actual_branch_targets, expected_branch_targets,
            "ambiguous if-branch dynamic call should persist proven candidates"
        );
        assert_eq!(ambiguous_relation_rows.rows.len(), 2);
        for row in &ambiguous_relation_rows.rows {
            assert_eq!(&row[2], &DataValue::from("DynamicFunction"));
            assert_eq!(&row[3], &DataValue::from("Dynamic"));
            assert_eq!(&row[4], &DataValue::from("Function"));
        }

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), match_site_db.clone());
        params.insert("target_id".to_string(), match_target_db);
        let match_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                target_id = $target_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            match_rows.rows.len(),
            1,
            "expected one persisted match-arm dynamic call_relation row"
        );
        assert_eq!(&match_rows.rows[0][2], &DataValue::from("DynamicFunction"));
        assert_eq!(&match_rows.rows[0][3], &DataValue::from("Dynamic"));
        assert_eq!(&match_rows.rows[0][4], &DataValue::from("Function"));

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), match_site_db.clone());
        let match_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(match_site_rows.rows.len(), 1);
        assert_eq!(&match_site_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(
            &match_site_rows.rows[0][2],
            &DataValue::List(vec![DataValue::from("local_target")])
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), match_site_db);
        let match_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(match_status_rows.rows.len(), 1);
        assert_eq!(&match_status_rows.rows[0][1], &DataValue::from("Dynamic"));
        assert_eq!(&match_status_rows.rows[0][2], &DataValue::from("Resolved"));
        assert_eq!(
            &match_status_rows.rows[0][3],
            &DataValue::from("LocalExact")
        );

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), match_ambiguous_site_db.clone());
        let match_ambiguous_site_rows = db.run_script(
            r#"?[id, call_kind, path] :=
                id = $call_site_id,
                *call_site{id, call_kind, path @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(match_ambiguous_site_rows.rows.len(), 1);
        assert_eq!(
            &match_ambiguous_site_rows.rows[0][1],
            &DataValue::from("Dynamic")
        );
        assert_eq!(&match_ambiguous_site_rows.rows[0][2], &DataValue::Null);

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), match_ambiguous_site_db.clone());
        let match_ambiguous_status_rows = db.run_script(
            r#"?[source_id, source_kind, status_kind, resolution_kind] :=
                source_id = $call_site_id,
                *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(match_ambiguous_status_rows.rows.len(), 1);
        assert_eq!(
            &match_ambiguous_status_rows.rows[0][1],
            &DataValue::from("Dynamic")
        );
        assert_eq!(
            &match_ambiguous_status_rows.rows[0][2],
            &DataValue::from("Ambiguous")
        );
        assert_eq!(&match_ambiguous_status_rows.rows[0][3], &DataValue::Null);

        let mut params = BTreeMap::new();
        params.insert("call_site_id".to_string(), match_ambiguous_site_db);
        let match_ambiguous_relation_rows = db.run_script(
            r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                source_id = $call_site_id,
                *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
            params,
            ScriptMutability::Immutable,
        )?;
        let mut actual_match_targets = match_ambiguous_relation_rows
            .rows
            .iter()
            .map(|row| row[1].clone())
            .collect::<Vec<_>>();
        let mut expected_match_targets = match_ambiguous_targets;
        actual_match_targets.sort();
        expected_match_targets.sort();
        assert_eq!(
            actual_match_targets, expected_match_targets,
            "ambiguous match-arm dynamic call should persist proven candidates"
        );
        assert_eq!(match_ambiguous_relation_rows.rows.len(), 2);
        for row in &match_ambiguous_relation_rows.rows {
            assert_eq!(&row[2], &DataValue::from("DynamicFunction"));
            assert_eq!(&row[3], &DataValue::from("Dynamic"));
            assert_eq!(&row[4], &DataValue::from("Function"));
        }

        Ok(())
    }

    // CALL_GRAPH_GATE:db-projection - strict projection assertion for the feature-enabled DB slice.
    #[cfg(feature = "call_graph")]
    #[test]
    fn test_call_graph_projection_for_method_edge_and_external_path_call()
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
}
