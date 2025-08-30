// Defines the typed response received when calling for a model provider endpoint using `ProvEnd`
// in `provider_endpoint.rs`.
//
// Plans (in progress)
// The typed response, `ModelProvider`, should implement Serialize and Deserialize for ergonomic
// deserializeation of the response from the call for all of the available endpoints for a
// specific model.
// This information should (if we understand the API structure correctly) be the data required to
// call into the `chat/completions` endpoints to request a generated response from the OpenRouter
// API.
//  - See `crates/ploke-tui/docs/openrouter/request_structure.md` for the OpenRouter official
//  documentation on the request structure.
//
// Desired functionality:
//  - We should take the typed response, `ModelProvider`, and be capable of transforming it into a
//  `CompReq`, a completion request to the OpenRouter API, ideally through `Serialize`.
//      - NOTE: The `CompReq` will deprecate the `llm::`
//  - We can add the `ModelProvider` to a cache of `ModelProvider` that forms our official
//  `ModelRegistry`.

pub struct ModelProvider {}

#[derive(Serialize, Debug)]
pub struct CompReq<'a> {
    // OpenRouter docs: "Either "messages" or "prompt" is required"
    // corresponding json: `messages?: Message[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<RequestMessage>>,
    // corresponding json: `prompt?: string;`
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    // OpenRouter docs: "If "model" is unspecified, uses the user's default"
    //  - Note: This default is set on the OpenRouter website
    //  - If we get errors for "No model available", provide the user with a message suggesting
    //  they check their OpenRouter account settings on the OpenRouter website for filtered
    //  providers as the cause of "No model available". If the user filters out all model providers
    //  that fulfill our (in ploke) filtering requirements (e.g. for tool-calling), this can lead
    //  to no models being available for the requests we send.
    // corresponding json: `model?: string;`
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    // TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
    // `crates/ploke-tui/src/tools/mod.rs`, since this is a constant value
    // corresponding json: `response_format?: { type: 'json_object' };`
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<JsonObjMarker>,
    // corresponding json: `stop?: string | string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    // OpenRouter docs: "Enable streaming"
    // corresponding json: `stream?: boolean;`
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    // corresponding json: `max_tokens?: number; // Range: [1, context_length)`
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    // corresponding json: `temperature?: number; // Range: [0, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    // OpenRouter docs:
    //  Tool calling
    //  Will be passed down as-is for providers implementing OpenAI's interface.
    //  For providers with custom interfaces, we transform and map the properties.
    //  Otherwise, we transform the tools into a YAML template. The model responds with an assistant message.
    //  See models supporting tool calling: openrouter.ai/models?supported_parameters=tools
    // NOTE: Do not use the website quoted above `openrouter.ai/models?supported_parameters=tools`
    // for API calls, this is a website and not an API endpoint... fool me once, *sigh*
    // corresponding json: `tools?: Tool[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    // corresponding json: tool_choice?: ToolChoice;
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>, 
    // corresponding json: `seed?: number; // Integer only`
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>, 
    // corresponding json: `top_p?: number; // Range: (0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    // corresponding json: `top_k?: number; // Range: [1, Infinity) Not available for OpenAI models`
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<f32>,
    // corresponding json: `frequency_penalty?: number; // Range: [-2, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    // corresponding json: `presence_penalty?: number; // Range: [-2, 2]`
    // corresponding json: `repetition_penalty?: number; // Range: (0, 2]`
    // corresponding json: `logit_bias?: { [key: number]: number };`
    // corresponding json: `top_logprobs: number; // Integer only`
    // corresponding json: `min_p?: number; // Range: [0, 1]`
    // corresponding json: `top_a?: number; // Range: [0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ProviderPreferences>,
}

// TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
// `crates/ploke-tui/src/tools/mod.rs`, since this can onlly have the value (in json):
//  `{ type: 'json_object' }`
pub struct JsonObjMarker;
