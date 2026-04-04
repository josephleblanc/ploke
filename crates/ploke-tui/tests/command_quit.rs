use ploke_tui::app::commands::parser::{Command, parse};
use ploke_tui::mock::create_mock_app;
use ploke_tui::user_config::CommandStyle;

#[tokio::test]
async fn slash_quit_parses_to_quit_command() {
    let app = create_mock_app().await;
    let parsed = parse(&app, "/quit", CommandStyle::Slash);
    assert!(matches!(parsed, Command::Quit));
}
