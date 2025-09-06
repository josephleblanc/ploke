use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::llm::openrouter::openrouter_catalog::ModelPricing;
use crate::utils::consts::OPENROUTER_BASE_STR;
use crate::utils::consts::OPENROUTER_ENDPOINT_STR;
use crate::utils::se_de::string_or_f64;
use crate::utils::se_de::string_or_f64_opt;
use lazy_static::lazy_static;
use ploke_core::ArcStr;
use ploke_test_utils::workspace_root;
use reqwest::Url;
use serde::{Deserialize, Serialize};

const MODEL_ENDPOINT_RESP_DIR: &str = "crates/ploke-tui/data/endpoints/";

lazy_static! {
    pub static ref OPENROUTER_MODELS_URL: Url =
        Url::parse("https://openrouter.ai/api/v1/models").expect("Invalid OpenRouter models URL");
}

use std::convert::TryFrom;

/// A more strongly typed and segmented type that can be used to make a call to a model endpoint
/// with OpenRouter's API
// TODO: need to implement Deserialize on ProvEnd, likely a custom implementation so we can
// deserialize the `canonical_slug` from the json files into `ProvEnd`.
#[derive(Debug, Clone)]
pub struct ProvEnd {
    pub author: ArcStr,
    pub model: ArcStr,
}

impl ProvEnd {
    #[tracing::instrument(level = "debug", skip_all, fields(author = ?self.author, model = ?self.model))]
    pub(crate) async fn call_endpoint_raw(&self) -> color_eyre::Result<serde_json::Value> {
        // Use test harness/env helper to obtain URL and API key (dotenv fallback allowed)
        let op = crate::test_harness::openrouter_env()
            .ok_or_else(|| color_eyre::eyre::eyre!("OPENROUTER_API_KEY not set"))?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(crate::test_harness::default_headers())
            .build()?;

        let url = op
            .base_url
            .join(&format!(
                "models/{}/{}/endpoints",
                self.author.as_ref(),
                self.model.as_ref()
            ))
            .map_err(|e| color_eyre::eyre::eyre!("Malformed URL: {}", e))?;

        let resp = client
            .get(url)
            .bearer_auth(op.key)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?;
        let v = resp.json::<serde_json::Value>().await?;
        Ok(v)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(author = ?self.author, model = ?self.model))]
    async fn persist_resp_raw(&self) -> color_eyre::Result<()> {
        use std::fs;
        use std::io::Write;
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = ploke_test_utils::workspace_root();
        let mut dir = root.clone();
        dir.push(MODEL_ENDPOINT_RESP_DIR);
        dir.push(self.author.to_string());
        fs::create_dir_all(&dir)?;

        // Timestamp suffix for filename
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut path = dir.clone();
        path.push(format!("{}-{}.json", self.model.as_ref(), ts));

        let v = self.call_endpoint_raw().await?;
        let mut f = fs::File::create(&path)?;
        let s = serde_json::to_string_pretty(&v)?;
        f.write_all(s.as_bytes())?;
        tracing::info!("saved endpoint response to {}", path.display());
        Ok(())
    }

    // TODO: Think about what else we need to add. We are going to need at least one function here
    // that will lean on our strong typing (created for this purpose) in `../providers.rs` and in
    // this file.
    // - Show me a first pass
    // - keep to zero-copy/alloc where possible (not always possible with the given structs here,
    // but we don't want to allocate needlessly)
}

// TODO: (After confirming shape of response data)
//  - Need

impl<'de> Deserialize<'de> for ProvEnd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // splits to return at most three items, since the pattern is like:
        // `deepinfra/fp8/deepseek-r1`

        // let (author_str, rem) = s.split_n('/');
        // let (maybe_quant, model_str) = s
        //     .rsplit_once('/')
        //     .ok_or_else(|| serde::de::Error::custom("missing model in canonical_slug"))?;

        // let author_str = parts
        //     .next()
        //     .ok_or_else(|| serde::de::Error::custom("missing author in canonical_slug"))?;
        // let model_str = parts
        //     .next()
        //     .ok_or_else(|| serde::de::Error::custom("missing model in canonical_slug"))?;

        // splits to return at most three items, since the pattern can be like:
        // `deepinfra/fp8/deepseek-chat-v3`
        //  or 
        // `novita/deepseek-chat-v3`
        let mut parts = s.split('/');
        let author_str = parts
            .next()
            .ok_or_else(|| serde::de::Error::custom("missing author in canonical_slug"))?;
        let model_str = parts
            .next_back()
            .ok_or_else(|| serde::de::Error::custom("missing model in canonical_slug"))?;
        Ok(ProvEnd {
            author: ArcStr::from(author_str),
            model: ArcStr::from(model_str),
        })
    }
}

