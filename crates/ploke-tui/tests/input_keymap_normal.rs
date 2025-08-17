use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ploke_tui::app::input::keymap::{to_action, Action};
use ploke_tui::app::types::Mode;
use ploke_tui::user_config::CommandStyle;

#[test]
fn ctrl_c_is_quit_in_any_mode() {
    for mode in [Mode::Normal, Mode::Insert, Mode::Command] {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let act = to_action(mode, key, CommandStyle::NeoVim);
        assert!(matches!(act, Some(Action::Quit)));
    }
}

#[test]
fn normal_mode_basic_navigation() {
    let style = CommandStyle::NeoVim;

    let down_j = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), style);
    assert!(matches!(down_j, Some(Action::NavigateListDown)));

    let up_k = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE), style);
    assert!(matches!(up_k, Some(Action::NavigateListUp)));

    let left_h = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), style);
    assert!(matches!(left_h, Some(Action::BranchPrev)));

    let right_l = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE), style);
    assert!(matches!(right_l, Some(Action::BranchNext)));
}

#[test]
fn normal_mode_paging_and_toggles() {
    let style = CommandStyle::NeoVim;

    let pg_down = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('J'), KeyModifiers::NONE), style);
    assert!(matches!(pg_down, Some(Action::PageDown)));

    let pg_up = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE), style);
    assert!(matches!(pg_up, Some(Action::PageUp)));

    let toggle_preview = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('P'), KeyModifiers::NONE), style);
    assert!(matches!(toggle_preview, Some(Action::TogglePreview)));
}

#[test]
fn normal_mode_command_openers() {
    // '/' opens command (style determines inserted prefix later in App::handle_action)
    let slash_cmd = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), CommandStyle::Slash);
    assert!(matches!(slash_cmd, Some(Action::OpenCommand)));

    // ':' only valid in NeoVim style
    let colon_cmd = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE), CommandStyle::NeoVim);
    assert!(matches!(colon_cmd, Some(Action::OpenCommandColon)));

    // Quick model and help shortcuts
    let model = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), CommandStyle::NeoVim);
    assert!(matches!(model, Some(Action::OpenQuickModel)));

    let help = to_action(Mode::Normal, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE), CommandStyle::NeoVim);
    assert!(matches!(help, Some(Action::OpenHelp)));
}
