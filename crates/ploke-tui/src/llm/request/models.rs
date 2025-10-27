#![allow(clippy::bool_assert_comparison)]
// The Response parsed from API endpoints for the list of models served,
// e.g. for openrouter this is: `https://openrouter.ai/api/v1/models`
//
// ## Re: Tests
// Tests on structs in fields are noted below:
//  - `llm::types::model_types`
//      - `ModelId`
//      - `Architecture`
//  - `llm::router_only::openrouter`
//      - `TopProvider` (todo)
//  - `llm::request`
//      - `ModelPricing`
//  - `llm::types::enums`
//      - `SupportedParameters`
//
//  The other fields are basic types, and `ArcStr` will have its own tests in `ploke_core`.
//  As for all the types listed above, they have tests for their properties as well as reading
//  recorded responses from the `/models` endpoint written to files, as well as tests for
//  deserialization.
use std::collections::HashMap;

use ploke_core::ArcStr;
use ploke_test_utils::workspace_root;
use serde::{Deserialize, Serialize};

use crate::llm::{
        router_only::{openrouter::TopProvider, HasModelId}, types::{model_types::Architecture, newtypes::ModelName}, ModelId, SupportedParameters, SupportsTools
    };

use once_cell::sync::Lazy;
use serde_json::Value;

pub static EXAMPLE_JSON: Lazy<Value> = Lazy::new(|| {
    // Read at compile-time, parse at runtime once.
    static RAW: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/api/data/models/all_raw.json"
    ));

    serde_json::from_str(RAW).expect("valid test JSON")
});

use super::ModelPricing;

/// Represents a model `/models` from OpenRouter's API.
/// https://openrouter.ai/api/v1/models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Response {
    /// List of available model endpoints from OpenRouter.
    pub data: Vec<ResponseItem>,
}

impl IntoIterator for Response {
    type Item = ResponseItem;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }

    type IntoIter = std::vec::IntoIter<ResponseItem>;
}

/// Represents a single model endpoint from OpenRouter's API.
///
/// This is the response shape from: `https://openrouter.ai/api/v1/models`
/// After doing some analysis on the data on Aug 29, 2025, the following fields have some nuance:
///     - hugging_face_id: missing for 43/323 models
///     - top_provider.max_completion_tokens: missing ~half the time, 151/323
///     - architecture.instruct_type: missing for most (~65%), 208/323
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResponseItem {
    /// canonical endpoint name ({author}/{slug}:{variant}), e.g.
    /// - deepseek/deepseek-chat-v3.1
    /// - but also possible: deepseek/deepseek-chat-v3.1:free
    pub(crate) id: ModelId,
    /// User-friendly name, e.g. DeepSeek: DeepSeek V3.1
    /// Examples from responses to OpenRouter API:
    /// id -    "qwen/qwen3-next-80b-a3b-thinking",
    /// name -  "Qwen: Qwen3 Next 80B A3B Thinking"
    pub(crate) name: ModelName,
    /// Unix timestamp, e.g. 1755779628
    // TODO: Get serde to deserialize into proper type
    pub(crate) created: i64,
    /// User-facing description. Kind of long.
    pub(crate) description: ArcStr,
    /// Things like tokenizer, modality, etc. See `Architecture` struct.
    pub(crate) architecture: Architecture,
    /// Top provider info (often carries context length when model-level is missing).
    #[serde(default)]
    pub(crate) top_provider: TopProvider,
    /// Input/output pricing; maps from OpenRouter's prompt/completion when present.
    pub(crate) pricing: ModelPricing,
    /// For example:
    /// - "canonical_slug": "qwen/qwen3-30b-a3b-thinking-2507",
    /// - "canonical_slug": "x-ai/grok-code-fast-1",
    /// - "canonical_slug": "nousresearch/hermes-4-70b",
    #[serde(rename = "canonical_slug", default)]
    pub(crate) canonical: Option<ModelId>,
    /// Context window size if known (model-level).
    #[serde(default)]
    pub(crate) context_length: Option<u32>,
    /// Presumably the huggingface model card
    #[serde(default)]
    pub(crate) hugging_face_id: Option<String>,
    /// null on all values so far, but it is there in the original so I'll include it.
    #[serde(default)]
    pub(crate) per_request_limits: Option<HashMap<String, serde_json::Value>>,
    /// Parameters supported as options by this endpoint, includes things like:
    /// - tools
    /// - top_k
    /// - stop
    /// - include_reasoning
    ///
    /// See SupportedParameters for full enum of observed values.
    /// (also appears in endpoints)
    #[serde(default)]
    pub(crate) supported_parameters: Option<Vec<SupportedParameters>>,
}

impl HasModelId for ResponseItem {
    fn model_id(&self) -> ModelId {
        self.id.clone()
    }
}

impl SupportsTools for ResponseItem {
    fn supports_tools(&self) -> bool {
        self.supported_parameters
            .as_ref()
            .is_some_and(|sp| sp.supports_tools())
    }
}

#[cfg(test)]
mod tests {
    use crate::llm::{InputModality, Modality, OutputModality, SupportedParameters, Tokenizer};
    use ploke_core::ArcStr;
    use serde_json::json;

    use super::super::models;