impl Serialize for ProvEnd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize back to canonical slug form "author/model"
        let s = format!("{}/{}", self.author.as_ref(), &self.model.as_ref());
        serializer.serialize_str(&s)
    }
}

impl std::fmt::Display for ProvEnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.author.as_ref(), &self.model.as_ref())
    }
}

#[derive(Debug)]
pub enum ProvEndParseError {
    NoPathSegments,
    BadPrefix, // not /api/v1/models/...
    MissingProviderSlug,
    MissingModel,
    BadSuffix,        // not .../endpoints
    TrailingSegments, // extra segments beyond expected
}

impl std::fmt::Display for ProvEndParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvEndParseError::NoPathSegments => write!(f, "URL has no path segments"),
            ProvEndParseError::BadPrefix => write!(f, "expected path to start with /api/v1/models"),
            ProvEndParseError::MissingProviderSlug => write!(f, "missing author segment"),
            ProvEndParseError::MissingModel => write!(f, "missing model segment"),
            ProvEndParseError::BadSuffix => write!(f, "expected path to end with /endpoints"),
            ProvEndParseError::TrailingSegments => write!(f, "unexpected trailing path segments"),
        }
    }
}
impl std::error::Error for ProvEndParseError {}

impl TryFrom<&Url> for ProvEnd {
    type Error = ProvEndParseError;

