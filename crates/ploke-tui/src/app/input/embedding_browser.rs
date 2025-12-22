use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_core::ArcStr;

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::view::components::context_browser::StepEnum;
use crate::app::view::components::embedding_browser::EmbeddingDetail;
use crate::app::view::components::embedding_browser::EmbeddingBrowserState;

pub fn handle_embedding_browser_input(
    eb: &mut EmbeddingBrowserState,
    key: KeyEvent,
) -> Vec<OverlayAction> {
    let mut actions = Vec::new();
    let modifiers = key.modifiers;

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            actions.push(OverlayAction::CloseOverlay(OverlayKind::EmbeddingBrowser));
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if eb.selected > 0 {
                eb.selected -= 1;
            } else {
                eb.selected = eb.items.len().saturating_sub(1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if eb.items.is_empty() {
                // nothing to do
            } else if eb.selected + 1 < eb.items.len() {
                eb.selected += 1;
            } else {
                eb.selected = 0;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(item) = eb.items.get_mut(eb.selected) {
                item.detail = match item.detail {
                    EmbeddingDetail::Collapsed => EmbeddingDetail::Summary,
                    _ => EmbeddingDetail::Collapsed,
                };
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Right => {
            if let Some(item) = eb.items.get_mut(eb.selected) {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    item.detail = item.detail.most();
                } else {
                    item.detail = item.detail.next_clamped();
                }
            }
        }
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Left => {
            if let Some(item) = eb.items.get_mut(eb.selected) {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    item.detail = item.detail.least();
                } else {
                    item.detail = item.detail.prev_clamped();
                }
            }
        }
        KeyCode::Char('s') => {
            if let Some(item) = eb.items.get(eb.selected) {
                actions.push(OverlayAction::SelectEmbeddingModel {
                    model_id: item.id.clone(),
                    provider: Some(ArcStr::from("openrouter")),
                });
            }
        }
        KeyCode::Char('?') => {
            eb.help_visible = !eb.help_visible;
        }
        _ => {}
    }

    actions
}
