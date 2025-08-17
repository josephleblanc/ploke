use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ploke_tui::app::input::keymap::{to_action, Action};
use ploke_tui::app::types::Mode;
use ploke_tui::user_config::CommandStyle;

#[test]
fn insert_char_maps_to_action() {
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let act = to_action(Mode::Insert, key, CommandStyle::NeoVim);
    assert!(matches!(act, Some(Action::InsertChar('x'))));
}

#[test]
fn slash_prefix_is_insert_char_in_slash_style() {
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    let act = to_action(Mode::Insert, key, CommandStyle::Slash);
    // Mode switching is handled by App::handle_action, keymap only emits InsertChar('/')
    assert!(matches!(act, Some(Action::InsertChar('/'))));
}

#[test]
fn command_enter_executes_command() {
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let act = to_action(Mode::Command, key, CommandStyle::NeoVim);
    assert!(matches!(act, Some(Action::ExecuteCommand)));
}
