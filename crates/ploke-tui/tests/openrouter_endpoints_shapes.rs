#![cfg(test)]

#[test]
fn endpoints_response_deserialize_minimal() {
    use ploke_tui::llm::openrouter::model_provider::{EndpointsResponse, Endpoint};
    let body = r#"{
        "data": {
          "endpoints": [
            {
              "provider_slug": "nebius",
              "context_length": 262144,
              "pricing": { "prompt": "0.0000001", "completion": "0.0000003" },
              "supported_parameters": ["tools", "max_tokens"],
              "name": "Nebius | qwen/qwen-2.5-7b-instruct"
            }
          ]
        }
      }"#;
    let parsed: EndpointsResponse = serde_json::from_str(body).expect("parse endpoints");
    assert_eq!(parsed.data.endpoints.len(), 1);
    let ep: &Endpoint = &parsed.data.endpoints[0];
    assert_eq!(ep.preferred_provider_slug(), "nebius");
    assert!(ep.supports_tools());
    assert_eq!(ep.context_length, Some(262144));
    assert!(ep.pricing.prompt_or_default() > 0.0);
    assert!(ep.pricing.completion_or_default() > 0.0);
}

