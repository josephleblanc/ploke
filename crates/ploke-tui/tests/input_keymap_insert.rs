use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ploke_tui::app::input::keymap::{to_action, Action};
use ploke_tui::app::types::Mode;
use ploke_tui::user_config::CommandStyle;

#[test]
fn insert_mode_enter_and_backspace() {
    let enter = to_action(Mode::Insert, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), CommandStyle::NeoVim);
    assert!(matches!(enter, Some(Action::Submit)));

    let back = to_action(Mode::Insert, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), CommandStyle::NeoVim);
    assert!(matches!(back, Some(Action::Backspace)));
}

#[test]
fn insert_mode_slash_is_just_a_char() {
    let slash = to_action(Mode::Insert, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), CommandStyle::Slash);
    assert!(matches!(slash, Some(Action::InsertChar('/'))));
}
