use std::collections::BTreeMap;

use super::*;
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

impl LLMParameters {
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn top_k(mut self, top_k: f32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.frequency_penalty = Some(frequency_penalty);
        self
    }

    pub fn presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.presence_penalty = Some(presence_penalty);
        self
    }

    pub fn repetition_penalty(mut self, repetition_penalty: f32) -> Self {
        self.repetition_penalty = Some(repetition_penalty);
        self
    }

    pub fn logit_bias(mut self, logit_bias: BTreeMap<i32, f32>) -> Self {
        self.logit_bias = Some(logit_bias);
        self
    }

    pub fn top_logprobs(mut self, top_logprobs: i32) -> Self {
        self.top_logprobs = Some(top_logprobs);
        self
    }

    pub fn min_p(mut self, min_p: f32) -> Self {
        self.min_p = Some(min_p);
        self
    }

    pub fn top_a(mut self, top_a: f32) -> Self {
        self.top_a = Some(top_a);
        self
    }
}
