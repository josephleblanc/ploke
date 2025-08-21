use ploke_tui::app::commands::parser::{parse, Command};
use ploke_tui::user_config::{CommandStyle, ProviderRegistryStrictness};

#[test]
fn parses_model_commands() {
    assert!(matches!(parse("/model list", CommandStyle::Slash), Command::ModelList));
    assert!(matches!(parse(":model list", CommandStyle::NeoVim), Command::ModelList));
    assert!(matches!(parse("/model info", CommandStyle::Slash), Command::ModelInfo));

    match parse("/model use gpt-4o", CommandStyle::Slash) {
        Command::ModelUse(a) => assert_eq!(a, "gpt-4o"),
        other => panic!("unexpected: {:?}", other),
    }

    match parse("/model refresh", CommandStyle::Slash) {
        Command::ModelRefresh { remote } => assert!(remote),
        other => panic!("unexpected: {:?}", other),
    }
    match parse("/model refresh --local", CommandStyle::Slash) {
        Command::ModelRefresh { remote } => assert!(!remote),
        other => panic!("unexpected: {:?}", other),
    }

    match parse("/model load", CommandStyle::Slash) {
        Command::ModelLoad(None) => {}
        other => panic!("unexpected: {:?}", other),
    }
    match parse("/model load /tmp/a.toml", CommandStyle::Slash) {
        Command::ModelLoad(Some(p)) => assert_eq!(p, "/tmp/a.toml"),
        other => panic!("unexpected: {:?}", other),
    }

    match parse("/model save", CommandStyle::Slash) {
        Command::ModelSave { path, with_keys } => {
            assert!(path.is_none());
            assert!(!with_keys);
        }
        other => panic!("unexpected: {:?}", other),
    }
    match parse("/model save /tmp/a.toml --with-keys", CommandStyle::Slash) {
        Command::ModelSave { path, with_keys } => {
            assert_eq!(path.unwrap(), "/tmp/a.toml");
            assert!(with_keys);
        }
        other => panic!("unexpected: {:?}", other),
    }

    match parse("/model search gemini", CommandStyle::Slash) {
        Command::ModelSearch(kw) => assert_eq!(kw, "gemini"),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn parses_help_topics() {
    assert!(matches!(parse("/help", CommandStyle::Slash), Command::Help));
    match parse("/help model", CommandStyle::Slash) {
        Command::HelpTopic(t) => assert_eq!(t, "model"),
        other => panic!("unexpected: {:?}", other),
    }
    match parse(":help edit", CommandStyle::NeoVim) {
        Command::HelpTopic(t) => assert_eq!(t, "edit"),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn parses_provider_strictness() {
    match parse("/provider strictness openrouter-only", CommandStyle::Slash) {
        Command::ProviderStrictness(mode) => assert!(matches!(mode, ProviderRegistryStrictness::OpenRouterOnly)),
        other => panic!("unexpected: {:?}", other),
    }

    match parse("/provider strictness allow-custom", CommandStyle::Slash) {
        Command::ProviderStrictness(mode) => assert!(matches!(mode, ProviderRegistryStrictness::AllowCustom)),
        other => panic!("unexpected: {:?}", other),
    }

    match parse("/provider strictness allow-any", CommandStyle::Slash) {
        Command::ProviderStrictness(mode) => assert!(matches!(mode, ProviderRegistryStrictness::AllowAny)),
        other => panic!("unexpected: {:?}", other),
    }
}
