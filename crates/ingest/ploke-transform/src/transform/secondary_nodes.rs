use syn_parser::parser::types::GenericParamKind;

use super::*;
use crate::{
    schema::secondary_nodes::{
        FieldNodeSchema, GenericConstNodeSchema, GenericLifetimeNodeSchema, GenericTypeNodeSchema,
        ParamNodeSchema,
    },
    transform::functions::LOG_TARGET_TRANSFORM,
};

pub(super) fn process_attributes(
    owner_id: AnyNodeId,
    i: usize,
    attr: &Attribute,
) -> BTreeMap<String, DataValue> {
    let schema = AttributeNodeSchema::SCHEMA;
    let value = attr
        .value
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);
    let args: Vec<DataValue> = attr
        .args
        .iter()
        .map(|s| DataValue::from(s.as_str()))
        .collect();

    let attr_params = BTreeMap::from([
        (schema.owner_id().to_string(), owner_id.to_cozo_uuid()),
        (schema.index().to_string(), DataValue::from(i as i64)),
        (
            schema.name().to_string(),
            DataValue::from(attr.name.as_str()),
        ),
        (schema.value().to_string(), value),
        (schema.args().to_string(), DataValue::List(args)),
    ]);
    attr_params
}

pub(super) fn process_params(
    function: &FunctionNode,
    param_schema: &ParamNodeSchema,
    i: usize,
    param: &ParamData,
) -> BTreeMap<String, DataValue> {
    let param_name = param
        .name
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);
    // TODO: Consider implementing a name for unnamed parameters.
    //  - Test if there are cases where there is more than one unnamed function parameter,
    //  - If the unnamed parameter is not actually empty, this could be problematic for
    //  generating the NodeId of the parameter names.
    // .unwrap_or_else(|| {
    //     log::error!(target: LOG_TARGET_TRANSFORM,
    //         "{} {} {} {} {}",
    //         "Error: Invariant Violated".log_error(),
    //         "Expected:".log_magenta(),
    //         "param_data to have a name, e.g. ",
    //         "Actual:".log_yellow(),
    //         "param_schema does not have a name upon database insertion."
    //     );
    //     panic!()
    // });

    let param_params = BTreeMap::from([
        (param_schema.function_id().to_string(), function.id.into()),
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
    param_params
}

pub(super) fn process_generic_params(
    any_id: AnyNodeId,
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

    // Handle variant-unique fields differently
    // (possibly change handling of "bounds" to be more general)
    // let (schema_relation, params) = match generic_param.kind {
    match generic_param.kind {
        GenericParamKind::Type {
            name: _,
            bounds,
            default,
        } => {
            let schema = GenericTypeNodeSchema::SCHEMA;
            let cozo_bounds: Vec<DataValue> =
                bounds.into_iter().map(|t| t.to_cozo_uuid()).collect();

            let cozo_default = default.map_or(DataValue::Null, |t| t.to_cozo_uuid());

            // Common fields to all variants
            let params = BTreeMap::from([
                (schema.id().to_string(), cozo_id),
                (schema.owner_id().to_string(), any_id.to_cozo_uuid()),
                (
                    schema.param_index().to_string(),
                    DataValue::Num(cozo::Num::Int(i)),
                ),
                (schema.kind().to_string(), DataValue::Str("Type".into())),
                (schema.name().to_string(), name),
                (schema.bounds().to_string(), DataValue::List(cozo_bounds)),
                (schema.default().to_string(), cozo_default),
            ]);
            let script = schema.script_put(&params);
            log::trace!(target: LOG_TARGET_TRANSFORM,
                "{}: {} | {}",
                "Form Script".log_step(),
                schema.relation.log_name(),
                script.log_magenta()
            );
            (params, script)
        }
        // NOTE: Lifetime bounds currently just a String. When we actually start handling
        // lifetime bounds this will have to be improved.
        GenericParamKind::Lifetime { name: _, bounds } => {
            let schema = GenericLifetimeNodeSchema::SCHEMA;
            let cozo_lifetime_bounds: Vec<DataValue> =
                bounds.into_iter().map(DataValue::from).collect();
            let params = BTreeMap::from([
                (schema.id().to_string(), cozo_id),
                (schema.owner_id().to_string(), any_id.to_cozo_uuid()),
                (
                    schema.param_index().to_string(),
                    DataValue::Num(cozo::Num::Int(i)),
                ),
                (schema.kind().to_string(), DataValue::Str("Type".into())),
                (schema.name().to_string(), name),
                (
                    schema.bounds().to_string(),
                    DataValue::List(cozo_lifetime_bounds),
                ),
            ]);
            let script = schema.script_put(&params);
            log::trace!(target: LOG_TARGET_TRANSFORM,
                "{}: {} | {}",
                "Form Script".log_step(),
                schema.relation.log_name(),
                script.log_magenta()
            );
            (params, script)
        }
        GenericParamKind::Const { name: _, type_id } => {
            let schema = GenericConstNodeSchema::SCHEMA;
            let params = BTreeMap::from([
                (schema.id().to_string(), cozo_id),
                (schema.owner_id().to_string(), any_id.to_cozo_uuid()),
                (
                    schema.param_index().to_string(),
                    DataValue::Num(cozo::Num::Int(i)),
                ),
                (schema.kind().to_string(), DataValue::Str("Type".into())),
                (schema.name().to_string(), name),
                (schema.type_id().to_string(), type_id.to_cozo_uuid()),
            ]);
            let script = schema.script_put(&params);
            log::trace!(target: LOG_TARGET_TRANSFORM,
                "{}: {} | {}",
                "Form Script".log_step(),
                schema.relation.log_name(),
                script.log_magenta()
            );
            (params, script)
        }
    }
}

pub(super) fn process_fields(
    strukt: &StructNode,
    schema: &FieldNodeSchema,
    i: usize,
    field: &FieldNode,
) -> BTreeMap<String, DataValue> {
    let (vis_kind, vis_path) = vis_to_dataval(field);

    let cozo_name = DataValue::from(
        field
            .name
            .as_ref()
            .expect("Invariant: Fields must have names.")
            .as_str(),
    );
    let cfgs: Vec<DataValue> = field
        .cfgs
        .iter()
        .map(|s| DataValue::from(s.as_str()))
        .collect();

    let cozo_id = field.id.as_any().to_cozo_uuid();
    let type_id = field.type_id;

    let field_params = BTreeMap::from([
        (schema.id().to_string(), cozo_id),
        (schema.name().to_string(), cozo_name),
        (schema.owner_id().to_string(), strukt.id.into()),
        (schema.index().to_string(), DataValue::from(i as i64)),
        (schema.type_id().to_string(), type_id.into()),
        (schema.vis_kind().to_string(), vis_kind),
        (
            schema.vis_path().to_string(),
            vis_path.unwrap_or(DataValue::Null),
        ),
        (schema.cfgs().to_string(), DataValue::List(cfgs)),
    ]);
    field_params
}

fn vis_to_dataval(field: &FieldNode) -> (DataValue, Option<DataValue>) {
    let (vis_kind, vis_path) = match &field.visibility {
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

    use cozo::{Db, MemStorage, ScriptMutability};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::{
        parser::{nodes::AsAnyNodeId, ParsedCodeGraph},
        utils::LogStyle,
    };

    use crate::schema::{primary_nodes::FunctionNodeSchema, secondary_nodes::AttributeNodeSchema};

    use super::process_attributes;

    #[test]
    fn test_attribute_insertion() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        // Choose target with generic functions old_function
        let successful_graphs = run_phases_and_collect("fixture_attributes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let func_schema = &FunctionNodeSchema::SCHEMA;
        func_schema.log_create_script();

        let schema = func_schema.script_create();
        let db_result = db.run_script(&schema, BTreeMap::new(), cozo::ScriptMutability::Mutable)?;
        log_db_result(db_result);

        // target:
        //  #[deprecated(since = "0.1.0", note = "Use new_function instead")] // deprecated attribute
        //  pub fn old_function() {}
        let func_node = merged
            .graph
            .functions
            .iter()
            .inspect(|f| println!("function: {}", f.name.log_name()))
            .find(|f| f.name == "old_function")
            .cloned()
            .expect("Cannot find target function node");

        let attribute_type_schema = AttributeNodeSchema::SCHEMA;
        let db_result = db.run_script(
            &attribute_type_schema.script_create(),
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
        log_db_result(db_result);

        attribute_type_schema.log_create_script();

        for (i, attr) in func_node.attributes.into_iter().enumerate() {
            let attr_params = process_attributes(func_node.id.as_any(), i, &attr);
            let script = attribute_type_schema.script_put(&attr_params);
            let db_result = db.run_script(&script, attr_params, cozo::ScriptMutability::Mutable)?;
            log_db_result(db_result);
        }
        Ok(())
    }

    fn log_db_result(db_result: cozo::NamedRows) {
        log::info!(target: "transform_function",
            "{} {:#?}",
            "  Db return: ".log_step(),
            db_result,
        );
    }
}
