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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    Goto(Jump),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayChoice {
    Approve(DecisionGroup),
    Deny(DecisionGroup),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jump {
    Top,
    Bottom,
    FarLeft,
    FarRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionGroup {
    Single,
    All,
}

pub fn map_choice_key(key: KeyEvent) -> Option<OverlayChoice> {
    match (key.modifiers, key.code) {
        (m, KeyCode::Char('y')) => {
            if m.is_empty() {
                Some(OverlayChoice::Approve(DecisionGroup::Single))
            } else if m.contains(KeyModifiers::SHIFT) {
                Some(OverlayChoice::Approve(DecisionGroup::All))
            } else {
                None
            }
        }
        (m, KeyCode::Char('n')) if m.is_empty() => {
            if m.is_empty() {
                Some(OverlayChoice::Deny(DecisionGroup::Single))
            } else if m.contains(KeyModifiers::SHIFT) {
                Some(OverlayChoice::Deny(DecisionGroup::All))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Map raw key input into shared navigation intent, if any.
pub fn map_navigation_key(key: KeyEvent) -> Option<OverlayNavAction> {
    match (key.modifiers, key.code) {
        (m, KeyCode::Up | KeyCode::Char('k')) => {
            if m.is_empty() {
                Some(OverlayNavAction::Up)
            } else if m.contains(KeyModifiers::SHIFT) {
                Some(OverlayNavAction::Goto(Jump::Top))
            } else {
                None
            }
        }
        (m, KeyCode::Down | KeyCode::Char('j')) if m.is_empty() => Some(OverlayNavAction::Down),
        (m, KeyCode::Left | KeyCode::Char('h')) if m.is_empty() => Some(OverlayNavAction::Left),
        (m, KeyCode::Right | KeyCode::Char('l')) if m.is_empty() => Some(OverlayNavAction::Right),
        (m, KeyCode::PageUp) if m.is_empty() => Some(OverlayNavAction::PageUp),
        (m, KeyCode::PageDown) if m.is_empty() => Some(OverlayNavAction::PageDown),
        (m, KeyCode::Home) if m.is_empty() => Some(OverlayNavAction::Home),
        (m, KeyCode::End) if m.is_empty() => Some(OverlayNavAction::End),
        (m, KeyCode::Tab) => {
            if m.is_empty() {
                Some(OverlayNavAction::NextTab)
            } else if m.contains(KeyModifiers::SHIFT) {
                Some(OverlayNavAction::PrevTab)
            } else {
                None
            }
        }
        (m, KeyCode::BackTab) if m.is_empty() => Some(OverlayNavAction::PrevTab),
        (m, KeyCode::Char('p')) if m.contains(KeyModifiers::CONTROL) => Some(OverlayNavAction::Up),
        (m, KeyCode::Char('n')) if m.contains(KeyModifiers::CONTROL) => {
            Some(OverlayNavAction::Down)
        }
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
