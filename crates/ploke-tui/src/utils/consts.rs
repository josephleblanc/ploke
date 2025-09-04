/// "https://openrouter.ai/api/v1/"
/// Conevenience const to help form a Url for OpenRouter
pub(crate) const OPENROUTER_BASE_STR: &str = "https://openrouter.ai/api/v1/";
/// "endpoints"
/// Conevenience const to help form a Url for OpenRouter call to find the endpoints that provide
/// the target model.
pub(crate) const OPENROUTER_ENDPOINT_STR: &str = "/endpoints";

// --- debug consts ---
//
// Used for debug targets with tracing
pub(crate) const DEBUG_TOOLS: &str = "dbg_tools";
pub(crate) const DBG_EVENTS: &str = "dbg_events";
