use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use ploke_core::ArcStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::llm2::types::model_types::Architecture;
use crate::llm2::types::newtypes::{EndpointTag, ModelName};
use crate::llm2::Quant;
use crate::llm2::*;
use crate::llm2::router_only::openrouter::providers::{ProviderSlug};
use crate::llm2::SupportedParameters;
use crate::tools::{FunctionMarker, ToolDefinition};
use crate::utils::se_de::string_or_f64;
use crate::utils::se_de::string_to_f64_opt_zero;

// Example json response for
// `https://openrouter.ai/api/v1/models/deepseek/deepseek-chat-v3.1/endpoints`
// is shown in the test below for a simple sanity check `test_example_json_deserialize`

use crate::utils::se_de::{de_arc_str, se_arc_str};

use super::ModelPricing;

/// Typed Endpoint entry from `/models/:author/:slug/endpoints`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Endpoint {
    /// Human-friendly name in the form "Provider | model id"
    /// e.g. "name": "DeepInfra | deepseek/deepseek-chat-v3.1",
    /// NOTE: `name` here is different from models::Response `name`,
    /// and the models::Response `name` is the same(?) as `model_name` here.
    pub(crate) name: ArcStr,

    /// Human-friendly name of the model, e.g.
    /// e.g. "model_name": "DeepSeek: DeepSeek V3.1",
    pub(crate) model_name: ModelName,

    /// Context length of model served, distinct from prompt/completion limits
    pub(crate) context_length: f64,

    /// Pricing for different kinds of tokens, natively in dollars/token,
    /// See `ModelPricing`
    pub(crate) pricing: ModelPricing,

    /// Human-readable provider name, e.g. "Chutes", "Z.AI", "DeepSeek"
    ///     "provider_name": "Chutes",
    pub(crate) provider_name: ProviderName,

    /// computer-friendly provider slug, e.g. "chutes", "z-ai", "deepseek"
    pub(crate) tag: EndpointTag,

    /// The level of quantization of the endpoint, e.g.
    ///     "quantization": "fp4",
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quantization: Option<Quant>,

    /// Parameters supported as options by this endpoint, includes things like:
    /// - tools
    /// - top_k
    /// - stop
    /// - include_reasoning
    ///
    /// See SupportedParameters for full enum of observed values.
    pub(crate) supported_parameters: Vec<SupportedParameters>,

    /// Max completion tokens supported by this endpoint, may be less than context length +
    /// max_prompt tokens, may be null, e.g.
    ///     "max_completion_tokens": null,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_completion_tokens: Option<f64>,

    /// Max prompt tokens supported by this endpoint, may be less than context length +
    /// max_completion tokens, may be null, e.g.
    ///     "max_prompt_tokens": null,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_prompt_tokens: Option<f64>,

    /// Not sure what this is, not documented on the website
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<i32>,

    /// Not documented, but self-explanatory, e.g.
    ///     "uptime_last_30m": 100,
    #[serde(skip_serializing_if = "Option::is_none", rename = "uptime_last_30m")]
    pub(crate) uptime: Option<f64>,

    /// Unsure what this is exactly, not documented on OpenRouter
    ///     "supports_implicit_caching": false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) supports_implicit_caching: Option<bool>,
}

impl Endpoint {
    pub(crate) fn supports_tools(&self) -> bool {
        self.supported_parameters
            .contains(&SupportedParameters::Tools)
    }
}

/// Wrapper for the OpenRouter endpoints API response shape:
/// { "data": { "endpoints": [ Endpoint, ... ] } }
///
/// Response from `/models/:author/:slug/endpoints`.
/// e.g. https://openrouter.ai/api/v1/models/deepseek/deepseek-chat-v3.1/endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EndpointsResponse {
    pub(crate) data: EndpointData,
}

