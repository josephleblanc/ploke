use std::path::PathBuf;

use syn_parser::discovery::CrateContext;

use crate::schema::crate_node::CrateContextSchema;

use super::*;

/// Transforms a CrateContext into a node in the database using CrateContextSchema
pub(super) fn transform_crate_context(
    db: &Db<MemStorage>,
    crate_context: CrateContext,
) -> Result<(), TransformError> {
    let schema = &CrateContextSchema::SCHEMA;
    let crate_params = process_crate_context(&crate_context, schema)?;

    let script = schema.script_put(&crate_params);
    db.run_script(&script, crate_params, ScriptMutability::Mutable)
        .inspect_err(|e| {
            log::error!(target: "transform_crate",
                "{} {}\n\t{} {}\n\t{} {}",
                "CrateContext:".log_header(),
                e.to_string(),
                "create schema:".log_step(),
                schema.script_create(),
                "put script:".log_step(),
                script
            );
        })?;
    Ok(())
}

fn process_crate_context(
    ctx: &CrateContext,
    schema: &CrateContextSchema,
) -> Result<BTreeMap<String, DataValue>, TransformError> {
    let root_file = cozo_file(&ctx.root_path)?;

    let files = ctx.files.iter().map(cozo_file);
    let mut cozo_files: Vec<DataValue> = Vec::new();
    for file in files {
        let f = file?;
        cozo_files.push(DataValue::from(f));
    }

    let ctx_params = BTreeMap::from([
        (
            schema.id().to_string(),
            DataValue::Uuid(cozo::UuidWrapper(ctx.namespace)),
        ),
        (
            schema.name().to_string(),
            DataValue::from(ctx.name.as_str()),
        ),
        (schema.version().to_string(), cozo_string(&ctx.version)),
        (
            schema.namespace().to_string(),
            DataValue::Uuid(cozo::UuidWrapper(ctx.namespace)),
        ),
        (schema.root_path().to_string(), DataValue::from(root_file)),
        (schema.files().to_string(), DataValue::List(cozo_files)),
    ]);

    Ok(ctx_params)
}

fn cozo_file(file: &PathBuf) -> Result<&str, TransformError> {
    let f = file
        .as_os_str()
        .to_str()
        .ok_or_else(|| TransformError::Transformation(format!("Could not parse root file")))?;
    Ok(f)
}

fn cozo_string(s: &str) -> DataValue {
    DataValue::from(s)
}

#[cfg(test)]
mod test {
    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::schema::crate_node::CrateContextSchema;

    use super::transform_crate_context;

    #[test]
    fn test_transform_crate_context() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert union schema
        CrateContextSchema::create_and_insert_schema(&db)?;

        let crate_ctx = merged
            .crate_context
            .expect("Crate context should be preserved in main graph");

        transform_crate_context(&db, crate_ctx)?;
        Ok(())
    }
}
