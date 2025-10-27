use insta::assert_snapshot;
use ploke_tui::app::view::components::model_browser::{snapshot_text_for_test, TestModelItem, TestProviderRow};

#[test]
fn snapshot_model_browser_initial_loading() {
    let text = snapshot_text_for_test(vec![], "qwen", 0, false, 0);
    assert_snapshot!("model_browser_initial_loading", text);
}

#[test]
fn snapshot_model_browser_with_providers_selection() {
    // Build one model item with two providers and active provider selection
    let item = TestModelItem {
        id: "deepseek/deepseek-chat-v3.1".to_string(),
        name: Some("DeepSeek: DeepSeek V3.1".to_string()),
        context_length: Some(163840),
        input_cost: Some(0.2),
        output_cost: Some(0.8),
        supports_tools: true,
        providers: vec![
            TestProviderRow { provider_slug: "chutes".to_string(), context_length: 163840, input_cost: 0.2, output_cost: 0.8, supports_tools: true },
            TestProviderRow { provider_slug: "deepinfra".to_string(), context_length: 131072, input_cost: 0.25, output_cost: 0.9, supports_tools: true },
        ],
        expanded: true,
        loading_providers: false,
    };
    let text = snapshot_text_for_test(vec![item], "deepseek", 0, true, 0);
    assert_snapshot!("model_browser_providers_select_first", text);
}

#[test]
fn snapshot_model_browser_with_providers_selection_second() {
    let item = TestModelItem {
        id: "deepseek/deepseek-chat-v3.1".to_string(),
        name: Some("DeepSeek: DeepSeek V3.1".to_string()),
        context_length: Some(163840),
        input_cost: Some(0.2),
        output_cost: Some(0.8),
        supports_tools: true,
        providers: vec![
            TestProviderRow { provider_slug: "chutes".to_string(), context_length: 163840, input_cost: 0.2, output_cost: 0.8, supports_tools: true },
            TestProviderRow { provider_slug: "deepinfra".to_string(), context_length: 131072, input_cost: 0.25, output_cost: 0.9, supports_tools: true },
        ],
        expanded: true,
        loading_providers: false,
    };
    let text = snapshot_text_for_test(vec![item], "deepseek", 0, true, 1);
    assert_snapshot!("model_browser_providers_select_second", text);
}
#[test]
fn snapshot_model_browser_enter_expand_loading() {
    // Expanded, but loading providers
    let item = TestModelItem {
        id: "openai/gpt-4o".to_string(),
        name: Some("OpenAI: GPT-4o".to_string()),
        context_length: Some(128000),
        input_cost: Some(5.0),
        output_cost: Some(15.0),
        supports_tools: true,
        providers: vec![],
        expanded: true,
        loading_providers: true,
    };
    let text = snapshot_text_for_test(vec![item], "openai", 0, false, 0);
    assert_snapshot!("model_browser_enter_expand_loading", text);
}

#[test]
fn snapshot_many_results_openai_multiple_expanded() {
    // Simulate many results; expand multiple models
    let mut models = Vec::new();
    for i in 0..8 {
        let expanded = i % 2 == 0; // expand every other
        let providers = if expanded {
            vec![
                TestProviderRow { provider_slug: "openai".to_string(), context_length: 128000, input_cost: 5.0 + i as f64, output_cost: 15.0 + i as f64, supports_tools: true },
                TestProviderRow { provider_slug: "azure".to_string(), context_length: 128000, input_cost: 5.2 + i as f64, output_cost: 15.2 + i as f64, supports_tools: true },
            ]
        } else { vec![] };
        models.push(TestModelItem {
            id: format!("openai/gpt-4o-{}", i),
            name: Some(format!("OpenAI: GPT-4o {}", i)),
            context_length: Some(128000),
            input_cost: Some(5.0 + i as f64),
            output_cost: Some(15.0 + i as f64),
            supports_tools: true,
            providers,
            expanded,
            loading_providers: false,
        });
    }
    // Select an item beyond the top to illustrate long content and selection position
    let text = snapshot_text_for_test(models, "openai", 5, true, 1);
    assert_snapshot!("model_browser_many_results_openai_multiple_expanded", text);
}
