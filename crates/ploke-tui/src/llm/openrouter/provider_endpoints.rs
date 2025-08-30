use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::utils::consts::OPENROUTER_BASE_STR;
use crate::utils::consts::OPENROUTER_ENDPOINT_STR;
use crate::utils::de::string_or_f64;
use crate::utils::de::string_or_f64_opt;
use lazy_static::lazy_static;
use ploke_test_utils::workspace_root;
use reqwest::Url;
use serde::{Deserialize, Serialize};

const MODEL_ENDPOINT_RESP_DIR: &str = "crates/ploke-tui/data/endpoints/";

lazy_static! {
    pub static ref OPENROUTER_MODELS_URL: Url =
        Url::parse("https://openrouter.ai/api/v1/models").expect("Invalid OpenRouter models URL");
}

use std::{convert::TryFrom, str::FromStr};

// TODO: This is the shape of the response from the call for all models. 
// Next we need to: 
// Agent TODO (starting with TODOs in this file to help identify where to start following points):
//  - (ongoing): Update impl-logs with reasoning + steps taken ongoing.
//      - request human input when blockers encountered and/or instructions too unclear to
//      implement, create report explaining why blocker cannot be solved independently and requires
//      human input, bring questions, attempt to resolve and continue, if not possible stop and
//      request human input
//      - request human input when tests needed behind cfg gating
//      - otherwise continue working
//  - ensure we can deserialize the response well (e.g. all types take expected values + test)
//      - Can start with the values we care about most for now, 
//          - "supported_parameters" -> SupportedParameters
//          - "pricing" -> Pricing
//          - "canonical_slug" -> ProvEnd
//      - keep allocations as low as possible
//      - leverage serde where possible (should completely deserialize)
//  - take the "canonical_slug" or "id" and get all the models.
//  - save all the models called to a single file
//  - use the `crates/ploke-tui/src/llm/openrouter/json_visitor.rs` functions to analyze the shape
//  of the response across models
//  - If shape is the same/similar to ModelEndpoint, then either use ModelEndpoint to test
//  deserailization (if same) or create a similar struct (if different) and test same.
//  - Create a persistent Model registry (we have a semi-working version now, but it is not
//  grounded in the truth of the API expectations)
//  - Transform response + filter providers/sort for desired fields
//      - Use offical docs on API saved in `crates/ploke-tui/docs/openrouter/request_structure.md`
//  - Develop a set of tests to make sure endpoint responses come back as expected.
//      - happy paths
//      - requests we expect to fail
//      - gate behind cfg feature "live_api_tests"
//  - Add documentation to all items. Create module-level documentation on API structure, expected
//  values, use-cases, examples, etc.
//  - Evaluate and streamline:
//      - add benchmarks, both online/offline
//      - record benches
//      - profile performance for later comparison
//      - smooth any super jagged edges
//  - Evaluate current handling of endpoint cycle in `ploke-tui` crate so far, and identify how to
//  streamline to make more ergonomic, simplify call sites where possible while improving
//  performance.
//  - Migrate system to use new approach
//      - slash and burn for old approach where tests are repeated.
//      - replace e2e tests with approach using gated TEST_APP in `test_harness.rs` behind
//      `#[cfg(feature = "test_harness")]` for realistic end-to-end testing with multithreaded
//      event system.
//      - include snapshot testing, ensure UI/UX does not regress
//  - TBD
//
//  Human:
//  - Integrate and/or build out trait-based Tool calling system, starting with
//  `request_more_context` tool that uses vector similarity + bm25 search
//      - test new trait system in unit tests
//      - test e2e with TEST_APP and live API calls
//      - if trait system valid, extend to other tools + refine approach
//  - expand db methods for targeted code context search
//      - get neighboring code items in module tree
//      - get all code items in current file
//  - expand tool calls
//      - add tests + benches
//  - invest more design time into agentic system (not yet created)
//      - overall simple loops
//      - prompt decomposition
//      - planning
//      - revisit tool design, re-evaluate current system
//  - ensure current API system works as expected, and that we can make the expected calls
//      - agent TODO list above finished
//      - UI smoothed out for selecting model (currently buggy re: selecting model provider)
//      - accurate + comprehensive model registry exists
//      - API tested + validated, shapes of responses recorded, strong typing on all
//      request/response schema for ergonimic use and mutation (filter, destructure, etc)
//      - performant (efficient, low alloc, no dynamic dispatch, static dispatch)
//  - fill out tools + API calls into working, complete system
//      - e2e tests exist and validate all testable properties offline
//      - e2e + live tests exist and validate all testable properties online on a wide variety of
//      endpoints
//      - tests for happy + fail paths, observe expected defined errors where expected
//      - snapshots and UI + UX are good, hotkeys exist, simple interactions in live TUI are good
//  - revisit context management, arrive at clear design for a functioning memory system
//      - implement memory system using db as primary storage
//      - add observability tools (already written but need tests + integration)
//  - integrate memory system with workflow, ensure modular + actor design maintains integrity or
//      improves on integrity + organization (somewhat rats-nest of CommandState + AppEvent +
//      EventBus)
//  - revisit safety system + decide on sandboxing environment
//      - integrate + test + TBD
//  - begin using agents
//      - refine + test + bench
//          - prompts
//          - observability
//          - task complexity
//      - experiment with agent organization systems
//      - parallel agentic execution (branching + batched conversations)
//  - begin deploying ploke-defined agents to improve ploke itself
//      - start of self-evolutionary loop
//      - start with refactors + clean up code base
//      - extend features, e.g. 80% complete type resolution -> full implementation
//  - revisit design of user profile creation + maintenance
//      - integrate tools + memory
//      - unify design
//      - experiment
//
//
// {
//   "data": [
//     {
//       "architecture": {
//         "input_modalities": [
//           "text"
//         ],
//         "instruct_type": null,
//         "modality": "text->text",
//         "output_modalities": [
//           "text"
//         ],
//         "tokenizer": "Qwen3"
//       },
//       "canonical_slug": "qwen/qwen3-30b-a3b-thinking-2507",
//       "context_length": 262144,
//       "created": 1756399192,
//       "description": "Qwen3-30B-A3B-Thinking-2507 is a 30B parameter Mixture-of-Experts reasoning model optimized for complex tasks requiring extended multi-step thinking. The model is designed specifically for “thinking mode,” where internal reasoning traces are separated from final answers.\n\nCompared to earlier Qwen3-30B releases, this version improves performance across logical reasoning, mathematics, science, coding, and multilingual benchmarks. It also demonstrates stronger instruction following, tool use, and alignment with human preferences. With higher reasoning efficiency and extended output budgets, it is best suited for advanced research, competitive problem solving, and agentic applications requiring structured long-context reasoning.",
//       "hugging_face_id": "Qwen/Qwen3-30B-A3B-Thinking-2507",
//       "id": "qwen/qwen3-30b-a3b-thinking-2507",
//       "name": "Qwen: Qwen3 30B A3B Thinking 2507",
//       "per_request_limits": null,
//       "pricing": {
//         "completion": "0.0000002852",
//         "image": "0",
//         "internal_reasoning": "0",
//         "prompt": "0.0000000713",
//         "request": "0",
//         "web_search": "0"
//       },
//       "supported_parameters": [
//         "frequency_penalty",
//         "include_reasoning",
//         "logit_bias",
//         "logprobs",
//         "max_tokens",
//         "min_p",
//         "presence_penalty",
//         "reasoning",
//         "repetition_penalty",
//         "response_format",
//         "seed",
//         "stop",
//         "temperature",
//         "tool_choice",
//         "tools",
//         "top_k",
//         "top_logprobs",
//         "top_p"
//       ],
//       "top_provider": {
//         "context_length": 262144,
//         "is_moderated": false,
//         "max_completion_tokens": 262144
//       }
//     },
//     // There are 15k lines in the original file, this is a semi-typical example
//   ]
// }

