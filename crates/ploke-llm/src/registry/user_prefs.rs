use fxhash::FxHashMap as HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::{
    Author, EndpointKey, LLMParameters, ModelKey, ModelSlug, ProviderKey, SupportedParameters,
    router_only::{RouterVariants, openrouter::ProviderPreferences},
    types::model_types::ModelVariant,
};

pub(crate) static DEFAULT_MODEL: Lazy<ModelKey> = Lazy::new(|| ModelKey {
    author: Author::new("moonshotai").expect("author"),
    slug: ModelSlug::new("kimi-k2").expect("model slug"),
});

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd)]
pub(crate) struct ProfileName(String);

impl Default for ProfileName {
    fn default() -> Self {
        Self(String::from("default-profile"))
    }
}

impl ProfileName {
    pub(crate) fn from(other: impl AsRef<str>) -> Self {
        Self(other.as_ref().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelProfile {
    // user-named param sets per model
    pub name: ProfileName, // e.g. "creative-0.8" or "eval-sweep"
    pub model_key: ModelKey,
    pub params: LLMParameters,
    pub variant: Option<ModelVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelPrefs {
    // canonical id of form {author}/{model},
    // note that it does not include `:{variant}` as in `ModelId`, which varies by provider,
    // and is included under `ModelProfile` in `variant`
    pub model_key: ModelKey,
    pub default_profile: Option<ModelProfile>,
    // name from profiles
    pub profiles: HashMap<ProfileName, ModelProfile>,
    // API routing server, this gives us url, e.g. OpenRouter, OpenAI
    pub allowed_routers: Vec<RouterVariants>,
    // for explicit routing
    pub selected_endpoints: Vec<EndpointKey>,
}

impl ModelPrefs {
    pub(crate) fn get_default_profile(&self) -> Option<&ModelProfile> {
        self.default_profile
            .as_ref()
            .and_then(|def_pr| self.profiles.get(&def_pr.name))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryPrefs {
    pub global_default_profile: Option<ModelProfile>,
    pub models: HashMap<ModelKey, ModelPrefs>,
    pub strictness: ModelRegistryStrictness,
    pub router_prefs: HashMap<RouterVariants, ProviderPreferences>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
/// Policy for allowed providers when switching the active provider.
pub enum ModelRegistryStrictness {
    /// Only allow selecting OpenRouter providers
    OpenRouterOnly,
    /// Allow OpenRouter and Custom routers/providers (default)
    #[default]
    AllowCustom,
    /// No restrictions (future-friendly)
    AllowAny,
}
