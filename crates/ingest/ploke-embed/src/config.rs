use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct LocalModelConfig {
    pub model_id: String,
}

// NEW: Backend config structs
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct HuggingFaceConfig {
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct CozoConfig {
    pub api_key: Option<String>,
}
