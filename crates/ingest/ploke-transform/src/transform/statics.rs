#[cfg(not(feature = "multi_embedding_schema"))]
use crate::schema::primary_nodes::StaticNodeSchema;
#[cfg(feature = "multi_embedding_schema")]
use crate::schema::primary_nodes_multi::StaticNodeSchema;

use crate::macro_traits::CommonFields;

use super::*;

pub(super) fn transform_statics(
    db: &Db<MemStorage>,
    statics: Vec<StaticNode>,
) -> Result<(), TransformError> {
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

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{
        schema::{primary_nodes::StaticNodeSchema, secondary_nodes::AttributeNodeSchema},
        transform::statics::transform_statics,
    };

    #[test]
    fn test_transform_statics() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        StaticNodeSchema::create_and_insert_schema(&db)?;

        // transform and insert impls into cozo
        transform_statics(&db, merged.graph.statics)?;

        Ok(())
    }
}
