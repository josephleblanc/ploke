use std::{borrow::Borrow, collections::BTreeMap, sync::Arc};

use super::*;
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
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

    pub(crate) fn with_union(mut self, other: &Self) -> Self {
        self.max_tokens = self.max_tokens.or(other.max_tokens);
        self.temperature = self.temperature.or(other.temperature);
        self.seed = self.seed.or(other.seed);
        self.top_p = self.top_p.or(other.top_p);
        self.top_k = self.top_k.or(other.top_k);
        self.frequency_penalty = self.frequency_penalty.or(other.frequency_penalty);
        self.presence_penalty = self.presence_penalty.or(other.presence_penalty);
        self.repetition_penalty = self.repetition_penalty.or(other.repetition_penalty);
        self.logit_bias = self.logit_bias.clone().or_else(|| other.logit_bias.clone());
        self.top_logprobs = self.top_logprobs.or(other.top_logprobs);
        self.min_p = self.min_p.or(other.min_p);
        self.top_a = self.top_a.or(other.top_a);
        self
    }

    pub(crate) fn apply_union(&mut self, other: &Self) {
        self.max_tokens = self.max_tokens.or(other.max_tokens);
        self.temperature = self.temperature.or(other.temperature);
        self.seed = self.seed.or(other.seed);
        self.top_p = self.top_p.or(other.top_p);
        self.top_k = self.top_k.or(other.top_k);
        self.frequency_penalty = self.frequency_penalty.or(other.frequency_penalty);
        self.presence_penalty = self.presence_penalty.or(other.presence_penalty);
        self.repetition_penalty = self.repetition_penalty.or(other.repetition_penalty);
        self.logit_bias = self.logit_bias.clone().or_else(|| other.logit_bias.clone());
        self.top_logprobs = self.top_logprobs.or(other.top_logprobs);
        self.min_p = self.min_p.or(other.min_p);
        self.top_a = self.top_a.or(other.top_a);
    }

    pub(crate) fn with_intersection(mut self, other: &Self) -> Self {
        self.max_tokens = if self.max_tokens == other.max_tokens { self.max_tokens } else { None };
        self.temperature = if self.temperature == other.temperature { self.temperature } else { None };
        self.seed = if self.seed == other.seed { self.seed } else { None };
        self.top_p = if self.top_p == other.top_p { self.top_p } else { None };
        self.top_k = if self.top_k == other.top_k { self.top_k } else { None };
        self.frequency_penalty = if self.frequency_penalty == other.frequency_penalty { self.frequency_penalty } else { None };
        self.presence_penalty = if self.presence_penalty == other.presence_penalty { self.presence_penalty } else { None };
        self.repetition_penalty = if self.repetition_penalty == other.repetition_penalty { self.repetition_penalty } else { None };
        self.logit_bias = if self.logit_bias == other.logit_bias { self.logit_bias.clone() } else { None };
        self.top_logprobs = if self.top_logprobs == other.top_logprobs { self.top_logprobs } else { None };
        self.min_p = if self.min_p == other.min_p { self.min_p } else { None };
        self.top_a = if self.top_a == other.top_a { self.top_a } else { None };
        self
    }

    pub(crate) fn apply_intersection(&mut self, other: &Self) {
        if self.max_tokens != other.max_tokens {
            self.max_tokens = None;
        }
        if self.temperature != other.temperature {
            self.temperature = None;
        }
        if self.seed != other.seed {
            self.seed = None;
        }
        if self.top_p != other.top_p {
            self.top_p = None;
        }
        if self.top_k != other.top_k {
            self.top_k = None;
        }
        if self.frequency_penalty != other.frequency_penalty {
            self.frequency_penalty = None;
        }
        if self.presence_penalty != other.presence_penalty {
            self.presence_penalty = None;
        }
        if self.repetition_penalty != other.repetition_penalty {
            self.repetition_penalty = None;
        }
        if self.logit_bias != other.logit_bias {
            self.logit_bias = None;
        }
        if self.top_logprobs != other.top_logprobs {
            self.top_logprobs = None;
        }
        if self.min_p != other.min_p {
            self.min_p = None;
        }
        if self.top_a != other.top_a {
            self.top_a = None;
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
    use super::*;
    use color_eyre::Result;

    static LLM_TEST_PARAMS: LLMParameters = LLMParameters {
        max_tokens: Some(8192),
        temperature: Some(0.7),
        seed: Some(42),
        top_p: Some(0.9),
        top_k: Some(1.5),
        frequency_penalty: Some(0.1),
        presence_penalty: Some(0.1),
        repetition_penalty: Some(1.1),
        logit_bias: None,
        top_logprobs: Some(5),
        min_p: Some(0.1),
        top_a: Some(0.2),
    };

    #[test]
    fn test_builder_fields() {
        let mut p = LLMParameters::default();

        assert_eq!(p.max_tokens, None);
        let max_tokens = 8192;
        p = p.with_max_tokens(max_tokens);
        assert_eq!(p.max_tokens, Some(8192));

        assert_eq!(p.temperature, None);
        p = p.with_temperature(0.7);
        assert_eq!(p.temperature, Some(0.7));

        assert_eq!(p.seed, None);
        p = p.with_seed(42);
        assert_eq!(p.seed, Some(42));

        assert_eq!(p.top_p, None);
        p = p.with_top_p(0.9);
        assert_eq!(p.top_p, Some(0.9));

        assert_eq!(p.top_k, None);
        p = p.with_top_k(1.5);
        assert_eq!(p.top_k, Some(1.5));

        assert_eq!(p.frequency_penalty, None);
        p = p.with_frequency_penalty(0.1);
        assert_eq!(p.frequency_penalty, Some(0.1));

        assert_eq!(p.presence_penalty, None);
        p = p.with_presence_penalty(0.1);
        assert_eq!(p.presence_penalty, Some(0.1));

        assert_eq!(p.repetition_penalty, None);
        p = p.with_repetition_penalty(1.1);
        assert_eq!(p.repetition_penalty, Some(1.1));

        assert_eq!(p.logit_bias, None);
        let mut logit_bias = BTreeMap::new();
        logit_bias.insert(123, 0.5);
        p = p.with_logit_bias(logit_bias.clone());
        assert_eq!(p.logit_bias, Some(logit_bias));

        assert_eq!(p.top_logprobs, None);
        p = p.with_top_logprobs(5);
        assert_eq!(p.top_logprobs, Some(5));

        assert_eq!(p.min_p, None);
        p = p.with_min_p(0.1);
        assert_eq!(p.min_p, Some(0.1));

        assert_eq!(p.top_a, None);
        p = p.with_top_a(0.2);
        assert_eq!(p.top_a, Some(0.2));
    }

    #[test]
    fn test_with_overrides() {
        let base = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.5),
            seed: Some(123),
            top_p: Some(0.8),
            ..Default::default()
        };

        let overrides = LLMParameters {
            max_tokens: Some(2000),
            temperature: Some(0.7),
            ..Default::default()
        };

        let result = base.with_overrides(&overrides);

        assert_eq!(result.max_tokens, Some(2000));
        assert_eq!(result.temperature, Some(0.7));
        assert_eq!(result.seed, Some(123)); // unchanged
        assert_eq!(result.top_p, Some(0.8)); // unchanged
    }

    #[test]
    fn test_apply_overrides() {
        let mut base = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.5),
            seed: Some(123),
            top_p: Some(0.8),
            ..Default::default()
        };

        let overrides = LLMParameters {
            max_tokens: Some(2000),
            temperature: Some(0.7),
            seed: None, // Should not override
            ..Default::default()
        };
        assert_ne!(base, overrides);

        base.apply_overrides(&overrides);

        assert_eq!(base.max_tokens, Some(2000));
        assert_eq!(base.temperature, Some(0.7));
        assert_eq!(base.seed, Some(123)); // unchanged because override is None
        assert_eq!(base.top_p, Some(0.8)); // unchanged
    }

    #[test]
    fn test_apply_union() {
        let mut base = LLMParameters {
            max_tokens: None,
            temperature: Some(0.5),
            seed: None,
            top_p: Some(0.8),
            ..Default::default()
        };

        let source = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.7),
            seed: Some(42),
            top_p: Some(0.9),
            ..Default::default()
        };

        base.apply_union(&source);

        assert_eq!(base.max_tokens, Some(1000)); // filled from None
        assert_eq!(base.temperature, Some(0.5)); // unchanged (not None)
        assert_eq!(base.seed, Some(42)); // filled from None
        assert_eq!(base.top_p, Some(0.8)); // unchanged (not None)
    }

    #[test]
    fn test_with_union() {
        let base = LLMParameters {
            max_tokens: None,
            temperature: Some(0.5),
            seed: None,
            top_p: Some(0.8),
            ..Default::default()
        };

        let source = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.7),
            seed: Some(42),
            top_p: Some(0.9),
            ..Default::default()
        };

        let result = base.with_union(&source);

        assert_eq!(result.max_tokens, Some(1000)); // filled from None
        assert_eq!(result.temperature, Some(0.5)); // unchanged (not None)
        assert_eq!(result.seed, Some(42)); // filled from None
        assert_eq!(result.top_p, Some(0.8)); // unchanged (not None)
    }

    #[test]
    fn test_with_intersection() {
        let base = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.5),
            seed: Some(123),
            top_p: Some(0.8),
            ..Default::default()
        };

        let other = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.7),
            seed: Some(123),
            top_p: Some(0.9),
            ..Default::default()
        };

        let result = base.with_intersection(&other);

        assert_eq!(result.max_tokens, Some(1000)); // same in both
        assert_eq!(result.temperature, None); // different
        assert_eq!(result.seed, Some(123)); // same in both
        assert_eq!(result.top_p, None); // different
    }

    #[test]
    fn test_apply_intersection() {
        let mut base = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.5),
            seed: Some(123),
            top_p: Some(0.8),
            ..Default::default()
        };

        let other = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.7),
            seed: Some(123),
            top_p: Some(0.9),
            ..Default::default()
        };

        base.apply_intersection(&other);

        assert_eq!(base.max_tokens, Some(1000)); // same in both
        assert_eq!(base.temperature, None); // different
        assert_eq!(base.seed, Some(123)); // same in both
        assert_eq!(base.top_p, None); // different
    }

    #[test]
    fn test_union_with_none_values() {
        let base = LLMParameters {
            max_tokens: Some(1000),
            temperature: None,
            ..Default::default()
        };

        let source = LLMParameters {
            max_tokens: None,
            temperature: Some(0.5),
            ..Default::default()
        };

        let result = base.with_union(&source);

        assert_eq!(result.max_tokens, Some(1000)); // base has value
        assert_eq!(result.temperature, Some(0.5)); // filled from source
    }

    #[test]
    fn test_intersection_with_none_values() {
        let base = LLMParameters {
            max_tokens: Some(1000),
            temperature: None,
            ..Default::default()
        };

        let other = LLMParameters {
            max_tokens: Some(1000),
            temperature: Some(0.5),
            ..Default::default()
        };

        let result = base.with_intersection(&other);

        assert_eq!(result.max_tokens, Some(1000)); // same in both
        assert_eq!(result.temperature, None); // base is None, so intersection is None
    }

    #[test]
    fn test_serialization_roundtrip_default() -> Result<()> {
        let params = LLMParameters::default();
        let json = serde_json::to_string(&params)?;
        let deserialized: LLMParameters = serde_json::from_str(&json)?;
        assert_eq!(params, deserialized);
        Ok(())
    }

    #[test]
    fn test_serialization_roundtrip_with_values() -> Result<()> {
        let params = LLMParameters {
            max_tokens: Some(1024),
            temperature: Some(0.8),
            seed: Some(12345),
            top_p: Some(0.9),
            top_k: Some(50.0),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.3),
            repetition_penalty: Some(1.1),
            logit_bias: {
                let mut map = BTreeMap::new();
                map.insert(1, 0.5);
                map.insert(2, -0.3);
                Some(map)
            },
            top_logprobs: Some(5),
            min_p: Some(0.1),
            top_a: Some(0.2),
        };
        let json = serde_json::to_string(&params)?;
        let deserialized: LLMParameters = serde_json::from_str(&json)?;
        assert_eq!(params, deserialized);
        Ok(())
    }

    #[test]
    fn test_serialization_skips_none_values() -> Result<()> {
        let params = LLMParameters {
            max_tokens: Some(100),
            ..Default::default()
        };
        let json = serde_json::to_string(&params)?;
        assert!(!json.contains("temperature"));
        assert!(!json.contains("seed"));
        assert!(json.contains("max_tokens"));
        
        let deserialized: LLMParameters = serde_json::from_str(&json)?;
        assert_eq!(params, deserialized);
        Ok(())
    }
}
