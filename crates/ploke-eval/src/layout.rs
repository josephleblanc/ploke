use std::env;
use std::path::PathBuf;

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

pub fn runs_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("runs"))
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

pub fn cache_dir() -> Result<PathBuf, PrepareError> {
    Ok(ploke_eval_home()?.join("cache"))
}

pub fn starting_db_cache_dir() -> Result<PathBuf, PrepareError> {
    Ok(cache_dir()?.join("starting-dbs"))
}

pub fn workspace_root_for_key(dataset_key: &str) -> Result<PathBuf, PrepareError> {
    let entry = builtin_dataset_registry_entry(dataset_key)
        .ok_or_else(|| PrepareError::UnknownDatasetKey(dataset_key.to_string()))?;
    Ok(repos_dir()?.join(entry.org).join(entry.repo))
}
