use std::path::PathBuf;
use tracing::instrument;
use uuid::Uuid;

use syn_parser::discovery::{CrateContext, DependencyMap, DependencySpec};

use crate::schema::crate_node::{CrateContextSchema, CrateDependencySchema};

use super::*;

/// Transforms a CrateContext into a node in the database using CrateContextSchema
#[instrument(skip_all)]
pub(super) fn transform_crate_context(
    db: &Db<MemStorage>,
    crate_context: CrateContext,
) -> Result<(), TransformError> {
    let schema = &CrateContextSchema::SCHEMA;
    let crate_params = process_crate_context(&crate_context, schema)?;

    let script = schema.script_put(&crate_params);
    db.run_script(&script, crate_params, ScriptMutability::Mutable)
        .inspect_err(|e| {
            tracing::error!(target: "transform_crate",
                "{} {}\n\t{} {}\n\t{} {}",
                "CrateContext:".log_header(),
                e.to_string(),
                "create schema:".log_step(),
                schema.script_create(),
                "put script:".log_step(),
                script
            );
        })?;
    for dep_params in process_crate_dependencies(&crate_context)? {
        let schema = &CrateDependencySchema::SCHEMA;
        let script = schema.script_put(&dep_params);
        db.run_script(&script, dep_params, ScriptMutability::Mutable)
            .inspect_err(|e| {
                tracing::error!(target: "transform_crate",
                    "{} {}\n\t{} {}\n\t{} {}",
                    "CrateDependency:".log_header(),
                    e.to_string(),
                    "create schema:".log_step(),
                    schema.script_create(),
                    "put script:".log_step(),
                    script
                );
            })?;
    }
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

fn process_crate_dependencies(
    ctx: &CrateContext,
) -> Result<Vec<BTreeMap<String, DataValue>>, TransformError> {
    let mut rows = Vec::new();
    let normal = ctx
        .dependencies()
        .iter()
        .map(|(name, spec)| dependency_params(ctx, "normal", name, spec));
    let dev = ctx
        .dev_dependencies()
        .iter()
        .map(|(name, spec)| dependency_params(ctx, "dev", name, spec));
    for row in normal.chain(dev) {
        rows.push(row?);
    }
    Ok(rows)
}

fn dependency_params(
    ctx: &CrateContext,
    dep_kind: &str,
    dep_name: &str,
    spec: &DependencySpec,
) -> Result<BTreeMap<String, DataValue>, TransformError> {
    let schema = &CrateDependencySchema::SCHEMA;
    let id = Uuid::new_v5(
        &ctx.namespace,
        format!("crate_dependency:{dep_kind}:{dep_name}").as_bytes(),
    );

    Ok(BTreeMap::from([
        (
            schema.id().to_string(),
            DataValue::Uuid(cozo::UuidWrapper(id)),
        ),
        (
            schema.namespace().to_string(),
            DataValue::Uuid(cozo::UuidWrapper(ctx.namespace)),
        ),
        (
            schema.crate_name().to_string(),
            DataValue::from(ctx.name.as_str()),
        ),
        (
            schema.dep_name().to_string(),
            DataValue::from(dep_name.to_string()),
        ),
        (
            schema.dep_kind().to_string(),
            DataValue::from(dep_kind.to_string()),
        ),
        (
            schema.version().to_string(),
            dependency_version(spec)
                .map(DataValue::from)
                .unwrap_or(DataValue::Null),
        ),
        (schema.path().to_string(), optional_str(spec.path())),
        (schema.git().to_string(), optional_str(spec.git())),
        (schema.branch().to_string(), optional_str(spec.branch())),
        (schema.tag().to_string(), optional_str(spec.tag())),
        (schema.rev().to_string(), optional_str(spec.rev())),
        (
            schema.features().to_string(),
            spec.features()
                .map(|features| {
                    DataValue::List(
                        features
                            .iter()
                            .map(|f| DataValue::from(f.as_str()))
                            .collect(),
                    )
                })
                .unwrap_or(DataValue::Null),
        ),
        (
            schema.optional().to_string(),
            spec.is_optional()
                .map(DataValue::from)
                .unwrap_or(DataValue::Null),
        ),
        (
            schema.default_features().to_string(),
            spec.has_default_features()
                .map(DataValue::from)
                .unwrap_or(DataValue::Null),
        ),
    ]))
}

fn dependency_version(spec: &DependencySpec) -> Option<&str> {
    spec.as_version()
        .map(String::as_str)
        .or_else(|| spec.version())
}

fn optional_str(value: Option<&str>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

#[allow(clippy::ptr_arg)]
fn cozo_file(file: &PathBuf) -> Result<&str, TransformError> {
    let f = file
        .as_os_str()
        .to_str()
        .ok_or_else(|| TransformError::Transformation("Could not parse root file".to_string()))?;
    Ok(f)
}

fn cozo_string(s: &str) -> DataValue {
    DataValue::from(s)
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use cozo::{DataValue, Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::schema::crate_node::{CrateContextSchema, CrateDependencySchema};

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
        CrateDependencySchema::create_and_insert_schema(&db)?;

        let crate_ctx = merged
            .crate_context
            .expect("Crate context should be preserved in main graph");

        transform_crate_context(&db, crate_ctx)?;
        let rows = db.run_script(
            r#"
?[crate_name, dep_name, dep_kind, version, features, path, optional, default_features] :=
    *crate_dependency {
        crate_name,
        dep_name,
        dep_kind,
        version,
        features,
        path,
        optional,
        default_features @ 'NOW'
    }
"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Immutable,
        )?;
        assert_eq!(rows.rows.len(), 1);
        let row = &rows.rows[0];
        assert_eq!(row[0], DataValue::from("fixture_nodes"));
        assert_eq!(row[1], DataValue::from("serde"));
        assert_eq!(row[2], DataValue::from("normal"));
        assert_eq!(row[3], DataValue::from("1.0"));
        assert_eq!(row[4], DataValue::List(vec![DataValue::from("derive")]));
        assert_eq!(row[5], DataValue::Null);
        assert_eq!(row[6], DataValue::from(false));
        assert_eq!(row[7], DataValue::from(true));
        Ok(())
    }
}
