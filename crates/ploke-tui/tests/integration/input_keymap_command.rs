use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ploke_tui::app::input::keymap::{Action, to_action};
use ploke_tui::app::types::Mode;
use ploke_tui::user_config::CommandStyle;

#[test]
fn command_mode_enter_executes_and_esc_exits() {
    let exec = to_action(
        Mode::Command,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        CommandStyle::NeoVim,
    );
    assert!(matches!(exec, Some(Action::ExecuteCommand)));

    let esc = to_action(
        Mode::Command,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        CommandStyle::NeoVim,
    );
    assert!(matches!(esc, Some(Action::SwitchMode(Mode::Normal))));
}

#[test]
fn command_mode_backspace_is_forwarded() {
    let back = to_action(
        Mode::Command,
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        CommandStyle::NeoVim,
    );
    assert!(matches!(back, Some(Action::Backspace)));
}
