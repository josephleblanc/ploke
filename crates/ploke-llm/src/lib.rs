pub(crate) mod error;
pub(crate) mod manager;
pub(crate) mod registry;
pub(crate) mod request;
pub(crate) mod response;
pub(crate) mod router_only;
pub(crate) mod types;
pub(crate) mod utils;
pub(crate) mod wire;

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

pub use manager::{ chat_step, ChatHttpConfig, ChatStepOutcome, RequestMessage, handle_endpoint_request_async };

pub use router_only::{HasModels, Router};

use serde::{Deserialize, Serialize};

/// The default number of seconds for timeout on LLM request loop.
// TODO: Add this to user config
pub const LLM_TIMEOUT_SECS: u64 = 45;
