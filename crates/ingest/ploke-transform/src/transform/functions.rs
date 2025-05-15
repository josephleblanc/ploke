use cozo::Num;
use itertools::Itertools;
// crate-local imports
use crate::schema::primary_nodes::FunctionNodeSchema;
use crate::schema::secondary_nodes::{AttributeNodeSchema, ParamNodeSchema};

pub const LOG_TARGET_TRANSFORM: &str = "transform";

use super::*;
/// Transforms function nodes into the functions relation
pub(super) fn transform_functions(
    db: &Db<MemStorage>,
    functions: Vec<FunctionNode>,
    tree: &ModuleTree,
) -> Result<(), cozo::Error> {
    for mut function in functions.into_iter() {
        let function_any_id = function.id.as_any();
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &FunctionNodeSchema::SCHEMA;
        let func_params = process_func(tree, &mut function, schema);

        let script = script_put(&func_params, schema.relation);
        db.run_script(&script, func_params, ScriptMutability::Mutable)?;

        let param_schema = &ParamNodeSchema::SCHEMA;
        // Add function parameters
        for (i, param) in function.parameters.iter().enumerate() {
            let param_params = process_params(&function, param_schema, i, param);
            let script = script_put(&param_params, param_schema.relation);

            db.run_script(&script, param_params, ScriptMutability::Mutable)?;
        }

        // Add generic parameters
        for (i, generic_param) in function.generic_params.into_iter().enumerate() {
            let (params, script) = process_generic_params(function_any_id, i as i64, generic_param);
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in function.attributes.iter().enumerate() {
            let attr_params = process_attributes(function.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
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

    use cozo::DataValue;
    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::nodes::AsAnyNodeId;
    use syn_parser::parser::ParsedCodeGraph;
    use syn_parser::utils::{LogStyle, LogStyleDebug};

    use crate::schema::primary_nodes::FunctionNodeSchema;
    use crate::schema::secondary_nodes::ParamNodeSchema;
    use crate::transform::functions::script_put;
    use crate::transform::functions::{process_func, process_generic_params};

    #[test]
    fn func_transform() -> Result<(), cozo::Error> {
        // TODO: Make separate tests for each of these steps:
        //  - function processing
        //  - param processing
        //  - [✔] generic param processing
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
            "Printing function schema".log_step(),
            func_schema.script_create()
        );

        let schema = func_schema.script_create();
        let db_result = db.run_script(&schema, BTreeMap::new(), cozo::ScriptMutability::Mutable);
        log::info!(target: "transform_function",
            "{}: {:?}",
            "  db return".log_step(),
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
        let script = script_put(&func_params, func_schema.relation);
        log::info!(target: "transform_function",
            "{}: {:#?}",
            "Build func script".log_step(),
            script,
        );

        let db_result = db.run_script(&script, func_params, cozo::ScriptMutability::Mutable)?;
        log::info!(target: "transform_function",
            "{} {:#?}",
            "  Db return: ".log_step(),
            db_result,
        );

        let param_schema = ParamNodeSchema::SCHEMA;

        // TODO: Replace this with the actual function for processing params, and insert the
        // logging into the actual function `process_params`
        for (i, param) in func_node.parameters.iter().enumerate() {
            let param_name = param
                .name
                .as_ref()
                .map(|s| DataValue::from(s.as_str()))
                .unwrap_or(DataValue::Null);

            let param_params = BTreeMap::from([
                (param_schema.function_id().to_string(), func_node.id.into()),
                (
                    param_schema.param_index().to_string(),
                    DataValue::from(i as i64),
                ),
                (param_schema.name().to_string(), param_name),
                (param_schema.type_id().to_string(), param.type_id.into()),
                (
                    param_schema.is_mutable().to_string(),
                    DataValue::from(param.is_mutable),
                ),
                (
                    param_schema.is_self().to_string(),
                    DataValue::from(param.is_self),
                ),
            ]);

            log::info!(target: "transform_function",
                "{}: {:#?}",
                "Build param_params".log_step(),
                param_params,
            );

            let db_result = db.run_script(
                &param_schema.script_create(),
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            );
            log::info!(target: "transform_function",
                "{}: {:?}\n{} {:?}",
                "param schema created".log_step(),
                db_result,
                "schema:".log_magenta(),
                &param_schema.script_create(),
            );

            let script = script_put(&param_params, param_schema.relation);

            db.run_script(
                &script,
                param_params.clone(),
                cozo::ScriptMutability::Mutable,
            )
            .inspect_err(|_| {
                log::error!(target: "transform_function",
                    "{} {}\n{} {:?}\n{}\n{:#?}\n{} {:?}",
                    "Error:".log_error(),
                    "db.run_script faild with arguments:",
                    "script:".log_error(),
                    script,
                    "BTreeMap:".log_error(),
                    param_params,
                    "ScriptMutability: ",
                    cozo::ScriptMutability::Mutable
                );
            })?;

            log::info!(target: "transform_function",
                "{} {:#?}",
                "  Db return: ".log_step(),
                db_result,
            );
            for (i, attr) in func_node.attributes.iter().enumerate() {
                let value = attr
                    .value
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let attr_params = BTreeMap::from([
                    ("owner_id".to_string(), func_node.id.into()),
                    ("attr_index".to_string(), DataValue::from(i as i64)),
                    ("name".to_string(), DataValue::from(attr.name.as_str())),
                    ("value".to_string(), value),
                ]);

                let script = script_put(&attr_params, "attributes");
                db.run_script(
                    // "?[owner_id, attr_index, name, value] <- [[$owner_id, $attr_index, $name, $value]] :put attributes",
                    &script,
                    attr_params,
                    cozo::ScriptMutability::Mutable,
                )
                .inspect_err(|_| {
                    log::error!(target: "transform_function",
                        "{} {} {}\n{} {:?}\n{}\n{:#?}\n{} {:?}",
                        "Error:".log_error(),
                        "generic_params".log_foreground_primary(),
                        "db.run_script faild with arguments:",
                        "script:".log_error(),
                        script,
                        "BTreeMap:".log_error(),
                        param_params,
                        "ScriptMutability: ",
                        cozo::ScriptMutability::Mutable
                    );
                })?;
            }
        }

        // This function doesn't have any generics, so this is kind of a non-op,
        // but good to check it works on targets without generics as well I suppose.
        for (i, generic_param) in func_node.generic_params.into_iter().enumerate() {
            let (params, script) =
                process_generic_params(func_node.id.as_any(), i as i64, generic_param);
            db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;
        }

        Ok(())
    }

    fn log_db_result(db_result: cozo::NamedRows) {
        log::info!(target: "transform_function",
            "{} {}",
            "  Db return: ".log_step(),
            db_result.log_comment_debug(),
        );
    }
}
