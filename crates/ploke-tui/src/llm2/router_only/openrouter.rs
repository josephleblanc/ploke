use serde::{Deserialize, Serialize};

use crate::llm2::ProviderSlug;

/// Provider-specific information about the model.
/// - Unique type only used by OpenRouter
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct TopProvider {
    /// Whether this model is subject to content moderation.
    pub(crate) is_moderated: bool,
    pub(crate) context_length: Option<u32>,
    pub(crate) max_completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ChatCompFields {
    // OpenRouter docs: See "Prompt Transforms" section: openrouter.ai/docs/transforms
    // corresponding json: `transforms?: string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transforms: Option<Vec<String>>,
    // OpenRouter docs: See "Model Routing" section: openrouter.ai/docs/model-routing
    // corresponding json: `models?: string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) models: Option<Vec<String>>,
    // the docs literally just have the string 'fallback' here. No idea what this means, maybe they
    // read the string as a bool?
    // corresponding json: `route?: 'fallback';`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) route: Option<FallbackMarker>,
    // OpenRouter docs: See "Provider Routing" section: openrouter.ai/docs/provider-routing
    // corresponding json: `provider?: ProviderPreferences;`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) provider: Option<ProviderPreferences>,
    // corresponding json: `user?: string; // A stable identifier for your end-users. Used to help detect and prevent abuse.`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<DataCollection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<MaxPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DataCollection {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SortBy {
    Price,
    Throughput,
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
    pub fn with_quantizations<I: IntoIterator<Item = String>>(mut self, quantizations: I) -> Self {
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