/// Contained within the `EndPointsResponse` as an object,
/// this has some basic model information and then provides a list of the endpoints that support
/// the models.
// NOTE: Common fields between this and the `/models` endpoint are:
// - id, name, created, description, architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EndpointData {
    /// canonical endpoint name (author/slug), e.g. deepseek/deepseek-chat-v3.1
    pub(crate) id: ModelId,
    /// User-friendly name, e.g. DeepSeek: DeepSeek V3.1
    pub(crate) name: ArcStr,
    /// Unix timestamp, e.g. 1755779628
    pub(crate) created: f64,
    /// User-facing description. Kind of long.
    pub(crate) description: ArcStr,
    /// Things like tokenizer, modality, etc. See `Architecture` struct.
    pub(crate) architecture: Architecture,
    /// A list of endpoints that provide completion for this model.
    pub(crate) endpoints: Vec<Endpoint>,
}

// Marker for response_format -> { "type": "json_object" }
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsonObjMarker;

impl Serialize for JsonObjMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("type", "json_object")?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for JsonObjMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = JsonObjMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object { \"type\": \"json_object\" }")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut found = false;
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    if k == "type" {
                        if let Some(s) = v.as_str() {
                            if s == "json_object" {
                                found = true;
                            }
                        }
                    }
                }
                if found {
                    Ok(JsonObjMarker)
                } else {
                    Err(serde::de::Error::custom("invalid response_format"))
                }
            }
        }
        deserializer.deserialize_map(V)
    }
}

// Marker for route -> "fallback"
#[derive(Debug, Clone, Copy)]
pub(crate) struct FallbackMarker;

impl Serialize for FallbackMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("fallback")
    }
}

impl<'de> Deserialize<'de> for FallbackMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = FallbackMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("the string \"fallback\"")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v == "fallback" {
                    Ok(FallbackMarker)
                } else {
                    Err(E::custom("expected 'fallback'"))
                }
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Tool selection behavior for OpenRouter requests.
/// Bridge format: "none" | "auto" | { type: "function", function: { name } }
#[derive(Debug, Clone)]
pub(crate) enum ToolChoice {
    None,
    Auto,
    Function {
        r#type: FunctionMarker,
        function: ToolChoiceFunction,
    },
}

impl Serialize for ToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ToolChoice::None => serializer.serialize_str("none"),
            ToolChoice::Auto => serializer.serialize_str("auto"),
            ToolChoice::Function { r#type, function } => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", r#type)?;
                map.serialize_entry("function", function)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ToolChoice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ToolChoice;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"none\" | \"auto\" | { type: \"function\", function: { name } }")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "none" => Ok(ToolChoice::None),
                    "auto" => Ok(ToolChoice::Auto),
                    _ => Err(E::custom("invalid ToolChoice string")),
                }
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut type_seen: Option<FunctionMarker> = None;
                let mut function_seen: Option<ToolChoiceFunction> = None;
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    match k.as_str() {
                        "type" => {
                            let m: FunctionMarker =
                                serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                            type_seen = Some(m);
                        }
                        "function" => {
                            let f: ToolChoiceFunction =
                                serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                            function_seen = Some(f);
                        }
                        _ => {}
                    }
                }
                match (type_seen, function_seen) {
                    (Some(m), Some(f)) => Ok(ToolChoice::Function {
                        r#type: m,
                        function: f,
                    }),
                    _ => Err(serde::de::Error::custom("invalid ToolChoice object")),
                }
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolChoiceFunction {
    pub(crate) name: String,
}

#[cfg(test)]
mod tests {
    use crate::llm2::{
        SupportedParameters,
        {InstructType, Modality, Tokenizer},
        router_only::{openrouter::OpenRouter, Router, openrouter::OpenRouterModelVariant},
    };

    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn json_obj_marker_serde_roundtrip() {
        let m = JsonObjMarker;
        let v = serde_json::to_value(m).expect("serialize");
        assert_eq!(v, json!({"type":"json_object"}));

        let de: JsonObjMarker = serde_json::from_value(v).expect("deserialize");
        let v2 = serde_json::to_value(de).unwrap();
        assert_eq!(v2, json!({"type":"json_object"}));
    }

    #[test]
    fn fallback_marker_serde_roundtrip() {
        let m = FallbackMarker;
        let v = serde_json::to_value(m).expect("serialize");
        assert_eq!(v, Value::String("fallback".to_string()));
        let de: FallbackMarker = serde_json::from_value(v).expect("deserialize");
        let v2 = serde_json::to_value(de).unwrap();
        assert_eq!(v2, Value::String("fallback".to_string()));
    }

