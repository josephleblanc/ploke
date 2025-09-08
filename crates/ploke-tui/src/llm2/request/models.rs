use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::llm2::{
    Architecture, SupportedParameters, newtypes::ModelId, router_only::openrouter::TopProvider,
};

use super::ModelPricing;

/// Represents a model `/models` from OpenRouter's API.
/// https://openrouter.ai/api/v1/models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    /// List of available model endpoints from OpenRouter.
    pub data: Vec<ModelResponseItem>,
}

/// Represents a single model endpoint from OpenRouter's API.
///
/// This is the response shape from: `https://openrouter.ai/api/v1/models`
/// After doing some analysis on the data on Aug 29, 2025, the following fields have some nuance:
///     - hugging_face_id: missing for 43/323 models
///     - top_provider.max_completion_tokens: missing ~half the time, 151/323
///     - architecture.instruct_type: missing for most (~65%), 208/323
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponseItem {
    /// canonical endpoint name (author/slug), e.g. deepseek/deepseek-chat-v3.1
    pub id: ModelId,
    /// User-friendly name, e.g. DeepSeek: DeepSeek V3.1
    pub name: ArcStr,
    /// Unix timestamp, e.g. 1755779628
    // TODO: Get serde to deserialize into proper type
    pub created: i64,
    /// User-facing description. Kind of long.
    pub description: ArcStr,
    /// Things like tokenizer, modality, etc. See `Architecture` struct.
    pub architecture: Architecture,
    /// Top provider info (often carries context length when model-level is missing).
    #[serde(default)]
    pub top_provider: TopProvider,
    /// Input/output pricing; maps from OpenRouter's prompt/completion when present.
    pub pricing: ModelPricing,
    /// For example:
    /// - "canonical_slug": "qwen/qwen3-30b-a3b-thinking-2507",
    /// - "canonical_slug": "x-ai/grok-code-fast-1",
    /// - "canonical_slug": "nousresearch/hermes-4-70b",
    #[serde(rename = "canonical_slug", default)]
    pub canonical: Option<ModelId>,
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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ploke_core::ArcStr;
    use super::*;

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
        
        let response: ModelsResponse = serde_json::from_value(js).unwrap();
        assert_eq!(response.data.len(), 1);
        
        let model = &response.data[0];
        
        // Test architecture fields
        assert_eq!(model.architecture.input_modalities, vec!["image", "text"]);
        assert_eq!(model.architecture.modality, "text+image->text");
        assert_eq!(model.architecture.output_modalities, vec!["text"]);
        assert_eq!(model.architecture.tokenizer, "Llama4");
        assert_eq!(model.architecture.instruct_type, None);
        
        // Test model identification
        assert_eq!(model.id.as_str(), "deepcogito/cogito-v2-preview-llama-109b-moe");
        assert_eq!(model.name.as_str(), "Cogito V2 Preview Llama 109B");
        assert_eq!(model.created, 1756831568);
        assert_eq!(model.description.as_str(), "An instruction-tuned, hybrid-reasoning Mixture-of-Experts model built on Llama-4-Scout-17B-16E. Cogito v2 can answer directly or engage an extended \"thinking\" phase, with alignment guided by Iterated Distillation & Amplification (IDA). It targets coding, STEM, instruction following, and general helpfulness, with stronger multilingual, tool-calling, and reasoning performance than size-equivalent baselines. The model supports long-context use (up to 10M tokens) and standard Transformers workflows. Users can control the reasoning behaviour with the `reasoning` `enabled` boolean. [Learn more in our docs](https://openrouter.ai/docs/use-cases/reasoning-tokens#enable-reasoning-with-default-config)");
        assert_eq!(model.canonical.as_ref().unwrap().as_str(), "deepcogito/cogito-v2-preview-llama-109b-moe");
        
        // Test metadata
        assert_eq!(model.context_length, Some(32767));
        assert_eq!(model.hugging_face_id.as_ref().unwrap(), "deepcogito/cogito-v2-preview-llama-109B-MoE");
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
        ];
        assert_eq!(model.supported_parameters.as_ref().unwrap(), &expected_params);
        
        // Test top provider
        assert_eq!(model.top_provider.context_length, Some(32767));
        assert_eq!(model.top_provider.is_moderated, false);
        assert_eq!(model.top_provider.max_completion_tokens, None);
    }
}
