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


// AI: Use the following spec to complete the `ProviderPreferences` below, including builder
// pattern methods. Each of the following items is optional. AI!
// From the OpenRouter docs:
// | Field | Type | Default | Description |
// | --- | --- | --- | --- |
// | `order` | string[] | - | List of provider slugs to try in order (e.g. `["anthropic", "openai"]`). [Learn more](#ordering-specific-providers) |
// | `allow_fallbacks` | boolean | `true` | Whether to allow backup providers when the primary is unavailable. [Learn more](#disabling-fallbacks) |
// | `require_parameters` | boolean | `false` | Only use providers that support all parameters in your request. [Learn more](#requiring-providers-to-support-all-parameters-beta) |
// | `data_collection` | "allow" \| "deny" | "allow" | Control whether to use providers that may store data. [Learn more](#requiring-providers-to-comply-with-data-policies) |
// | `only` | string[] | - | List of provider slugs to allow for this request. [Learn more](#allowing-only-specific-providers) |
// | `ignore` | string[] | - | List of provider slugs to skip for this request. [Learn more](#ignoring-providers) |
// | `quantizations` | string[] | - | List of quantization levels to filter by (e.g. `["int4", "int8"]`). [Learn more](#quantization) |
// | `sort` | string | - | Sort providers by price or throughput. (e.g. `"price"` or `"throughput"`). [Learn more](#provider-sorting) |
// | `max_price` | object | - | The maximum pricing you want to pay for this request. [Learn more](#maximum-price) |

/// OpenRouter "provider" routing preferences (typed).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<ProviderSlug>>,
}

impl ProviderPreferences {
    /// Add an allow preference.
    pub fn with_only<I: IntoIterator<Item = ProviderSlug>>(mut self, allow: I) -> Self {
        self.only = Some( allow.into_iter().collect() );
        self
    }

    /// Add a ignore list.
    pub fn with_ignore<I: IntoIterator<Item = ProviderSlug>>(mut self, ignore: I ) -> Self {
        self.ignore = Some( ignore.into_iter().collect() );
        self
    }

    /// Validate that there is no intersection between allow and deny lists.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let (Some(only), Some(ignore)) = (&self.only, &self.ignore) {
            if only.iter().any(|slug| ignore.contains(slug)) {
                return Err("ProviderPreferences only and ignore lists have overlapping entries");
            }
        }
        Ok(())
    }
}

