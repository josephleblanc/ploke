use crate::{
    macro_traits::CommonFields,
    schema::{assoc_nodes::MethodNodeSchema, primary_nodes::ImplNodeSchema},
};

use super::*;

/// Transforms impl nodes into the impls relation
pub(super) fn transform_impls(
    db: &Db<MemStorage>,
    impls: Vec<ImplNode>,
) -> Result<(), TransformError> {
    for imple in impls.into_iter() {
        let imple_any_id = imple.any_id();
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &ImplNodeSchema::SCHEMA;
        let mut imple_params: BTreeMap<String, DataValue> = process_impl(&imple);

        let method_schema = &MethodNodeSchema::SCHEMA;

        // Add enum variants
        // using vec for now, switch to iterator later.
        let mut method_ids: Vec<DataValue> = Vec::new();
        for method in imple.methods.into_iter() {
            method_ids.push(method.cozo_id());
            let method_params = process_methods(imple_any_id, method);
            let script = method_schema.script_put(&method_params);

            log::trace!(
                "  {} {} {:?}",
                "method put:".log_step(),
                script,
                method_params
            );
            db.run_script(&script, method_params, ScriptMutability::Mutable)?;
        }

        imple_params.insert(schema.methods().to_string(), DataValue::List(method_ids));
        let script = schema.script_put(&imple_params);
        log::trace!(
            "  {} {} {:?}",
            "impl put:".log_step(),
            script,
            &imple_params
        );
        db.run_script(&script, imple_params, ScriptMutability::Mutable)?;

        // Add generic parameters
        for (i, generic_param) in imple.generic_params.into_iter().enumerate() {
            let (params, script) = process_generic_params(imple_any_id, i as i64, generic_param);
            log::trace!(
                "  {} {} {:?}",
                "generic_param put:".log_step(),
                script,
                params
            );
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // TODO: Add attributes
        //
        // let attr_schema = AttributeNodeSchema::SCHEMA;
        // for (i, attr) in imple.attributes.iter().enumerate() {
        //     let attr_params = process_attributes(imple.id.as_any(), i, attr);
        //
        //     let script = attr_schema.script_put(&attr_params);
        //     log::trace!("  {} {} {:?}", "attr put:".log_step(), script, attr_params);
        //     db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        // }
    }
    Ok(())
}

fn process_impl(imple: &ImplNode) -> BTreeMap<String, DataValue> {
    let cozo_id = imple.id.as_any().to_cozo_uuid();
    let cozo_self_ty = imple.self_type.to_cozo_uuid();

    let span_start = DataValue::Num(Num::Int(imple.span.0 as i64));
    let span_end = DataValue::Num(Num::Int(imple.span.1 as i64));
    let cozo_span = DataValue::List(Vec::from([span_start, span_end]));

    let cozo_trait_id_ty = imple
        .trait_type
        .map_or(DataValue::Null, |t| t.to_cozo_uuid());
    let cfgs_vec: Vec<DataValue> = imple
        .cfgs
        .iter()
        .map(|s| DataValue::from(s.as_str()))
        .collect();
    let cozo_cfgs = DataValue::List(cfgs_vec);

    let schema = ImplNodeSchema::SCHEMA;
    BTreeMap::from([
        (schema.id().to_string(), cozo_id),
        (schema.self_type().to_string(), cozo_self_ty),
        (schema.span().to_string(), cozo_span),
        (schema.trait_type().to_string(), cozo_trait_id_ty),
        (schema.cfgs().to_string(), cozo_cfgs),
    ])
}

pub(super) fn process_methods(
    imple_any_id: AnyNodeId,
    method: MethodNode,
) -> BTreeMap<String, DataValue> {
    let schema = &MethodNodeSchema::SCHEMA;
    let mut params = method.cozo_btree();
    let cozo_body = method
        .body
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);
    params.insert(schema.body().to_string(), cozo_body);
    params.insert(schema.owner_id().to_string(), imple_any_id.to_cozo_uuid());
    params
}

#[cfg(test)]
mod tests {
    use crate::{
        schema::{
            assoc_nodes::MethodNodeSchema, create_and_insert_generic_schema,
            primary_nodes::ImplNodeSchema, secondary_nodes::AttributeNodeSchema,
        },
        transform::impls::transform_impls,
    };
    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    #[test]
    fn test_transform_impls() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
        // let tree = merged.build_module_tree().unwrap_or_else(|e| {
        //     log::error!(target: "transform_function",
        //         "Error building tree: {}",
        //         e
        //     );
        //     panic!()
        // });

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        // create and insert generic schema
        create_and_insert_generic_schema(&db)?;
        // create and insert method schema
        MethodNodeSchema::create_and_insert_schema(&db)?;
        // create and insert impl schema
        ImplNodeSchema::create_and_insert_schema(&db)?;

        // transform and insert impls into cozo
        transform_impls(&db, merged.graph.impls)?;

        Ok(())
    }
}