    fn try_from(u: &Url) -> Result<Self, Self::Error> {
        // Percent-decoded, non-allocating iterator over segments.
        let mut segs = u.path_segments().ok_or(ProvEndParseError::NoPathSegments)?;

        // Validate fixed prefix: /api/v1/models
        match (segs.next(), segs.next(), segs.next()) {
            (Some("api"), Some("v1"), Some("models")) => {}
            _ => return Err(ProvEndParseError::BadPrefix),
        }

        // {author}
        let author_str = segs.next().ok_or(ProvEndParseError::MissingProviderSlug)?;

        // {model}
        let model_str: &str = segs.next().ok_or(ProvEndParseError::MissingModel)?;
        let model_arc: ArcStr = ArcStr::from(model_str);

        // suffix must be "endpoints"
        match segs.next() {
            Some("endpoints") => {}
            _ => return Err(ProvEndParseError::BadSuffix),
        }

        // there must be nothing after "endpoints"
        if segs.next().is_some() {
            return Err(ProvEndParseError::TrailingSegments);
        }

        Ok(ProvEnd {
            author: ArcStr::from(author_str),
            model: model_arc,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Typed response to deserialize the response from:
/// https://openrouter.ai/api/v1/models
pub struct ModelsEndpointResponse {
    pub endpoint: Vec<ModelsEndpoint>,
}

impl ModelsEndpointResponse {
    /// Returns the static URL for the OpenRouter models endpoint.
    ///
    /// This URL points to the official OpenRouter API endpoint that lists all available models.
    pub fn url() -> Url {
        OPENROUTER_MODELS_URL.clone()
    }
}

// WARN: This is not really used anywhere, I'm not sure if that is because it is the shape of the
// response from the call to the `/endpoints` or not. Need more information, leaving it here for
// now.
// DO NOT DELETE!
// UPDATE: After examining project, it appears we are using this struct, but perhaps erroneously?
// Hard to say. Very possibly this is causing serious errors we have encountered lately in API
// calls within the App's tool calling loops that prompted us to start refactoring the API + Tool
// system in the first place.
///// Container for the list of model endpoints returned by the OpenRouter API.

// OpenRouter `https://openrouter.ai/api/v1/models` example.
//
// {
//   "data": [
//     {
//       "id": "openrouter/sonoma-dusk-alpha",
//       "canonical_slug": "openrouter/sonoma-dusk-alpha",
//       "hugging_face_id": "",
//       "name": "Sonoma Dusk Alpha",
//       "created": 1757093247,
//       "description": "This is a cloaked model provided to the community to gather feedback. A fast and intelligent general-purpose frontier model with a 2 million token context window. Supports image inputs and parallel tool calling.\n\nNote: It’s free to use during this testing period, and prompts and completions are logged by the model creator for feedback and training.",
//       "context_length": 2000000,
//       "architecture": {
//         "modality": "text+image->text",
//         "input_modalities": [
//           "text",
//           "image"
//         ],
//         "output_modalities": [
//           "text"
//         ],
//         "tokenizer": "Other",
//         "instruct_type": null
//       },
//       "pricing": {
//         "prompt": "0",
//         "completion": "0",
//         "request": "0",
//         "image": "0",
//         "web_search": "0",
//         "internal_reasoning": "0"
//       },
//       "top_provider": {
//         "context_length": 2000000,
//         "max_completion_tokens": null,
//         "is_moderated": false
//       },
//       "per_request_limits": null,
//       "supported_parameters": [
//         "max_tokens",
//         "response_format",
//         "structured_outputs",
//         "tool_choice",
//         "tools"
//       ]
//     },

/// Represents a single model endpoint from OpenRouter's API.
///
/// This is the response shape from: `https://openrouter.ai/api/v1/models`
/// After doing some analysis on the data on Aug 29, 2025, the following fields have some nuance:
///     - hugging_face_id: missing for 43/323 models
///     - top_provider.max_completion_tokens: missing ~half the time, 151/323
///     - architecture.instruct_type: missing for most (~65%), 208/323
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsEndpoint {
    /// canonical endpoint name (author/slug), e.g. deepseek/deepseek-chat-v3.1
    pub id: String,
    /// User-friendly name, e.g. DeepSeek: DeepSeek V3.1
    pub name: String,
    /// Unix timestamp, e.g. 1755779628
    // TODO: Get serde to deserialize into proper type
    pub created: i64,
    /// User-facing description. Kind of long.
    pub description: String,
    /// Things like tokenizer, modality, etc. See `Architecture` struct.
    pub architecture: Architecture,
    /// Top provider info (often carries context length when model-level is missing).
    #[serde(default)]
    pub top_provider: TopProvider,
    /// Input/output pricing; maps from OpenRouter's prompt/completion when present.
    #[serde(default)]
    pub pricing: ModelPricing,
    /// For example:
    /// - "canonical_slug": "qwen/qwen3-30b-a3b-thinking-2507",
    /// - "canonical_slug": "x-ai/grok-code-fast-1",
    /// - "canonical_slug": "nousresearch/hermes-4-70b",
    #[serde(rename = "canonical_slug", default)]
    pub canonical: Option<ProvEnd>,
    /// Context window size if known (model-level).
    #[serde(default)]
    pub context_length: Option<u32>,
    /// Presumably the huggingface model card
    #[serde(default)]
    pub hugging_face_id: Option<String>,
    /// null on all values so far, but it is there in the original so I'll include it.
    #[serde(default)]
    pub per_request_limits: Option<HashMap<String, serde_json::Value>>,
    /// Parameters supported as options by this endpoint, includes things like:
    /// - tools
    /// - top_k
    /// - stop
    /// - include_reasoning
    ///
    /// See SupportedParameters for full enum of observed values.
    /// (also appears in endpoints)
    #[serde(default)]
    pub supported_parameters: Option<Vec<SupportedParameters>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsEndpointsData {
    /// List of available model endpoints from OpenRouter.
    pub data: Vec<ModelsEndpoint>,
}

pub(crate) trait SupportsTools {
    fn supports_tools(&self) -> bool;
}

impl SupportsTools for &[SupportedParameters] {
    fn supports_tools(&self) -> bool {
        self.contains(&SupportedParameters::Tools)
    }
}
impl SupportsTools for &Vec<SupportedParameters> {
    fn supports_tools(&self) -> bool {
        self.contains(&SupportedParameters::Tools)
    }
}

impl SupportsTools for ModelsEndpoint {
    fn supports_tools(&self) -> bool {
        self.supported_parameters
            .as_ref()
            .is_some_and(|sp| sp.supports_tools())
    }
}

/// Parameters supported by OpenRouter models.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SupportedParameters {
    FrequencyPenalty,
    IncludeReasoning,
    LogitBias,
    Logprobs,
    MaxTokens,
    MinP,
    PresencePenalty,
    Reasoning,
    RepetitionPenalty,
    ResponseFormat,
    Seed,
    Stop,
    StructuredOutputs,
    Temperature,
    ToolChoice,
    Tools,
    TopA,
    TopK,
    TopLogprobs,
    TopP,
    WebSearchOptions,
}

/// Architecture details of a model, including input/output modalities and tokenizer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Architecture {
    /// Input modalities supported by this model (text, image, audio, video).
    pub input_modalities: Vec<InputModality>,
    pub output_modalities: Vec<OutputModality>,
    pub tokenizer: Tokenizer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruct_type: Option<InstructType>,
}

/// Possible input modalities that a model can accept.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModality {
    Text,
    Image,
    Audio,
    Video,
    File,
}

/// Possible output modalities that a model can produce.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputModality {
    Text,
    Image,
    Audio, // no endpoints actually have the audio field?
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Modality {
    #[serde(rename = "text->text")]
    TextToText,
    #[serde(rename = "text+image->text")]
    TextImageToText,
    #[serde(rename = "text+image->text+image")]
    TextImageToTextImage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Tokenizer {
    Claude,
    Cohere,
    DeepSeek,
    GPT,
    Gemini,
    Grok,
    Llama2,
    Llama3,
    Llama4,
    Mistral,
    Nova,
    Other,
    Qwen,
    Qwen3,
    Router,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InstructType {
    #[serde(rename = "qwq")]
    Qwq,
    #[serde(rename = "phi3")]
    Phi3,
    #[serde(rename = "vicuna")]
    Vicuna,
    #[serde(rename = "qwen3")]
    Qwen3,
    #[serde(rename = "code-llama")]
    CodeLlama,
    #[serde(rename = "deepseek-v3.1")]
    DeepSeekV31,
    #[serde(rename = "chatml")]
    ChatML,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "airoboros")]
    Airoboros,
    #[serde(rename = "deepseek-r1")]
    DeepSeekR1,
    #[serde(rename = "llama3")]
    Llama3,
    #[serde(rename = "gemma")]
    Gemma,
    #[serde(rename = "alpaca")]
    Alpaca,
    #[serde(rename = "none")]
    None,
}

/// Provider-specific information about the model.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopProvider {
    /// Whether this model is subject to content moderation.
    pub(crate) is_moderated: bool,
    pub(crate) context_length: Option<u32>,
    pub(crate) max_completion_tokens: Option<u64>,
}

// --- serde helpers for flexible number-or-string fields ---

fn de_f64_from_str_or_num<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| {
            <D::Error as serde::de::Error>::custom("number not representable as f64")
        }),
        serde_json::Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map_err(<D::Error as serde::de::Error>::custom),
        serde_json::Value::Null => Ok(0.0),
        other => Err(<D::Error as serde::de::Error>::custom(format!(
            "expected number or string, got {}",
            other
        ))),
    }
}