// From OpenRouter docs:
//  - https://openrouter.ai/api/v1/models/author/slug/endpoints
//  NOTE: Naming here is extremely confusing. What is referred to as an "author" here is called
//  a "slug" from the providers endpoint. We are going with "author" instead, and "model"
//  instead of the "author", such that
//  - "author/slug" -> "author/model"
//  - https://openrouter.ai/api/v1/models/author/slug/endpoints
//      -> https://openrouter.ai/api/v1/models/author/model/endpoints
//  - https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b-thinking-2507/endpoints
//      - author:   qwen
//      - model:    qwen3-30b-a3b-thinking-2507
//
/// A more strongly typed and segmented type that can be used to make a call to a model endpoint
/// with OpenRouter's API
// TODO: need to implement Deserialize on ProvEnd, likely a custom implementation so we can
// deserialize the `canonical_slug` from the json files into `ProvEnd`.
#[derive(Debug, Clone)]
pub struct ProvEnd {
    pub author: crate::llm::providers::Author,
    pub model: Arc<str>,
}

impl ProvEnd {
    #[tracing::instrument(level = "debug", skip_all, fields(author = %self.author, model = %self.model))]
    pub(crate) async fn call_endpoint_raw(&self) -> color_eyre::Result<serde_json::Value> {
        // Use test harness/env helper to obtain URL and API key (dotenv fallback allowed)
        let op = crate::test_harness::openrouter_env()
            .ok_or_else(|| color_eyre::eyre::eyre!("OPENROUTER_API_KEY not set"))?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(crate::test_harness::default_headers())
            .build()?;

        let url = op
            .url
            .join(&format!("models/{}/{}/endpoints", self.author, self.model))
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

    #[tracing::instrument(level = "debug", skip_all, fields(author = %self.author, model = %self.model))]
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
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut path = dir.clone();
        path.push(format!("{}-{}.json", &self.model, ts));

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
        let mut parts = s.splitn(2, '/');
        let author_str = parts
            .next()
            .ok_or_else(|| serde::de::Error::custom("missing author in canonical_slug"))?;
        let model_str = parts
            .next()
            .ok_or_else(|| serde::de::Error::custom("missing model in canonical_slug"))?;
        let author = Author::from_str(author_str).map_err(|_| serde::de::Error::custom(format!("invalid author: {}", author_str)))?;
        Ok(ProvEnd { author, model: Arc::<str>::from(model_str) })
    }
}

