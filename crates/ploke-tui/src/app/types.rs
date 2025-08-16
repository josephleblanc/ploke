/*!
Types and lightweight data structures for the app module.

This module centralizes small cross-submodule types to decouple rendering,
input handling, and the app runtime. Keeping these here reduces cyclic
dependencies and clarifies ownership.

Intended usage:
- view and message_item consume RenderableMessage for measurement/rendering.
- input::keymap uses Mode to decide the active keymap.
- The App struct stores Mode for modal behavior (Normal/Insert/Command).
*/

use crate::chat_history::MessageKind;
use uuid::Uuid;

/// Editing/interaction mode for the TUI.
#[derive(Default, Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Vim-like Normal mode: navigation and commands.
    Normal,
    /// Default text input mode.
    #[default]
    Insert,
    /// Command palette/prompt mode (":…" or "/…").
    Command,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Normal => write!(f, "Normal"),
            Mode::Insert => write!(f, "Insert"),
            Mode::Command => write!(f, "Command"),
        }
    }
}

/// Minimal message data required by the renderer. This is intentionally
/// a small, copy-on-read snapshot separate from the full chat model.
#[derive(Debug, Clone)]
pub struct RenderableMessage {
    pub(crate) id: Uuid,
    pub(crate) kind: MessageKind,
    pub(crate) content: String,
}
