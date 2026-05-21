use std::sync::OnceLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use fxhash::FxBuildHasher;
use ploke_llm::manager::{ChatHttpConfig, ChatStepOutcome, RequestMessage, chat_step};
use ploke_llm::request::models::ModelRouteSource;
use ploke_llm::response::OpenAiResponse;
use ploke_llm::router_only::google::Google;
use ploke_llm::router_only::openrouter::{OpenRouter, ProviderPreferences};
use ploke_llm::router_only::{ChatCompRequest, Router};
use ploke_llm::{AttemptTimeout, ModelId, ProviderSlug, ReasoningConfig, ReasoningEffort};
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
    #[serde(default, skip_serializing_if = "ModelRouteSource::is_openrouter")]
    pub route_source: ModelRouteSource,
    pub provider_slug: Option<String>,
    pub timeout_secs: u64,
    pub max_attempts: u32,
    pub max_tokens: u32,
}

impl Default for JsonLlmConfig {
    fn default() -> Self {
        Self {
            model_id: "moonshotai/kimi-k2".to_string(),
            route_source: ModelRouteSource::OpenRouter,
            provider_slug: None,
            timeout_secs: 30,
            max_attempts: 1,
            max_tokens: 400,
        }
    }
}

impl JsonLlmConfig {
    pub fn provider_display(&self) -> &str {
        if self.route_source.is_direct_google() {
            "google"
        } else {
            self.provider_slug.as_deref().unwrap_or("auto/openrouter")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonLlmProvenance {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "ModelRouteSource::is_openrouter")]
    pub route_source: ModelRouteSource,
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
    #[error("invalid llm route config: {detail}")]
    InvalidConfig { detail: String },
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
        if self.cfg.route_source.is_direct_google() {
            "google_json_chat"
        } else {
            "openrouter_json_chat"
        }
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
                route_source: self.cfg.route_source,
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
    match parse_protocol_json_content_once::<T>(content) {
        Ok(parsed) => Ok(parsed),
        Err(detail) => {
            if let Some(repaired) = repair_unterminated_final_rationale(content, &detail) {
                if let Ok(parsed) = parse_protocol_json_content_once::<T>(&repaired) {
                    return Ok(parsed);
                }
            }
            Err(ProtocolLlmError::ParseJson {
                detail,
                content: content.to_string(),
            })
        }
    }
}

fn parse_protocol_json_content_once<T: DeserializeOwned>(content: &str) -> Result<T, String> {
    match serde_json::from_str::<T>(content) {
        Ok(parsed) => Ok(parsed),
        Err(original_err) => {
            let mut value: Value =
                serde_json::from_str(content).map_err(|_| original_err.to_string())?;

            if !normalize_protocol_json_aliases(&mut value) {
                return Err(original_err.to_string());
            }

            serde_json::from_value::<T>(value).map_err(|err| err.to_string())
        }
    }
}

fn repair_unterminated_final_rationale(content: &str, detail: &str) -> Option<String> {
    if !detail.contains("EOF while parsing a string") {
        return None;
    }

    let trimmed = content.trim_end();
    if !trimmed.starts_with('{')
        || !(trimmed.contains("\"rationale\"") || trimmed.contains("\"overall_rationale\""))
        || !has_odd_unescaped_quotes(trimmed)
    {
        return None;
    }

    let mut repaired = trimmed.to_string();
    if repaired.ends_with('}') {
        let insert_at = repaired.len() - 1;
        repaired.insert(insert_at, '"');
    } else {
        repaired.push('"');
        repaired.push('}');
    }
    Some(repaired)
}

fn has_odd_unescaped_quotes(input: &str) -> bool {
    let mut escaped = false;
    let mut quotes = 0usize;

    for ch in input.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => quotes += 1,
            _ => {}
        }
    }

    quotes % 2 == 1
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
    let model = parse_json_model(cfg)?;

    let http = chat_http_config_for_json_llm(cfg);

    if let Some(interval) = json_llm_min_request_interval(cfg) {
        wait_for_json_llm_rate_slot(interval).await;
    }

    let response = if cfg.route_source.is_direct_google() {
        let request = google_json_request(model, cfg, prompt)?;
        chat_step(client, &request, &http)
            .await
            .map_err(|err| ProtocolLlmError::Request(err.to_string()))?
    } else {
        let request = openrouter_json_request(model, cfg, prompt);
        chat_step(client, &request, &http)
            .await
            .map_err(|err| ProtocolLlmError::Request(err.to_string()))?
    };

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

fn parse_json_model(cfg: &JsonLlmConfig) -> Result<ModelId, ProtocolLlmError> {
    cfg.model_id
        .parse()
        .map_err(|err: ploke_llm::IdError| ProtocolLlmError::InvalidModelId {
            model_id: cfg.model_id.clone(),
            detail: err.to_string(),
        })
}

fn base_json_request<R: Router>(
    model: ModelId,
    cfg: &JsonLlmConfig,
    prompt: &JsonChatPrompt,
) -> ChatCompRequest<R> {
    R::default_chat_completion()
        .with_model(model)
        .with_messages(vec![
            RequestMessage::new_system(prompt.system.clone()),
            RequestMessage::new_user(prompt.user.clone()),
        ])
        .with_json_response()
        .with_max_tokens(cfg.max_tokens)
        .with_reasoning(ReasoningConfig::default().with_effort(ReasoningEffort::None))
        .non_streaming()
}