fn ser_f64_as_string<S>(v: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "todo"]
    async fn mock_test() -> color_eyre::Result<()> {
        // placeholder
        Ok(())
    }

    // #[tokio::test]
    // #[cfg(feature = "live_api_tests")]
    // async fn test_call_known_model() -> color_eyre::Result<()> {
    //     let Some(_op) = crate::test_harness::openrouter_env() else {
    //         eprintln!("Skipping live test: OPENROUTER_API_KEY not set");
    //         return Ok(());
    //     };
    //     let prov = ProvEnd { author: crate::llm::providers::ProviderSlug::qwen, model: ArcStr::from("qwen3-30b-a3b-thinking-2507") };
    //     let v = prov.call_endpoint_raw().await?;
    //     assert!(v.get("data").is_some(), "live response missing data");
    //     Ok(())
    // }
    //
    // #[tokio::test]
    // #[cfg(feature = "live_api_tests")]
    // async fn test_persist_resp_raw_creates_file() -> color_eyre::Result<()> {
    //     let Some(_op) = crate::test_harness::openrouter_env() else {
    //         eprintln!("Skipping live test: OPENROUTER_API_KEY not set");
    //         return Ok(());
    //     };
    //     let prov = ProvEnd { author: crate::llm::providers::ProviderSlug::qwen, model: ArcStr::from("qwen3-30b-a3b-thinking-2507") };
    //     prov.persist_resp_raw().await?;
    //     let mut dir = ploke_test_utils::workspace_root();
    //     dir.push(MODEL_ENDPOINT_RESP_DIR);
    //     dir.push(prov.author.to_string());
    //     assert!(dir.exists(), "output dir does not exist");
    //     Ok(())
    // }

    #[test]
    fn prov_end_from_canonical_slug() {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(rename = "canonical_slug")]
            canonical: ProvEnd,
        }
        let w: Wrapper =
            serde_json::from_str("{\"canonical_slug\":\"qwen/qwen3-30b-a3b-thinking-2507\"}")
                .expect("parse provend");
        assert_eq!(w.canonical.author.to_string(), "qwen");
        assert_eq!(&*w.canonical.model, "qwen3-30b-a3b-thinking-2507");
        let s = serde_json::to_string(&w.canonical).unwrap();
        assert_eq!(s.trim_matches('"'), "qwen/qwen3-30b-a3b-thinking-2507");
    }

    #[test]
    #[ignore]
    fn prov_end_try_from_url() {
        let url = reqwest::Url::parse(
            "https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b-thinking-2507/endpoints",
        )
        .unwrap();
        let pe = ProvEnd::try_from(&url).expect("try_from url");
        assert_eq!(pe.author.to_string(), "qwen");
        assert_eq!(&*pe.model, "qwen3-30b-a3b-thinking-2507");
    }
}
