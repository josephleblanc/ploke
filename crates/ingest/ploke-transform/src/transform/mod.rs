//! Transforms CodeGraph into CozoDB relations

use consts::transform_consts;
// -- external
use cozo::{DataValue, Db, MemStorage, Num, ScriptMutability};
use enums::transform_enums;
use impls::transform_impls;
use macros::transform_macros;
use module::transform_modules;
use statics::transform_statics;
use std::collections::BTreeMap;
use structs::transform_structs;
use traits::transform_traits;
use type_alias::transform_type_aliases;
use unions::transform_unions;
// -- from workspace
use syn_parser::parser::nodes::*;
use syn_parser::parser::types::TypeNode;
use syn_parser::parser::{graph::CodeGraph, nodes::TypeDefNode, types::VisibilityKind};
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::resolve::RelationIndexer;
use syn_parser::utils::LogStyle;

mod fields;
mod secondary_nodes;
// -- primary nodes --
mod consts;
mod enums;
mod functions;
mod impls;
mod macros;
mod module;
mod statics;
mod structs;
mod traits;
mod type_alias;
mod unions;

// -- primary node transforms
use functions::transform_functions;

// -- secondary node transformations
use secondary_nodes::{process_attributes, process_generic_params, process_params};

// -- schema
use crate::schema::secondary_nodes::AttributeNodeSchema;

/// Transforms a CodeGraph into CozoDB relations
pub fn transform_code_graph(
    db: &Db<MemStorage>,
    code_graph: CodeGraph,
    tree: &ModuleTree,
) -> Result<(), cozo::Error> {
    #[cfg(not(feature = "type_bearing_ids"))]
    let relations = code_graph.relations;
    // Transform types
    // [ ] Refactored
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

    // TODO: ImportNode

    // Transform relations
    // [ ] Refactored
    #[cfg(not(feature = "type_bearing_ids"))]
    transform_relations(db, code_graph)?;

    Ok(())
}

fn transform_defined_types(
    db: &Db<MemStorage>,
    defined_types: Vec<TypeDefNode>,
) -> Result<(), cozo::Error> {
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

/// Transforms type nodes into the types relation
fn transform_types(db: &Db<MemStorage>, type_graph: Vec<TypeNode>) -> Result<(), cozo::Error> {
    for type_node in type_graph {
        let kind = match &type_node.kind {
            ploke_core::TypeKind::Named { .. } => "Named",
            ploke_core::TypeKind::Reference { .. } => "Reference",
            ploke_core::TypeKind::Slice { .. } => "Slice",
            ploke_core::TypeKind::Array { .. } => "Array",
            ploke_core::TypeKind::Tuple { .. } => "Tuple",
            ploke_core::TypeKind::Never => "Never",
            ploke_core::TypeKind::Inferred => "Inferred",
            ploke_core::TypeKind::RawPointer { .. } => "RawPointer",
            ploke_core::TypeKind::ImplTrait { .. } => "ImplTrait",
            ploke_core::TypeKind::TraitObject { .. } => "TraitObject",
            ploke_core::TypeKind::Macro { .. } => "Macro",
            ploke_core::TypeKind::Unknown { .. } => "Unknown",
            ploke_core::TypeKind::Function {
                is_unsafe: _,
                is_extern: _,
                abi: _,
            } => "Function",
            ploke_core::TypeKind::Paren { .. } => "Paren",
        };

        // Create a simplified string representation of the type
        let type_str = format!("{:?}", type_node.kind);

        let params = BTreeMap::from([
            ("id".to_string(), type_node.id.into()),
            ("kind".to_string(), DataValue::from(kind)),
            ("type_str".to_string(), DataValue::from(type_str)),
        ]);

        db.run_script(
            "?[id, kind, type_str] <- [[$id, $kind, $type_str]] :put types",
            params,
            ScriptMutability::Mutable,
        )?;

        // Add type relations for related types
        for (i, related_id) in type_node.related_types.iter().enumerate() {
            let relation_params = BTreeMap::from([
                ("type_id".to_string(), type_node.id.into()),
                ("related_index".to_string(), DataValue::from(i as i64)),
                ("related_type_id".to_string(), related_id.into()),
            ]);

            db.run_script(
                "?[type_id, related_index, related_type_id] <- [[$type_id, $related_index, $related_type_id]] :put type_relations",
                relation_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add type details
        match &type_node.kind {
            ploke_core::TypeKind::Reference {
                lifetime,
                is_mutable,
                ..
            } => {
                let lifetime_value = lifetime
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
                    ("is_mutable".to_string(), DataValue::from(*is_mutable)),
                    ("lifetime".to_string(), lifetime_value),
                    ("abi".to_string(), DataValue::Null),
                    ("is_unsafe".to_string(), DataValue::from(false)),
                    ("is_extern".to_string(), DataValue::from(false)),
                    ("dyn_token".to_string(), DataValue::from(false)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            ploke_core::TypeKind::RawPointer { is_mutable, .. } => {
                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
                    ("is_mutable".to_string(), DataValue::from(*is_mutable)),
                    ("lifetime".to_string(), DataValue::Null),
                    ("abi".to_string(), DataValue::Null),
                    ("is_unsafe".to_string(), DataValue::from(false)),
                    ("is_extern".to_string(), DataValue::from(false)),
                    ("dyn_token".to_string(), DataValue::from(false)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            ploke_core::TypeKind::Function {
                is_unsafe,
                is_extern,
                abi,
                ..
            } => {
                let abi_value = abi
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
                    ("is_mutable".to_string(), DataValue::from(false)),
                    ("lifetime".to_string(), DataValue::Null),
                    ("abi".to_string(), abi_value),
                    ("is_unsafe".to_string(), DataValue::from(*is_unsafe)),
                    ("is_extern".to_string(), DataValue::from(*is_extern)),
                    ("dyn_token".to_string(), DataValue::from(false)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            ploke_core::TypeKind::TraitObject { dyn_token, .. } => {
                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
                    ("is_mutable".to_string(), DataValue::from(false)),
                    ("lifetime".to_string(), DataValue::Null),
                    ("abi".to_string(), DataValue::Null),
                    ("is_unsafe".to_string(), DataValue::from(false)),
                    ("is_extern".to_string(), DataValue::from(false)),
                    ("dyn_token".to_string(), DataValue::from(*dyn_token)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            _ => {}
        }
    }

    Ok(())
}
