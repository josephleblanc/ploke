use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use ploke_core::WorkspaceId;
use syn_parser::{discovery::workspace::WorkspaceMetadataSection, ParsedWorkspace};

use crate::error::TransformError;
use crate::schema::crate_node::WorkspaceMetadataSchema;

use super::transform_parsed_graph;

/// Transforms workspace metadata into a database row and then transforms each parsed crate graph.
pub fn transform_parsed_workspace(
    db: &Db<MemStorage>,
    parsed_workspace: ParsedWorkspace,
) -> Result<(), TransformError> {
    transform_workspace_metadata(db, &parsed_workspace.workspace)?;

    for parsed_crate in parsed_workspace.crates {
        let mut parser_output = parsed_crate.parser_output;
        let merged_graph = parser_output.extract_merged_graph().ok_or_else(|| {
            TransformError::Transformation(
                "ParsedWorkspace crate was missing its merged graph".to_string(),
            )
        })?;
        let module_tree = parser_output.extract_module_tree().ok_or_else(|| {
            TransformError::Transformation(
                "ParsedWorkspace crate was missing its module tree".to_string(),
            )
        })?;

        transform_parsed_graph(db, merged_graph, &module_tree)?;
    }

    Ok(())
}

pub(super) fn transform_workspace_metadata(
    db: &Db<MemStorage>,
    workspace: &WorkspaceMetadataSection,
) -> Result<(), TransformError> {
    let schema = &WorkspaceMetadataSchema::SCHEMA;
    let workspace_params = process_workspace_metadata(workspace, schema)?;

    let script = schema.script_put(&workspace_params);
    db.run_script(&script, workspace_params, ScriptMutability::Mutable)
        .inspect_err(|e| {
            tracing::error!(target: "transform_workspace",
                "WorkspaceMetadataSection: {}\n\tcreate schema: {}\n\tput script: {}",
                e,
                schema.script_create(),
                script
            );
        })?;

    Ok(())
}

fn process_workspace_metadata(
    workspace: &WorkspaceMetadataSection,
    schema: &WorkspaceMetadataSchema,
) -> Result<BTreeMap<String, DataValue>, TransformError> {
    let workspace_id = WorkspaceId::from_root_path(&workspace.path);
    let root_path = cozo_file(&workspace.path)?;
    let members = DataValue::List(
        workspace
            .members
            .iter()
            .map(cozo_file_value)
            .collect::<Result<Vec<_>, _>>()?,
    );
    let exclude = workspace
        .exclude
        .as_ref()
        .map(|paths| {
            paths
                .iter()
                .map(cozo_file_value)
                .collect::<Result<Vec<_>, _>>()
                .map(DataValue::List)
        })
        .transpose()?
        .unwrap_or(DataValue::Null);
    let resolver = workspace
        .resolver
        .as_deref()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null);
    let package_version = workspace
        .package_version()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null);

    Ok(BTreeMap::from([
        (
            schema.id().to_string(),
            DataValue::Uuid(cozo::UuidWrapper(workspace_id.uuid())),
        ),
        (
            schema.namespace().to_string(),
            DataValue::Uuid(cozo::UuidWrapper(workspace_id.uuid())),
        ),
        (
            schema.root_path().to_string(),
            DataValue::from(root_path.to_string()),
        ),
        (schema.resolver().to_string(), resolver),
        (schema.members().to_string(), members),
        (schema.exclude().to_string(), exclude),
        (schema.package_version().to_string(), package_version),
    ]))
}

fn cozo_file(path: &Path) -> Result<&str, TransformError> {
    path.as_os_str()
        .to_str()
        .ok_or_else(|| TransformError::Transformation("Could not parse workspace path".to_string()))
}

fn cozo_file_value(path: &PathBuf) -> Result<DataValue, TransformError> {
    cozo_file(path).map(DataValue::from)
}

#[cfg(test)]
mod tests {
    use cozo::{Db, MemStorage};
    use std::fs;
    use std::path::PathBuf;
    use syn_parser::{discovery::workspace::WorkspaceMetadataSection, parse_workspace};
    use uuid::Uuid;

    use crate::{
        schema::{crate_node::WorkspaceMetadataSchema, create_schema_all},
        transform::transform_parsed_workspace,
    };

    use super::transform_workspace_metadata;

    #[test]
    fn test_transform_workspace_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        WorkspaceMetadataSchema::create_and_insert_schema(&db)?;

        let workspace = WorkspaceMetadataSection {
            path: PathBuf::from("/tmp/ploke-workspace"),
            exclude: Some(vec![PathBuf::from("/tmp/ploke-workspace/ignored")]),
            resolver: Some("2".to_string()),
            members: vec![PathBuf::from("/tmp/ploke-workspace/crate_a")],
            package: None,
        };

        transform_workspace_metadata(&db, &workspace)?;
        Ok(())
    }

    #[test]
    fn test_transform_parsed_workspace() -> Result<(), Box<dyn std::error::Error>> {
        let workspace_root =
            std::env::temp_dir().join(format!("ploke-workspace-{}", Uuid::new_v4()));
        let test_result = (|| -> Result<(), Box<dyn std::error::Error>> {
            fs::create_dir_all(workspace_root.join("crate_a/src"))?;
            fs::create_dir_all(workspace_root.join("crate_b/src"))?;
            fs::write(
                workspace_root.join("Cargo.toml"),
                r#"[workspace]
members = ["crate_a", "crate_b"]
resolver = "2"

[workspace.package]
version = "0.1.0"
"#,
            )?;
            fs::write(
                workspace_root.join("crate_a/Cargo.toml"),
                r#"[package]
name = "crate_a"
version.workspace = true
edition = "2021"
"#,
            )?;
            fs::write(
                workspace_root.join("crate_a/src/lib.rs"),
                "pub fn a() -> usize { 1 }\n",
            )?;
            fs::write(
                workspace_root.join("crate_b/Cargo.toml"),
                r#"[package]
name = "crate_b"
version.workspace = true
edition = "2021"
"#,
            )?;
            fs::write(
                workspace_root.join("crate_b/src/lib.rs"),
                "pub fn b() -> usize { crate_a::a() }\n",
            )?;

            let parsed_workspace = parse_workspace(&workspace_root, None)?;

            let db = Db::new(MemStorage::default()).expect("Failed to create database");
            db.initialize().expect("Failed to initialize database");
            create_schema_all(&db)?;

            transform_parsed_workspace(&db, parsed_workspace)?;
            Ok(())
        })();

        let cleanup_result = fs::remove_dir_all(&workspace_root);
        test_result?;
        cleanup_result?;
        Ok(())
    }
}
