mod newtypes;
mod chat_msg;
mod wire;
mod request;
mod router_only;
mod enums;
mod params;

use crate::llm2::enums::*;
pub(crate) use newtypes::{
    ApiKeyEnv, Author, BaseUrl, EndpointKey, ModelKey, ProviderKey, ProviderName, ProviderSlug,
    Slug, IdError, Transport, ProviderConfig
};
pub(crate) use chat_msg::OaiChatReq;
pub(crate) use wire::WireRequest;
pub(crate) use enums::SupportedParameters;
pub(crate) use params::LLMParameters;


// re-export for child modules
use serde::{Deserialize, Serialize};
/// OpenRouter "provider" routing preferences (typed).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<ProviderSlug>>,
}

impl ProviderPreferences {
    /// Add an allow preference.
    pub fn with_allow<I: IntoIterator<Item = ProviderSlug>>(mut self, allow: Option< I >) -> Self {
        self.allow = allow.map(|i| i.into_iter().collect());
        self
    }

    /// Add an ordering preference.
    pub fn with_order<I: IntoIterator<Item = ProviderSlug>>(mut self, order: Option< I >) -> Self {
        self.order = order.map(|i| i.into_iter().collect());
        self
    }

    /// Add a deny list.
    pub fn with_deny<I: IntoIterator<Item = ProviderSlug>>(mut self, deny: Option< I >) -> Self {
        self.deny = deny.map(|i| i.into_iter().collect());
        self
    }

    /// Validate that there is no intersection between allow and deny lists.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let (Some(allow), Some(deny)) = (&self.allow, &self.deny) {
            if allow.iter().any(|slug| deny.contains(slug)) {
                return Err("ProviderPreferences allow and deny lists have overlapping entries");
            }
        }
        Ok(())
    }
}

// --- common types ---
/// Architecture details of a model, including input/output modalities and tokenizer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Architecture {
    /// Input modalities supported by this model (text, image, audio, video).
    pub input_modalities: Vec<InputModality>,
    pub output_modalities: Vec<OutputModality>,
    pub tokenizer: Tokenizer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruct_type: Option<InstructType>,
}

