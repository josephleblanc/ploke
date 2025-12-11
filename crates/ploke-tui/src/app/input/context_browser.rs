use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::app::view::components::context_browser::{
    ContextBrowserMode, ShowMetaDetails, ShowPreview, StepEnum as _,
};

pub fn handle_context_browser_input(app: &mut App, key: KeyEvent) {
    // Intercept keys for model browser overlay when visible

    if let Some(cb) = app.context_browser.as_mut() {
        use KeyCode::*;
        match cb.mode {
            ContextBrowserMode::Insert => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    cb.mode = ContextBrowserMode::Normal;
                    return;
                }
                (KeyCode::Enter, _) => {
                    cb.mode = ContextBrowserMode::Normal;
                    cb.mark_dirty();
                }
                (KeyCode::Backspace, _) => {
                    cb.input.backspace();
                    cb.mark_dirty();
                }
                (KeyCode::Delete, _) => {
                    cb.input.delete();
                    cb.mark_dirty();
                }
                (KeyCode::Left, _) => cb.input.move_left(),
                (KeyCode::Right, _) => cb.input.move_right(),
                (KeyCode::Home, _) => cb.input.move_home(),
                (KeyCode::End, _) => cb.input.move_end(),
                (KeyCode::Char(c), mods)
                    if mods.is_empty() || mods == KeyModifiers::SHIFT =>
                {
                    cb.input.insert_char(c);
                    cb.mark_dirty();
                }
                _ => {}
            },
            ContextBrowserMode::Normal => match (key.code, key.modifiers) {
                (KeyCode::Enter, _) => {
                    cb.mode = ContextBrowserMode::Normal;
                    cb.mark_dirty();
                }
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => cb.select_prev(),
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => cb.select_next(),
                (KeyCode::Char('q'), _) => {
                    app.context_browser = None;
                    return;
                }
                (KeyCode::Char('i'), _) | (KeyCode::Char('/'), _) => {
                    cb.mode = ContextBrowserMode::Insert;
                    return;
                }
                (KeyCode::Char('?'), _) => {
                    cb.help_visible = !cb.help_visible;
                }
            // TODO: Add a way to make the item name and/or text snippet the term used for the next
            // search, probably using `s` to search for item name and/or `shift+s` to search using
            // the text snippet of the selected item as the search term.
                _ => {}
            },
        }

        // Shared navigation + item toggles (available in both modes)
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ')
                if matches!(cb.mode, ContextBrowserMode::Normal) =>
            {
                let idx = cb.selected_index();
                if let Some(item) = cb.items.get_mut(idx) {
                    item.expanded = !item.expanded;
                }
            }
            KeyCode::Char('l') if matches!(cb.mode, ContextBrowserMode::Normal) => {
                let idx = cb.selected_index();
                if let Some(item) = cb.items.get_mut(idx) {
                    // Switch to expanding item, repeated press doesn't toggle
                    if !item.expanded {
                        item.expanded = true;
                    }
                    item.show_preview = item.show_preview.next_clamped();
                    item.show_meta_details = item.show_meta_details.next_clamped();
                    tracing::debug!(hit_l_show_preview = ?item.show_preview);
                }
            }
            KeyCode::Char('h') if matches!(cb.mode, ContextBrowserMode::Normal) => {
                // Switch to collapsed item, repeated press doesn't toggle
                let idx = cb.selected_index();
                if let Some(item) = cb.items.get_mut(idx) && item.expanded {
                    if item.show_preview == ShowPreview::NoPreview {
                        item.expanded = false;
                    }
                    item.show_preview = item.show_preview.prev_clamped();
                    item.show_meta_details = item.show_meta_details.prev_clamped();
                    tracing::debug!(hit_h_show_preview = ?item.show_preview);
                }
            }
            _ => {}
        }
    }
    app.needs_redraw = true;
}
