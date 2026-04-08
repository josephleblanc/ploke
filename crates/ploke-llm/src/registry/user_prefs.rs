#![allow(
    dead_code,
    unused_variables,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]
use fxhash::FxHashMap as HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::{
    Author, EndpointKey, LLMParameters, ModelKey, ModelSlug, ProviderKey,
    router_only::{
        RouterVariants,
        openrouter::{OpenRouter, ProviderPreferences},
    },
    types::model_types::ModelVariant,
};

pub(crate) static DEFAULT_MODEL: Lazy<ModelKey> = Lazy::new(|| ModelKey {
    author: Author::new("moonshotai").expect("author"),
    slug: ModelSlug::new("kimi-k2").expect("model slug"),
});

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd)]
pub struct ProfileName(String);

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
    pub fn get_default_profile(&self) -> Option<&ModelProfile> {
        self.default_profile
            .as_ref()
            .and_then(|def_pr| self.profiles.get(&def_pr.name))
    }

    pub fn selected_endpoint(&self) -> Option<&EndpointKey> {
        self.selected_endpoints.last()
    }

    pub fn selected_provider_preferences(&self) -> Option<ProviderPreferences> {
        self.selected_endpoint().map(|endpoint| {
            ProviderPreferences::default()
                .with_only([endpoint.provider.slug.clone()])
                .with_allow_fallbacks(false)
                .with_require_parameters(true)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryPrefs {
    pub global_default_profile: Option<ModelProfile>,
    pub models: HashMap<ModelKey, ModelPrefs>,
    pub strictness: ModelRegistryStrictness,
    pub router_prefs: HashMap<RouterVariants, ProviderPreferences>,
}

impl RegistryPrefs {
    pub fn select_model_provider(
        &mut self,
        model_id: &crate::ModelId,
        provider_key: Option<&ProviderKey>,
    ) {
        let crate::ModelId { key, variant } = model_id.clone();
        let mp = self
            .models
            .entry(key.clone())
            .or_insert_with(|| ModelPrefs {
                model_key: key,
                ..Default::default()
            });

        if !mp
            .allowed_routers
            .iter()
            .any(|r| matches!(r, RouterVariants::OpenRouter(_)))
        {
            mp.allowed_routers
                .push(RouterVariants::OpenRouter(OpenRouter));
        }

        if let Some(provider) = provider_key {
            let ek = EndpointKey {
                model: model_id.key.clone(),
                provider: provider.clone(),
                variant,
            };
            mp.selected_endpoints.retain(|e| e != &ek);
            mp.selected_endpoints.push(ek);
        }
    }
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
