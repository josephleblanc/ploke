use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use ploke_core::WorkspaceId;
use syn_parser::{
    ParsedCodeGraph, ParsedWorkspace,
    discovery::{CrateContext, DependencyMap, workspace::WorkspaceMetadataSection},
    resolve::{
        call_resolution::{
            CallWorkspace, WorkspaceCrate, resolve_call_relations_after_tree_with_workspace,
        },
        module_tree::ModuleTree,
    },
};

use crate::error::TransformError;
use crate::schema::crate_node::WorkspaceMetadataSchema;
use tracing::instrument;

use super::transform_parsed_graph_with_call_report;

struct ParsedCrateSlice {
    context: CrateContext,
    graph: ParsedCodeGraph,
    tree: ModuleTree,
}

/// Transforms workspace metadata into a database row and then transforms each parsed crate graph.
#[instrument(skip_all, fields(crate_count = parsed_workspace.crates.len()))]
pub fn transform_parsed_workspace(
    db: &Db<MemStorage>,
    parsed_workspace: ParsedWorkspace,
) -> Result<(), TransformError> {
    transform_workspace_metadata(db, &parsed_workspace.workspace)?;

    let mut crates = Vec::new();
    for parsed_crate in parsed_workspace.crates {
        let context = parsed_crate.crate_context;
        let mut parser_output = parsed_crate.parser_output;
        let graph = parser_output.extract_merged_graph().ok_or_else(|| {
            TransformError::Transformation(
                "ParsedWorkspace crate was missing its merged graph".to_string(),
            )
        })?;
        let tree = parser_output.extract_module_tree().ok_or_else(|| {
            TransformError::Transformation(
                "ParsedWorkspace crate was missing its module tree".to_string(),
            )
        })?;

        crates.push(ParsedCrateSlice {
            context,
            graph,
            tree,
        });
    }

    let reports = crates
        .iter()
        .enumerate()
        .map(|(idx, krate)| {
            let deps = dependency_crates(&crates, idx);
            let workspace = CallWorkspace {
                crates: deps.as_slice(),
            };
            resolve_call_relations_after_tree_with_workspace(&krate.graph, &krate.tree, workspace)
                .map_err(|err| {
                    TransformError::Transformation(format!(
                        "typed workspace call relation resolution failed: {err}"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    for (krate, report) in crates.into_iter().zip(reports) {
        transform_parsed_graph_with_call_report(db, krate.graph, &krate.tree, report)?;
    }

    Ok(())
}

fn dependency_crates<'a>(
    crates: &'a [ParsedCrateSlice],
    source_idx: usize,
) -> Vec<WorkspaceCrate<'a>> {
    let source = &crates[source_idx];
    let mut deps = Vec::new();
    collect_deps(
        &mut deps,
        source.context.dependencies(),
        source,
        crates,
        source_idx,
    );
    collect_deps(
        &mut deps,
        source.context.dev_dependencies(),
        source,
        crates,
        source_idx,
    );
    deps
}

fn collect_deps<'a>(
    deps: &mut Vec<WorkspaceCrate<'a>>,
    manifest: &'a impl DependencyMap,
    source: &'a ParsedCrateSlice,
    crates: &'a [ParsedCrateSlice],
    source_idx: usize,
) {
    for (name, dep_path) in manifest.path_dependencies() {
        for (idx, target) in crates.iter().enumerate() {
            if idx == source_idx {
                continue;
            }
            if !paths_match(
                &source.context.root_path,
                dep_path,
                &target.context.root_path,
            ) {
                continue;
            }
            if deps.iter().any(|dep| {
                dep.dependency_name == name
                    && dep.graph.crate_namespace == target.graph.crate_namespace
            }) {
                continue;
            }
            deps.push(WorkspaceCrate {
                dependency_name: name,
                graph: &target.graph,
                tree: &target.tree,
            });
        }
    }
}

fn paths_match(source_root: &Path, dep_path: &str, target_root: &Path) -> bool {
    let dep_path = Path::new(dep_path);
    let resolved = if dep_path.is_absolute() {
        normalize(dep_path)
    } else {
        normalize(source_root.join(dep_path))
    };
    resolved == normalize(target_root)
}

fn normalize(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
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
    use std::fs;
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

    #[test]
    fn transform_parsed_workspace_classifies_workspace_reexported_external_receiver_alias()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let root = workspace.path();
        let provider = root.join("provider");
        let consumer = root.join("consumer");
        fs::create_dir_all(provider.join("src"))?;
        fs::create_dir_all(consumer.join("src"))?;

        fs::write(
            root.join("Cargo.toml"),
            r#"[workspace]
members = ["provider", "consumer"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
provider = { path = "provider" }
"#,
        )?;
        fs::write(
            provider.join("Cargo.toml"),
            r#"[package]
name = "provider"
version.workspace = true
edition.workspace = true

[lib]
path = "src/lib.rs"

[dependencies]
http = "1"
"#,
        )?;
        fs::write(
            provider.join("src/lib.rs"),
            "pub type Request<T = ()> = http::Request<T>;\n",
        )?;
        fs::write(
            consumer.join("Cargo.toml"),
            r#"[package]
name = "consumer"
version.workspace = true
edition.workspace = true

[lib]
path = "src/lib.rs"

[dependencies]
provider = { workspace = true }
"#,
        )?;
        fs::write(
            consumer.join("src/lib.rs"),
            r#"use provider::Request;

pub fn call_workspace_reexported_external_receiver<B>(mut req: Request<B>) {
    req.extensions_mut();
}
"#,
        )?;

        let parsed_workspace = parse_workspace(root, None)?;
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_schema_all(&db)?;

        transform_parsed_workspace(&db, parsed_workspace)?;

        let rows = db.run_script(
            r#"?[owner_name, method_name, receiver_kind, receiver_path, status_kind, resolution_kind] :=
                *function { id: owner_id, name: owner_name @ 'NOW' },
                owner_name = "call_workspace_reexported_external_receiver",
                *call_site {
                    id: site_id,
                    owner_id,
                    call_kind: "Method",
                    method_name,
                    receiver_kind,
                    receiver_path @ 'NOW'
                },
                method_name = "extensions_mut",
                *call_resolution_status {
                    source_id: site_id,
                    status_kind,
                    resolution_kind @ 'NOW'
                }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Immutable,
        )?;

        assert_eq!(
            rows.rows.len(),
            1,
            "workspace re-exported external receiver alias should project one method row: {rows:#?}"
        );
        let row = &rows.rows[0];
        assert_eq!(&row[1], &DataValue::from("extensions_mut"));
        assert_eq!(&row[2], &DataValue::from("LocalBinding"));
        assert_eq!(&row[3], &DataValue::List(vec![DataValue::from("req")]));
        assert_eq!(&row[4], &DataValue::from("External"));
        assert_eq!(&row[5], &DataValue::Null);

        Ok(())
    }
}
