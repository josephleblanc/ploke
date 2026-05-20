use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ploke_llm::Router;
use ploke_llm::request::models::{Response, ResponseItem};
use ploke_llm::router_only::HasModels;
use ploke_llm::router_only::google::Google;
use ploke_llm::router_only::openrouter::OpenRouter;
use ploke_llm::{HTTP_REFERER, HTTP_TITLE, ModelId, ModelKey};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::layout::{
    active_model_file, model_registry_file, models_dir, parent_patcher_model_file,
};
use crate::spec::PrepareError;

pub type ModelRegistry = Response;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveModelSelection {
    pub model_id: ModelId,
}

pub fn model_registry_path() -> Result<PathBuf, PrepareError> {
    model_registry_file()
}

pub fn active_model_path() -> Result<PathBuf, PrepareError> {
    active_model_file()
}

pub fn parent_patcher_model_path() -> Result<PathBuf, PrepareError> {
    parent_patcher_model_file()
}

fn openrouter_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("http-referer"),
        HeaderValue::from_str(HTTP_REFERER).expect("valid referer"),
    );
    headers.insert(
        HeaderName::from_static("x-title"),
        HeaderValue::from_str(HTTP_TITLE).expect("valid title"),
    );
    headers
}

pub async fn refresh_model_registry() -> Result<ModelRegistry, PrepareError> {
    let dir = models_dir()?;
    fs::create_dir_all(&dir).map_err(|source| PrepareError::CreateOutputDir {
        path: dir.clone(),
        source,
    })?;
    let mut registry = fetch_model_registry().await?;
    registry.data.sort_by(|a, b| a.id.cmp(&b.id));
    save_model_registry(&registry)?;
    Ok(registry)
}

pub async fn fetch_model_registry() -> Result<ModelRegistry, PrepareError> {
    let mut registries = Vec::new();
    let mut errors = Vec::new();

    match fetch_openrouter_model_registry().await {
        Ok(registry) => registries.push(registry),
        Err(err) => errors.push(format!("openrouter: {err}")),
    }

    match fetch_google_model_registry().await {
        Ok(registry) => registries.push(registry),
        Err(err) => errors.push(format!("google: {err}")),
    }

    if registries.is_empty() {
        return Err(PrepareError::DatabaseSetup {
            phase: "fetch_model_registry",
            detail: format!(
                "no model registry source refreshed successfully ({})",
                errors.join("; ")
            ),
        });
    }

    Ok(merge_model_registries(registries))
}

pub async fn fetch_openrouter_model_registry() -> Result<ModelRegistry, PrepareError> {
    let api_key = OpenRouter::resolve_api_key().map_err(|source| PrepareError::DatabaseSetup {
        phase: "resolve_openrouter_api_key",
        detail: source.to_string(),
    })?;
    let client = Client::builder()
        .default_headers(openrouter_headers())
        .build()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "build_model_registry_client",
            detail: source.to_string(),
        })?;
    let response = client
        .get(OpenRouter::MODELS_URL)
        .bearer_auth(api_key)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "fetch_model_registry",
            detail: source.to_string(),
        })?
        .error_for_status()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "fetch_model_registry_status",
            detail: source.to_string(),
        })?;

    response
        .json::<ModelRegistry>()
        .await
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "parse_model_registry_response",
            detail: source.to_string(),
        })
}

pub async fn fetch_google_model_registry() -> Result<ModelRegistry, PrepareError> {
    let _api_key = Google::resolve_api_key().map_err(|source| PrepareError::DatabaseSetup {
        phase: "resolve_google_api_key",
        detail: source.to_string(),
    })?;
    let client = Client::builder()
        .build()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "build_google_model_registry_client",
            detail: source.to_string(),
        })?;
    let response = <Google as HasModels>::fetch_models(&client)
        .await
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "fetch_google_model_registry",
            detail: source.to_string(),
        })?;
    Ok(ModelRegistry {
        data: response.into_iter().map(Into::into).collect(),
    })
}

fn merge_model_registries(registries: Vec<ModelRegistry>) -> ModelRegistry {
    let mut by_id = BTreeMap::new();
    for registry in registries {
        for item in registry.data {
            let replace = by_id.get(&item.id).is_none_or(|existing: &ResponseItem| {
                item.route_source.is_direct_google() && existing.route_source.is_openrouter()
            });
            if replace {
                by_id.insert(item.id.clone(), item);
            }
        }
    }
    ModelRegistry {
        data: by_id.into_values().collect(),
    }
}

pub fn load_model_registry() -> Result<ModelRegistry, PrepareError> {
    load_model_registry_at(model_registry_path()?)
}

