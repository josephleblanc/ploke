//! Transforms CodeGraph into CozoDB relations

// -- external
use cozo::{DataValue, Db, MemStorage, Num, ScriptMutability};

// -- from workspace
use syn_parser::parser::nodes::*;
use syn_parser::parser::types::TypeNode;
use syn_parser::parser::{graph::CodeGraph, nodes::TypeDefNode, types::VisibilityKind};
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::resolve::RelationIndexer;
use syn_parser::utils::LogStyle;

// ---- local imports ----
// -- error handling --
use crate::error::TransformError;

// -- script creations
// use crate::schema::*;

// -- transforms
use consts::transform_consts;
use edges::transform_relations;
use enums::transform_enums;
use impls::transform_impls;
use imports::transform_imports;
use macros::transform_macros;
use module::transform_modules;
use statics::transform_statics;
use type_node::transform_types;
use std::collections::BTreeMap;
use structs::transform_structs;
use traits::transform_traits;
use type_alias::transform_type_aliases;
use unions::transform_unions;


mod fields;
mod secondary_nodes;
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
mod type_node;

// -- primary node transforms
use functions::transform_functions;

// -- secondary node transformations
use secondary_nodes::{process_attributes, process_generic_params, process_params};

// -- schema
use crate::schema::secondary_nodes::AttributeNodeSchema;

// -- edges

/// Transforms a CodeGraph into CozoDB relations
pub fn transform_code_graph(
    db: &Db<MemStorage>,
    code_graph: CodeGraph,
    tree: &ModuleTree,
) -> Result<(), TransformError> {
    // Transform types
    // [✔] Refactored
    transform_types(db, code_graph.type_graph)?;

    // Transform functions
    // [✔] Refactored
    transform_functions(db, code_graph.functions, tree)?;

    // Transform defined types (structs, enums, etc.)
    // [✔] Refactored
    //  - [✔] Struct Refactored
    //  - [✔] Enum Refactored
    //  - [✔] Union Refactored
    //  - [✔] TypeAlias Refactored
    //  TODO: Refactor CodeGraph to split these nodes into their own collections.
    transform_defined_types(db, code_graph.defined_types)?;

    // Transform traits
    // [✔] Refactored
    transform_traits(db, code_graph.traits)?;

    // Transform impls
    // [✔] Refactored
    transform_impls(db, code_graph.impls)?;

    // Transform modules
    // [✔] Refactored
    transform_modules(db, code_graph.modules)?;

    // Transform consts
    // [✔] Refactored
    transform_consts(db, code_graph.consts)?;

    // Transoform statics
    // [✔] Refactored
    transform_statics(db, code_graph.statics)?;

    // Transform macros
    // [✔] Refactored
    transform_macros(db, code_graph.macros)?;

    // Transform imports/reexports
    // [✔] Refactored
    transform_imports(db, code_graph.use_statements)?;

    // Transform relations
    // [✔] Refactored
    transform_relations(db, code_graph.relations)?;

    Ok(())
}

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
    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{error::TransformError, schema::create_schema_all};

    use super::transform_code_graph;

    #[test]
    fn test_insert_all() -> Result<(), TransformError> { 

        // initialize db
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        // create and insert schema for all nodes
        create_schema_all(&db)?;

        // run the parser
        let successful_graphs = run_phases_and_collect("fixture_nodes");
        // merge results from all files
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        // build module tree
        let tree = merged.build_module_tree().unwrap_or_else(|e| {
            log::error!(target: "transform_function",
                "Error building tree: {}",
                e
            );
            panic!()
        });

        transform_code_graph(&db, merged.graph, &tree)?;
        Ok(())
    }
}
