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
    pub(crate) fn with_overrides(mut self, other: &Self) -> Self {
        if let Some(max_tokens) = other.max_tokens {
            self.max_tokens = Some(max_tokens);
        }
        if let Some(temperature) = other.temperature {
            self.temperature = Some(temperature);
        }
        if let Some(seed) = other.seed {
            self.seed = Some(seed);
        }
        if let Some(top_p) = other.top_p {
            self.top_p = Some(top_p);
        }
        if let Some(top_k) = other.top_k {
            self.top_k = Some(top_k);
        }
        if let Some(frequency_penalty) = other.frequency_penalty {
            self.frequency_penalty = Some(frequency_penalty);
        }
        if let Some(presence_penalty) = other.presence_penalty {
            self.presence_penalty = Some(presence_penalty);
        }
        if let Some(repetition_penalty) = other.repetition_penalty {
            self.repetition_penalty = Some(repetition_penalty);
        }
        if let Some(logit_bias) = other.logit_bias.as_ref() {
            self.logit_bias = Some(logit_bias.clone());
        }
        if let Some(top_logprobs) = other.top_logprobs {
            self.top_logprobs = Some(top_logprobs);
        }
        if let Some(min_p) = other.min_p {
            self.min_p = Some(min_p);
        }
        if let Some(top_a) = other.top_a {
            self.top_a = Some(top_a);
        }

        self
    }

    pub(crate) fn apply_overrides(&mut self, other: &Self) {
        if let Some(max_tokens) = other.max_tokens {
            self.max_tokens = Some(max_tokens);
        }
        if let Some(temperature) = other.temperature {
            self.temperature = Some(temperature);
        }
        if let Some(seed) = other.seed {
            self.seed = Some(seed);
        }
        if let Some(top_p) = other.top_p {
            self.top_p = Some(top_p);
        }
        if let Some(top_k) = other.top_k {
            self.top_k = Some(top_k);
        }
        if let Some(frequency_penalty) = other.frequency_penalty {
            self.frequency_penalty = Some(frequency_penalty);
        }
        if let Some(presence_penalty) = other.presence_penalty {
            self.presence_penalty = Some(presence_penalty);
        }
        if let Some(repetition_penalty) = other.repetition_penalty {
            self.repetition_penalty = Some(repetition_penalty);
        }
        if let Some(logit_bias) = other.logit_bias.as_ref() {
            self.logit_bias = Some(logit_bias.clone());
        }
        if let Some(top_logprobs) = other.top_logprobs {
            self.top_logprobs = Some(top_logprobs);
        }
        if let Some(min_p) = other.min_p {
            self.min_p = Some(min_p);
        }
        if let Some(top_a) = other.top_a {
            self.top_a = Some(top_a);
        }
    }

    pub(crate) fn fill_missing_from<T>(&mut self, other: &T) 
    where
        T: AsRef<Self> + ?Sized
    {
        let other = other.as_ref();

        if self.max_tokens.is_none() {
            self.max_tokens = other.max_tokens;
        }
        if self.temperature.is_none() {
            self.temperature = other.temperature;
        }
        if self.seed.is_none() {
            self.seed = other.seed;
        }
        if self.top_p.is_none() {
            self.top_p = other.top_p;
        }
        if self.top_k.is_none() {
            self.top_k = other.top_k;
        }
        if self.frequency_penalty.is_none() {
            self.frequency_penalty = other.frequency_penalty;
        }
        if self.presence_penalty.is_none() {
            self.presence_penalty = other.presence_penalty;
        }
        if self.repetition_penalty.is_none() {
            self.repetition_penalty = other.repetition_penalty;
        }
        if self.logit_bias.is_none() {
            self.logit_bias = other.logit_bias.clone();
        }
        if self.top_logprobs.is_none() {
            self.top_logprobs = other.top_logprobs;
        }
        if self.min_p.is_none() {
            self.min_p = other.min_p;
        }
        if self.top_a.is_none() {
            self.top_a = other.top_a;
        }
    }

    /// Set max tokens parameter - Range: [1, context_length)
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature parameter - Range: [0, 2]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set seed parameter - Integer only
    pub fn with_seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set top_p parameter - Range: (0, 1]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set top_k parameter - Range: [1, Infinity) Not available for OpenAI models
    pub fn with_top_k(mut self, top_k: f32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set frequency_penalty parameter - Range: [-2, 2]
    pub fn with_frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.frequency_penalty = Some(frequency_penalty);
        self
    }

    /// Set presence_penalty parameter - Range: [-2, 2]
    pub fn with_presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.presence_penalty = Some(presence_penalty);
        self
    }

    /// Set repetition_penalty parameter - Range: (0, 2]
    pub fn with_repetition_penalty(mut self, repetition_penalty: f32) -> Self {
        self.repetition_penalty = Some(repetition_penalty);
        self
    }

    /// Set logit_bias parameter - { [key: number]: number }
    pub fn with_logit_bias(mut self, logit_bias: BTreeMap<i32, f32>) -> Self {
        self.logit_bias = Some(logit_bias);
        self
    }

    /// Set top_logprobs parameter - Integer only
    pub fn with_top_logprobs(mut self, top_logprobs: i32) -> Self {
        self.top_logprobs = Some(top_logprobs);
        self
    }

    /// Set min_p parameter - Range: [0, 1]
    pub fn with_min_p(mut self, min_p: f32) -> Self {
        self.min_p = Some(min_p);
        self
    }

    /// Set top_a parameter - Range: [0, 1]
    pub fn with_top_a(mut self, top_a: f32) -> Self {
        self.top_a = Some(top_a);
        self
    }
}

#[cfg(test)]
mod tests {
    use color_eyre::Result;
    use super::*;

    // complete for repeated testing AI!
    static LLM_TEST_PARAMS: LLMParameters = LLMParameters {
        max_tokens: 8192,
        temperature: todo!(),
        seed: todo!(),
        top_p: todo!(),
        top_k: todo!(),
        frequency_penalty: todo!(),
        presence_penalty: todo!(),
        repetition_penalty: todo!(),
        logit_bias: todo!(),
        top_logprobs: todo!(),
        min_p: todo!(),
        top_a: todo!(),
    };

    #[test]
    fn test_builder_fields() {
        let mut p = LLMParameters::default();

        assert_eq!(p.max_tokens, None);
        let max_tokens = 8192;
        p = p.with_max_tokens(max_tokens);
        assert_eq!(p.max_tokens, Some(8192));

        // add other fields more here
    }


    #[test]
    fn test_with_overrides() {
        // todo
    }

    #[test]
    fn test_apply_overrides() {
        // todo
    }

    #[test]
    fn test_fill_missing_from() {
        // todo
    }
}
