use crate::{
    macro_traits::CommonFields,
    schema::{assoc_nodes::MethodNodeSchema, primary_nodes::TraitNodeSchema},
    transform::impls::process_methods,
};

use super::*;

pub(super) fn transform_traits(
    db: &Db<MemStorage>,
    traits: Vec<TraitNode>,
) -> Result<(), TransformError> {
    // trait->trayt (rust keywords)
    for trayt in traits.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &TraitNodeSchema::SCHEMA;
        let trayt_any_id = trayt.id.as_any();
        let mut trayt_params = trayt.cozo_btree();

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in trayt.attributes.iter().enumerate() {
            let attr_params = process_attributes(trayt.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }

        let method_schema = &MethodNodeSchema::SCHEMA;
        let mut method_ids: Vec<DataValue> = Vec::new();
        for method in trayt.methods.into_iter() {
            method_ids.push(method.cozo_id());
            let method_params = process_methods(trayt_any_id, method);
            let script = method_schema.script_put(&method_params);

            log::trace!(
                "  {} {} {:?}",
                "method put:".log_step(),
                script,
                method_params
            );
            db.run_script(&script, method_params, ScriptMutability::Mutable)?;
        }
        let cozo_methods = if method_ids.is_empty() {
            DataValue::Null
        } else {
            DataValue::List(method_ids)
        };
        trayt_params.insert(schema.methods().to_string(), cozo_methods);
        let script = schema.script_put(&trayt_params);
        db.run_script(&script, trayt_params, ScriptMutability::Mutable)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::schema::{
        assoc_nodes::MethodNodeSchema, primary_nodes::TraitNodeSchema,
        secondary_nodes::AttributeNodeSchema,
    };

    use super::transform_traits;
    #[test]
    fn test_transform_traits() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        // create and insert method schema
        MethodNodeSchema::create_and_insert_schema(&db)?;
        // create and insert trait schema
        TraitNodeSchema::create_and_insert_schema(&db)?;

        // transform and insert impls into cozo
        transform_traits(&db, merged.graph.traits)?;

        Ok(())
    }
}
