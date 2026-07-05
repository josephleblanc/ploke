use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use cozo::{DataValue, UuidWrapper};
use uuid::Uuid;

use crate::{
    Database, DbError,
    database::{CrateContextRow, to_string, to_string_list, to_uuid},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateDependencyRow {
    pub id: Uuid,
    pub namespace: Uuid,
    pub crate_name: String,
    pub dep_name: String,
    pub dep_kind: String,
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub features: Option<Vec<String>>,
    pub optional: Option<bool>,
    pub default_features: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDependencyCandidate {
    pub dependency: CrateDependencyRow,
    pub target: CrateContextRow,
}

impl Database {
    pub fn crate_dependencies_for_namespace(
        &self,
        namespace: Uuid,
    ) -> Result<Vec<CrateDependencyRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "namespace".to_string(),
            DataValue::Uuid(UuidWrapper(namespace)),
        );

        let rows = self.raw_query_params(
            r#"
?[id, namespace, crate_name, dep_name, dep_kind, version, path, git, branch, tag, rev, features, optional, default_features] :=
    namespace = $namespace,
    *crate_dependency {
        id,
        namespace,
        crate_name,
        dep_name,
        dep_kind,
        version,
        path,
        git,
        branch,
        tag,
        rev,
        features,
        optional,
        default_features @ 'NOW'
    }
"#,
            params,
        )?;

        rows.rows.iter().map(|row| dependency_row(row)).collect()
    }

    pub fn workspace_dependency_candidates(
        &self,
        namespace: Uuid,
    ) -> Result<Vec<WorkspaceDependencyCandidate>, DbError> {
        let dependencies = self.crate_dependencies_for_namespace(namespace)?;
        let contexts = self.list_crate_context_rows()?;
        let source = contexts
            .iter()
            .find(|context| context.namespace == namespace)
            .ok_or(DbError::NotFound)?;

        let mut candidates = Vec::new();
        for dependency in dependencies {
            let Some(dep_path) = dependency.path.as_deref() else {
                continue;
            };
            for target in &contexts {
                if target.namespace == namespace {
                    continue;
                }
                if paths_match(&source.root_path, dep_path, &target.root_path) {
                    candidates.push(WorkspaceDependencyCandidate {
                        dependency: dependency.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
        Ok(candidates)
    }
}

fn dependency_row(row: &[DataValue]) -> Result<CrateDependencyRow, DbError> {
    Ok(CrateDependencyRow {
        id: row_value(row, 0, "crate_dependency.id").and_then(to_uuid)?,
        namespace: row_value(row, 1, "crate_dependency.namespace").and_then(to_uuid)?,
        crate_name: row_value(row, 2, "crate_dependency.crate_name").and_then(to_string)?,
        dep_name: row_value(row, 3, "crate_dependency.dep_name").and_then(to_string)?,
        dep_kind: row_value(row, 4, "crate_dependency.dep_kind").and_then(to_string)?,
        version: row_value(row, 5, "crate_dependency.version").and_then(optional_string)?,
        path: row_value(row, 6, "crate_dependency.path").and_then(optional_string)?,
        git: row_value(row, 7, "crate_dependency.git").and_then(optional_string)?,
        branch: row_value(row, 8, "crate_dependency.branch").and_then(optional_string)?,
        tag: row_value(row, 9, "crate_dependency.tag").and_then(optional_string)?,
        rev: row_value(row, 10, "crate_dependency.rev").and_then(optional_string)?,
        features: row_value(row, 11, "crate_dependency.features").and_then(optional_strings)?,
        optional: row_value(row, 12, "crate_dependency.optional").and_then(optional_bool)?,
        default_features: row_value(row, 13, "crate_dependency.default_features")
            .and_then(optional_bool)?,
    })
}

fn row_value<'a>(
    row: &'a [DataValue],
    index: usize,
    field: &str,
) -> Result<&'a DataValue, DbError> {
    row.get(index)
        .ok_or_else(|| DbError::QueryExecution(format!("missing {field}")))
}

fn optional_string(value: &DataValue) -> Result<Option<String>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        _ => to_string(value).map(Some),
    }
}

fn optional_strings(value: &DataValue) -> Result<Option<Vec<String>>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        _ => to_string_list(value).map(Some),
    }
}

fn optional_bool(value: &DataValue) -> Result<Option<bool>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        DataValue::Bool(value) => Ok(Some(*value)),
        other => Err(DbError::Cozo(format!("Expected Bool?, found {other:?}"))),
    }
}

fn paths_match(source_root: &str, dep_path: &str, target_root: &str) -> bool {
    let dep_path = Path::new(dep_path);
    let resolved = if dep_path.is_absolute() {
        normalize(dep_path)
    } else {
        normalize(Path::new(source_root).join(dep_path))
    };
    resolved == normalize(Path::new(target_root))
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
