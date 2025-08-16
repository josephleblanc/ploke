//! View/Rendering scaffolding for ploke-tui.
//! We will progressively extract measurement and rendering here.

use uuid::Uuid;
use ratatui::Frame;

use super::RenderableMessage;
use super::App;

/// Placeholder view model for future snapshot-based rendering tests.
#[allow(dead_code)]
pub struct ViewModel<'a> {
    pub messages: &'a [RenderableMessage],
    pub current_id: Uuid,
    pub show_context_preview: bool,
}

/// Temporary delegating draw function (not yet wired).
#[allow(dead_code)]
pub(crate) fn draw(_app: &mut App, _frame: &mut Frame, _path: &[RenderableMessage], _current_id: Uuid) {
    // Intentionally empty: in subsequent refactors, move App::draw logic here.
}
