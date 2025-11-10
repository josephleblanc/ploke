use fxhash::FxHashSet as HashSet;
use ploke_core::ArcStr;
use serde::{Deserialize, Serialize};

use crate::llm::ModelId;
use crate::llm::error::LlmError;
use crate::llm::registry::user_prefs::{ModelPrefs, RegistryPrefs};
use crate::llm::request::endpoint::EndpointsResponse;
use crate::llm::types::model_types::ModelVariant;
use crate::llm::{Author, EndpointKey, IdError, ModelKey, ModelSlug, ProviderSlug, Quant};

use super::{ApiRoute, HasEndpoint, HasModels, Router, RouterModelId, RouterVariants};

pub(crate) mod providers;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Default, Hash, Eq)]
pub(crate) struct OpenRouter;

impl HasModels for OpenRouter {
    type Response = crate::llm::request::models::Response;
    type Models = crate::llm::request::models::ResponseItem;
    type Error = ploke_error::Error;
}

impl Router for OpenRouter {
    type CompletionFields = ChatCompFields;
    type RouterModelId = OpenRouterModelId;
    const BASE_URL: &str = "https://openrouter.ai/api/v1";
    const COMPLETION_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
    const MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
    const ENDPOINTS_TAIL: &str = "endpoints";
    const API_KEY_NAME: &str = "OPENROUTER_API_KEY";
    const PROVIDERS_URL: &str = "https://openrouter.ai/api/v1/providers";
}

impl TryFrom<RouterVariants> for OpenRouter {
    type Error = LlmError;

    fn try_from(value: RouterVariants) -> Result<Self, Self::Error> {
        match value {
            RouterVariants::OpenRouter(open_router) => Ok(OpenRouter),
            RouterVariants::Anthropic(anthropic) => Err(LlmError::Conversion(String::from(
                "Invalid conversion from Anthropic to OpenRouter",
            ))),
        }
    }
}

impl Into<RouterVariants> for OpenRouter {
    fn into(self) -> RouterVariants {
        RouterVariants::OpenRouter(self)
    }
}

impl ApiRoute for ChatCompFields {
    type Parent = OpenRouter;
}

impl HasEndpoint for OpenRouter {
    type EpResponse = EndpointsResponse;

    type Error = LlmError;
}

/// Provider-specific information about the model.
/// - Unique type only used by OpenRouter
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, PartialOrd, Eq)]
pub(crate) struct TopProvider {
    /// Whether this model is subject to content moderation.
    pub(crate) is_moderated: bool,
    pub(crate) context_length: Option<u32>,
    pub(crate) max_completion_tokens: Option<u64>,
}

impl TopProvider {
    /// Set whether this model is subject to content moderation.
    pub(crate) fn with_moderated(mut self, moderated: bool) -> Self {
        self.is_moderated = moderated;
        self
    }

    /// Set the context length for this model.
    pub(crate) fn with_context_length(mut self, length: u32) -> Self {
        self.context_length = Some(length);
        self
    }

