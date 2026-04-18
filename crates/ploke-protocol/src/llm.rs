use std::time::Duration;

use async_trait::async_trait;
use fxhash::FxBuildHasher;
use ploke_llm::ProviderSlug;
use ploke_llm::manager::{ChatHttpConfig, ChatStepOutcome, RequestMessage, chat_step};
use ploke_llm::response::OpenAiResponse;
use ploke_llm::router_only::Router;
use ploke_llm::router_only::openrouter::{OpenRouter, ProviderPreferences};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::core::ExecutorKind;
use crate::step::{StepExecution, StepExecutor, StepSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonChatPrompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonLlmConfig {
    pub model_id: String,
    pub provider_slug: Option<String>,
    pub timeout_secs: u64,
    pub max_tokens: u32,
}

impl Default for JsonLlmConfig {
    fn default() -> Self {
        Self {
            model_id: "moonshotai/kimi-k2".to_string(),
            provider_slug: None,
            timeout_secs: 30,
            max_tokens: 400,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonLlmProvenance {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    pub raw_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub response: OpenAiResponse,
}

#[derive(Debug, Error)]
pub enum ProtocolLlmError {
    #[error("invalid model id '{model_id}': {detail}")]
    InvalidModelId { model_id: String, detail: String },
    #[error("llm request failed: {0}")]
    Request(String),
    #[error("expected content response but received tool calls")]
    UnexpectedToolCalls,
    #[error("llm returned no content")]
    MissingContent,
    #[error("failed to parse json response: {detail}; content was: {content}")]
    ParseJson { detail: String, content: String },
}

pub trait JsonAdjudicationSpec: StepSpec
where
    Self::OutputState: DeserializeOwned,
{
    fn build_prompt(&self, input: &Self::InputState) -> JsonChatPrompt;
}

#[derive(Debug, Clone)]
pub struct JsonAdjudicator {
    client: reqwest::Client,
    cfg: JsonLlmConfig,
}

impl JsonAdjudicator {
    pub fn new(client: reqwest::Client, cfg: JsonLlmConfig) -> Self {
        Self { client, cfg }
    }

    pub fn config(&self) -> &JsonLlmConfig {
        &self.cfg
    }
}

#[async_trait]
impl<Spec> StepExecutor<Spec> for JsonAdjudicator
where
    Spec: JsonAdjudicationSpec + Send + Sync,
    Spec::InputState: Send,
    Spec::OutputState: DeserializeOwned + Send,
{
    type Provenance = JsonLlmProvenance;
    type Error = ProtocolLlmError;

    fn kind(&self) -> ExecutorKind {
        ExecutorKind::LlmAdjudicator
    }

    fn label(&self) -> &'static str {
        "openrouter_json_chat"
    }

    async fn execute(
        &self,
        spec: &Spec,
        input: Spec::InputState,
    ) -> Result<StepExecution<Spec::OutputState, Self::Provenance>, Self::Error> {
        let prompt = spec.build_prompt(&input);
        let parsed = adjudicate_json::<Spec::OutputState>(&self.client, &self.cfg, &prompt).await?;
        Ok(StepExecution {
            state: parsed.parsed,
            provenance: JsonLlmProvenance {
                model_id: self.cfg.model_id.clone(),
                provider_slug: self.cfg.provider_slug.clone(),
                raw_content: parsed.content,
                reasoning: parsed.reasoning,
                response: parsed.response,
            },
            disposition: spec.disposition(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct JsonLlmResult<T> {
    pub parsed: T,
    pub content: String,
    pub reasoning: Option<String>,
    pub response: OpenAiResponse,
}

const JSON_ALIAS_KEYS: &[&str] = &["rationale", "overall_rationale"];

fn parse_protocol_json_content<T: DeserializeOwned>(content: &str) -> Result<T, ProtocolLlmError> {
    match serde_json::from_str::<T>(content) {
        Ok(parsed) => Ok(parsed),
        Err(original_err) => {
            let mut value: Value =
                serde_json::from_str(content).map_err(|_| ProtocolLlmError::ParseJson {
                    detail: original_err.to_string(),
                    content: content.to_string(),
                })?;

            if !normalize_protocol_json_aliases(&mut value) {
                return Err(ProtocolLlmError::ParseJson {
                    detail: original_err.to_string(),
                    content: content.to_string(),
                });
            }

            serde_json::from_value::<T>(value).map_err(|err| ProtocolLlmError::ParseJson {
                detail: err.to_string(),
                content: content.to_string(),
            })
        }
    }
}

fn normalize_protocol_json_aliases(value: &mut Value) -> bool {
    match value {
        Value::Object(map) => {
            let mut changed = false;
            for canonical in JSON_ALIAS_KEYS {
                if map.contains_key(*canonical) {
                    continue;
                }
                let alias_key = map
                    .keys()
                    .find(|key| key.eq_ignore_ascii_case(canonical))
                    .cloned();
                if let Some(alias_key) = alias_key {
                    if alias_key != *canonical {
                        if let Some(alias_value) = map.remove(&alias_key) {
                            map.insert((*canonical).to_string(), alias_value);
                            changed = true;
                        }
                    }
                }
            }
            for nested in map.values_mut() {
                changed |= normalize_protocol_json_aliases(nested);
            }
            changed
        }
        Value::Array(items) => items.iter_mut().fold(false, |changed, item| {
            changed | normalize_protocol_json_aliases(item)
        }),
        _ => false,
    }
}

pub async fn adjudicate_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    cfg: &JsonLlmConfig,
    prompt: &JsonChatPrompt,
) -> Result<JsonLlmResult<T>, ProtocolLlmError> {
    let model = cfg.model_id.parse().map_err(|err: ploke_llm::IdError| {
        ProtocolLlmError::InvalidModelId {
            model_id: cfg.model_id.clone(),
            detail: err.to_string(),
        }
    })?;

    let mut request = OpenRouter::default_chat_completion()
        .with_model(model)
        .with_messages(vec![
            RequestMessage::new_system(prompt.system.clone()),
            RequestMessage::new_user(prompt.user.clone()),
        ])
        .with_json_response()
        .with_max_tokens(cfg.max_tokens)
        .non_streaming();

    if let Some(provider_slug) = cfg.provider_slug.as_ref() {
        let mut only = std::collections::HashSet::with_hasher(FxBuildHasher::default());
        only.insert(ProviderSlug::new(provider_slug));
        let provider = ProviderPreferences {
            only: Some(only),
            allow_fallbacks: Some(false),
            ..Default::default()
        };
        request = request.with_router_bundle(
            ploke_llm::router_only::openrouter::ChatCompFields::default().with_provider(provider),
        );
    }

    let mut http = ChatHttpConfig::default();
    http.timeout = Duration::from_secs(cfg.timeout_secs);

    let response = chat_step(client, &request, &http)
        .await
        .map_err(|err| ProtocolLlmError::Request(err.to_string()))?;

    match response.outcome {
        ChatStepOutcome::Content { content, reasoning } => {
            let content = content.ok_or(ProtocolLlmError::MissingContent)?.to_string();
            let parsed = parse_protocol_json_content::<T>(&content)?;
            Ok(JsonLlmResult {
                parsed,
                content,
                reasoning: reasoning.map(|r| r.to_string()),
                response: response.full_response,
            })
        }
        ChatStepOutcome::ToolCalls { .. } => Err(ProtocolLlmError::UnexpectedToolCalls),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct ReviewLike {
        verdict: String,
        confidence: String,
        rationale: String,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct OverallLike {
        overall_rationale: String,
    }

    #[test]
    fn parse_protocol_json_content_recovers_capitalized_rationale_key() {
        let parsed = parse_protocol_json_content::<ReviewLike>(
            r#"{"verdict":"helpful_but_non_essential","confidence":"medium","Rationale":"useful context"}"#,
        )
        .expect("parser should recover rationale alias");

        assert_eq!(
            parsed,
            ReviewLike {
                verdict: "helpful_but_non_essential".to_string(),
                confidence: "medium".to_string(),
                rationale: "useful context".to_string(),
            }
        );
    }

    #[test]
    fn parse_protocol_json_content_recovers_capitalized_overall_rationale_key() {
        let parsed = parse_protocol_json_content::<OverallLike>(
            r#"{"Overall_Rationale":"coherent sequence"}"#,
        )
        .expect("parser should recover overall rationale alias");

        assert_eq!(
            parsed,
            OverallLike {
                overall_rationale: "coherent sequence".to_string(),
            }
        );
    }
}
