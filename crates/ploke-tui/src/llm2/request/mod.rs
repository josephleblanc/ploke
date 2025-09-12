mod completion;
pub(crate) mod endpoint;
mod marker;
pub(crate) mod models;

#[cfg(test)]
mod tests;

use crate::utils::se_de::string_or_f64;
use crate::utils::se_de::string_or_f64_opt;
use crate::utils::se_de::string_to_f64_opt_zero;
use serde::{Deserialize, Serialize};

pub(crate) use completion::ChatCompReqCore;
pub(crate) use marker::JsonObjMarker;

// --- common types for requests ---
//
// These work for OpenRouter, may need to be adjusted into a more generic struct for other
// routers/providers

/// Pricing information for a model.
/// USD per token returned from OpenRouter (assumed comprehensive to other routers for now)
/// WARNING: Repeating for emphasis, this is price in USD/token, not USD per million tokens.
/// - need to watch out for float errors
/// - round to nearest 100th of a cent, e.g. $0.0001, when presenting to user and/or transforming
/// into a crate-local format
#[derive(Debug, Clone, PartialOrd, PartialEq, Serialize, Deserialize, Copy)]
pub struct ModelPricing {
    // Price per token in USD for system(?) prompt
    // All models at https://openrouter.ai/api/v1/models have this (323/323 tested)
    #[serde(default, deserialize_with = "string_or_f64")]
    pub prompt: f64,
    // Price per token in USD for generated tokens
    // All models at https://openrouter.ai/api/v1/models have this (323/323 tested)
    #[serde(default, deserialize_with = "string_or_f64")]
    pub completion: f64,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub audio: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub image: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub image_output: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_cache_read: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_cache_write: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub internal_reasoning: Option<f64>,
    // Price per token in USD for system(?) prompt
    // Most have this, 322/323 have it, so all but one
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub request: Option<f64>,
    // Again all but one have this
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub web_search: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_to_f64_opt_zero",
        skip_serializing_if = "Option::is_none"
    )]
    pub discount: Option<f64>,
}

#[cfg(test)]
mod pricing_tests {
    use super::*;
    use serde_json::json;

    // file with a `Vec<ModelPricing>` pretty-printed as json
    use crate::llm2::router_only::{MODELS_JSON_PRICING, cli::test_data::MODELS_PRICING_JSON_LAZY};

    macro_rules! approx_eq {
        ($a:expr, $b:expr, $eps:expr) => {{
            let (a, b, eps) = ($a as f64, $b as f64, $eps as f64);
            assert!((a - b).abs() <= eps, "{} != {} with eps {}", a, b, eps);
        }};
    }

