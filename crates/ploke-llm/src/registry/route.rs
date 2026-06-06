use serde::{Deserialize, Serialize};

use crate::{
    ModelId, ProviderKey, SupportsTools,
    request::{endpoint::Endpoint, models::ResponseItem},
    router_only::{RouterVariants, google::Google, nebius::Nebius, openrouter::OpenRouter},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmRoute {
    OpenRouter(OpenRouterRoute),
    Google(GoogleRoute),
    Nebius(NebiusRoute),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterRoute {
    pub model: ModelId,
    pub provider: ProviderKey,
    pub endpoint: Endpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleRoute {
    pub model: ModelId,
    pub supports_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NebiusRoute {
    pub model: ModelId,
    pub supports_tools: bool,
}

impl LlmRoute {
    pub fn openrouter(model: ModelId, provider: ProviderKey, endpoint: Endpoint) -> Self {
        Self::OpenRouter(OpenRouterRoute {
            model,
            provider,
            endpoint,
        })
    }

    pub fn google(model: ModelId, supports_tools: bool) -> Self {
        Self::Google(GoogleRoute {
            model,
            supports_tools,
        })
    }

    pub fn nebius(model: ModelId, supports_tools: bool) -> Self {
        Self::Nebius(NebiusRoute {
            model,
            supports_tools,
        })
    }

    pub fn direct_google_model(model: &ResponseItem) -> Self {
        Self::google(model.id.clone(), model.supports_tools())
    }

    pub fn direct_nebius_model(model: &ResponseItem) -> Self {
        Self::nebius(model.id.clone(), model.supports_tools())
    }

    pub fn model(&self) -> &ModelId {
        match self {
            Self::OpenRouter(route) => &route.model,
            Self::Google(route) => &route.model,
            Self::Nebius(route) => &route.model,
        }
    }

    pub fn provider_key(&self) -> Option<&ProviderKey> {
        match self {
            Self::OpenRouter(route) => Some(&route.provider),
            Self::Google(_) => None,
            Self::Nebius(_) => None,
        }
    }

    pub fn selected_provider_slug(&self) -> String {
        match self {
            Self::OpenRouter(route) => route.provider.slug.as_str().to_string(),
            Self::Google(_) => "google".to_string(),
            Self::Nebius(_) => "nebius".to_string(),
        }
    }

    pub fn router(&self) -> RouterVariants {
        match self {
            Self::OpenRouter(_) => RouterVariants::OpenRouter(OpenRouter),
            Self::Google(_) => RouterVariants::Google(Google),
            Self::Nebius(_) => RouterVariants::Nebius(Nebius),
        }
    }

    pub fn is_direct_google(&self) -> bool {
        matches!(self, Self::Google(_))
    }

    pub fn is_direct_nebius(&self) -> bool {
        matches!(self, Self::Nebius(_))
    }
}

impl SupportsTools for LlmRoute {
    fn supports_tools(&self) -> bool {
        match self {
            Self::OpenRouter(route) => route.endpoint.supports_tools(),
            Self::Google(route) => route.supports_tools,
            Self::Nebius(route) => route.supports_tools,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, ModelKey, ModelSlug};

    fn google_model_id() -> ModelId {
        ModelId {
            key: ModelKey {
                author: Author::new("google").expect("author"),
                slug: ModelSlug::new("gemini-2.5-flash").expect("slug"),
            },
            variant: None,
        }
    }

    fn nebius_model_id() -> ModelId {
        ModelId {
            key: ModelKey {
                author: Author::new("meta-llama").expect("author"),
                slug: ModelSlug::new("Meta-Llama-3.1-70B-Instruct").expect("slug"),
            },
            variant: None,
        }
    }

    #[test]
    fn direct_google_route_has_no_provider_endpoint() {
        let route = LlmRoute::google(google_model_id(), true);

        assert_eq!(route.model().to_string(), "google/gemini-2.5-flash");
        assert!(route.provider_key().is_none());
        assert_eq!(route.selected_provider_slug(), "google");
        assert!(route.supports_tools());
        assert!(route.is_direct_google());
    }

    #[test]
    fn direct_nebius_route_has_no_provider_endpoint() {
        let route = LlmRoute::nebius(nebius_model_id(), true);

        assert_eq!(
            route.model().to_string(),
            "meta-llama/Meta-Llama-3.1-70B-Instruct"
        );
        assert!(route.provider_key().is_none());
        assert_eq!(route.selected_provider_slug(), "nebius");
        assert!(route.supports_tools());
        assert!(route.is_direct_nebius());
        assert!(matches!(route.router(), RouterVariants::Nebius(_)));
    }
}