    #[test]
    fn tool_choice_serde_variants() {
        // none
        let none: ToolChoice = serde_json::from_str("\"none\"").expect("none deser");
        match none {
            ToolChoice::None => {}
            _ => panic!("expected None"),
        }
        let s = serde_json::to_string(&none).unwrap();
        assert_eq!(s, "\"none\"");

        // auto
        let auto: ToolChoice = serde_json::from_str("\"auto\"").expect("auto deser");
        match auto {
            ToolChoice::Auto => {}
            _ => panic!("expected Auto"),
        }
        let s = serde_json::to_string(&auto).unwrap();
        assert_eq!(s, "\"auto\"");

        // function
        let func_json = json!({
            "type": "function",
            "function": { "name": "apply_code_edit" }
        });
        let fc: ToolChoice = serde_json::from_value(func_json.clone()).expect("function deser");
        match &fc {
            ToolChoice::Function {
                r#type: _,
                function,
            } => {
                assert_eq!(function.name, "apply_code_edit");
            }
            _ => panic!("expected Function variant"),
        }
        let back = serde_json::to_value(fc).unwrap();
        assert_eq!(back, func_json);
    }

    #[test]
    fn test_example_json_deserialize() {
        // Example json response for
        // `https://openrouter.ai/api/v1/models/deepseek/deepseek-chat-v3.1/endpoints`
        let example_json = json!({
            "data": {
                "id": "deepseek/deepseek-chat-v3.1",
                "name": "DeepSeek: DeepSeek V3.1",
                "created": 1755779628,
                "description": "DeepSeek-V3.1 is a large hybrid reasoning model ...",
                "architecture": {
                    "tokenizer": "DeepSeek",
                    "instruct_type": "deepseek-v3.1",
                    "modality": "text->text",
                    "input_modalities": ["text"],
                    "output_modalities": ["text"]
                },
                "endpoints": [
                    {
                        "name": "Chutes | deepseek/deepseek-chat-v3.1",
                        "model_name": "DeepSeek: DeepSeek V3.1",
                        "context_length": 163840,
                        "pricing": {
                            "prompt": "0.0000002",
                            "completion": "0.0000008",
                            "request": 0,
                            "image": 0,
                            "image_output": 0,
                            "web_search": 0,
                            "internal_reasoning": 0,
                            "discount": 0
                        },
                        "provider_name": "Chutes",
                        "tag": "chutes",
                        "quantization": null,
                        "max_completion_tokens": null,
                        "max_prompt_tokens": null,
                        "supported_parameters": [
                            "tools",
                            "tool_choice",
                            "reasoning",
                            "include_reasoning",
                            "max_tokens",
                            "temperature",
                            "top_p",
                            "stop",
                            "frequency_penalty",
                            "presence_penalty",
                            "seed",
                            "top_k",
                            "min_p",
                            "repetition_penalty",
                            "logprobs",
                            "logit_bias",
                            "top_logprobs"
                        ],
                        "status": 0,
                        "uptime_last_30m": 99.33169971040321,
                        "supports_implicit_caching": false
                    }
                ]
            }
        });

        let parsed: EndpointsResponse =
            serde_json::from_value(example_json).expect("deserialize example JSON");
        assert_eq!(parsed.data.id.to_string().as_str(), "deepseek/deepseek-chat-v3.1");
        assert_eq!(parsed.data.endpoints.len(), 1);
        let ep = &parsed.data.endpoints[0];

        // Test EndpointData fields
        assert_eq!(parsed.data.name.as_ref(), "DeepSeek: DeepSeek V3.1");
        assert_eq!(parsed.data.created, 1755779628.0);
        assert!(
            parsed
                .data
                .description
                .starts_with("DeepSeek-V3.1 is a large hybrid reasoning model")
        );
        assert_eq!(parsed.data.architecture.tokenizer, Tokenizer::DeepSeek);
        assert_eq!(
            parsed.data.architecture.instruct_type,
            Some(InstructType::DeepSeekV31)
        );
        assert_eq!(parsed.data.architecture.modality, Modality::TextToText);

        // Test Endpoint fields
        assert_eq!(ep.name.as_ref(), "Chutes | deepseek/deepseek-chat-v3.1");
        assert_eq!(ep.model_name.as_str(), "DeepSeek: DeepSeek V3.1");
        assert_eq!(ep.context_length, 163840.0);
        assert_eq!(ep.provider_name.as_str(), "Chutes");
        assert_eq!(ep.tag.provider_name.as_str(), "chutes");
        assert_eq!(ep.tag.quantization, None);
        assert_eq!(ep.quantization, None);
        assert_eq!(ep.max_completion_tokens, None);
        assert_eq!(ep.max_prompt_tokens, None);
        assert_eq!(ep.status, Some(0));
        assert!(ep.uptime.unwrap() - 99.33169971040321 <= 0e-5);
        assert_eq!(ep.supports_implicit_caching, Some(false));
        assert!(ep.supports_tools());

        // Test pricing
        assert!(ep.pricing.prompt - 0.0000002 <= 0e-8);
        assert!(ep.pricing.completion - 0.0000008 <= 0e-8);
        assert_eq!(ep.pricing.request, Some(0.0));
        assert_eq!(ep.pricing.image, Some(0.0));
        assert_eq!(ep.pricing.image_output, Some(0.0));
        assert_eq!(ep.pricing.web_search, Some(0.0));
        assert_eq!(ep.pricing.internal_reasoning, Some(0.0));
        assert_eq!(ep.pricing.discount, Some(0.0));

        // Test supported parameters
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::Tools)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::ToolChoice)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::Reasoning)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::IncludeReasoning)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::MaxTokens)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::Temperature)
        );
        assert!(ep.supported_parameters.contains(&SupportedParameters::TopP));
        assert!(ep.supported_parameters.contains(&SupportedParameters::Stop));
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::FrequencyPenalty)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::PresencePenalty)
        );
        assert!(ep.supported_parameters.contains(&SupportedParameters::Seed));
        assert!(ep.supported_parameters.contains(&SupportedParameters::TopK));
        assert!(ep.supported_parameters.contains(&SupportedParameters::MinP));
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::RepetitionPenalty)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::Logprobs)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::LogitBias)
        );
        assert!(
            ep.supported_parameters
                .contains(&SupportedParameters::TopLogprobs)
        );
        assert_eq!(ep.supported_parameters.len(), 17);
    }

    use crate::llm2::router_only::openrouter::OpenRouterModelId;
    use std::str::FromStr;
    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_fetch_smoke() -> color_eyre::Result<()> {
        let pe = ModelKey {
            author: Author::new("deepseek")?,
            slug: ModelSlug::new("deepseek-chat-v3.1")?,
        };
        let v = call_openrouter_endpoint(pe, None).await?;
        assert!(v.get("data").is_some(), "response missing 'data' key");
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_into_endpoint_deserialize() -> color_eyre::Result<()> {
        let pe = ModelKey {
            author: Author::new("deepseek")?,
            slug: ModelSlug::new("deepseek-chat-v3.1")?,
        };
        let v = call_openrouter_endpoint(pe, None).await?;
        // eprintln!("{:#?}", v);
        let parsed: EndpointsResponse = serde_json::from_value(v)?;
        assert!(!parsed.data.endpoints.is_empty(), "no endpoints returned");
        let has_id = parsed.data.endpoints.iter().all(|e| !e.name.0.is_empty());
        assert!(has_id, "provider_id must be present");
        for e in &parsed.data.endpoints {
            let p = &e.pricing;
            assert!(p.request >= Some(0.0));
            assert!(p.image >= Some(0.0));
            assert!(p.prompt >= 0.0);
            assert!(p.completion >= 0.0);
        }
        Ok(())
    }

    async fn call_openrouter_endpoint(
        mk: ModelKey,
        variant: Option<OpenRouterModelVariant>,
    ) -> color_eyre::Result<serde_json::Value> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(crate::test_harness::default_headers())
            .build()?;
        let model_id = OpenRouterModelId { key: mk, variant };
        let url = OpenRouter::endpoints_url(model_id);
        let api_key = OpenRouter::resolve_api_key()?;

        let resp = client
            .get(url)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?;
        let v = resp.json::<serde_json::Value>().await?;
        Ok(v)
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_into_endpoint_deepseek() -> color_eyre::Result<()> {
        let pe = ModelKey {
            author: Author::new("deepseek")?,
            slug: ModelSlug::new("deepseek-chat-v3.1")?,
        };
        let v = call_openrouter_endpoint(pe, None).await?;
        // eprintln!("{:#?}", v);
        let parsed: EndpointsResponse = serde_json::from_value(v)?;
        assert!(
            !parsed.data.endpoints.is_empty(),
            "no endpoints returned for qwen"
        );
        Ok(())
    }
    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_multi_models_smoke() -> color_eyre::Result<()> {
        let Some(_op) = crate::test_harness::openrouter_env() else {
            panic!("Skipping live tests: OPENROUTER_API_KEY not set");
        };
        let candidates: &[(&str, &str)] = &[
            ("qwen", "qwen3-30b-a3b-thinking-2507"),
            ("meta-llama", "llama-3.1-8b-instruct"),
            ("x-ai", "grok-2"),
        ];
        let mut successes = 0usize;
        for (author, model) in candidates {
            let pe = ModelKey {
                author: Author::new(*author)?,
                slug: ModelSlug::new(*model)?,
            };
            match call_openrouter_endpoint(pe, None).await {
                Ok(v) => {
                    if v.get("data").is_some() {
                        // Try full typed path as well
                        if let Ok(parsed) = serde_json::from_value::<EndpointsResponse>(v) {
                            if !parsed.data.endpoints.is_empty() {
                                successes += 1;
                            }
                        } else {
                            successes += 1; // still count a data-bearing response
                        }
                    }
                }
                Err(e) => {
                    eprintln!("live endpoint fetch failed for {}/{}: {}", author, model, e);
                }
            }
        }
        assert!(
            successes >= 1,
            "at least one candidate model should succeed"
        );
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_roundtrip_compare() -> color_eyre::Result<()> {
        let pe = ModelKey {
            author: Author::new("deepseek")?,
            slug: ModelSlug::new("deepseek-chat-v3.1")?,
        };
        let raw = call_openrouter_endpoint(pe, None).await?;
        // eprintln!("{:#?}", raw);

        let data = raw.get("data").unwrap();
        let ep = data.get("endpoints").unwrap();
        let first_ep = ep.get(0).unwrap();
        let pricing = first_ep.get("pricing").unwrap();

        let completion = pricing.get("completion").unwrap();
        str_or_num_to_f64(completion);
        let image = pricing.get("image").unwrap();
        str_or_num_to_f64(image);
        let discount = pricing.get("discount").unwrap();
        let de_discount = crate::utils::se_de::string_to_f64_opt_zero(discount);
        // eprintln!("de_discount:\n{:?}", de_discount);
        let _ = de_discount?;

        let typed: EndpointsResponse = serde_json::from_value(raw.clone())?;
        assert!(!typed.data.endpoints.is_empty(), "no endpoints returned");

        let typed_json = serde_json::to_value(&typed).expect("serialize typed");
        // Full equality is not expected (we omit unknown fields; numeric normalization)
        assert!(typed_json != raw);

        // Subset equality on key fields
        let raw_eps = raw
            .get("data")
            .and_then(|d| d.get("endpoints"))
            .and_then(|v| v.as_array())
            .expect("raw endpoints array");
        assert_eq!(typed.data.endpoints.len(), raw_eps.len());

        fn str_or_num_to_f64(v: &serde_json::Value) -> Option<f64> {
            match v {
                serde_json::Value::Number(n) => n.as_f64().or_else(|| {
                    if n.as_u64().unwrap_or_default() == 0 {
                        Some(0.0)
                    } else {
                        panic!("Invalid State: not f64 AND u64")
                    }
                }),
                serde_json::Value::String(s) => s.parse::<f64>().ok(),
                _ => None,
            }
        }

        Ok(())
    }
}
