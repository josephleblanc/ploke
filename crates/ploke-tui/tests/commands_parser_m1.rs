use ploke_tui::app::App;
use ploke_tui::app::commands::parser::{Command, parse};
use ploke_tui::app_state::core::PreviewMode;
use ploke_tui::mock::create_mock_app;
use ploke_tui::user_config::CommandStyle;
use uuid::Uuid;

#[test]
#[ignore = "needs refactor, fails without tokio runtime"]
fn parses_edit_approve_and_deny() {
    let id = Uuid::new_v4();
    let cmd = format!("edit approve {}", id);
    let mut app: App = create_mock_app();
    app.set_selected_model("openrouter/moonshotai/kimi-k2".to_string());
    match parse(&app, &cmd, CommandStyle::NeoVim) {
        Command::EditApprove(x) => assert_eq!(x, id),
        other => panic!("unexpected parse: {:?}", other),
    }

    let cmd = format!("edit deny {}", id);
    match parse(&app, &cmd, CommandStyle::Slash) {
        Command::EditDeny(x) => assert_eq!(x, id),
        other => panic!("unexpected parse: {:?}", other),
    }
}

#[test]
#[ignore = "needs refactor, fails without tokio runtime"]
fn parses_edit_preview_mode_and_lines_and_auto() {
    let mut app: App = create_mock_app();
    app.set_selected_model("openrouter/moonshotai/kimi-k2".to_owned());
    match parse(&app, "edit preview mode diff", CommandStyle::NeoVim) {
        Command::EditSetPreviewMode(PreviewMode::Diff) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse(&app, "edit preview mode code", CommandStyle::NeoVim) {
        Command::EditSetPreviewMode(PreviewMode::CodeBlock) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse(&app, "edit preview lines 123", CommandStyle::NeoVim) {
        Command::EditSetPreviewLines(n) => assert_eq!(n, 123),
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse(&app, "edit auto on", CommandStyle::NeoVim) {
        Command::EditSetAutoConfirm(true) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse(&app, "edit auto off", CommandStyle::NeoVim) {
        Command::EditSetAutoConfirm(false) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
}
