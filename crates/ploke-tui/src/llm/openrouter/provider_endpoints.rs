use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::llm::providers::Author;
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
pub struct ProvEnd {
    pub author: Author,
    pub model: Arc<str>,
}

impl ProvEnd {
    async fn call_endpoint(&self) {
        // TODO: Call the endpoint at the url following the pattern in the notes above.
        // - as near zero-alloc as possible
        // - zero-copy where possible, but prefer zero-alloc
        // - defer handling error codes for now, need another function for that (not within this
        // impl, perhaps a trait). See `llm/mod.rs` for some known errors form OpenRouter +
        // providers
    }

    // AI: add tracing for sanity check using `instrument`
    async fn persist_resp_raw(&self) -> color_eyre::Result<()>{
        let mut json_outfile = workspace_root();
        json_outfile.push(MODEL_ENDPOINT_RESP_DIR);
        // TODO:
        //  - deserialize to serde_json_value
        //  - push author/model once known
        //      - create new dir with name of `author`
        //      - file should have name `model` + short timestamp and be .json
        //  - use tracing to log saved filepath
        //  - note: this will help us understand the response type from the API
        todo!()
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



#[derive(Debug)]
pub enum ProvEndParseError {
    NoPathSegments,
    BadPrefix, // not /api/v1/models/...
    MissingAuthor,
    InvalidAuthor(String),
    MissingModel,
    BadSuffix,        // not .../endpoints
    TrailingSegments, // extra segments beyond expected
}

impl std::fmt::Display for ProvEndParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ProvEndParseError::*;
        match self {
            NoPathSegments => write!(f, "URL has no path segments"),
            BadPrefix => write!(f, "expected path to start with /api/v1/models"),
            MissingAuthor => write!(f, "missing author segment"),
            InvalidAuthor(s) => write!(f, "invalid author segment: {}", s),
            MissingModel => write!(f, "missing model segment"),
            BadSuffix => write!(f, "expected path to end with /endpoints"),
            TrailingSegments => write!(f, "unexpected trailing path segments"),
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
            .map_err(|_| ProvEndParseError::InvalidAuthor(author_str.to_owned()))?;

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
    // TODO: Use this for now, then reconsider once we have the download from the call to
    // `/endpoints` if this is just a duplicate of a similar struct in `openrouter_catalog` or if
    // there is an actual reason for this to be different.
    // - need to see response shape before making decision
    pub data: Vec<ModelEndpoint>
    // WARN: DO NOT DELETE!
    // pub data: ModelEndpointsData,
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
///// Container for the list of model endpoints returned by the OpenRouter API.
//#[derive(Debug, Clone, Serialize, Deserialize)]
//pub struct ModelEndpointsData {
//    /// List of available model endpoints from OpenRouter.
//    pub endpoints: Vec<ModelEndpoint>,
//}

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
    #[serde(default)]
    /// For example:
    /// - "canonical_slug": "qwen/qwen3-30b-a3b-thinking-2507",
    /// - "canonical_slug": "x-ai/grok-code-fast-1",
    /// - "canonical_slug": "nousresearch/hermes-4-70b",
    pub canonical_slug: String,
    #[serde(default)]
    pub context_length: u64,
    #[serde(default)]
    pub hugging_face_id: Option<String>,
    #[serde(default)]
    pub per_request_limits: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub supported_parameters: Vec<SupportedParameters>,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Pricing {
    // Accept either numbers or strings from the API, but keep a numeric type internally.
    #[serde(deserialize_with = "string_or_f64")]
    prompt: f64,
    #[serde(deserialize_with = "string_or_f64")]
    completion: f64,
    #[serde(
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    image: Option<f64>,
    #[serde(
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    request: Option<f64>,
    #[serde(
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    web_search: Option<f64>,
    #[serde(
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    internal_reasoning: Option<f64>,
    #[serde(
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    input_cache_read: Option<f64>,
    #[serde(
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    input_cache_write: Option<f64>,
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

/// Lightweight getters so callers can compute price hints without poking at serde internals.
impl Pricing {
    pub fn prompt_or_default(&self) -> f64 {
        self.prompt
    }
    pub fn completion_or_default(&self) -> f64 {
        self.completion
    }
}

#[cfg(test)]
mod tests {
    use reqwest::{Client, ClientBuilder};

    #[tokio::test]
    #[ignore = "todo"]
    async fn mock_test() -> color_eyre::Result<()> {
        // test formation of URL + Client
        todo!()
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_call_known_model() -> color_eyre::Result<()> {
        let author = "qwen";
        let model = "qwen3-30b-a3b-thinking-2507";
        // test real call against endpoint w/ 200 response expected
        // use Client/ClientBuilder
        todo!()
    }

    // TODO: Add a test that will call a model and save the output to a file with `persist_resp_raw` defined in an env
    // + gate behind cfg "live_api_tests"
    
    // TODO: Add a test that will load all items from workspace_root() + `REL_MODEL_ALL_DATA` and
    // use serde_json to load them as a `ModelEndpointData`, then iterates over all of them to
    // verify that we can parse a `ProvEnd` from them. 
}
