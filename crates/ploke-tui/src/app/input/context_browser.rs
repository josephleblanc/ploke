use crossterm::event::KeyEvent;
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

use crate::ModelId;
use crate::app::App;
use crate::app::view::components::context_browser::{ShowMetaDetails, ShowPreview, StepEnum as _};
use crate::llm::manager::events::endpoint;
use crate::llm::router_only::RouterVariants;
use crate::llm::router_only::openrouter::OpenRouter;
use crate::llm::{LlmEvent, ProviderKey};

pub fn handle_context_browser_input(app: &mut App, key: KeyEvent) {
    // Intercept keys for model browser overlay when visible

    if let Some(cb) = app.context_browser.as_mut() {
        use KeyCode::*;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if cb.preview_select_active {
                    cb.preview_select_active = false;
                } else {
                    app.context_browser = None;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                cb.selected = cb.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                cb.selected = cb
                    .selected
                    .saturating_add(1)
                    .min(cb.items.len().saturating_sub(1));
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Toggle expanded item off/on
                if let Some(item) = cb.items.get_mut(cb.selected) {
                    item.expanded = !item.expanded;
                }
            }
            KeyCode::Char('l') => {
                if let Some(item) = cb.items.get_mut(cb.selected) {
                    // Switch to expanding item, repeated press doesn't toggle
                    if !item.expanded {
                        item.expanded = true;
                    }
                    item.show_preview = item.show_preview.next_clamped();
                    item.show_meta_details = item.show_meta_details.next_clamped();
                    tracing::debug!(hit_l_show_preview = ?item.show_preview);
                }
            }
            KeyCode::Char('h') => {
                // Switch to collapsed item, repeated press doesn't toggle
                if let Some(item) = cb.items.get_mut(cb.selected)
                    && item.expanded
                {
                    if item.show_preview == ShowPreview::NoPreview {
                        item.expanded = false;
                    }
                    item.show_preview = item.show_preview.prev_clamped();
                    item.show_meta_details = item.show_meta_details.prev_clamped();
                    tracing::debug!(hit_h_show_preview = ?item.show_preview);
                }
            }
            // TODO: Add a way to make the item name and/or text snippet the term used for the next
            // search, probably using `s` to search for item name and/or `shift+s` to search using
            // the text snippet of the selected item as the search term.
            KeyCode::Char('s') => {}
            KeyCode::Char('?') => {
                cb.help_visible = !cb.help_visible;
            }
            _ => {}
        }
    }
    app.needs_redraw = true;
}
