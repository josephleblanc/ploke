use std::collections::BTreeMap;

use super::*;
// AI: add builder methods for this struct AI!
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub(crate) struct LLMParameters {
    // corresponding json: `max_tokens?: number; // Range: [1, context_length)`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    // corresponding json: `temperature?: number; // Range: [0, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f32>,
    // corresponding json: `seed?: number; // Integer only`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) seed: Option<i64>,
    // corresponding json: `top_p?: number; // Range: (0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<f32>,
    // corresponding json: `top_k?: number; // Range: [1, Infinity) Not available for OpenAI models`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_k: Option<f32>,
    // corresponding json: `frequency_penalty?: number; // Range: [-2, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) frequency_penalty: Option<f32>,
    // corresponding json: `presence_penalty?: number; // Range: [-2, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) presence_penalty: Option<f32>,
    // corresponding json: `repetition_penalty?: number; // Range: (0, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repetition_penalty: Option<f32>,
    // corresponding json: `logit_bias?: { [key: number]: number };`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) logit_bias: Option<BTreeMap<i32, f32>>,
    // corresponding json: `top_logprobs: number; // Integer only`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_logprobs: Option<i32>,
    // corresponding json: `min_p?: number; // Range: [0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) min_p: Option<f32>,
    // corresponding json: `top_a?: number; // Range: [0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_a: Option<f32>,
}
