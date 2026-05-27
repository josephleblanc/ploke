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
use syn_parser::resolve::module_tree::ModuleTree;
#[cfg(not(feature = "typed_type_graph"))]
use syn_parser::resolve::type_resolution::resolve_type_uses_after_tree;
#[cfg(feature = "typed_type_graph")]
use syn_parser::resolve::type_resolution_v2::resolve_type_relations_after_tree;
use syn_parser::utils::LogStyle;

// ---- local imports ----
// -- error handling --
use crate::error::TransformError;

// -- script creations
// use crate::schema::*;

// -- transforms
use consts::transform_consts;
use edges::transform_relations;
#[cfg(not(feature = "typed_type_graph"))]
use edges::transform_resolved_type_uses;
#[cfg(feature = "typed_type_graph")]
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
#[cfg(feature = "typed_type_graph")]
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
#[cfg(feature = "typed_type_graph")]
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
#[cfg(feature = "typed_type_graph")]
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

    tracing::trace!("{}: Starting", "crate_context".log_step());
    transform_crate_context(db, crate_context)?;

    Ok(())
}

/// Transforms a CodeGraph into CozoDB relations, inserts into the cozo database
#[cfg(not(feature = "typed_type_graph"))]
#[instrument(skip_all)]
pub fn transform_parsed_graph(
    db: &Db<MemStorage>,
    parsed_graph: ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<(), TransformError> {
    let type_resolution_report =
        resolve_type_uses_after_tree(&parsed_graph, tree).map_err(|err| {
            TransformError::Transformation(format!("late type resolution failed: {err}"))
        })?;

    // ANCHOR: transform_parsed_graph_methods
    let code_graph = parsed_graph.graph;
    let crate_context = parsed_graph
        .crate_context
        .expect("Invariant: All Code Graphs must have a Crate Context");

    tracing::trace!("{}: Starting", "types".log_step());
    transform_types(db, code_graph.type_graph)?;
    tracing::trace!("{}: Starting", "functions".log_step());
    transform_functions(db, code_graph.functions, tree)?;

    //  TODO: Refactor CodeGraph to split these nodes into their own collections.
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
    // TODO(import-backlinks): Keep import nodes and import-bearing relations in lockstep here.
    // We now rely on `ImportNode`s plus `ModuleImports` / `ReExports` / `ImportedBy` as real
    // graph facts, so downstream transform/schema/query layers must not treat imports as
    // second-class or optional metadata. When relation handling changes, verify import nodes are
    // still transformed and that import relations are still inserted and consumed end-to-end.
    transform_imports(db, code_graph.use_statements)?;
    tracing::trace!("{}: Starting", "relations".log_step());
    transform_relations(db, code_graph.relations)?;
    tracing::trace!("{}: Starting", "resolved_type_uses".log_step());
    transform_resolved_type_uses(db, &type_resolution_report)?;

    tracing::trace!("{}: Starting", "crate_context".log_step());
    transform_crate_context(db, crate_context)?;

    Ok(())
    // ANCHOR_END: transform_parsed_graph_methods
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
    use cozo::{Db, MemStorage, ScriptMutability};
    use ploke_test_utils::test_run_phases_and_collect;
    use std::collections::BTreeMap;
    use syn_parser::parser::ParsedCodeGraph;

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

        #[cfg(not(feature = "typed_type_graph"))]
        let resolved_type_rows = db.run_script(
            r#"?[owner_id, type_id, target_id] :=
                *resolved_type_use {
                    owner_id,
                    type_id,
                    target_id,
                    role: "method_return" @ 'NOW'
                }"#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )?;
        #[cfg(feature = "typed_type_graph")]
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
}
