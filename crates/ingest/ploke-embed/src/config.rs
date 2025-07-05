use serde::Deserialize;

// NEW: Backend config structs
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HuggingFaceConfig {
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CozoConfig {
    pub api_key: Option<String>,
}
