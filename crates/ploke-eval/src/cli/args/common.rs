use ploke_llm::request::models::ModelRouteSource;
use serde::{Deserialize, Serialize};

use crate::campaign::EmbeddingRoute;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum InspectOutputFormat {
    Table,
    Json,
}

pub(crate) fn parse_model_route_source(value: &str) -> Result<ModelRouteSource, String> {
    match value {
        "openrouter" | "open-router" | "open_router" => Ok(ModelRouteSource::OpenRouter),
        "direct-google" | "direct_google" | "google" => Ok(ModelRouteSource::DirectGoogle),
        other => Err(format!(
            "invalid route source '{other}'; expected openrouter or direct-google"
        )),
    }
}

pub(crate) fn parse_embedding_route(value: &str) -> Result<EmbeddingRoute, String> {
    match value {
        "openrouter" | "open-router" | "open_router" => Ok(EmbeddingRoute::OpenRouter),
        "direct-openai" | "direct_openai" | "openai" => Ok(EmbeddingRoute::DirectOpenAi),
        other => Err(format!(
            "invalid embedding route '{other}'; expected openrouter or direct-openai"
        )),
    }
}