impl Serialize for ProvEnd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize back to canonical slug form "author/model"
        let s = format!("{}/{}", self.author, &self.model);
        serializer.serialize_str(&s)
    }
}

impl std::fmt::Display for ProvEnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.author, &self.model)
    }
}
use crate::llm::providers::Author;
#[derive(Debug)]
pub enum ProvEndParseError {
    NoPathSegments,
    BadPrefix, // not /api/v1/models/...
    MissingAuthor,
    MissingModel,
    BadSuffix,        // not .../endpoints
    TrailingSegments, // extra segments beyond expected
}

impl std::fmt::Display for ProvEndParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvEndParseError::NoPathSegments => write!(f, "URL has no path segments"),
            ProvEndParseError::BadPrefix => write!(f, "expected path to start with /api/v1/models"),
            ProvEndParseError::MissingAuthor => write!(f, "missing author segment"),
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
        let author_str = segs.next().ok_or(ProvEndParseError::MissingAuthor)?;
        let author = Author::from_str(author_str)
            .map_err(|_| ProvEndParseError::MissingAuthor)?;

        // {model}
        let model_str: &str = segs.next().ok_or(ProvEndParseError::MissingModel)?;
        let model_arc: Arc<str> = Arc::<str>::from(model_str);

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
            author,
            model: model_arc,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Typed response to deserialize the response from:
/// https://openrouter.ai/api/v1/endpoints
pub struct ModelEndpointsResponse {
    pub data: ModelEndpointsData,
}

impl ModelEndpointsResponse {
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


/// Represents a single model endpoint from OpenRouter's API.
///
/// After doing some analysis on the data on Aug 29, 2025, the following fields have some nuance:
///     - hugging_face_id: missing for 43/323 models
///     - top_provider.max_completion_tokens: missing ~half the time, 151/323
///     - architecture.instruct_type: missing for most (~65%), 208/323
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEndpoint {
    /// Unique identifier for this model.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    /// Unix timestamp
    // TODO: Get serde to deserialize into proper type
    pub created: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub architecture: Architecture,
    #[serde(default)]
    pub top_provider: TopProvider,
    #[serde(default)]
    pub pricing: Pricing,
    /// For example:
    /// - "canonical_slug": "qwen/qwen3-30b-a3b-thinking-2507",
    /// - "canonical_slug": "x-ai/grok-code-fast-1",
    /// - "canonical_slug": "nousresearch/hermes-4-70b",
    #[serde(rename = "canonical_slug", default)]
    pub canonical: Option<ProvEnd>,
    #[serde(default)]
    pub context_length: u64,
    #[serde(default)]
    pub hugging_face_id: Option<String>,
    #[serde(default)]
    pub per_request_limits: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub supported_parameters: Vec<SupportedParameters>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEndpointsData {
    /// List of available model endpoints from OpenRouter.
    pub endpoints: Vec<ModelEndpoint>,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Architecture {
    /// Input modalities supported by this model (text, image, audio, video).
    pub input_modalities: Vec<InputModality>,
    #[serde(default)]
    pub output_modalities: Vec<OutputModality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<Tokenizer>,
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
}

/// Provider-specific information about the model.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopProvider {
    /// Whether this model is subject to content moderation.
    #[serde(default)]
    is_moderated: bool,
    #[serde(default)]
    context_length: u64,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

/// Pricing information for using the model.
#[deprecated(note = "Use openrouter_catalog::ModelPricing; this alias will be removed after migration.")]
pub type Pricing = crate::llm::openrouter::openrouter_catalog::ModelPricing;

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
    //     let prov = ProvEnd { author: crate::llm::providers::Author::qwen, model: std::sync::Arc::<str>::from("qwen3-30b-a3b-thinking-2507") };
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
    //     let prov = ProvEnd { author: crate::llm::providers::Author::qwen, model: std::sync::Arc::<str>::from("qwen3-30b-a3b-thinking-2507") };
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
        struct Wrapper { #[serde(rename = "canonical_slug")] canonical: ProvEnd }
        let w: Wrapper = serde_json::from_str("{\"canonical_slug\":\"qwen/qwen3-30b-a3b-thinking-2507\"}").expect("parse provend");
        assert_eq!(w.canonical.author.to_string(), "qwen");
        assert_eq!(&*w.canonical.model, "qwen3-30b-a3b-thinking-2507");
        let s = serde_json::to_string(&w.canonical).unwrap();
        assert_eq!(s.trim_matches('"'), "qwen/qwen3-30b-a3b-thinking-2507");
    }

    #[test]
    #[ignore]
    fn prov_end_try_from_url() {
        let url = reqwest::Url::parse("https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b-thinking-2507/endpoints").unwrap();
        let pe = ProvEnd::try_from(&url).expect("try_from url");
        assert_eq!(pe.author.to_string(), "qwen");
        assert_eq!(&*pe.model, "qwen3-30b-a3b-thinking-2507");
    }
}
