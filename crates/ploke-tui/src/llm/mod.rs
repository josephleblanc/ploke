// pub(crate) mod error;
pub(crate) mod manager;
pub(crate) use manager::ChatHistoryTarget;
pub use manager::events::{
    ContextPlan, ContextPlanExcludedMessage, ContextPlanMessage, ContextPlanRagPart,
};
pub use manager::{
    ChatEvt, LlmEvent, Prototype1TraceContext, RequestMessage, set_prototype1_trace_context,
};
#[cfg(feature = "test_harness")]
pub use manager::{
    RequestTapGuard, ResponseTapGuard, clear_recorded_response_tape, clear_request_tap,
    clear_response_tap, install_recorded_response_prefix_then_live,
    install_recorded_response_prefix_then_live_steps, install_recorded_response_tape,
    install_request_tap, install_response_tap,
};

pub(crate) use ploke_llm::error;
pub(crate) use ploke_llm::registry;
pub(crate) use ploke_llm::request;
pub(crate) use ploke_llm::response;
pub(crate) use ploke_llm::router_only;
pub(crate) use ploke_llm::types;
pub(crate) use ploke_llm::wire;
// pub(crate) use ploke_llm::manager::LlmEvent;
pub(crate) use ploke_llm::request::endpoint::EndpointsResponse;
pub(crate) use ploke_llm::types::enums::*;
pub(crate) use ploke_llm::types::meta::LLMMetadata;
pub(crate) use ploke_llm::types::model_types::{ModelId, ModelKey, ModelVariant};
pub(crate) use ploke_llm::types::newtypes::{
    ApiKeyEnv, Author, BaseUrl, EndpointKey, IdError, ModelName, ModelSlug, ProviderConfig,
    ProviderKey, ProviderName, ProviderSlug, Transport,
};
pub(crate) use ploke_llm::types::params::LLMParameters;
pub(crate) use ploke_llm::wire::WireRequest;

pub(crate) use ploke_llm::router_only::{HasModels, Router};
