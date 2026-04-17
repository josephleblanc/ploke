use std::fs;
use std::path::{Path, PathBuf};

use ploke_llm::Router;
use ploke_llm::request::models::{Response, ResponseItem};
use ploke_llm::router_only::openrouter::OpenRouter;
use ploke_llm::{HTTP_REFERER, HTTP_TITLE, ModelId, ModelKey};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::layout::{active_model_file, model_registry_file, models_dir};
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
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PrepareError::MissingActiveModel(path.to_path_buf())
        } else {
            PrepareError::ReadActiveModel {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseActiveModel {
        path: path.to_path_buf(),
        source,
    })
}

pub fn save_active_model(model_id: &ModelId) -> Result<(), PrepareError> {
    save_active_model_at(active_model_path()?, model_id)
}

pub fn save_active_model_at(
    path: impl AsRef<Path>,
    model_id: &ModelId,
) -> Result<(), PrepareError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteActiveModel {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let selection = ActiveModelSelection {
        model_id: model_id.clone(),
    };
    let json =
        serde_json::to_string_pretty(&selection).map_err(PrepareError::SerializeActiveModel)?;
    fs::write(path, json).map_err(|source| PrepareError::WriteActiveModel {
        path: path.to_path_buf(),
        source,
    })
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
    use serde_json::json;
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
}
