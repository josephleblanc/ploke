/*!
Key mapping for the TUI.

This module translates low-level KeyEvent inputs into high-level Actions that
the App can handle in a mode-agnostic way. This keeps `App::on_key_event`
simple and makes keybindings testable.

Intended usage:
- Call `to_action(mode, key, command_style)` from the App input loop.
- Match on `Action` in a single handler to update UI state or dispatch
  `StateCommand`s.
*/

use crate::app::types::Mode;
use crate::user_config::CommandStyle;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// High-level, mode-agnostic actions produced by the keymap.
/// App translates these into UI updates and StateCommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // App lifecycle
    Quit,
    // Mode changes
    SwitchMode(Mode),

    // Text entry / command handling
    InsertChar(char),
    Backspace,
    Submit,         // Enter in Insert mode
    ExecuteCommand, // Enter in Command mode

    // Navigation (list/branches)
    NavigateListUp,
    NavigateListDown,
    BranchPrev,
    BranchNext,

    // Scrolling
    PageUp,
    PageDown,
    ScrollLineUp,
    ScrollLineDown,
    JumpTop,       // 'G'
    GotoSequenceG, // 'g' (first press; App decides if this becomes 'gg')

    // Command palette openers
    OpenCommand,         // '/', or ':hybrid' starter depending on style
    OpenCommandColon,    // ':' (Neovim style)
    OpenQuickModel,      // 'm'
    OpenHelp,            // '?'
    TogglePreview,       // 'P'
    OpenApprovals,       // 'a'
    OpenContextSearch,   // `s`
    ToggleToolVerbosity, // 'v'
    OpenConfigOverlay,   // 'o'

    // Input widget scrolling (testing/dev keys)
    InputScrollPrev, // Ctrl+Up
    InputScrollNext, // Ctrl+Down
}

/// Map a KeyEvent to an Action based on the current editing Mode and CommandStyle.
/// Returns None for unmapped keys.
pub fn to_action(mode: Mode, key: KeyEvent, style: CommandStyle) -> Option<Action> {
    // Global bindings
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    match mode {
        Mode::Insert => match (key.modifiers, key.code) {
            (m, KeyCode::Esc) if m.is_empty() => Some(Action::SwitchMode(Mode::Normal)),
            (m, KeyCode::Enter) if m.is_empty() || m == KeyModifiers::SHIFT => Some(Action::Submit),
            (m, KeyCode::Backspace) if m.is_empty() || m == KeyModifiers::SHIFT => {
                Some(Action::Backspace)
            }
            (m, KeyCode::Char(c)) if m.is_empty() || m == KeyModifiers::SHIFT => {
                Some(Action::InsertChar(c))
            }
            (m, KeyCode::Up) if m.contains(KeyModifiers::CONTROL) => Some(Action::InputScrollPrev),
            (m, KeyCode::Down) if m.contains(KeyModifiers::CONTROL) => {
                Some(Action::InputScrollNext)
            }
            _ => None,
        },
        Mode::Command => match (key.modifiers, key.code) {
            (m, KeyCode::Esc) if m.is_empty() => Some(Action::SwitchMode(Mode::Normal)),
            (m, KeyCode::Enter) if m.is_empty() || m == KeyModifiers::SHIFT => {
                Some(Action::ExecuteCommand)
            }
            (m, KeyCode::Backspace) if m.is_empty() || m == KeyModifiers::SHIFT => {
                Some(Action::Backspace)
            }
            (m, KeyCode::Char(c)) if m.is_empty() || m == KeyModifiers::SHIFT => {
                Some(Action::InsertChar(c))
            }
            _ => None,
        },
        Mode::Normal => {
            // Ctrl-based scrolling
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return match key.code {
                    KeyCode::Char('n') => Some(Action::ScrollLineDown),
                    KeyCode::Char('p') => Some(Action::ScrollLineUp),
                    _ => None,
                };
            }

            match key.code {
                KeyCode::Char('q') => Some(Action::Quit),
                KeyCode::Char('i') => Some(Action::SwitchMode(Mode::Insert)),

                KeyCode::Char('/') => Some(Action::OpenCommand),
                KeyCode::Char(':') if matches!(style, CommandStyle::NeoVim) => {
                    Some(Action::OpenCommandColon)
                }
                KeyCode::Char('m') => Some(Action::OpenQuickModel),
                KeyCode::Char('?') => Some(Action::OpenHelp),
                KeyCode::Char('P') => Some(Action::TogglePreview),
                KeyCode::Char('v') => Some(Action::ToggleToolVerbosity),
                KeyCode::Char('e') => Some(Action::OpenApprovals),
                KeyCode::Char('s') => Some(Action::OpenContextSearch),
                KeyCode::Char('o') => Some(Action::OpenConfigOverlay),

                KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateListUp),
                KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateListDown),

                KeyCode::Char('J') => Some(Action::PageDown),
                KeyCode::Char('K') => Some(Action::PageUp),

                KeyCode::Char('g') => Some(Action::GotoSequenceG),
                KeyCode::Char('G') => Some(Action::JumpTop),

                // Placeholder for changing conversation branches
                KeyCode::Char('h') | KeyCode::Left => Some(Action::BranchPrev),
                KeyCode::Char('l') | KeyCode::Right => Some(Action::BranchNext),

                _ => None,
            }
        }
    }
}
