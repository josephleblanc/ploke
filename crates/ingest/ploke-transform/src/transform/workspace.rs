use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use ploke_core::WorkspaceId;
use syn_parser::{ParsedWorkspace, discovery::workspace::WorkspaceMetadataSection};

use crate::error::TransformError;
use crate::schema::crate_node::WorkspaceMetadataSchema;
use tracing::instrument;

use super::transform_parsed_graph;

/// Transforms workspace metadata into a database row and then transforms each parsed crate graph.
#[instrument(skip_all, fields(crate_count = parsed_workspace.crates.len()))]
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
    use cozo::{DataValue, Db, MemStorage, UuidWrapper};
    use ploke_common::workspace_root;
    use ploke_core::WorkspaceId;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use syn_parser::{discovery::workspace::WorkspaceMetadataSection, parse_workspace};

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
    fn transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
        let parsed_workspace = parse_workspace(&fixture_workspace_root, None)?;

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_schema_all(&db)?;

        transform_parsed_workspace(&db, parsed_workspace)?;

        let workspace_rows = db.run_script(
            "?[id, namespace, root_path, resolver, members, exclude, package_version] := \
             *workspace_metadata { id, namespace, root_path, resolver, members, exclude, package_version }",
            BTreeMap::new(),
            cozo::ScriptMutability::Immutable,
        )?;

        assert_eq!(workspace_rows.rows.len(), 1);
        let row = &workspace_rows.rows[0];
        let expected_workspace_id = WorkspaceId::from_root_path(&fixture_workspace_root).uuid();
        let expected_members = vec![
            DataValue::from(
                fixture_workspace_root
                    .join("member_root")
                    .display()
                    .to_string(),
            ),
            DataValue::from(
                fixture_workspace_root
                    .join("nested/member_nested")
                    .display()
                    .to_string(),
            ),
        ];

        assert_eq!(row[0], DataValue::Uuid(UuidWrapper(expected_workspace_id)));
        assert_eq!(row[1], DataValue::Uuid(UuidWrapper(expected_workspace_id)));
        assert_eq!(
            row[2],
            DataValue::from(fixture_workspace_root.display().to_string())
        );
        assert_eq!(row[3], DataValue::from("2"));
        assert_eq!(row[4], DataValue::List(expected_members));
        assert_eq!(row[5], DataValue::Null);
        assert_eq!(row[6], DataValue::from("0.2.0"));

        Ok(())
    }
}
