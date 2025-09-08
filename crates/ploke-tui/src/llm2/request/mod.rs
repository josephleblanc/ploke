mod completion;
mod marker;
pub(crate) mod endpoint;
mod models;

use crate::utils::se_de::string_to_f64_opt_zero;
use crate::utils::se_de::string_or_f64_opt;
use crate::utils::se_de::string_or_f64;
use serde::{Deserialize, Serialize};

pub(crate) use completion::ChatCompReqCore;
pub(crate) use marker::JsonObjMarker;

// --- common types for requests ---
// 
// These work for OpenRouter, may need to be adjusted into a more generic struct for other
// routers/providers

/// Pricing information for a model.
/// USD per token
#[derive(Debug, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
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
