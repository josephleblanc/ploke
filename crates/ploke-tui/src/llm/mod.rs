pub(crate) mod wire;
pub(crate) mod request;
pub(crate) mod router_only;
pub(crate) mod response;
pub(crate) mod types;
pub(crate) mod error;
pub(crate) mod manager;
pub(crate) mod registry;

pub(crate) use types::model_types::{ ModelId, ModelKey, ModelVariant };
pub(crate) use types::enums::*;
pub(crate) use types::newtypes::{
    ApiKeyEnv, Author, BaseUrl, EndpointKey, ProviderKey, ProviderName, ProviderSlug,
    ModelSlug, IdError, Transport, ProviderConfig, ModelName
};
pub(crate) use types::meta::LLMMetadata;
pub(crate) use wire::WireRequest;
pub(crate) use types::params::LLMParameters;
pub(crate) use request::endpoint::EndpointsResponse;
pub(crate) use manager::{ LlmEvent, ChatHistoryTarget  };

pub(crate) use router_only::{ HasModels, Router };

use serde::{Deserialize, Serialize};