pub fn load_model_registry_at(path: impl AsRef<Path>) -> Result<ModelRegistry, PrepareError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PrepareError::MissingModelRegistry(path.to_path_buf())
        } else {
            PrepareError::ReadModelRegistry {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseModelRegistry {
        path: path.to_path_buf(),
        source,
    })
}

pub fn save_model_registry(registry: &ModelRegistry) -> Result<(), PrepareError> {
    save_model_registry_at(model_registry_path()?, registry)
}

pub fn save_model_registry_at(
    path: impl AsRef<Path>,
    registry: &ModelRegistry,
) -> Result<(), PrepareError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteModelRegistry {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut sorted = registry.clone();
    sorted.data.sort_by(|a, b| a.id.cmp(&b.id));

    let json =
        serde_json::to_string_pretty(&sorted).map_err(PrepareError::SerializeModelRegistry)?;
    fs::write(path, json).map_err(|source| PrepareError::WriteModelRegistry {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_active_model() -> Result<ActiveModelSelection, PrepareError> {
    load_active_model_at(active_model_path()?)
}

pub fn load_active_model_at(path: impl AsRef<Path>) -> Result<ActiveModelSelection, PrepareError> {
    load_model_selection_at(
        path,
        PrepareError::MissingActiveModel,
        |path, source| PrepareError::ReadActiveModel { path, source },
        |path, source| PrepareError::ParseActiveModel { path, source },
    )
}

pub fn load_parent_patcher_model() -> Result<ActiveModelSelection, PrepareError> {
    load_parent_patcher_model_at(parent_patcher_model_path()?)
}

pub fn load_parent_patcher_model_at(
    path: impl AsRef<Path>,
) -> Result<ActiveModelSelection, PrepareError> {
    load_model_selection_at(
        path,
        PrepareError::MissingParentPatcherModel,
        |path, source| PrepareError::ReadParentPatcherModel { path, source },
        |path, source| PrepareError::ParseParentPatcherModel { path, source },
    )
}

fn load_model_selection_at(
    path: impl AsRef<Path>,
    missing: impl Fn(PathBuf) -> PrepareError,
    read: impl Fn(PathBuf, std::io::Error) -> PrepareError,
    parse: impl Fn(PathBuf, serde_json::Error) -> PrepareError,
) -> Result<ActiveModelSelection, PrepareError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            missing(path.to_path_buf())
        } else {
            read(path.to_path_buf(), source)
        }
    })?;
    serde_json::from_str(&text).map_err(|source| parse(path.to_path_buf(), source))
}

pub fn save_active_model(model_id: &ModelId) -> Result<(), PrepareError> {
    save_active_model_at(active_model_path()?, model_id)
}

pub fn save_active_model_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
) -> Result<(), PrepareError> {
    save_model_selection_at(
        path,
        model_id,
        |path, source| PrepareError::WriteActiveModel { path, source },
        PrepareError::SerializeActiveModel,
    )
}

pub fn save_parent_patcher_model(model_id: &ModelId) -> Result<(), PrepareError> {
    save_parent_patcher_model_at(parent_patcher_model_path()?, model_id)
}

pub fn save_parent_patcher_model_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
) -> Result<(), PrepareError> {
    save_model_selection_at(
        path,
        model_id,
        |path, source| PrepareError::WriteParentPatcherModel { path, source },
        PrepareError::SerializeParentPatcherModel,
    )
}

fn save_model_selection_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
    write: impl Fn(PathBuf, std::io::Error) -> PrepareError,
    serialize: impl Fn(serde_json::Error) -> PrepareError,
) -> Result<(), PrepareError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| write(parent.to_path_buf(), source))?;
    }

    let selection = ActiveModelSelection {
        model_id: model_id.clone(),
    };
    let json = serde_json::to_string_pretty(&selection).map_err(serialize)?;
    fs::write(path, json).map_err(|source| write(path.to_path_buf(), source))
}

pub fn registry_has_model(registry: &ModelRegistry, model_id: &ModelId) -> bool {
    registry.data.iter().any(|item| item.id == *model_id)
}

pub fn resolve_model_for_run(
    explicit_model_id: Option<&ModelId>,
    use_default_model: bool,
) -> Result<ResponseItem, PrepareError> {
    let path = model_registry_path()?;
    let registry = load_model_registry()?;
    let model_id = if let Some(model_id) = explicit_model_id {
        model_id.clone()
    } else if use_default_model {
        ModelId::from(ModelKey::default())
    } else {
        load_active_model()?.model_id
    };
    registry
        .data
        .into_iter()
        .find(|item| item.id == model_id)
        .ok_or_else(|| PrepareError::UnknownModelInRegistry {
            model: model_id.to_string(),
            path,
        })
}