    #[test]
    fn test_pricing_basic() {
        // Number values
        let raw_num = json!({
            "prompt": 0.000001,
            "completion": 0.0000025,
            "audio": 0.0001,
            "image": 0.001,
            "image_output": 0.0003,
            "input_cache_read": 0.0,
            "input_cache_write": 0.0,
            "internal_reasoning": 0.0,
            "request": 0.0,
            "web_search": 0.0,
            "discount": 0.0
        });
        let p_num: ModelPricing = serde_json::from_value(raw_num).expect("parse numeric pricing");
        approx_eq!(p_num.prompt, 0.000001, 1e-12);
        approx_eq!(p_num.completion, 0.0000025, 1e-12);
        assert_eq!(p_num.audio, Some(0.0001));
        assert_eq!(p_num.image, Some(0.001));
        assert_eq!(p_num.image_output, Some(0.0003));
        assert_eq!(p_num.input_cache_read, Some(0.0));
        assert_eq!(p_num.input_cache_write, Some(0.0));
        assert_eq!(p_num.internal_reasoning, Some(0.0));
        assert_eq!(p_num.request, Some(0.0));
        assert_eq!(p_num.web_search, Some(0.0));
        assert_eq!(p_num.discount, Some(0.0));

        // String values
        let raw_str = json!({
            "prompt": "0.000001",
            "completion": "0.0000025",
            "audio": "0.0001",
            "image": "0.001",
            "image_output": "0.0003",
            "input_cache_read": "0",
            "input_cache_write": "0",
            "internal_reasoning": "0",
            "request": "0",
            "web_search": "0",
            "discount": "0"
        });
        let p_str: ModelPricing = serde_json::from_value(raw_str).expect("parse string pricing");
        approx_eq!(p_str.prompt, 0.000001, 1e-12);
        approx_eq!(p_str.completion, 0.0000025, 1e-12);
        assert_eq!(p_str.audio, Some(0.0001));
        assert_eq!(p_str.image, Some(0.001));
        assert_eq!(p_str.image_output, Some(0.0003));
        assert_eq!(p_str.input_cache_read, Some(0.0));
        assert_eq!(p_str.input_cache_write, Some(0.0));
        assert_eq!(p_str.internal_reasoning, Some(0.0));
        assert_eq!(p_str.request, Some(0.0));
        assert_eq!(p_str.web_search, Some(0.0));
        assert_eq!(p_str.discount, Some(0.0));

        // Round-trip JSON preserves fields
        let s = serde_json::to_string(&p_str).expect("serialize pricing");
        let reparsed: ModelPricing = serde_json::from_str(&s).expect("reparse pricing");
        assert_eq!(reparsed, p_str);
    }

    #[test]
    fn test_pricing_from_file_counts() {
        let json_text = &*MODELS_PRICING_JSON_LAZY;
        // Helpful context for test logs
        tracing::info!(
            file = MODELS_JSON_PRICING,
            len = json_text.len(),
            "loaded pricing json"
        );

        let items: Vec<ModelPricing> = serde_json::from_str(json_text)
            .unwrap_or_else(|e| panic!("failed to parse {}: {}", MODELS_JSON_PRICING, e));
        let total = items.len();
        assert!(total > 0, "no pricing items parsed");

        // All entries must have prompt/completion and be non-negative
        assert!(items.iter().all(|p| p.prompt >= 0.0));
        assert!(items.iter().all(|p| p.completion >= 0.0));

        // Count presence of optional fields
        let count_present = |f: fn(&ModelPricing) -> bool| items.iter().filter(|p| f(p)).count();
        let audio_n = count_present(|p| p.audio.is_some());
        let image_n = count_present(|p| p.image.is_some());
        let image_out_n = count_present(|p| p.image_output.is_some());
        let cache_r_n = count_present(|p| p.input_cache_read.is_some());
        let cache_w_n = count_present(|p| p.input_cache_write.is_some());
        let reason_n = count_present(|p| p.internal_reasoning.is_some());
        let req_n = count_present(|p| p.request.is_some());
        let web_n = count_present(|p| p.web_search.is_some());
        let disc_n = count_present(|p| p.discount.is_some());

        // Log stats for observability during test runs
        tracing::info!(
            total,
            audio_n,
            image_n,
            image_out_n,
            cache_r_n,
            cache_w_n,
            reason_n,
            req_n,
            web_n,
            disc_n,
            "ModelPricing optional field presence counts"
        );

        // Sanity: at least some models should have optional fields populated
        assert!(audio_n <= total);
        assert!(image_n <= total);
        assert!(image_out_n <= total);
        assert!(cache_r_n <= total);
        assert!(cache_w_n <= total);
        assert!(reason_n <= total);
        assert!(req_n <= total);
        assert!(web_n <= total);
        assert!(disc_n <= total);

        // Expect many models to have request/web_search populated empirically
        assert!(req_n > 0, "expected some request pricing present");
        assert!(web_n > 0, "expected some web_search pricing present");
    }

    // more..

    // in one tests, also print the number of times each field is used with `tracing::info!`, as a
    // number of times the field is present vs. total `ModelPricing` parsed.
}
