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
    ModelSlug, IdError, Transport, ProviderConfig
};
pub(crate) use chat_msg::OaiChatReq;
pub(crate) use wire::WireRequest;
pub(crate) use enums::SupportedParameters;
pub(crate) use params::LLMParameters;

use serde::{Deserialize, Serialize};

// --- common types ---
/// Architecture details of a model, including input/output modalities and tokenizer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Architecture {
    /// Input modalities supported by this model (text, image, audio, video).
    pub input_modalities: Vec<InputModality>,
    pub modality: Modality,
    pub output_modalities: Vec<OutputModality>,
    pub tokenizer: Tokenizer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruct_type: Option<InstructType>,
}

