use ploke_tui::app::commands::parser::{parse, Command};
use ploke_tui::app_state::core::PreviewMode;
use ploke_tui::user_config::CommandStyle;
use uuid::Uuid;

#[test]
fn parses_edit_approve_and_deny() {
    let id = Uuid::new_v4();
    let cmd = format!("edit approve {}", id);
    match parse(&cmd, CommandStyle::NeoVim) {
        Command::EditApprove(x) => assert_eq!(x, id),
        other => panic!("unexpected parse: {:?}", other),
    }

    let cmd = format!("edit deny {}", id);
    match parse(&cmd, CommandStyle::Slash) {
        Command::EditDeny(x) => assert_eq!(x, id),
        other => panic!("unexpected parse: {:?}", other),
    }
}

#[test]
fn parses_edit_preview_mode_and_lines_and_auto() {
    match parse("edit preview mode diff", CommandStyle::NeoVim) {
        Command::EditSetPreviewMode(PreviewMode::Diff) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse("edit preview mode code", CommandStyle::NeoVim) {
        Command::EditSetPreviewMode(PreviewMode::CodeBlock) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse("edit preview lines 123", CommandStyle::NeoVim) {
        Command::EditSetPreviewLines(n) => assert_eq!(n, 123),
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse("edit auto on", CommandStyle::NeoVim) {
        Command::EditSetAutoConfirm(true) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
    match parse("edit auto off", CommandStyle::NeoVim) {
        Command::EditSetAutoConfirm(false) => {}
        other => panic!("unexpected parse: {:?}", other),
    }
}