    /// Set the maximum completion tokens for this model.
    pub(crate) fn with_max_completion_tokens(mut self, tokens: u64) -> Self {
        self.max_completion_tokens = Some(tokens);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub(crate) struct OpenRouterModelId {
    pub(crate) key: ModelKey,
    pub(crate) variant: Option<OpenRouterModelVariant>,
}

impl From<ModelId> for OpenRouterModelId {
    fn from(m: ModelId) -> Self {
        let ModelId { key, variant } = m;
        Self {
            key,
            variant: variant.map(OpenRouterModelVariant::from),
        }
    }
}

impl OpenRouterModelId {
    fn from_parts(value: EndpointKey, variant: Option<ModelVariant>) -> Self {
        let EndpointKey {
            model,
            variant,
            provider,
        } = value;
        Self {
            key: model,
            variant: variant.map(OpenRouterModelVariant::from),
        }
    }

    fn with_variant(mut self, variant: OpenRouterModelVariant) -> Self {
        self.variant = Some(variant);
        self
    }
}

impl From<EndpointKey> for OpenRouterModelId {
    fn from(value: EndpointKey) -> Self {
        Self::from_parts(value, None)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum OpenRouterModelVariant {
    Free,
    Beta,
    Extended,
    Thinking,
    Online,
    Nitro,
    Floor,
    Other(ArcStr),
}

impl From<ModelVariant> for OpenRouterModelVariant {
    fn from(value: ModelVariant) -> Self {
        match value {
            ModelVariant::Free => Self::Free,
            ModelVariant::Beta => Self::Beta,
            ModelVariant::Extended => Self::Extended,
            ModelVariant::Thinking => Self::Thinking,
            ModelVariant::Online => Self::Online,
            ModelVariant::Nitro => Self::Nitro,
            ModelVariant::Floor => Self::Floor,
            ModelVariant::Other(s) => Self::Other(s),
        }
    }
}

impl OpenRouterModelVariant {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Free => "free",
            Self::Beta => "beta",
            Self::Extended => "extended",
            Self::Thinking => "thinking",
            Self::Online => "online",
            Self::Nitro => "nitro",
            Self::Floor => "floor",
            Self::Other(s) => s.as_ref(),
        }
    }
}

impl RouterModelId for OpenRouterModelId {
    fn key(&self) -> &ModelKey {
        &self.key
    }

    fn into_key(self) -> ModelKey {
        self.key
    }

    fn into_url_format(self) -> String {
        let mut base = format!("{}/{}", self.key.author.as_str(), self.key.slug.as_str());
        if let Some(v) = &self.variant {
            base.push_str("%3A");
            base.push_str(v.as_str());
        }
        base
    }
}

use std::{fmt, str::FromStr, sync::Arc};

impl fmt::Display for OpenRouterModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variant {
            None => write!(f, "{}/{}", self.key.author.as_str(), self.key.slug.as_str()),
            Some(v) => write!(
                f,
                "{}/{}:{}",
                self.key.author.as_str(),
                self.key.slug.as_str(),
                v.as_str()
            ),
        }
    }
}

impl FromStr for OpenRouterModelId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (author, rest) = s
            .trim()
            .split_once('/')
            .ok_or(IdError::Invalid("missing '/'"))?;
        let (slug, variant) = match rest.split_once(':') {
            Some((slug, v)) if !slug.is_empty() => (slug, Some(v)),
            _ if !rest.is_empty() => (rest, None),
            _ => return Err(IdError::Invalid("missing slug")),
        };
        Ok(Self {
            key: ModelKey {
                author: Author::new(author)?,
                slug: ModelSlug::new(slug)?,
            },
            variant: variant.map(|v| match v {
                "free" => OpenRouterModelVariant::Free,
                "beta" => OpenRouterModelVariant::Beta,
                "extended" => OpenRouterModelVariant::Extended,
                "thinking" => OpenRouterModelVariant::Thinking,
                "online" => OpenRouterModelVariant::Online,
                "nitro" => OpenRouterModelVariant::Nitro,
                "floor" => OpenRouterModelVariant::Floor,
                other => OpenRouterModelVariant::Other(ArcStr::from(Arc::<str>::from(other))),
            }),
        })
    }
}

