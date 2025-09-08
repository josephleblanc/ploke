use serde::{Deserialize, Serialize};

use crate::llm2::{ProviderSlug, enums::Quant, newtypes::ModelId};

/// Provider-specific information about the model.
/// - Unique type only used by OpenRouter
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct TopProvider {
    /// Whether this model is subject to content moderation.
    pub(crate) is_moderated: bool,
    pub(crate) context_length: Option<u32>,
    pub(crate) max_completion_tokens: Option<u64>,
}

impl TopProvider {
    /// Set whether this model is subject to content moderation.
    pub fn with_moderated(mut self, moderated: bool) -> Self {
        self.is_moderated = moderated;
        self
    }

    /// Set the context length for this model.
    pub fn with_context_length(mut self, length: u32) -> Self {
        self.context_length = Some(length);
        self
    }

    /// Set the maximum completion tokens for this model.
    pub fn with_max_completion_tokens(mut self, tokens: u64) -> Self {
        self.max_completion_tokens = Some(tokens);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ChatCompFields {
    /// OpenRouter docs: See "Prompt Transforms" section: openrouter.ai/docs/transforms
    /// From `https://openrouter.ai/docs/features/message-transforms`
    ///  This can be useful for situations where perfect recall is not required. The transform works
    ///  by removing or truncating messages from the middle of the prompt, until the prompt fits
    ///  within the model’s context window.
    /// Further, there is a note:
    ///  All OpenRouter endpoints with 8k (8,192 tokens) or less context length will default to
    ///  using middle-out. To disable this, set transforms: [] in the request body.
    /// corresponding json: `transforms?: string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transforms: Option<Transform>,
    /// OpenRouter docs: See "Model Routing" section: openrouter.ai/docs/model-routing
    /// From `https://openrouter.ai/docs/features/model-routing`
    ///  The models parameter lets you automatically try other models if the primary model’s
    ///  providers are down, rate-limited, or refuse to reply due to content moderation.
    ///
    /// corresponding json: `models?: string[];`
    /// example from OpenRouter:
    /// ```ignore
    ///  {
    ///    "models": ["anthropic/claude-3.5-sonnet", "gryphe/mythomax-l2-13b"],
    ///    ... // Other params
    ///  }
    /// ```
    /// Note that models here are in the form of canonical endpoint name (author/slug), e.g. deepseek/deepseek-chat-v3.1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) models: Option<Vec<ModelId>>,
    /// the docs literally just have the string 'fallback' here. No idea what this means, maybe they
    /// read the string as a bool?
    /// corresponding json: `route?: 'fallback';`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) route: Option<FallbackMarker>,
    /// OpenRouter docs: See "Provider Routing" section: openrouter.ai/docs/provider-routing
    /// corresponding json: `provider?: ProviderPreferences;`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) provider: Option<ProviderPreferences>,
    /// corresponding json: `user?: string; // A stable identifier for your end-users. Used to help detect and prevent abuse.`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user: Option<String>,
}

impl ChatCompFields {
    /// Set the transforms parameter.
    pub fn with_transforms(mut self, transforms: Transform) -> Self {
        self.transforms = Some(transforms);
        self
    }

    /// Set the models parameter for fallback routing.
    pub fn with_models<I: IntoIterator<Item = ModelId>>(mut self, models: I) -> Self {
        self.models = Some(models.into_iter().collect());
        self
    }

    /// Set the route parameter.
    pub fn with_route(mut self, route: FallbackMarker) -> Self {
        self.route = Some(route);
        self
    }

    /// Set the provider preferences.
    pub fn with_provider(mut self, provider: ProviderPreferences) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the user identifier.
    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Transform {
    MiddleOut([MiddleOutMarker; 1]),
    Disable([&'static str; 0]),
}

// Marker for route -> "middle-out"
#[derive(Debug, Clone, Copy)]
pub struct MiddleOutMarker;

impl Serialize for MiddleOutMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("middle-out")
    }
}

impl<'de> Deserialize<'de> for MiddleOutMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = MiddleOutMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("the string \"middle-out\"")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v == "middle-out" {
                    Ok(MiddleOutMarker)
                } else {
                    Err(E::custom("expected 'middle-out'"))
                }
            }
        }
        deserializer.deserialize_str(V)
    }
}

// Marker for route -> "fallback"
#[derive(Debug, Clone, Copy)]
pub struct FallbackMarker;

impl Serialize for FallbackMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("fallback")
    }
}

impl<'de> Deserialize<'de> for FallbackMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
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

/// OpenRouter "provider" routing preferences (typed).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data_collection: Option<DataCollection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<Quant>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<MaxPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DataCollection {
    Allow,
    #[default]
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SortBy {
    Price,
    Throughput,
    Latency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MaxPrice {
    pub prompt_tokens: Option<f64>,
    pub completion_tokens: Option<f64>,
    pub request: Option<f64>,
}

impl ProviderPreferences {
    /// Add an order preference.
    pub fn with_order<I: IntoIterator<Item = ProviderSlug>>(mut self, order: I) -> Self {
        self.order = Some(order.into_iter().collect());
        self
    }

    /// Set allow_fallbacks preference.
    pub fn with_allow_fallbacks(mut self, allow: bool) -> Self {
        self.allow_fallbacks = Some(allow);
        self
    }

    /// Set require_parameters preference.
    pub fn with_require_parameters(mut self, require: bool) -> Self {
        self.require_parameters = Some(require);
        self
    }

    /// Set data_collection preference.
    pub fn with_data_collection(mut self, collection: DataCollection) -> Self {
        self.data_collection = Some(collection);
        self
    }

    /// Add an allow preference.
    pub fn with_only<I: IntoIterator<Item = ProviderSlug>>(mut self, allow: I) -> Self {
        self.only = Some(allow.into_iter().collect());
        self
    }

    /// Add an ignore list.
    pub fn with_ignore<I: IntoIterator<Item = ProviderSlug>>(mut self, ignore: I) -> Self {
        self.ignore = Some(ignore.into_iter().collect());
        self
    }

    /// Add quantizations filter.
    pub fn with_quantizations<I: IntoIterator<Item = Quant>>(mut self, quantizations: I) -> Self {
        self.quantizations = Some(quantizations.into_iter().collect());
        self
    }

    /// Set sort preference.
    pub fn with_sort(mut self, sort: SortBy) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Set max_price preference.
    pub fn with_max_price(mut self, max_price: MaxPrice) -> Self {
        self.max_price = Some(max_price);
        self
    }

    /// Validate that there is no intersection between only and ignore lists.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let (Some(only), Some(ignore)) = (&self.only, &self.ignore) {
            if only.iter().any(|slug| ignore.contains(slug)) {
                return Err("ProviderPreferences only and ignore lists have overlapping entries");
            }
        }
        Ok(())
    }
}
