use crate::{macro_traits::CommonFields, schema::primary_nodes::ConstNodeSchema};

use super::*;

/// Transforms value nodes into the values relation
pub(super) fn transform_consts(
    db: &Db<MemStorage>,
    consts: Vec<ConstNode>,
) -> Result<(), cozo::Error> {
    for consta in consts.into_iter() {
        let schema = &ConstNodeSchema::SCHEMA;
        let mut consta_params = consta.cozo_btree();

        let value_cozo = consta
            .value
            .map_or(DataValue::Null, |v| DataValue::Str(v.into()));
        let cozo_ty_id = consta.type_id.to_cozo_uuid();

        consta_params.insert(schema.value().to_string(), value_cozo);
        consta_params.insert(schema.ty_id().to_string(), cozo_ty_id);

        let script = schema.script_put(&consta_params);
        db.run_script(&script, consta_params, ScriptMutability::Mutable)?;

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in consta.attributes.iter().enumerate() {
            let attr_params = process_attributes(consta.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::schema::primary_nodes::ConstNodeSchema;

    use super::transform_consts;
    #[test]
    fn test_transform_consts() -> Result<(), Box<cozo::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let const_schema = ConstNodeSchema::SCHEMA;

        const_schema.create_and_insert(&db)?;

        // transform and insert impls into cozo
        transform_consts(&db, merged.graph.consts)?;

        Ok(())
    }
}