impl serde::Serialize for OpenRouterModelId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use std::fmt::Write;
        let mut s = String::with_capacity(64);
        // identical to Display, but avoid a temporary if you like:
        write!(&mut s, "{}", self).unwrap();
        ser.serialize_str(&s)
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
    pub(crate) models: Option<Vec<OpenRouterModelId>>,
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
    /// Adds user model preferences
    // TODO: Try getting this into the `ApiRoute` trait
    pub(crate) fn preferences_union(mut self, pref: &RegistryPrefs) -> Self {
        if let Some(openrouter_prefs) = pref
            .router_prefs
            .get(&RouterVariants::OpenRouter(OpenRouter))
        {
            self.provider = self.provider.map(|p| p.merge_union(openrouter_prefs));
        }
        self
    }

    /// Set the transforms parameter.
    pub(crate) fn with_transforms(mut self, transforms: Transform) -> Self {
        self.transforms = Some(transforms);
        self
    }

    /// Set the models parameter for fallback routing.
    pub(crate) fn with_models<I: IntoIterator<Item = ModelId>>(mut self, models: I) -> Self {
        self.models = Some(models.into_iter().map(Into::into).collect());
        self
    }

    /// Set the route parameter.
    pub(crate) fn with_route(mut self, route: FallbackMarker) -> Self {
        self.route = Some(route);
        self
    }

    /// Set the provider preferences.
    pub(crate) fn with_provider(mut self, provider: ProviderPreferences) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the user identifier.
    pub(crate) fn with_user(mut self, user: String) -> Self {
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
pub(crate) struct MiddleOutMarker;

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
pub(crate) struct FallbackMarker;

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
    pub(crate) order: Option<HashSet<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) data_collection: Option<DataCollection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) only: Option<HashSet<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ignore: Option<HashSet<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quantizations: Option<HashSet<Quant>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sort: Option<SortBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_price: Option<MaxPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Copy)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DataCollection {
    Allow,
    #[default]
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SortBy {
    Price,
    Throughput,
    Latency,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub(crate) struct MaxPrice {
    pub(crate) prompt_tokens: Option<f64>,
    pub(crate) completion_tokens: Option<f64>,
    pub(crate) request: Option<f64>,
}

impl ProviderPreferences {
    pub(crate) fn merge_union(mut self, other: &Self) -> Self {
        if let Some(order) = other.order.as_ref() {
            self.order = self
                .order
                .map(|ord| ord.union(order).cloned().collect::<HashSet<ProviderSlug>>());
        }
        if let Some(allow_fallbacks) = other.allow_fallbacks {
            self.allow_fallbacks = other.allow_fallbacks;
        }
        if let Some(require_parameters) = other.require_parameters {
            self.require_parameters = other.require_parameters;
        }
        if let Some(data_collection) = other.data_collection {
            self.data_collection = other.data_collection;
        }
        if let Some(only) = other.only.as_ref() {
            self.only = self
                .only
                .map(|o| o.union(only).cloned().collect::<HashSet<ProviderSlug>>());
        }
        if let Some(ignore) = other.ignore.as_ref() {
            self.ignore = self
                .ignore
                .map(|i| i.union(ignore).cloned().collect::<HashSet<ProviderSlug>>());
        }
        if let Some(quantizations) = other.quantizations.as_ref() {
            self.quantizations = self
                .quantizations
                .map(|q| q.union(quantizations).cloned().collect::<HashSet<Quant>>());
        }
        if let Some(sort) = other.sort {
            self.sort = other.sort;
        }
        if let Some(max_price) = other.max_price.as_ref() {
            self.max_price = Some(*max_price);
        }

        self
    }

    /// Add an order preference.
    pub(crate) fn with_order<I: IntoIterator<Item = ProviderSlug>>(mut self, order: I) -> Self {
        self.order = Some(order.into_iter().collect());
        self
    }

    /// Set allow_fallbacks preference.
    pub(crate) fn with_allow_fallbacks(mut self, allow: bool) -> Self {
        self.allow_fallbacks = Some(allow);
        self
    }

    /// Set require_parameters preference.
    pub(crate) fn with_require_parameters(mut self, require: bool) -> Self {
        self.require_parameters = Some(require);
        self
    }

    /// Set data_collection preference.
    pub(crate) fn with_data_collection(mut self, collection: DataCollection) -> Self {
        self.data_collection = Some(collection);
        self
    }

    /// Add an allow preference.
    pub(crate) fn with_only<I: IntoIterator<Item = ProviderSlug>>(mut self, allow: I) -> Self {
        self.only = Some(allow.into_iter().collect());
        self
    }

    /// Add an ignore list.
    pub(crate) fn with_ignore<I: IntoIterator<Item = ProviderSlug>>(mut self, ignore: I) -> Self {
        self.ignore = Some(ignore.into_iter().collect());
        self
    }

    /// Add quantizations filter.
    pub(crate) fn with_quantizations<I: IntoIterator<Item = Quant>>(
        mut self,
        quantizations: I,
    ) -> Self {
        self.quantizations = Some(quantizations.into_iter().collect());
        self
    }

    /// Set sort preference.
    pub(crate) fn with_sort(mut self, sort: SortBy) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Set max_price preference.
    pub(crate) fn with_max_price(mut self, max_price: MaxPrice) -> Self {
        self.max_price = Some(max_price);
        self
    }

    /// Validate that there is no intersection between only and ignore lists.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if let (Some(only), Some(ignore)) = (&self.only, &self.ignore) {
            if only.iter().any(|slug| ignore.contains(slug)) {
                return Err("ProviderPreferences only and ignore lists have overlapping entries");
            }
        }
        Ok(())
    }
}
