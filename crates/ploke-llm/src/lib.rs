pub mod error;
pub mod manager;
pub mod registry;
pub mod request;
pub mod response;
pub mod router_only;
pub mod types;
pub mod utils;
pub mod wire;

pub use manager::LlmEvent;
pub use request::endpoint::EndpointsResponse;
pub use types::enums::*;
pub use types::meta::LLMMetadata;
pub use types::model_types::{ModelId, ModelKey, ModelVariant};
pub use types::newtypes::{
    ApiKeyEnv, Author, BaseUrl, EndpointKey, IdError, ModelName, ModelSlug, ProviderConfig,
    ProviderKey, ProviderName, ProviderSlug, Transport,
};
pub use types::params::LLMParameters;
pub use wire::WireRequest;
pub use error::LlmError;

pub use manager::{ chat_step, ChatHttpConfig, ChatStepOutcome, RequestMessage, handle_endpoint_request_async };

pub use router_only::{HasModels, Router};

use serde::{Deserialize, Serialize};

/// The default number of seconds for timeout on LLM request loop.
// TODO: Add this to user config
pub const LLM_TIMEOUT_SECS: u64 = 45;
