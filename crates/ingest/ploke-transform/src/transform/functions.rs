use cozo::Num;
use itertools::Itertools;
use log::trace;
// workspace imports
use syn_parser::resolve::RelationIndexer;
use syn_parser::{parser::types::GenericParamKind, utils::LogStyle};
// crate-local imports
use crate::schema::primary_nodes::FunctionNodeSchema;

pub const LOG_TARGET_TRANSFORM: &str = "transform";

use super::*;
/// Transforms function nodes into the functions relation
pub(super) fn transform_functions(
    db: &Db<MemStorage>,
    functions: Vec<FunctionNode>,
    tree: &ModuleTree,
) -> Result<(), cozo::Error> {
    for mut function in functions.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &FunctionNodeSchema::SCHEMA;
        let func_params = process_func(tree, &mut function, schema);

        let script = script_put(&func_params, "function");
        db.run_script(
            // "?[id, name, return_type_id, docstring, body, tracking_hash] <- [[$id, $name, $return_type_id, $docstring, $body, $tracking_hash]] :put functions",
            &script,
            func_params,
            ScriptMutability::Mutable,
        )?;

        // Add function parameters
        for (i, param) in function.parameters.iter().enumerate() {
            let param_name = param
                .name
                .as_ref()
                .map(|s| DataValue::from(s.as_str()))
                .unwrap_or(DataValue::Null);

            let param_params = BTreeMap::from([
                ("function_id".to_string(), function.id.into()),
                ("param_index".to_string(), DataValue::from(i as i64)),
                ("param_name".to_string(), param_name),
                ("type_id".to_string(), param.type_id.into()),
                ("is_mutable".to_string(), DataValue::from(param.is_mutable)),
                ("is_self".to_string(), DataValue::from(param.is_self)),
            ]);

            let script = script_put(&param_params, "function_params");

            db.run_script(
                // "?[function_id, param_index, param_name, type_id, is_mutable, is_self] <- [[$function_id, $param_index, $param_name, $type_id, $is_mutable, $is_self]] :put function_params",
                &script,
                param_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add generic parameters
        for (i, generic_param) in function.generic_params.into_iter().enumerate() {
            // let entries = ["owner_id", "param_index", "kind", "name", "type_id"];

            let (params, script) = generic_param_script(function.id, i as i64, generic_param);
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // Add attributes
        for (i, attr) in function.attributes.iter().enumerate() {
            let value = attr
                .value
                .as_ref()
                .map(|s| DataValue::from(s.as_str()))
                .unwrap_or(DataValue::Null);

            let attr_params = BTreeMap::from([
                ("owner_id".to_string(), function.id.into()),
                ("attr_index".to_string(), DataValue::from(i as i64)),
                ("name".to_string(), DataValue::from(attr.name.as_str())),
                ("value".to_string(), value),
            ]);

            let script = script_put(&attr_params, "attributes");
            db.run_script(
                // "?[owner_id, attr_index, name, value] <- [[$owner_id, $attr_index, $name, $value]] :put attributes",
                &script,
                attr_params,
                ScriptMutability::Mutable,
            )?;
        }
    }

    Ok(())
}

fn process_func(
    tree: &ModuleTree,
    function: &mut FunctionNode,
    schema: &FunctionNodeSchema,
) -> BTreeMap<String, DataValue> {
    let (vis_kind, vis_path) = vis_to_dataval(&*function);

    // return type optional
    // Might want to change this to `()`
    let return_type_id = function
        .return_type
        .map(|id| id.into())
        .unwrap_or(DataValue::Null);
    // Can be empty, None->Null

    // doc string might be empty
    let docstring = function
        .docstring
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);
    // Can be empty, None->Null

    // body can be empty
    let body = function
        .body
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);
    // Can be empty, None->Null

    let th_cozo = DataValue::Uuid(cozo::UuidWrapper(function.tracking_hash.take().unwrap_or_else(|| {
            panic!("Invariant Violated: FunctionNode must have TrackingHash upon database insertion")
        }).0));

    let span_start = DataValue::Num(Num::Int(function.span.0 as i64));
    let span_end = DataValue::Num(Num::Int(function.span.1 as i64));
    let span = DataValue::List(Vec::from([span_start, span_end]));

    // find containing module through relation in module tree
    let module_id = tree
        .get_iter_relations_to(&function.id.as_any())
        .find_map(|r| r.rel().source_contains(function.id.to_pid()))
        .unwrap_or_else(|| {
            panic!("Invariant Violated: FunctionNode must have Contains relation with module")
        });

    let cfgs: Vec<DataValue> = function
        .cfgs
        .iter()
        .map(|s| DataValue::from(s.as_str()))
        .collect();

    // Insert into functions table
    let func_params = BTreeMap::from([
        (schema.id().to_string(), function.id.into()),
        (
            schema.name().to_string(),
            DataValue::from(function.name.as_str()),
        ),
        (schema.docstring().to_string(), docstring),
        (schema.span().to_string(), span),
        (schema.tracking_hash().to_string(), th_cozo),
        (schema.cfgs().to_string(), DataValue::List(cfgs)),
        (schema.return_type_id().to_string(), return_type_id),
        (schema.body().to_string(), body),
        // Kind of awkward, might want to visibility its own entity. Maybe just visibility
        // path?
        (schema.vis_kind().to_string(), vis_kind),
        (
            schema.vis_path().to_string(),
            vis_path.unwrap_or(DataValue::Null),
        ),
        // May remove this. Might be useful for debugging, less sure about in queries vs. the
        // `Contains` edge. Needs testing in `ploke-db`
        (schema.module_id().to_string(), module_id.into()),
    ]);
    func_params
}

