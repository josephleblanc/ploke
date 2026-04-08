use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ploke_llm::{ModelId, ProviderKey};
use serde::{Deserialize, Serialize};

use crate::layout::provider_prefs_file;
use crate::spec::PrepareError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderPrefs {
    pub selected_providers: BTreeMap<String, ProviderKey>,
}

pub fn provider_prefs_path() -> Result<PathBuf, PrepareError> {
    provider_prefs_file()
}

pub fn load_provider_prefs() -> Result<ProviderPrefs, PrepareError> {
    load_provider_prefs_at(provider_prefs_path()?)
}

pub fn load_provider_prefs_at(path: impl AsRef<Path>) -> Result<ProviderPrefs, PrepareError> {
    let path = path.as_ref();
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|source| PrepareError::ParseProviderPrefs {
                path: path.to_path_buf(),
                source,
            })
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(ProviderPrefs::default())
        }
        Err(source) => Err(PrepareError::ReadProviderPrefs {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn save_provider_prefs(prefs: &ProviderPrefs) -> Result<(), PrepareError> {
    save_provider_prefs_at(provider_prefs_path()?, prefs)
}

pub fn save_provider_prefs_at(
    path: impl AsRef<Path>,
    prefs: &ProviderPrefs,
) -> Result<(), PrepareError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteProviderPrefs {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let json = serde_json::to_string_pretty(prefs).map_err(PrepareError::SerializeProviderPrefs)?;
    fs::write(path, json).map_err(|source| PrepareError::WriteProviderPrefs {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_provider_for_model(model_id: &ModelId) -> Result<Option<ProviderKey>, PrepareError> {
    let prefs = load_provider_prefs()?;
    Ok(prefs.selected_providers.get(&model_id.to_string()).cloned())
}

pub fn set_provider_for_model(
    model_id: &ModelId,
    provider_key: ProviderKey,
) -> Result<(), PrepareError> {
    set_provider_for_model_at(provider_prefs_path()?, model_id, provider_key)
}

pub fn set_provider_for_model_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
    provider_key: ProviderKey,
) -> Result<(), PrepareError> {
    let path = path.as_ref();
    let mut prefs = load_provider_prefs_at(path)?;
    prefs
        .selected_providers
        .insert(model_id.to_string(), provider_key);
    save_provider_prefs_at(path, &prefs)
}

pub fn clear_provider_for_model(model_id: &ModelId) -> Result<(), PrepareError> {
    clear_provider_for_model_at(provider_prefs_path()?, model_id)
}

pub fn clear_provider_for_model_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
) -> Result<(), PrepareError> {
    let path = path.as_ref();
    let mut prefs = load_provider_prefs_at(path)?;
    prefs.selected_providers.remove(&model_id.to_string());
    save_provider_prefs_at(path, &prefs)
}

pub fn load_provider_for_model_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
) -> Result<Option<ProviderKey>, PrepareError> {
    let prefs = load_provider_prefs_at(path)?;
    Ok(prefs.selected_providers.get(&model_id.to_string()).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_and_clear_provider_prefs() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("provider-preferences.json");
        let model = ModelId::from_str("openai/gpt-4").expect("model");
        let provider = ProviderKey::new("chutes").expect("provider");

        set_provider_for_model_at(&path, &model, provider.clone()).expect("set provider");
        let loaded = load_provider_for_model_at(&path, &model).expect("load provider");
        assert_eq!(loaded, Some(provider));

        clear_provider_for_model_at(&path, &model).expect("clear provider");
        let loaded = load_provider_for_model_at(&path, &model).expect("load provider");
        assert!(loaded.is_none());
    }
}
