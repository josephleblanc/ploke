use ploke_tui::app::commands::parser::{Command, parse};
use ploke_tui::mock::create_mock_app;
use ploke_tui::user_config::{CommandStyle, MessageVerbosityProfile};

#[tokio::test]
async fn slash_verbosity_profile_parses_to_set_command() {
    let app = create_mock_app();
    let parsed = parse(&app, "/verbosity profile verbose", CommandStyle::Slash);
    assert!(matches!(
        parsed,
        Command::VerbosityProfileSet(MessageVerbosityProfile::Verbose)
    ));
}

#[tokio::test]
async fn neovim_verbosity_profile_show_parses() {
    let app = create_mock_app();
    let parsed = parse(&app, ":verbosity profile", CommandStyle::NeoVim);
    assert!(matches!(parsed, Command::VerbosityProfileShow));
}
