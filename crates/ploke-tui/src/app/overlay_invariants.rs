//! Shared overlay invariants and helpers.
//!
//! Why a helper module instead of putting everything on the `Overlay` trait?
//! - We are mid-migration: helpers can be reused by both new trait-based overlays and legacy
//!   overlays without forcing a big-bang conversion.
//! - Not all invariants need to be enforced by the type system; some are convenience utilities
//!   (navigation mapping, list clamping) that are easier to adopt incrementally.
//! - Once the migration completes, we can move stable helpers into default trait methods or a
//!   supertrait for stronger guarantees.
//!
//! This module should focus on shared behavior that would otherwise be duplicated across overlays.

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::overlay::{OverlayAction, OverlayKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayNavAction {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    NextTab,
    PrevTab,
}

/// Map raw key input into shared navigation intent, if any.
pub fn map_navigation_key(key: KeyEvent) -> Option<OverlayNavAction> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(OverlayNavAction::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(OverlayNavAction::Down),
        KeyCode::Left | KeyCode::Char('h') => Some(OverlayNavAction::Left),
        KeyCode::Right | KeyCode::Char('l') => Some(OverlayNavAction::Right),
        KeyCode::PageUp => Some(OverlayNavAction::PageUp),
        KeyCode::PageDown => Some(OverlayNavAction::PageDown),
        KeyCode::Home => Some(OverlayNavAction::Home),
        KeyCode::End => Some(OverlayNavAction::End),
        KeyCode::Tab => Some(OverlayNavAction::NextTab),
        KeyCode::BackTab => Some(OverlayNavAction::PrevTab),
        _ => None,
    }
}

/// Default close intent for overlays that use `Esc` or `q` to exit.
pub fn map_close_key(key: KeyEvent, kind: OverlayKind) -> Option<OverlayAction> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Some(OverlayAction::CloseOverlay(kind)),
        _ => None,
    }
}

/// Clamp a list selection index to the available items.
pub fn clamp_selection(selected: usize, len: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        Some(selected.min(len.saturating_sub(1)))
    }
}
