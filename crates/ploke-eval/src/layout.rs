use std::env;
use std::path::{Component, Path, PathBuf};

use crate::registry::builtin_dataset_registry_entry;
use crate::spec::PrepareError;

const PLOKE_EVAL_HOME_ENV: &str = "PLOKE_EVAL_HOME";

pub fn ploke_eval_home() -> Result<PathBuf, PrepareError> {
    if let Some(path) = env::var_os(PLOKE_EVAL_HOME_ENV) {
        return Ok(PathBuf::from(path));
    }

    let home = dirs::home_dir().ok_or(PrepareError::MissingHomeDirectory)?;
    Ok(home.join(".ploke-eval"))
}

pub fn repos_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("repos"))
}

pub fn campaigns_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("campaigns"))
}

pub fn registries_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("registries"))
}

pub fn instances_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("instances"))
}

pub fn protocol_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("protocol"))
}

pub fn protocol_artifacts_dir_for_run(run_dir: &Path) -> PathBuf {
    protocol_root_for_run(run_dir).join(protocol_run_relative_path(run_dir))
}

pub fn legacy_protocol_artifacts_dir_for_run(run_dir: &Path) -> PathBuf {
    run_dir.join("protocol-artifacts")
}

pub fn protocol_artifact_read_dirs_for_run(run_dir: &Path) -> Vec<PathBuf> {
    let primary = protocol_artifacts_dir_for_run(run_dir);
    let legacy = legacy_protocol_artifacts_dir_for_run(run_dir);
    if primary == legacy {
        vec![primary]
    } else {
        vec![primary, legacy]
    }
}

fn protocol_root_for_run(run_dir: &Path) -> PathBuf {
    if let Some(eval_home) = eval_home_from_instances_run_dir(run_dir) {
        return eval_home.join("protocol");
    }
    run_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("protocol")
}

fn protocol_run_relative_path(run_dir: &Path) -> PathBuf {
    if let Some(eval_home) = eval_home_from_instances_run_dir(run_dir) {
        let instances_dir = eval_home.join("instances");
        if let Ok(relative) = run_dir.strip_prefix(instances_dir) {
            let sanitized = sanitize_layout_path(relative);
            if !sanitized.as_os_str().is_empty() {
                return sanitized;
            }
        }
    }
    PathBuf::from(protocol_run_dir_name(run_dir))
}

fn eval_home_from_instances_run_dir(run_dir: &Path) -> Option<PathBuf> {
    run_dir
        .ancestors()
        .filter_map(|ancestor| {
            (ancestor.file_name().and_then(|name| name.to_str()) == Some("instances"))
                .then(|| ancestor.parent().map(Path::to_path_buf))
                .flatten()
        })
        .last()
}

fn sanitize_layout_path(path: &Path) -> PathBuf {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        if let Component::Normal(value) = component {
            sanitized.push(sanitize_layout_component(&value.to_string_lossy()));
        }
    }
    sanitized
}

fn protocol_run_dir_name(run_dir: &Path) -> String {
    run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(sanitize_layout_component)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown-run".to_string())
}

fn sanitize_layout_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn batches_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("batches"))
}

pub fn datasets_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("datasets"))
}

pub fn models_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("models"))
}

pub fn model_registry_file() -> Result<PathBuf, PrepareError> {
    Ok(models_dir()?.join("registry.json"))
}

pub fn active_model_file() -> Result<PathBuf, PrepareError> {
    Ok(models_dir()?.join("active-model.json"))
}

pub fn parent_patcher_model_file() -> Result<PathBuf, PrepareError> {
    Ok(models_dir()?.join("parent-patcher-model.json"))
}

pub fn provider_prefs_file() -> Result<PathBuf, PrepareError> {
    Ok(models_dir()?.join("provider-preferences.json"))
}

pub fn embedding_model_registry_file() -> Result<PathBuf, PrepareError> {
    Ok(models_dir()?.join("embedding-models-openrouter.json"))
}

pub fn cache_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("cache"))
}

pub fn last_run_file() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("last-run.json"))
}

pub fn active_selection_file() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("selection.json"))
}

pub fn prototype1_monitor_target_file() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("prototype1-monitor-target.json"))
}

pub fn starting_db_cache_dir() -> Result<PathBuf, PrepareError> {
    Ok(cache_dir()?.join("starting-dbs"))
}

pub fn workspace_root_for_key(dataset_key: &str) -> Result<PathBuf, PrepareError> {
    let entry = builtin_dataset_registry_entry(dataset_key)
        .ok_or_else(|| PrepareError::UnknownDatasetKey(dataset_key.to_string()))?;
    Ok(repos_dir()?.join(entry.org).join(entry.repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_artifacts_dir_uses_eval_home_protocol_for_registered_runs() {
        let run_dir = Path::new("/tmp/eval-home")
            .join("instances")
            .join("prototype1")
            .join("campaign-a")
            .join("instances")
            .join("task-a")
            .join("runs")
            .join("run-123");

        assert_eq!(
            protocol_artifacts_dir_for_run(&run_dir),
            Path::new("/tmp/eval-home")
                .join("protocol")
                .join("prototype1")
                .join("campaign-a")
                .join("instances")
                .join("task-a")
                .join("runs")
                .join("run-123")
        );
    }

    #[test]
    fn protocol_artifact_read_dirs_include_legacy_run_local_dir() {
        let run_dir = Path::new("/tmp/eval-home")
            .join("instances")
            .join("task-a")
            .join("runs")
            .join("run-123");

        assert_eq!(
            protocol_artifact_read_dirs_for_run(&run_dir),
            vec![
                Path::new("/tmp/eval-home")
                    .join("protocol")
                    .join("task-a")
                    .join("runs")
                    .join("run-123"),
                run_dir.join("protocol-artifacts"),
            ]
        );
    }
}
