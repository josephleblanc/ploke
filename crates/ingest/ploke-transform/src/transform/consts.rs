use crate::{
    macro_traits::CommonFields, schema::primary_nodes::ConstNodeSchema, utils::log_db_error,
};

use super::*;

/// Transforms value nodes into the values relation
pub(super) fn transform_consts(
    db: &Db<MemStorage>,
    consts: Vec<ConstNode>,
) -> Result<(), TransformError> {
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
        db.run_script_log(&script, consta_params, ScriptMutability::Mutable)?;

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in consta.attributes.iter().enumerate() {
            let attr_params = process_attributes(consta.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script_log(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}

pub trait LogScript {
    fn run_script_log(
        &self,
        script: &str,
        params: BTreeMap<String, DataValue>,
        mutability: ScriptMutability,
    ) -> Result<(), TransformError>;
}

impl LogScript for &Db<MemStorage> {
    fn run_script_log(
        &self,
        script: &str,
        rel_params: BTreeMap<String, DataValue>,
        mutability: ScriptMutability,
    ) -> Result<(), TransformError> {
        self.run_script(script, rel_params.clone(), mutability)
            .inspect_err(|_| {
                tracing::error!(target: "db", "{} {}\n{} {:#?}",
                    "put script".log_step(),
                    &script,
                    "rel_params:".log_step(),
                    rel_params
                );
            })
            .map_err(log_db_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{
        error::TransformError,
        schema::{primary_nodes::ConstNodeSchema, secondary_nodes::AttributeNodeSchema},
    };

    use ploke_test_utils::init_test_tracing;
    use tracing::Level;

    use super::transform_consts;
    #[test]
    fn test_transform_consts() -> Result<(), Box<TransformError>> {
        // init_test_tracing(Level::TRACE);
        // let _ = env_logger::builder()
        //     .is_test(true)
        //     .format_timestamp(None) // Disable timestamps
        //     .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let const_schema = ConstNodeSchema::SCHEMA;
        tracing::info!("identity script:\n{}", const_schema.script_identity());
        tracing::info!("create script:\n{}", const_schema.script_create());
        const_schema.create_and_insert(&db)
            .inspect_err(|e| {
                tracing::error!("{e}");
            })?;

        let attribute_schema = &AttributeNodeSchema::SCHEMA;
        attribute_schema.create_and_insert(&db)?;

        // transform and insert impls into cozo
        transform_consts(&db, merged.graph.consts)?;

        Ok(())
    }
}
