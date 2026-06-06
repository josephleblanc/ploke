use ploke_llm::request::models::ModelRouteSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
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
