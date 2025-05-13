use crate::{macro_traits::CommonFields, schema::primary_nodes::TraitNodeSchema};

use super::*;

pub(super) fn transform_traits(
    db: &Db<MemStorage>,
    traits: Vec<TraitNode>,
) -> Result<(), cozo::Error> {
    // trait->trayt (rust keywords)
    for trayt in traits.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &TraitNodeSchema::SCHEMA;
        let trayt_params = trayt.cozo_btree();

        let script = schema.script_put(&trayt_params);
        db.run_script(&script, trayt_params, ScriptMutability::Mutable)?;

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in trayt.attributes.iter().enumerate() {
            let attr_params = process_attributes(trayt.id.as_any(), i, attr);

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
        schema::primary_nodes::TraitNodeSchema,
        test_utils::{create_attribute_schema, log_db_result},
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
        create_attribute_schema(&db)?;
        pub(crate) fn create_trait_schema(
            db: &Db<MemStorage>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let trait_schema = TraitNodeSchema::SCHEMA;
            let db_result = db.run_script(
                &trait_schema.script_create(),
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )?;
            log_db_result(db_result);
            Ok(())
        }
        create_trait_schema(&db)?;

        // transform and insert impls into cozo
        transform_traits(&db, merged.graph.traits)?;

        Ok(())
    }
}
