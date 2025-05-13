use crate::{macro_traits::CommonFields, schema::primary_nodes::StaticNodeSchema};

use super::*;

pub(super) fn transform_statics(
    db: &Db<MemStorage>,
    statics: Vec<StaticNode>,
) -> Result<(), cozo::Error> {
    for stat in statics.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &StaticNodeSchema::SCHEMA;
        let mut stat_params = stat.cozo_btree();

        let value_cozo = stat
            .value
            .map_or(DataValue::Null, |v| DataValue::Str(v.into()));
        let cozo_ty_id = stat.type_id.to_cozo_uuid();
        let cozo_is_mut = DataValue::Bool(stat.is_mutable);

        stat_params.insert(schema.value().to_string(), value_cozo);
        stat_params.insert(schema.ty_id().to_string(), cozo_ty_id);
        stat_params.insert(schema.is_mutable().to_string(), cozo_is_mut);

        let script = schema.script_put(&stat_params);
        db.run_script(&script, stat_params, ScriptMutability::Mutable)?;

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in stat.attributes.iter().enumerate() {
            let attr_params = process_attributes(stat.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}
#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{
        schema::primary_nodes::StaticNodeSchema,
        test_utils::{create_attribute_schema, log_db_result},
        transform::statics::transform_statics,
    };

    #[test]
    fn test_transform_statics() -> Result<(), Box<dyn std::error::Error>> {
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
        create_attribute_schema(&db)?;
        pub(crate) fn create_static_schema(
            db: &Db<MemStorage>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let static_schema = StaticNodeSchema::SCHEMA;
            let db_result = db.run_script(
                &static_schema.script_create(),
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )?;
            log_db_result(db_result);
            Ok(())
        }
        create_static_schema(&db)?;

        // transform and insert impls into cozo
        transform_statics(&db, merged.graph.statics)?;

        Ok(())
    }
}
