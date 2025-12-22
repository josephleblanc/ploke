use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::view::components::context_browser::{
    ContextBrowserMode, ContextSearchState, ShowMetaDetails, ShowPreview, StepEnum as _,
};

pub fn handle_context_browser_input(
    overlay: &mut ContextSearchState,
    key: KeyEvent,
) -> Vec<OverlayAction> {
    let mut actions = Vec::new();
    use KeyCode::*;
    match overlay.mode {
        ContextBrowserMode::Insert => match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                overlay.mode = ContextBrowserMode::Normal;
            }
            (KeyCode::Enter, _) => {
                overlay.mode = ContextBrowserMode::Normal;
                overlay.mark_dirty();
            }
            (KeyCode::Backspace, _) => {
                overlay.input.backspace();
                overlay.mark_dirty();
            }
            (KeyCode::Delete, _) => {
                overlay.input.delete();
                overlay.mark_dirty();
            }
            (KeyCode::Left, _) => overlay.input.move_left(),
            (KeyCode::Right, _) => overlay.input.move_right(),
            (KeyCode::Home, _) => overlay.input.move_home(),
            (KeyCode::End, _) => overlay.input.move_end(),
            (KeyCode::Char(c), mods) if mods.is_empty() || mods == KeyModifiers::SHIFT => {
                overlay.input.insert_char(c);
                overlay.mark_dirty();
            }
            _ => {}
        },
        ContextBrowserMode::Normal => match (key.code, key.modifiers) {
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => overlay.select_prev(),
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => overlay.select_next(),
            (KeyCode::Char('q'), _) => {
                actions.push(OverlayAction::CloseOverlay(OverlayKind::ContextBrowser));
            }
            (KeyCode::Char('i'), _) | (KeyCode::Char('/'), _) => {
                overlay.mode = ContextBrowserMode::Insert;
            }
            (KeyCode::Char('?'), _) => {
                overlay.help_visible = !overlay.help_visible;
            }
            (
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Left | KeyCode::Backspace,
                key_modifier,
            ) => {
                // Switch to collapsed item, repeated press doesn't toggle
                let idx = overlay.selected_index();
                if let Some(item) = overlay.items.get_mut(idx)
                    && item.expanded
                {
                    if item.show_preview == ShowPreview::NoPreview {
                        item.expanded = false;
                    }
                    if key_modifier == KeyModifiers::SHIFT {
                        item.show_preview = item.show_preview.least();
                        item.show_meta_details = item.show_meta_details.least();
                    } else {
                        item.show_preview = item.show_preview.prev_clamped();
                        item.show_meta_details = item.show_meta_details.prev_clamped();
                    }
                    tracing::debug!(hit_h_show_preview = ?item.show_preview);
                }
            }
            (
                KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Right | KeyCode::Char(' '),
                key_modifier,
            ) => {
                let idx = overlay.selected_index();
                if let Some(item) = overlay.items.get_mut(idx) {
                    // Switch to expanding item, repeated press doesn't toggle
                    if !item.expanded {
                        item.expanded = true;
                    }
                    if key_modifier == KeyModifiers::SHIFT {
                        item.show_preview = item.show_preview.most();
                        item.show_meta_details = item.show_meta_details.most();
                    } else {
                        item.show_preview = item.show_preview.next_clamped();
                        item.show_meta_details = item.show_meta_details.next_clamped();
                    }
                    tracing::debug!(hit_l_show_preview = ?item.show_preview);
                }
            }

            // TODO: Add a way to make the item name and/or text snippet the term used for the next
            // search, probably using `s` to search for item name and/or `shift+s` to search using
            // the text snippet of the selected item as the search term.
            _ => {}
        },
    }
    actions
}
