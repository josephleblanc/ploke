use crossterm::event::{KeyCode, KeyEvent};

use crate::app::App;
use crate::ModelId;

pub fn handle_embedding_browser_input(app: &mut App, key: KeyEvent) {
    let mut chosen_model: Option<ModelId> = None;

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
                    item.expanded = !item.expanded;
                }
            }
            KeyCode::Char('s') => {
                if let Some(item) = eb.items.get(eb.selected) {
                    chosen_model = Some(item.id.clone());
                }
            }
            KeyCode::Char('?') => {
                eb.help_visible = !eb.help_visible;
            }
            _ => {}
        }
    }

    if let Some(model_id) = chosen_model {
        app.apply_embedding_model_selection(model_id);
        app.close_embedding_browser();
    }
    app.needs_redraw = true;
}
