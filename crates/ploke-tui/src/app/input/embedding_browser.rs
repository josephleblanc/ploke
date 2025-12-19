use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_core::ArcStr;
use ploke_llm::ProviderSlug;

use crate::ModelId;
use crate::app::App;
use crate::app::view::components::context_browser::StepEnum;
use crate::app::view::components::embedding_browser::EmbeddingDetail;

pub fn handle_embedding_browser_input(app: &mut App, key: KeyEvent) {
    let mut chosen_model: Option<ModelId> = None;
    let mut chosen_provider: Option<ArcStr> = None;
    let modifiers = key.modifiers;

    if let Some(eb) = app.embedding_browser.as_mut() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.close_embedding_browser();
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
                    chosen_model = Some(item.id.clone());
                    // TODO: Either replace this or the provider field on expected field for
                    // provider in `EmbeddingSet`
                    chosen_provider = Some(ArcStr::from("openrouter"))
                }
            }
            KeyCode::Char('?') => {
                eb.help_visible = !eb.help_visible;
            }
            _ => {}
        }
    }

    if let Some(model_id) = chosen_model {
        app.apply_embedding_model_selection(model_id, chosen_provider);
        app.close_embedding_browser();
    }
    app.needs_redraw = true;
}