    #[test]
    fn test_deserialization_all() {
        let js = json!({
          "data": [
            {
              "architecture": {
                "input_modalities": [
                  "image",
                  "text"
                ],
                "instruct_type": null,
                "modality": "text+image->text",
                "output_modalities": [
                  "text"
                ],
                "tokenizer": "Llama4"
              },
              "canonical_slug": "deepcogito/cogito-v2-preview-llama-109b-moe",
              "context_length": 32767,
              "created": 1756831568,
              "description": "An instruction-tuned, hybrid-reasoning Mixture-of-Experts model built on Llama-4-Scout-17B-16E. Cogito v2 can answer directly or engage an extended “thinking” phase, with alignment guided by Iterated Distillation & Amplification (IDA). It targets coding, STEM, instruction following, and general helpfulness, with stronger multilingual, tool-calling, and reasoning performance than size-equivalent baselines. The model supports long-context use (up to 10M tokens) and standard Transformers workflows. Users can control the reasoning behaviour with the `reasoning` `enabled` boolean. [Learn more in our docs](https://openrouter.ai/docs/use-cases/reasoning-tokens#enable-reasoning-with-default-config)",
              "hugging_face_id": "deepcogito/cogito-v2-preview-llama-109B-MoE",
              "id": "deepcogito/cogito-v2-preview-llama-109b-moe",
              "name": "Cogito V2 Preview Llama 109B",
              "per_request_limits": null,
              "pricing": {
                "completion": "0.00000059",
                "image": "0",
                "internal_reasoning": "0",
                "prompt": "0.00000018",
                "request": "0",
                "web_search": "0"
              },
              "supported_parameters": [
                "frequency_penalty",
                "include_reasoning",
                "logit_bias",
                "max_tokens",
                "min_p",
                "presence_penalty",
                "reasoning",
                "repetition_penalty",
                "stop",
                "temperature",
                "tool_choice",
                "tools",
                "top_k",
                "top_p"
              ],
              "top_provider": {
                "context_length": 32767,
                "is_moderated": false,
                "max_completion_tokens": null
              }
            },
          ]
        });

        let response: models::Response = serde_json::from_value(js).unwrap();
        assert_eq!(response.data.len(), 1);

        let model = &response.data[0];

        // Test architecture fields
        assert_eq!(
            model.architecture.input_modalities,
            vec![InputModality::Image, InputModality::Text]
        );
        assert_eq!(model.architecture.modality, Modality::TextImageToText);
        assert_eq!(
            model.architecture.output_modalities,
            vec![OutputModality::Text]
        );
        assert_eq!(model.architecture.tokenizer, Tokenizer::Llama4);
        assert_eq!(model.architecture.instruct_type, None);

        // Test model identification
        assert_eq!(
            model.id.to_string(),
            "deepcogito/cogito-v2-preview-llama-109b-moe"
        );
        assert_eq!(model.name.as_str(), "Cogito V2 Preview Llama 109B");
        assert_eq!(model.created, 1756831568);
        assert_eq!(
            model.description.as_ref(),
            "An instruction-tuned, hybrid-reasoning Mixture-of-Experts model built on Llama-4-Scout-17B-16E. Cogito v2 can answer directly or engage an extended “thinking” phase, with alignment guided by Iterated Distillation & Amplification (IDA). It targets coding, STEM, instruction following, and general helpfulness, with stronger multilingual, tool-calling, and reasoning performance than size-equivalent baselines. The model supports long-context use (up to 10M tokens) and standard Transformers workflows. Users can control the reasoning behaviour with the `reasoning` `enabled` boolean. [Learn more in our docs](https://openrouter.ai/docs/use-cases/reasoning-tokens#enable-reasoning-with-default-config)"
        );
        assert_eq!(
            model.canonical.as_ref().unwrap().to_string(),
            "deepcogito/cogito-v2-preview-llama-109b-moe"
        );

        // Test metadata
        assert_eq!(model.context_length, Some(32767));
        assert_eq!(
            model.hugging_face_id.as_ref().unwrap(),
            "deepcogito/cogito-v2-preview-llama-109B-MoE"
        );
        assert_eq!(model.per_request_limits, None);

        // Test pricing
        let pricing = &model.pricing;
        assert_eq!(pricing.prompt, 0.00000018);
        assert_eq!(pricing.completion, 0.00000059);
        assert_eq!(pricing.image, Some(0.0));
        assert_eq!(pricing.internal_reasoning, Some(0.0));
        assert_eq!(pricing.request, Some(0.0));
        assert_eq!(pricing.web_search, Some(0.0));
        assert_eq!(pricing.audio, None);
        assert_eq!(pricing.input_cache_read, None);
        assert_eq!(pricing.input_cache_write, None);
        assert_eq!(pricing.discount, None);

        // Test supported parameters
        let expected_params = vec![
            SupportedParameters::FrequencyPenalty,
            SupportedParameters::IncludeReasoning,
            SupportedParameters::LogitBias,
            SupportedParameters::MaxTokens,
            SupportedParameters::MinP,
            SupportedParameters::PresencePenalty,
            SupportedParameters::Reasoning,
            SupportedParameters::RepetitionPenalty,
            SupportedParameters::Stop,
            SupportedParameters::Temperature,
            SupportedParameters::ToolChoice,
            SupportedParameters::Tools,
            SupportedParameters::TopK,
            SupportedParameters::TopP,
        ];
        assert_eq!(
            model.supported_parameters.as_ref().unwrap(),
            &expected_params
        );

        // Test top provider
        assert_eq!(model.top_provider.context_length, Some(32767));
        assert_eq!(model.top_provider.is_moderated, false);
        assert_eq!(model.top_provider.max_completion_tokens, None);
    }

    #[test]
    fn test_deserialization_with_fixture() {
        // Use the saved fixture to test deserialization of all variations
        let response: models::Response =
            serde_json::from_value(models::EXAMPLE_JSON.clone()).unwrap();

        // Ensure we can deserialize all items in the data array
        assert!(!response.data.is_empty());

        // Verify each item can be deserialized without errors
        for model in &response.data {
            // Just ensure we have the basic fields accessible
            assert!(!model.id.to_string().is_empty());
            assert!(!model.name.as_str().is_empty());
            assert!(model.created > 0);
        }
    }
}