pub fn find_models<'a>(registry: &'a ModelRegistry, query: &str) -> Vec<&'a ResponseItem> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return registry.data.iter().collect();
    }

    registry
        .data
        .iter()
        .filter(|item| {
            let id = item.id.to_string().to_ascii_lowercase();
            let name = item.name.as_str().to_ascii_lowercase();
            let canonical = item
                .canonical
                .as_ref()
                .map(|m| m.to_string().to_ascii_lowercase());

            id.contains(&needle)
                || name.contains(&needle)
                || canonical.is_some_and(|c| c.contains(&needle))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_llm::request::models::ModelRouteSource;
    use serde_json::json;
    use std::str::FromStr;
    use tempfile::tempdir;

    fn sample_registry() -> ModelRegistry {
        serde_json::from_value(json!({
            "data": [
                {
                    "id": "qwen/qwen2.5",
                    "name": "Qwen 2.5",
                    "created": 1,
                    "description": "",
                    "architecture": {
                        "input_modalities": ["text"],
                        "modality": "text->text",
                        "output_modalities": ["text"],
                        "tokenizer": "Other",
                        "instruct_type": null
                    },
                    "top_provider": {
                        "is_moderated": false,
                        "context_length": 8192,
                        "max_completion_tokens": 8192
                    },
                    "pricing": {
                        "prompt": 0,
                        "completion": 0
                    },
                    "canonical_slug": "qwen/qwen2.5",
                    "context_length": 8192,
                    "supported_parameters": ["tools", "temperature"]
                },
                {
                    "id": "anthropic/claude-3.5-sonnet",
                    "name": "Claude 3.5 Sonnet",
                    "created": 1,
                    "description": "",
                    "architecture": {
                        "input_modalities": ["text"],
                        "modality": "text->text",
                        "output_modalities": ["text"],
                        "tokenizer": "Other",
                        "instruct_type": null
                    },
                    "top_provider": {
                        "is_moderated": false,
                        "context_length": 8192,
                        "max_completion_tokens": 8192
                    },
                    "pricing": {
                        "prompt": 0,
                        "completion": 0
                    },
                    "canonical_slug": "anthropic/claude-3.5-sonnet",
                    "context_length": 8192,
                    "supported_parameters": ["tools"]
                }
            ]
        }))
        .expect("sample registry parses")
    }

    #[test]
    fn find_models_matches_query() {
        let registry = sample_registry();
        let matches = find_models(&registry, "qwen");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id.to_string(), "qwen/qwen2.5");
    }

    #[test]
    fn merge_model_registries_prefers_direct_google_row_on_id_collision() {
        let mut openrouter = sample_registry();
        let google_id = ModelId::from_str("google/gemini-2.5-flash").expect("model id");
        let mut openrouter_google = openrouter.data[0].clone();
        openrouter_google.id = google_id.clone();
        openrouter_google.route_source = ModelRouteSource::OpenRouter;
        openrouter.data.push(openrouter_google);

        let mut direct_google = openrouter.data[0].clone();
        direct_google.id = google_id.clone();
        direct_google.route_source = ModelRouteSource::DirectGoogle;

        let merged = merge_model_registries(vec![
            openrouter,
            ModelRegistry {
                data: vec![direct_google],
            },
        ]);

        let selected = merged
            .data
            .iter()
            .find(|item| item.id == google_id)
            .expect("merged google model");
        assert!(selected.route_source.is_direct_google());
    }

    #[test]
    fn save_and_load_registry_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("registry.json");
        let mut registry = sample_registry();
        registry.data.reverse();

        save_model_registry_at(&path, &registry).expect("save registry");
        let loaded = load_model_registry_at(&path).expect("load registry");

        assert_eq!(loaded.data.len(), 2);
        assert_eq!(loaded.data[0].id.to_string(), "anthropic/claude-3.5-sonnet");
        assert_eq!(loaded.data[1].id.to_string(), "qwen/qwen2.5");
    }

    #[test]
    fn save_and_load_active_model_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("active-model.json");
        let model_id = ModelId::from(ModelKey::default());

        save_active_model_at(&path, &model_id).expect("save active model");
        let loaded = load_active_model_at(&path).expect("load active model");

        assert_eq!(loaded.model_id, model_id);
    }

    #[test]
    fn save_and_load_parent_patcher_model_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("parent-patcher-model.json");
        let model_id = ModelId::from(ModelKey::default());

        save_parent_patcher_model_at(&path, &model_id).expect("save parent patcher model");
        let loaded = load_parent_patcher_model_at(&path).expect("load parent patcher model");

        assert_eq!(loaded.model_id, model_id);
    }
}