fn generic_param_script(
    func_id: FunctionNodeId,
    i: i64,
    generic_param: syn_parser::parser::types::GenericParamNode,
) -> (BTreeMap<String, DataValue>, String) {
    let cozo_id: DataValue = generic_param.id.into();
    let name: DataValue = generic_param
        .kind
        .name()
        .map(DataValue::from)
        .unwrap_or_else(|| {
            log::error!(target: "transform", "{}: {} | {:?}", "Error".log_error(), "Invalid State, Generic Param without name", generic_param);
            panic!("Invalid State")
        });

    // Common fields to all variants
    let mut params = BTreeMap::from([
        ("id".to_string(), cozo_id),
        ("owner_id".to_string(), func_id.into()),
        ("param_index".to_string(), DataValue::Num(cozo::Num::Int(i))),
        ("kind".to_string(), DataValue::Str("Type".into())),
        ("name".to_string(), name),
    ]);

    // Handle variant-unique fields differently
    // (possibly change handling of "bounds" to be more general)
    match generic_param.kind {
        GenericParamKind::Type {
            name: _,
            bounds,
            default,
        } => {
            let cozo_bounds: Vec<DataValue> =
                bounds.into_iter().map(|t| t.to_cozo_uuid()).collect();
            params.insert("bounds".to_string(), DataValue::List(cozo_bounds));

            let cozo_default = default.map_or(DataValue::Null, |t| t.to_cozo_uuid());
            params.insert("default".to_string(), cozo_default);
        }
        // NOTE: Lifetime bounds currently just a String. When we actually start handling
        // lifetime bounds this will have to be improved.
        GenericParamKind::Lifetime { name: _, bounds } => {
            let cozo_lifetime_bounds: Vec<DataValue> =
                bounds.into_iter().map(DataValue::from).collect();
            params.insert("bounds".to_string(), DataValue::List(cozo_lifetime_bounds));
        }
        GenericParamKind::Const { name: _, type_id } => {
            params.insert("type_id".to_string(), type_id.to_cozo_uuid());
        }
    }

    let script = script_put(&params, "generic_param");
    trace!(target: LOG_TARGET_TRANSFORM,
        "{}: {} | {}",
        "Form Script".log_step(),
        "GenericParamKind".log_name(),
        script.log_magenta()
    );
    (params, script)
}

fn script_put(params: &BTreeMap<String, DataValue>, relation_name: &str) -> String {
    let entry_names = params.keys().join(", ");
    let param_names = params.keys().map(|k| format!("${}", k)).join(", ");
    // Should come out looking like:
    // "?[owner_id, param_index, kind, name, type_id] <- [[$owner_id, $param_index, $kind, $name, $type_id]] :put generic_params",
    let script = format!(
        "?[{}] <- [[{}]] :put {}",
        entry_names, param_names, relation_name
    );
    script
}

fn vis_to_dataval(function: &FunctionNode) -> (DataValue, Option<DataValue>) {
    let (vis_kind, vis_path) = match &function.visibility {
        VisibilityKind::Public => (DataValue::from("public".to_string()), None),
        VisibilityKind::Crate => ("crate".into(), None),
        VisibilityKind::Restricted(path) => {
            let list = DataValue::List(
                path.iter()
                    .map(|p_string| DataValue::from(p_string.to_string()))
                    .collect(),
            );
            ("restricted".into(), Some(list))
        }
        VisibilityKind::Inherited => ("inherited".into(), None),
    };
    (vis_kind, vis_path)
}
#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::{graph::GraphAccess, ParsedCodeGraph};
    use syn_parser::utils::LogStyle;

    use crate::schema::primary_nodes::FunctionNodeSchema;
    use crate::transform::functions::process_func;
    use crate::transform::functions::script_put;

    #[test]
    fn func_transform() -> Result<(), cozo::Error> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = run_phases_and_collect("fixture_types");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
        let tree = merged.build_module_tree().unwrap_or_else(|e| {
            log::error!(target: "transform_function",
                "Error building tree: {}",
                e
            );
            panic!()
        });

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let func_schema = &FunctionNodeSchema::SCHEMA;
        log::info!(target: "transform_function",
            "{}: {:?}",
            "Printing function schema V2".log_step(),
            func_schema.schema_string()
        );

        let schema = func_schema.schema_string();
        let db_result = db.run_script(&schema, BTreeMap::new(), cozo::ScriptMutability::Mutable);
        log::info!(target: "transform_function",
            "{}: {:?}",
            "function schema created".log_step(),
            db_result
        );
        let mut func_node = merged
            .graph
            .functions
            .iter()
            .find(|f| f.name == "process_tuple")
            .cloned()
            .expect("Cannot find target function node");
        let func_params = process_func(&tree, &mut func_node, func_schema);
        log::info!(target: "transform_function",
            "{}: {:#?}",
            "Build func_params".log_step(),
            func_params,
        );
        let script = script_put(&func_params, "function");
        log::info!(target: "transform_function",
            "{}: {:#?}",
            "Build func script".log_step(),
            script,
        );
        let name = "FunctionNodeSchema";
        let suff = name.strip_suffix("NodeSchema").unwrap();

        db.run_script(&script, func_params, cozo::ScriptMutability::Mutable)?;
        Ok(())
    }
}