fn openrouter_json_request(
    model: ModelId,
    cfg: &JsonLlmConfig,
    prompt: &JsonChatPrompt,
) -> ChatCompRequest<OpenRouter> {
    let mut request = base_json_request::<OpenRouter>(model, cfg, prompt);

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

    request
}

fn google_json_request(
    model: ModelId,
    cfg: &JsonLlmConfig,
    prompt: &JsonChatPrompt,
) -> Result<ChatCompRequest<Google>, ProtocolLlmError> {
    if let Some(provider_slug) = cfg.provider_slug.as_deref()
        && provider_slug != "google"
    {
        return Err(ProtocolLlmError::InvalidConfig {
            detail: format!(
                "direct Google route does not accept OpenRouter provider '{provider_slug}'"
            ),
        });
    }

    Ok(base_json_request::<Google>(model, cfg, prompt))
}

fn chat_http_config_for_json_llm(cfg: &JsonLlmConfig) -> ChatHttpConfig {
    let mut http = ChatHttpConfig::default();
    http.attempt_timeout = AttemptTimeout::fixed(Duration::from_secs(cfg.timeout_secs));
    http.max_attempts = cfg.max_attempts;
    http
}

fn json_llm_min_request_interval(cfg: &JsonLlmConfig) -> Option<Duration> {
    let is_inception_mercury = cfg.provider_slug.as_deref() == Some("inception")
        && cfg.model_id.starts_with("inception/mercury-2");
    is_inception_mercury.then(|| Duration::from_millis(850))
}

async fn wait_for_json_llm_rate_slot(interval: Duration) {
    static LAST_REQUEST: OnceLock<tokio::sync::Mutex<Option<Instant>>> = OnceLock::new();

    let lock = LAST_REQUEST.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut last_request = lock.lock().await;
    if let Some(previous) = *last_request {
        let elapsed = previous.elapsed();
        if elapsed < interval {
            tokio::time::sleep(interval - elapsed).await;
        }
    }
    *last_request = Some(Instant::now());
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

    #[test]
    fn parse_protocol_json_content_repairs_unterminated_final_rationale() {
        let parsed = parse_protocol_json_content::<ReviewLike>(
            r#"{"verdict":"redundant_repeat","confidence":"high","rationale":"model stopped mid-rationale"#,
        )
        .expect("parser should repair a final unterminated rationale string");

        assert_eq!(
            parsed,
            ReviewLike {
                verdict: "redundant_repeat".to_string(),
                confidence: "high".to_string(),
                rationale: "model stopped mid-rationale".to_string(),
            }
        );
    }

    #[test]
    fn parse_protocol_json_content_repairs_final_rationale_before_object_close() {
        let parsed = parse_protocol_json_content::<ReviewLike>(
            r#"{"verdict":"redundant_repeat","confidence":"high","rationale":"model swallowed the closing brace.}"#,
        )
        .expect("parser should close the rationale before a trailing object brace");

        assert_eq!(
            parsed,
            ReviewLike {
                verdict: "redundant_repeat".to_string(),
                confidence: "high".to_string(),
                rationale: "model swallowed the closing brace.".to_string(),
            }
        );
    }

    #[test]
    fn json_llm_config_controls_http_attempt_count() {
        let cfg = JsonLlmConfig {
            model_id: "test/model".to_string(),
            route_source: ModelRouteSource::OpenRouter,
            provider_slug: None,
            timeout_secs: 42,
            max_attempts: 3,
            max_tokens: 400,
        };

        let http = chat_http_config_for_json_llm(&cfg);

        assert_eq!(http.max_attempts, 3);
        assert_eq!(http.attempt_timeout.for_attempt(1), Duration::from_secs(42));
    }

    #[test]
    fn google_json_request_serializes_direct_google_model() {
        let cfg = JsonLlmConfig {
            model_id: "google/gemini-2.5-flash".to_string(),
            route_source: ModelRouteSource::DirectGoogle,
            provider_slug: None,
            timeout_secs: 42,
            max_attempts: 1,
            max_tokens: 64,
        };
        let prompt = JsonChatPrompt {
            system: "Return JSON only.".to_string(),
            user: "Return {\"ok\":true}.".to_string(),
        };
        let model = parse_json_model(&cfg).expect("model id");
        let request = google_json_request(model, &cfg, &prompt).expect("google request");

        let value = serde_json::to_value(&request).expect("serialize request");
        assert_eq!(value["model"], "gemini-2.5-flash");
        assert_eq!(value["response_format"]["type"], "json_object");
        assert_eq!(value["max_tokens"], 64);
        assert!(value.get("provider").is_none());
        assert!(value.get("transforms").is_none());
    }

    #[test]
    fn google_json_request_rejects_openrouter_provider_pin() {
        let cfg = JsonLlmConfig {
            model_id: "google/gemini-2.5-flash".to_string(),
            route_source: ModelRouteSource::DirectGoogle,
            provider_slug: Some("inception".to_string()),
            timeout_secs: 42,
            max_attempts: 1,
            max_tokens: 64,
        };
        let prompt = JsonChatPrompt {
            system: "Return JSON only.".to_string(),
            user: "Return {\"ok\":true}.".to_string(),
        };
        let model = parse_json_model(&cfg).expect("model id");
        let err = google_json_request(model, &cfg, &prompt).expect_err("invalid provider pin");

        assert!(matches!(err, ProtocolLlmError::InvalidConfig { .. }));
    }
}
