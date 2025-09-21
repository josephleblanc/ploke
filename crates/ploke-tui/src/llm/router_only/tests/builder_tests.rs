use crate::llm::registry::user_prefs::{ModelPrefs, ProfileName};
use crate::tools::ToolDefinition;
use crate::{
    llm::{
        LLMParameters, ModelKey,
        manager::RequestMessage,
        registry::user_prefs::{ModelProfile, RegistryPrefs},
        router_only::{Router, openrouter::OpenRouter},
        types::model_types::ModelId,
    },
    tools::{RequestCodeContextGat, Tool},
};
use color_eyre::Result;
use fxhash::FxHashMap as HashMap;
use std::str::FromStr;

type TestChatCompRequest = super::super::ChatCompRequest<OpenRouter>;

#[test]
fn test_builder_with_core_bundle() -> Result<()> {
    let core = super::super::ChatCompReqCore::default()
        .with_model(ModelId::from_str("test/model")?)
        .with_message(RequestMessage::new_user("Hello".to_string()));

    let request = TestChatCompRequest::default().with_core_bundle(core.clone());

    assert_eq!(request.core.model, core.model);
    assert_eq!(request.core.messages.len(), 1);
    assert_eq!(request.core.messages[0].content, "Hello");
    Ok(())
}

#[test]
fn test_builder_with_param_bundle() -> Result<()> {
    let params = LLMParameters::default()
        .with_max_tokens(1000)
        .with_temperature(0.7);

    let request = TestChatCompRequest::default().with_param_bundle(params.clone());

    assert_eq!(request.llm_params.max_tokens, Some(1000));
    assert_eq!(request.llm_params.temperature, Some(0.7));
    Ok(())
}

#[test]
fn test_builder_with_model_key() -> Result<()> {
    let model_key: Option<ModelKey> = Some("author/model".try_into()?);

    let request = TestChatCompRequest::default().with_model_key(model_key.clone());

    assert_eq!(request.model_key, model_key);
    Ok(())
}

#[test]
fn test_builder_with_tools() -> Result<()> {
    let tools: Vec<ToolDefinition> = vec![RequestCodeContextGat::tool_def()];

    let request = TestChatCompRequest::default().with_tools(Some(tools.clone()));

    assert_eq!(request.tools, Some(tools));
    Ok(())
}

#[test]
fn test_builder_with_messages() -> Result<()> {
    let messages = vec![
        RequestMessage::new_system("You are helpful".to_string()),
        RequestMessage::new_user("Hello".to_string()),
    ];

    let request = TestChatCompRequest::default().with_messages(messages.clone());

    assert_eq!(request.core.messages.len(), 2);
    assert_eq!(request.core.messages[0].content, "You are helpful");
    assert_eq!(request.core.messages[1].content, "Hello");
    Ok(())
}

#[test]
fn test_builder_with_message() -> Result<()> {
    let message = RequestMessage::new_user("Single message".to_string());

    let request = TestChatCompRequest::default().with_message(message.clone());

    assert_eq!(request.core.messages.len(), 1);
    assert_eq!(request.core.messages[0].content, "Single message");
    Ok(())
}

#[test]
fn test_builder_with_prompt() -> Result<()> {
    let prompt = "This is a prompt".to_string();

    let request = TestChatCompRequest::default().with_prompt(prompt.clone());

    assert_eq!(request.core.prompt, Some(prompt));
    assert!(request.core.messages.is_empty());
    Ok(())
}

#[test]
fn test_builder_with_model() -> Result<()> {
    let model = ModelId::from_str("test/model")?;

    let request = TestChatCompRequest::default().with_model(model.clone());

    assert_eq!(request.core.model, model);
    Ok(())
}

#[test]
fn test_builder_with_model_str() -> Result<()> {
    let request = TestChatCompRequest::default().with_model_str("test/model")?;

    assert_eq!(request.core.model.to_string(), "test/model");
    Ok(())
}

#[test]
fn test_builder_with_json_response() -> Result<()> {
    let request = TestChatCompRequest::default().with_json_response();

    assert!(request.core.response_format.is_some());
    Ok(())
}

#[test]
fn test_builder_with_stop() -> Result<()> {
    let stop = vec!["stop1".to_string(), "stop2".to_string()];

    let request = TestChatCompRequest::default().with_stop(stop.clone());

    assert_eq!(request.core.stop, Some(stop));
    Ok(())
}

#[test]
fn test_builder_with_stop_sequence() -> Result<()> {
    let request = TestChatCompRequest::default().with_stop_sequence("single_stop".to_string());

    assert_eq!(request.core.stop, Some(vec!["single_stop".to_string()]));
    Ok(())
}

#[test]
fn test_builder_with_streaming() -> Result<()> {
    let request = TestChatCompRequest::default().with_streaming(true);

    assert_eq!(request.core.stream, Some(true));

    let request = request.with_streaming(false);
    assert_eq!(request.core.stream, Some(false));
    Ok(())
}

#[test]
fn test_builder_streaming_convenience() -> Result<()> {
    let request = TestChatCompRequest::default().streaming();

    assert_eq!(request.core.stream, Some(true));

    let request = request.non_streaming();
    assert_eq!(request.core.stream, Some(false));
    Ok(())
}

#[test]
fn test_builder_llm_param_methods() -> Result<()> {
    let request = TestChatCompRequest::default()
        .with_max_tokens(500)
        .with_temperature(0.5)
        .with_seed(42)
        .with_top_p(0.9)
        .with_top_k(50.0)
        .with_frequency_penalty(0.1)
        .with_presence_penalty(0.2)
        .with_repetition_penalty(1.1);

    assert_eq!(request.llm_params.max_tokens, Some(500));
    assert_eq!(request.llm_params.temperature, Some(0.5));
    assert_eq!(request.llm_params.seed, Some(42));
    assert_eq!(request.llm_params.top_p, Some(0.9));
    assert_eq!(request.llm_params.top_k, Some(50.0));
    assert_eq!(request.llm_params.frequency_penalty, Some(0.1));
    assert_eq!(request.llm_params.presence_penalty, Some(0.2));
    assert_eq!(request.llm_params.repetition_penalty, Some(1.1));
    Ok(())
}

#[test]
fn test_builder_chaining() -> Result<()> {
    let request = TestChatCompRequest::default()
        .with_model_str("test/model")?
        .with_message(RequestMessage::new_user("Hello".to_string()))
        .with_max_tokens(1000)
        .with_temperature(0.7)
        .streaming();

    assert_eq!(request.core.model.to_string(), "test/model");
    assert_eq!(request.core.messages.len(), 1);
    assert_eq!(request.core.messages[0].content, "Hello");
    assert_eq!(request.llm_params.max_tokens, Some(1000));
    assert_eq!(request.llm_params.temperature, Some(0.7));
    assert_eq!(request.core.stream, Some(true));
    Ok(())
}
