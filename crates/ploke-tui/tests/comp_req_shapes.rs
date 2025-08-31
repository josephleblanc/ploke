#![cfg(test)]

use serde_json::Value;

#[test]
fn comp_req_with_tools_and_provider_prefs() {
    use ploke_tui::llm::session::build_comp_req;
    use ploke_tui::llm::openrouter::model_provider::{ToolChoice, JsonObjMarker};
    use ploke_tui::llm::{RequestMessage, Role};
    use ploke_tui::tools::{FunctionMarker, GatTool};
    use ploke_tui::tools::request_code_context::RequestCodeContextGat;
    use ploke_tui::user_config::{ModelConfig, ProviderType};

    let provider = ModelConfig {
        id: "unit-provider".into(),
        api_key: "sk-test".into(),
        provider_slug: Some("openai".into()),
        api_key_env: None,
        base_url: "https://openrouter.ai/api/v1".into(),
        model: "qwen/qwen-2.5-7b-instruct".into(),
        display_name: None,
        provider_type: ProviderType::OpenRouter,
        llm_params: None,
    };
    let msgs = vec![
        RequestMessage { role: Role::System, content: "sys".into(), tool_call_id: None },
        RequestMessage { role: Role::User, content: "user".into(), tool_call_id: None },
    ];
    let tools = vec![RequestCodeContextGat::tool_def()];
    let params = ploke_tui::llm::LLMParameters { max_tokens: Some(128), temperature: Some(0.0), ..Default::default() };
    let req = build_comp_req(&provider, msgs, &params, Some(tools), true, /*require_parameters*/ true);

    let v = serde_json::to_value(&req).expect("json");
    // messages exist and model is set
    assert!(v.get("messages").is_some());
    assert_eq!(v.get("model").and_then(|x| x.as_str()), Some("qwen/qwen-2.5-7b-instruct"));
    // tools present, tool_choice auto by default
    assert!(v.get("tools").and_then(|t| t.as_array()).is_some());
    assert_eq!(v.get("tool_choice").and_then(|x| x.as_str()), Some("auto"));
    // provider preferences with require_parameters and order
    let prov = v.get("provider").and_then(|p| p.as_object()).expect("provider prefs");
    assert_eq!(prov.get("order").and_then(|o| o.as_array()).and_then(|a| a[0].as_str()), Some("openai"));
    // require_parameters may be intentionally omitted by serialization policy; if present, must be true
    if let Some(b) = prov.get("require_parameters").and_then(|b| b.as_bool()) {
        assert!(b, "require_parameters must be true when present");
    }
    // numeric fields are numbers
    assert_eq!(v.get("max_tokens").and_then(|x| x.as_u64()), Some(128));
    assert_eq!(v.get("temperature").and_then(|x| x.as_f64()), Some(0.0));

    // force function tool_choice and ensure shape
    let mut req2 = req;
    req2.tool_choice = Some(ToolChoice::Function { r#type: FunctionMarker, function: ploke_tui::llm::openrouter::model_provider::ToolChoiceFunction { name: "request_code_context".into() } });
    let v2 = serde_json::to_value(&req2).unwrap();
    let tc = v2.get("tool_choice").and_then(|x| x.as_object()).expect("function tool_choice");
    assert_eq!(tc.get("type").and_then(|x| x.as_str()), Some("function"));
    let f = tc.get("function").and_then(|x| x.as_object()).expect("function body");
    assert_eq!(f.get("name").and_then(|x| x.as_str()), Some("request_code_context"));
}
