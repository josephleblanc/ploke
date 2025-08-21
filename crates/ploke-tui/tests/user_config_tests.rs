use std::fs;
use std::path::PathBuf;

use ploke_tui::user_config::{
    ProviderConfig, ProviderRegistry, ProviderRegistryStrictness, ProviderType, UserConfig,
};

fn temp_path(name: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("tempdir");
    dir.into_path().join(name)
}

fn sample_registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            ProviderConfig {
                id: "openrouter-default".to_string(),
                api_key: "SECRET_OR".to_string(),
                api_key_env: None,
                base_url: "https://openrouter.ai/api/v1".to_string(),
                model: "openai/gpt-4o".to_string(),
                display_name: Some("GPT-4o".to_string()),
                provider_type: ProviderType::OpenRouter,
                llm_params: None,
            },
            ProviderConfig {
                id: "custom-local".to_string(),
                api_key: "SECRET_CUSTOM".to_string(),
                api_key_env: None,
                base_url: "http://localhost:9999".to_string(),
                model: "local/dev".to_string(),
                display_name: Some("Local Dev".to_string()),
                provider_type: ProviderType::Custom,
                llm_params: None,
            },
        ],
        active_provider: "openrouter-default".to_string(),
        aliases: std::collections::HashMap::new(),
        capabilities: std::collections::HashMap::new(),
        strictness: ProviderRegistryStrictness::AllowCustom,
    }
}

#[test]
fn save_load_redacts_keys_by_default() {
    let cfg = UserConfig {
        registry: sample_registry(),
        ..Default::default()
    };

    let path = temp_path("config.toml");
    cfg.save_to_path(&path, true).expect("save redacted");

    let content = fs::read_to_string(&path).expect("read");
    assert!(content.contains(r#"api_key = """#), "expected redacted keys");

    let loaded = UserConfig::load_from_path(&path).expect("load");
    for p in loaded.registry.providers {
        assert!(
            p.api_key.is_empty(),
            "redacted save should produce empty api_key, got {}",
            p.api_key
        );
    }
}

#[test]
fn save_load_preserves_keys_when_opted_in() {
    let cfg = UserConfig {
        registry: sample_registry(),
        ..Default::default()
    };

    let path = temp_path("config_with_keys.toml");
    cfg.save_to_path(&path, false).expect("save with keys");

    let content = fs::read_to_string(&path).expect("read");
    assert!(
        content.contains("SECRET_OR") && content.contains("SECRET_CUSTOM"),
        "expected secrets to be present when not redacted"
    );

    let loaded = UserConfig::load_from_path(&path).expect("load");
    let ids = loaded
        .registry
        .providers
        .into_iter()
        .map(|p| (p.id, p.api_key))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(ids.get("openrouter-default").unwrap(), "SECRET_OR");
    assert_eq!(ids.get("custom-local").unwrap(), "SECRET_CUSTOM");
}

#[test]
fn strictness_enforced_on_switch() {
    let mut reg = sample_registry();

    reg.strictness = ProviderRegistryStrictness::OpenRouterOnly;
    assert!(!reg.set_active("custom-local"), "custom should be rejected");
    assert!(reg.set_active("openrouter-default"), "openrouter is allowed");
    assert_eq!(reg.active_provider, "openrouter-default");

    reg.strictness = ProviderRegistryStrictness::AllowCustom;
    assert!(reg.set_active("custom-local"), "custom allowed now");
    assert_eq!(reg.active_provider, "custom-local");
}

#[test]
fn alias_lookup_and_switching() {
    let mut reg = sample_registry();
    reg.aliases.insert("gpt".to_string(), "openrouter-default".to_string());
    reg.aliases.insert("local".to_string(), "custom-local".to_string());

    assert!(reg.set_active("gpt"));
    assert_eq!(reg.active_provider, "openrouter-default");

    assert!(reg.set_active("local"));
    assert_eq!(reg.active_provider, "custom-local");
}
